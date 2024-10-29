mod api;
mod window;

use std::{collections::HashMap, error::Error, path::Path};

use pyo3::{prelude::*, types::PyFunction};
use tao::{
    dpi::PhysicalSize,
    error::OsError,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy},
    window::{Icon, Window, WindowBuilder},
};
use wry::{http::Request, Error as WryError, WebView, WebViewBuilder};

use api::{handle_api_requests, API_SCRIPT};
use window::{handle_window_requests, hit_test, HitTestResult, WINDOW_SCRIPT};

#[pymodule]
fn dry(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run, m)?)
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
    let event_loop = IEventLoop::new().unwrap();
    let window = build_window(
        &event_loop.instance,
        title,
        min_size,
        size,
        decorations,
        icon_path,
    )
    .unwrap();
    let ipc_handler = build_ipc_handler(api, event_loop.proxy.clone());
    let webview =
        build_webview(&window, ipc_handler, html, url, dev_tools).unwrap();
    event_loop.run(window, webview);
}

#[derive(Debug)]
enum UserEvent {
    EvaluateJavascript(String),
    Minimize,
    Maximize,
    DragWindow,
    CloseWindow,
    MouseDown(u32, u32),
    MouseMove(u32, u32),
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
        window: Window,
        webview: WebView,
    ) {
        let mut webview = Some(webview);
        self.instance.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::NewEvents(StartCause::Init) => {},
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                }
                | Event::UserEvent(UserEvent::CloseWindow) => {
                    let _ = webview.take();
                    *control_flow = ControlFlow::Exit
                },

                Event::UserEvent(e) => match e {
                    UserEvent::EvaluateJavascript(js_code) => {
                        if let Some(webview) = webview.as_ref() {
                            if let Err(err) = webview.evaluate_script(&js_code)
                            {
                                eprintln!("{:?}", err);
                            }
                        }
                    },
                    UserEvent::Minimize => window.set_minimized(true),
                    UserEvent::Maximize => {
                        window.set_maximized(!window.is_maximized())
                    },
                    UserEvent::DragWindow => window.drag_window().unwrap(),
                    UserEvent::MouseDown(x, y) => {
                        let res = hit_test(
                            window.inner_size(),
                            x,
                            y,
                            window.scale_factor(),
                        );
                        match res {
                            HitTestResult::Client | HitTestResult::NoWhere => {
                            },
                            _ => res.drag_resize_window(&window),
                        }
                    },
                    UserEvent::MouseMove(x, y) => {
                        hit_test(
                            window.inner_size(),
                            x,
                            y,
                            window.scale_factor(),
                        )
                        .change_cursor(&window);
                    },
                    UserEvent::CloseWindow => {},
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
    decorations: Option<bool>,
    icon_path: Option<&str>,
) -> Result<Window, OsError> {
    let min_size = PhysicalSize::new(min_size.0, min_size.1);
    let size = PhysicalSize::new(size.0, size.1);
    let mut window_builder = WindowBuilder::new()
        .with_title(title)
        .with_min_inner_size(min_size)
        .with_inner_size(size);
    if let Some(decorations) = decorations {
        window_builder = window_builder.with_decorations(decorations);
    }
    if let Some(icon_path) = icon_path {
        let icon = load_icon(Path::new(icon_path));
        window_builder = window_builder.with_window_icon(icon);
    }
    Ok(window_builder.build(event_loop)?)
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
    dev_tools: Option<bool>,
) -> Result<WebView, WryError> {
    let mut builder = WebViewBuilder::new()
        .with_initialization_script(API_SCRIPT)
        .with_initialization_script(WINDOW_SCRIPT)
        .with_accept_first_mouse(true)
        .with_ipc_handler(ipc_handler);
    builder = match (html, url) {
        (Some(html), _) => builder.with_html(html),
        (None, Some(url)) => builder.with_url(url),
        (None, None) => panic!("No html or url provided."),
    };
    if let Some(dev_tools) = dev_tools {
        builder = builder.with_devtools(dev_tools);
    }
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
    api: Option<HashMap<String, Py<PyFunction>>>,
    event_loop_proxy: EventLoopProxy<UserEvent>,
) -> impl Fn(Request<String>) + 'static {
    move |request| {
        let request_body = request.body();
        if !request_body.starts_with("{") {
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
