use pyo3::{
    prelude::*,
    types::{PyFunction, PyTuple},
};
use serde_json::json;
use std::{collections::HashMap, str::Split};
use tao::{
    dpi::PhysicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy},
    window::{Window, WindowBuilder},
};
use wry::{http::Request, WebView, WebViewBuilder};

// Custom event type for our event loop
enum ClientEvent {
    EvaluateScript(String),
}

fn build_window(
    event_loop: &EventLoop<ClientEvent>,
    title: String,
    min_width: u32,
    min_height: u32,
    width: u32,
    height: u32,
) -> Window {
    WindowBuilder::new()
        .with_title(title)
        .with_min_inner_size(PhysicalSize::new(min_width, min_height))
        .with_inner_size(PhysicalSize::new(width, height))
        .build(&event_loop)
        .unwrap()
}

fn build_ipc_handler(
    api: HashMap<String, Py<PyFunction>>,
    event_loop_proxy: EventLoopProxy<ClientEvent>,
) -> impl Fn(Request<String>) + 'static {
    move |request: Request<String>| {
        let request_body: &String = request.body();
        let mut request_iterator: Split<'_, [char; 2]> = request_body.split([':', ',']);
        let call_id: &str = request_iterator.next().unwrap_or("");
        let response = if let Some(command) = request_iterator.next() {
            if let Some(function) = api.get(command) {
                let request_arguments: Vec<String> =
                    request_iterator.map(|arg| arg.to_string()).collect();
                Python::with_gil(|py: Python<'_>| {
                    let function_arguments: Bound<'_, PyTuple> =
                        PyTuple::new_bound(py, request_arguments);
                    let result: Result<Bound<'_, PyAny>, PyErr> =
                        function.bind(py).call1(function_arguments);
                    match result {
                        Ok(py_any) => match py_any.str() {
                            Ok(py_string) => {
                                json!({
                                    "callId": call_id,
                                    "result": py_string.to_string(),
                                })
                            }
                            Err(_) => {
                                json!({
                                    "callId": call_id,
                                    "result": null,
                                })
                            }
                        },
                        Err(err) => {
                            eprintln!("Error calling Python function: {:?}", err);
                            json!({
                                "callId": call_id,
                                "error": err.to_string(),
                            })
                        }
                    }
                })
            } else {
                json!({
                    "callId": call_id,
                    "error": format!("Function '{}' not found", command),
                })
            }
        } else {
            json!({
                "callId": call_id,
                "error": "Invalid request format",
            })
        };
        let calling_ipc_callback = format!("window.ipcCallback('{}')", response.to_string());
        if let Err(err) =
            event_loop_proxy.send_event(ClientEvent::EvaluateScript(calling_ipc_callback))
        {
            eprintln!("Error sending event to event loop: {:?}", err.to_string());
        }
    }
}

const JAVASCRIPT: &str = r#"
if (!window.api) {
        window.api = new Proxy({}, {
        get: function(target, name) {
            return function() {
                return new Promise((resolve, reject) => {
                    const callId = Math.random().toString(36).substr(2, 9);
                    const args = Array.from(arguments).join(',');
                    const message = `${callId}:${name},${args}`;
                    window.ipcCallbacks = window.ipcCallbacks || {};
                    window.ipcCallbacks[callId] = { resolve, reject };
                    window.ipc.postMessage(message);
                });
            };
        }
    });
    window.ipcCallback = function(response) {
                const { callId, result, error } = JSON.parse(response);
        if (window.ipcCallbacks && window.ipcCallbacks[callId]) {
            if (error) {
                window.ipcCallbacks[callId].reject(new Error(error));
            } else {
                window.ipcCallbacks[callId].resolve(result);
            }
            delete window.ipcCallbacks[callId];
        }
    };
}
"#;

fn build_webview(
    window: &Window,
    ipc_handler: impl Fn(Request<String>) + 'static,
    html: String,
) -> WebView {
    let builder: WebViewBuilder<'_> = WebViewBuilder::new()
        .with_initialization_script(JAVASCRIPT)
        .with_html(html)
        .with_ipc_handler(ipc_handler)
        .with_accept_first_mouse(true);
    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    let webview: Result<WebView, wry::Error> = builder.build(&window);
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().unwrap();
        builder.build_gtk(vbox)?
    };
    webview.unwrap()
}

fn run_event_loop(event_loop: EventLoop<ClientEvent>, webview: WebView) {
    let mut webview: Option<WebView> = Some(webview);
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => println!("Started!"),
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                let _ = webview.take();
                *control_flow = ControlFlow::Exit
            }
            Event::UserEvent(ClientEvent::EvaluateScript(js_code)) => {
                if let Some(webview) = webview.as_ref() {
                    if let Err(err) = webview.evaluate_script(&js_code) {
                        eprintln!("Error evaluating script: {:?}", err);
                    }
                }
            }
            _ => (),
        }
    });
}

#[pyfunction]
fn run(
    title: String,
    min_width: u32,
    min_height: u32,
    width: u32,
    height: u32,
    html: String,
    api: HashMap<String, Py<PyFunction>>,
) {
    let event_loop: EventLoop<ClientEvent> =
        EventLoopBuilder::<ClientEvent>::with_user_event().build();
    let window: Window = build_window(&event_loop, title, min_width, min_height, width, height);
    let event_loop_proxy = event_loop.create_proxy();
    let ipc_handler = build_ipc_handler(api, event_loop_proxy);
    let webview: WebView = build_webview(&window, ipc_handler, html);
    run_event_loop(event_loop, webview);
}

#[pymodule]
fn dry(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run, m)?)
}
