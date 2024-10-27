use std::{collections::HashMap, error::Error};

use pyo3::{
    prelude::*,
    types::{PyFunction, PyTuple},
};
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use tao::{
    dpi::PhysicalSize,
    error::OsError,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy},
    window::{Window, WindowBuilder},
};
use wry::{http::Request, Error as WryError, WebView, WebViewBuilder};

#[pymodule]
fn dry(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run, m)?)
}

#[pyfunction]
fn run(
    title: &str,
    min_size: (u32, u32),
    size: (u32, u32),
    html: &str,
    startup_script: &str,
    api: HashMap<String, Py<PyFunction>>,
) {
    let event_loop = IEventLoop::new().unwrap();
    let window =
        build_window(&event_loop.instance, title, min_size, size).unwrap();
    let ipc_handler = build_ipc_handler(api, event_loop.proxy.clone());
    let webview =
        build_webview(&window, ipc_handler, startup_script, html).unwrap();
    event_loop.run(webview);
}

#[derive(Debug)]
enum UserEvent {
    EvaluateJavascript(String),
}

struct IEventLoop {
    instance: EventLoop<UserEvent>,
    proxy: EventLoopProxy<UserEvent>,
}

impl IEventLoop {
    fn new() -> Result<Self, Box<dyn Error>> {
        let event_loop =
            EventLoopBuilder::<UserEvent>::with_user_event().build();
        let event_loop_proxy = event_loop.create_proxy();
        Ok(Self {
            instance: event_loop,
            proxy: event_loop_proxy,
        })
    }

    fn run(
        self,
        webview: WebView,
    ) {
        let mut webview = Some(webview);
        self.instance.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::NewEvents(StartCause::Init) => println!("Started!"),
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    let _ = webview.take();
                    *control_flow = ControlFlow::Exit
                },
                Event::UserEvent(UserEvent::EvaluateJavascript(js_code)) => {
                    if let Some(webview) = webview.as_ref() {
                        if let Err(err) = webview.evaluate_script(&js_code) {
                            eprintln!("Error evaluating script: {:?}", err);
                        }
                    }
                },
                _ => (),
            }
        });
    }
}

fn build_window(
    event_loop: &EventLoop<UserEvent>,
    title: &str,
    min_size: (u32, u32),
    size: (u32, u32),
) -> Result<Window, OsError> {
    let min_size = PhysicalSize::new(min_size.0, min_size.1);
    let size = PhysicalSize::new(size.0, size.1);
    let window = WindowBuilder::new()
        .with_title(title)
        .with_min_inner_size(min_size)
        .with_inner_size(size)
        .build(event_loop)?;
    Ok(window)
}

fn build_webview(
    window: &Window,
    ipc_handler: impl Fn(Request<String>) + 'static,
    startup_script: &str,
    html: &str,
) -> Result<WebView, WryError> {
    let builder = WebViewBuilder::new()
        .with_initialization_script(startup_script)
        .with_html(html)
        .with_ipc_handler(ipc_handler)
        .with_accept_first_mouse(true);
    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    let webview = builder.build(window)?;
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox()?;
        builder.build_gtk(vbox)?
    };
    Ok(webview)
}

fn build_ipc_handler(
    api: HashMap<String, Py<PyFunction>>,
    event_loop_proxy: EventLoopProxy<UserEvent>,
) -> impl Fn(Request<String>) + 'static {
    move |request| {
        let call_request: CallRequest = match from_str(request.body()) {
            Ok(call_request) => call_request,
            Err(err) => {
                eprintln!(
                    "Error parsing request: {:?}. Request body: {}",
                    err,
                    request.body()
                );
                return;
            },
        };
        let call_response = match call_request.run(&api) {
            Ok(call_response) => call_response,
            Err(err) => {
                eprintln!("Error executing request: {:?}", err);
                CallResponse {
                    call_id: call_request.call_id,
                    result: None,
                    error: Some(err.to_string()),
                }
            },
        };
        if let Err(err) = call_response.run(&event_loop_proxy) {
            eprintln!("Error sending response: {:?}", err);
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, FromPyObject)]
#[serde(untagged)]
enum CommonKey {
    Boolean(bool),
    Integer(i64),
    String(String),
}

impl ToPyObject for CommonKey {
    fn to_object(
        &self,
        py: Python,
    ) -> PyObject {
        match self {
            CommonKey::Boolean(value) => value.to_object(py),
            CommonKey::Integer(value) => value.to_object(py),
            CommonKey::String(value) => value.to_object(py),
        }
    }
}

#[derive(FromPyObject, Serialize, Deserialize)]
#[serde(untagged)]
enum CommonType {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<CommonType>),
    Dict(HashMap<CommonKey, CommonType>),
}

impl ToPyObject for CommonType {
    fn to_object(
        &self,
        py: Python,
    ) -> PyObject {
        match self {
            CommonType::Boolean(value) => value.to_object(py),
            CommonType::Integer(value) => value.to_object(py),
            CommonType::Float(value) => value.to_object(py),
            CommonType::String(value) => value.to_object(py),
            CommonType::List(value) => value.to_object(py),
            CommonType::Dict(value) => value.to_object(py),
        }
    }
}

#[derive(Deserialize)]
struct CallRequest {
    call_id: String,
    function: String,
    arguments: Option<Vec<CommonType>>,
}

impl CallRequest {
    fn run(
        &self,
        api: &HashMap<String, Py<PyFunction>>,
    ) -> Result<CallResponse, Box<dyn Error>> {
        let py_func = api.get(&self.function).ok_or("Function not found")?;
        Python::with_gil(|py| {
            let py_args = match &self.arguments {
                Some(args) => PyTuple::new_bound(py, args),
                None => PyTuple::empty_bound(py),
            };
            let py_result: Option<CommonType> =
                py_func.call1(py, py_args)?.extract(py)?;
            Ok(CallResponse {
                call_id: self.call_id.clone(),
                result: py_result,
                error: None,
            })
        })
    }
}

#[derive(Serialize)]
struct CallResponse {
    call_id: String,
    result: Option<CommonType>,
    error: Option<String>,
}

impl CallResponse {
    fn run(
        &self,
        event_loop_proxy: &EventLoopProxy<UserEvent>,
    ) -> Result<(), Box<dyn Error>> {
        let response = format!("window.ipcCallback({})", to_string(self)?);
        println!("Response: {}", response);
        event_loop_proxy
            .send_event(UserEvent::EvaluateJavascript(response))?;
        Ok(())
    }
}
