mod api;
mod events;
mod types;
mod webview;
mod window;

use pyo3::{prelude::*, types::PyFunction};
use std::{collections::HashMap, sync::Mutex};
use tao::event_loop::EventLoopBuilder;

use events::{run_event_loop, AppEvent, PROXY};
use webview::{build_ipc_handler, build_webview};
use window::build_window;

#[pymodule]
fn dry(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run, m)?)?;
    m.add_function(wrap_pyfunction!(send_event, m)?)?;
    Ok(())
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
struct Settings {
    title: String,
    min_size: (u32, u32),
    size: (u32, u32),
    decorations: bool,
    icon_path: Option<String>,
    html: Option<String>,
    url: Option<String>,
    api: Option<HashMap<String, Py<PyFunction>>>,
    dev_tools: bool,
    user_data_folder: String,
}

#[pyfunction]
fn run(settings: Settings) {
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();

    let proxy = event_loop.create_proxy();
    PROXY.get_or_init(|| Mutex::new(Some(proxy.clone())));

    let window = build_window(
        &event_loop,
        settings.title,
        settings.min_size,
        settings.size,
        settings.decorations,
        settings.icon_path,
    )
    .expect("Failed to build window");

    let has_api = settings.api.is_some();
    let ipc_handler = build_ipc_handler(settings.api, proxy);

    let webview = build_webview(
        &window,
        ipc_handler,
        settings.html,
        settings.url,
        settings.decorations,
        has_api,
        settings.dev_tools,
        settings.user_data_folder,
    )
    .expect("Failed to build webview");

    run_event_loop(event_loop, window, webview);
}

#[pyfunction]
fn send_event(message: &str) -> PyResult<()> {
    if let Some(sender) = &*PROXY
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
