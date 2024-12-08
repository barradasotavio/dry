mod api;
mod window;

use std::{collections::HashMap, path::Path};

use pyo3::{prelude::*, types::PyFunction};
use std::sync::{Mutex, OnceLock};
use tao::{
    dpi::PhysicalSize,
    error::OsError,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy},
    window::{Icon, ResizeDirection, Window, WindowBuilder},
};
use wry::{http::Request, Error as WryError, WebView, WebViewBuilder};

use api::{handle_api_requests, API_JS};
use window::{
    handle_window_requests, WINDOW_BORDERS_JS, WINDOW_EVENTS_JS,
    WINDOW_FUNCTIONS_JS,
};

static PYTHON_EVENT_SENDER: OnceLock<Mutex<Option<EventLoopProxy<AppEvent>>>> =
    OnceLock::new();

#[pymodule]
fn dry(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run, m)?)?;
    m.add_function(wrap_pyfunction!(send_message, m)?)?;
    Ok(())
}

#[pyfunction(signature=(
    title,
    min_size,
    size,
    decorations=None,
    icon_path=None,
    html=None,
    url=None,
    api=None,
    dev_tools=None,
))]
fn run(
    title: &str,
    min_size: (u32, u32),
    size: (u32, u32),
    decorations: Option<bool>,
    icon_path: Option<&str>,
    html: Option<&str>,
    url: Option<&str>,
    api: Option<HashMap<String, Py<PyFunction>>>,
    dev_tools: Option<bool>,
) {
    let (is_decorations, is_api, is_dev_tools) = (
        decorations.unwrap_or(true),
        api.is_some(),
        dev_tools.unwrap_or(false),
    );

    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();

    let proxy = event_loop.create_proxy();

    PYTHON_EVENT_SENDER.get_or_init(|| Mutex::new(Some(proxy.clone())));

    let window = build_window(
        &event_loop,
        title,
        min_size,
        size,
        is_decorations,
        icon_path,
    )
    .unwrap();

    let ipc_handler = build_ipc_handler(api, proxy);

    let webview = build_webview(
        &window,
        ipc_handler,
        html,
        url,
        is_decorations,
        is_api,
        is_dev_tools,
    )
    .unwrap();

    run_event_loop(event_loop, window, webview);
}

#[derive(Debug)]
enum AppEvent {
    RunJavascript(String),
    DragWindow,
    MinimizeWindow,
    MaximizeWindow,
    CloseWindow,
    ResizeWindow(ResizeDirection),
    FromPython(String),
}

#[pyfunction]
fn send_message(message: &str) -> PyResult<()> {
    if let Some(sender) = &*PYTHON_EVENT_SENDER
        .get()
        .ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "Event loop not initialized",
            )
        })?
        .lock()
        .map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Lock poisoned")
        })?
    {
        sender
            .send_event(AppEvent::FromPython(message.to_string()))
            .map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                    e.to_string(),
                )
            })?;
        Ok(())
    } else {
        Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
            "Event loop not running",
        ))
    }
}

fn run_event_loop(
    event_loop: EventLoop<AppEvent>,
    window: Window,
    webview: WebView,
) {
    let mut webview = webview;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {
                println!("{} started", window.title());
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

fn exit_app(
    webview: &mut WebView,
    control_flow: &mut ControlFlow,
) {
    let mut webview = Some(webview);
    webview.take();
    *control_flow = ControlFlow::Exit;
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
        AppEvent::MinimizeWindow => window.set_minimized(true),
        AppEvent::MaximizeWindow => toggle_maximize(window),
        AppEvent::DragWindow => drag(window),
        AppEvent::ResizeWindow(direction) => {
            if let Err(err) = window.drag_resize_window(direction) {
                eprintln!("Failed to resize window: {:?}", err);
            }
        },
        AppEvent::FromPython(message) => println!("{}", message),
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

fn toggle_maximize(window: &Window) {
    let is_maximized = window.is_maximized();
    window.set_maximized(!is_maximized);
}

fn drag(window: &Window) {
    if let Err(err) = window.drag_window() {
        eprintln!("Failed to drag window: {:?}", err);
    }
}

fn build_window(
    event_loop: &EventLoop<AppEvent>,
    title: &str,
    min_size: (u32, u32),
    size: (u32, u32),
    decorations: bool,
    icon_path: Option<&str>,
) -> Result<Window, OsError> {
    let min_size = PhysicalSize::new(min_size.0, min_size.1);
    let size = PhysicalSize::new(size.0, size.1);
    let mut window_builder = WindowBuilder::new()
        .with_title(title)
        .with_min_inner_size(min_size)
        .with_inner_size(size)
        .with_decorations(decorations);
    if let Some(icon_path) = icon_path {
        let icon = load_icon(Path::new(icon_path));
        window_builder = window_builder.with_window_icon(icon);
    }
    let window = window_builder.build(event_loop)?;
    Ok(window)
}

fn load_icon(path: &Path) -> Option<Icon> {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    Icon::from_rgba(icon_rgba, icon_width, icon_height).ok()
}

fn build_webview(
    window: &Window,
    ipc_handler: impl Fn(Request<String>) + 'static,
    html: Option<&str>,
    url: Option<&str>,
    decorations: bool,
    api: bool,
    dev_tools: bool,
) -> Result<WebView, WryError> {
    let mut builder = WebViewBuilder::new()
        .with_initialization_script(WINDOW_FUNCTIONS_JS)
        .with_initialization_script(WINDOW_EVENTS_JS)
        .with_devtools(dev_tools)
        .with_ipc_handler(ipc_handler);
    if api {
        builder = builder.with_initialization_script(API_JS);
    }
    if !decorations {
        builder = builder.with_initialization_script(WINDOW_BORDERS_JS);
    }
    builder = match (html, url) {
        (Some(html), _) => builder.with_html(html),
        (None, Some(url)) => builder.with_url(url),
        (None, None) => panic!("No html or url provided."),
    };
    let webview = builder.build(window)?;
    Ok(webview)
}

fn build_ipc_handler(
    api: Option<HashMap<String, Py<PyFunction>>>,
    event_loop_proxy: EventLoopProxy<AppEvent>,
) -> impl Fn(Request<String>) + 'static {
    move |request| {
        let request_body = request.body();

        if request_body.starts_with("window_control") {
            handle_window_requests(request_body, &event_loop_proxy);
            return;
        }

        if let Some(api) = &api {
            if let Err(err) =
                handle_api_requests(request_body, api, &event_loop_proxy)
            {
                eprintln!("{:?}", err);
            }
        }
    }
}
