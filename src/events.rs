use std::sync::{Mutex, OnceLock};
use tao::{
  event::{Event, StartCause, WindowEvent},
  event_loop::{ControlFlow, EventLoop, EventLoopProxy},
  window::{ResizeDirection, Window},
};
use wry::WebView;

use crate::{logs, window::resize};

pub static PROXY: OnceLock<Mutex<Option<EventLoopProxy<AppEvent>>>> = OnceLock::new();

/// Hands an AppEvent to the running event loop, from whichever thread holds
/// it. Every reply to a Call travels this way: a callback finishes on a pool
/// thread or on the asyncio loop, and the JavaScript that settles its Promise
/// has to be evaluated back on the thread that owns the window.
pub fn send_to_event_loop(event: AppEvent) -> Result<(), String> {
  let proxy = PROXY.get().ok_or("The Bridge is not initialized.")?;
  let guard = proxy.lock().map_err(|_| "The Bridge is poisoned.")?;
  let sender = guard.as_ref().ok_or("The Bridge is not running.")?;
  sender
    .send_event(event)
    .map_err(|err| format!("The Event could not be sent: {err}"))
}

#[derive(Debug)]
pub enum AppEvent {
  RunJavascript(String),
  DragWindow,
  MinimizeWindow,
  MaximizeWindow,
  CloseWindow,
  ResizeWindow(ResizeDirection),
  ResizeDragged(resize::Drag),
  FromPython(String),
}

pub fn run_event_loop(
  event_loop: EventLoop<AppEvent>, window: Window, webview: WebView,
) {
  let mut webview = webview;
  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => {
        logs::debug(
          logs::WEBVIEW,
          format!("Webview '{}' started.", window.title()),
        );
      },
      Event::WindowEvent { event, .. } => {
        handle_window_event(event, &mut webview, control_flow)
      },
      Event::UserEvent(app_event) => {
        handle_app_event(app_event, &window, &mut webview, control_flow)
      },
      _ => (),
    }
  });
}

fn handle_window_event(
  event: WindowEvent, webview: &mut WebView, control_flow: &mut ControlFlow,
) {
  match event {
    WindowEvent::CloseRequested => exit_app(webview, control_flow),
    _ => (),
  }
}

fn handle_app_event(
  event: AppEvent, window: &Window, webview: &mut WebView,
  control_flow: &mut ControlFlow,
) {
  match event {
    AppEvent::RunJavascript(js) => run_javascript(webview, &js),
    AppEvent::CloseWindow => exit_app(webview, control_flow),
    AppEvent::MinimizeWindow => toggle_minimize(window),
    AppEvent::MaximizeWindow => toggle_maximize(window),
    AppEvent::DragWindow => drag(window),
    AppEvent::ResizeWindow(direction) => {
      if let Err(err) = window.drag_resize_window(direction) {
        logs::error(
          logs::WEBVIEW,
          format!("The window could not be resized: {err}"),
        );
      }
    },
    AppEvent::ResizeDragged(drag) => resize::apply(&drag, window),
    AppEvent::FromPython(message) => handle_python_event(&message),
  }
}

fn run_javascript(webview: &WebView, js: &str) {
  if let Err(err) = webview.evaluate_script(js) {
    logs::error(
      logs::BRIDGE,
      format!("The JavaScript could not be evaluated: {err}"),
    );
  }
}

fn exit_app(webview: &mut WebView, control_flow: &mut ControlFlow) {
  let mut webview = Some(webview);
  webview.take();
  *control_flow = ControlFlow::Exit;
}

fn toggle_minimize(window: &Window) {
  let minimized = window.is_minimized();
  window.set_minimized(!minimized);
}

fn toggle_maximize(window: &Window) {
  let is_maximized = window.is_maximized();
  window.set_maximized(!is_maximized);
}

fn drag(window: &Window) {
  if let Err(err) = window.drag_window() {
    logs::error(
      logs::WEBVIEW,
      format!("The window could not be dragged: {err}"),
    );
  }
}

fn handle_python_event(message: &str) {
  logs::debug(logs::BRIDGE, format!("Event from Python: {message}"));
}
