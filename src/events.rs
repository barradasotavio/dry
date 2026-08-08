use pyo3::{
  Bound, Python,
  types::{PyAnyMethods, PyModule},
};
use std::sync::{Mutex, OnceLock};
use tao::{
  event::{Event, StartCause, WindowEvent},
  event_loop::{ControlFlow, EventLoop, EventLoopProxy},
  window::{ResizeDirection, Window},
};
use wry::WebView;

use crate::{logs, window::resize};

#[cfg(test)]
mod tests;

pub static PROXY: OnceLock<Mutex<Option<EventLoopProxy<AppEvent>>>> = OnceLock::new();

/// The module that answers for Python on the way out: it holds the close hook,
/// the asyncio loop and the thread pool.
const PORTAL: &str = "dry.portal";

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
    WindowEvent::CloseRequested => close(webview, control_flow),
    _ => (),
  }
}

fn handle_app_event(
  event: AppEvent, window: &Window, webview: &mut WebView,
  control_flow: &mut ControlFlow,
) {
  match event {
    AppEvent::RunJavascript(js) => run_javascript(webview, &js),
    AppEvent::CloseWindow => close(webview, control_flow),
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

/// Every way the Webview can be closed comes through here — the titlebar
/// button, the window manager, and `window.dry.close()` from the frontend
/// alike — because a close the application is allowed to refuse is not much of
/// a guarantee if one of the routes in skips the asking.
///
/// The order is the whole point. The application is asked first, while the
/// window is still there to be kept; only then does Python shut down, with the
/// Calls still in flight given their chance to finish; and only then does the
/// event loop exit, which on this platform means the process goes with it.
fn close(webview: &mut WebView, control_flow: &mut ControlFlow) {
  if !closing_allowed() {
    logs::debug(
      logs::WEBVIEW,
      "The close was refused by the close hook, so the window stays open.",
    );
    return;
  }

  shut_down_python();
  exit_app(webview, control_flow);
}

/// Asks Python whether the Webview may close.
///
/// A portal that cannot even be reached cannot refuse: an unreachable module
/// has registered no hook, and a window nothing can close is worse than one
/// that closes without asking.
fn closing_allowed() -> bool {
  Python::attach(|py| match py.import(PORTAL) {
    Ok(portal) => allowed_by(&portal),
    Err(err) => {
      logs::error(
        logs::BRIDGE,
        format!("The close hook could not be reached: {err}"),
      );
      true
    },
  })
}

/// The half of `closing_allowed` a test can hold a portal up to.
fn allowed_by(portal: &Bound<'_, PyModule>) -> bool {
  match portal.call_method0("closing").and_then(|it| it.extract()) {
    Ok(allowed) => allowed,
    Err(err) => {
      logs::error(
        logs::BRIDGE,
        format!("The close hook could not be asked: {err}"),
      );
      true
    },
  }
}

/// Shuts Python down and waits for it, holding the event loop here until it is
/// done. The wait is Python's to bound — the portal gives an in-flight Call a
/// grace period and cuts it short after that — and the GIL is released for the
/// whole of it, so the Calls being waited for can actually run.
fn shut_down_python() {
  Python::attach(|py| match py.import(PORTAL) {
    Ok(portal) => shut_down_through(&portal),
    Err(err) => {
      logs::error(
        logs::BRIDGE,
        format!("Python could not be shut down: {err}"),
      );
    },
  });
}

/// The half of `shut_down_python` a test can hold a portal up to.
fn shut_down_through(portal: &Bound<'_, PyModule>) {
  if let Err(err) = portal.call_method0("closed") {
    logs::error(
      logs::BRIDGE,
      format!("Python did not shut down cleanly: {err}"),
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
