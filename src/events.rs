use std::sync::{Mutex, OnceLock};
use tao::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
    window::{ResizeDirection, Window},
};
use wry::WebView;

pub static PROXY: OnceLock<Mutex<Option<EventLoopProxy<AppEvent>>>> =
    OnceLock::new();

#[derive(Debug)]
pub enum AppEvent {
    RunJavascript(String),
    DragWindow,
    MinimizeWindow,
    MaximizeWindow,
    CloseWindow,
    ResizeWindow(ResizeDirection),
    FromPython(String),
}

pub fn run_event_loop(
    event_loop: EventLoop<AppEvent>,
    window: Window,
    webview: WebView,
) {
    let mut webview = webview;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {
                println!("{} started.", window.title());
            },
            Event::WindowEvent { event, .. } => {
                handle_window_event(event, &mut webview, control_flow)
            },
            Event::UserEvent(app_event) => handle_app_event(
                app_event,
                &window,
                &mut webview,
                control_flow,
            ),
            _ => (),
        }
    });
}

fn handle_window_event(
    event: WindowEvent,
    webview: &mut WebView,
    control_flow: &mut ControlFlow,
) {
    match event {
        WindowEvent::CloseRequested => exit_app(webview, control_flow),
        _ => (),
    }
}

fn handle_app_event(
    event: AppEvent,
    window: &Window,
    webview: &mut WebView,
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
                eprintln!("Failed to resize window: {:?}", err);
            }
        },
        AppEvent::FromPython(message) => handle_python_event(&message),
    }
}

fn run_javascript(
    webview: &WebView,
    js: &str,
) {
    if let Err(err) = webview.evaluate_script(js) {
        eprintln!("Failed to evaluate JavaScript: {:?}", err);
    }
}

fn exit_app(
    webview: &mut WebView,
    control_flow: &mut ControlFlow,
) {
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
        eprintln!("Failed to drag window: {:?}", err);
    }
}

fn handle_python_event(message: &str) {
    println!("{}", message);
}
