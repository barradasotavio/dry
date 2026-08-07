mod api;
mod errors;
mod events;
mod logs;
mod types;
mod webview;
mod window;

use pyo3::prelude::*;
use std::{collections::HashMap, sync::Mutex};
use tao::{
  event_loop::{EventLoop, EventLoopBuilder},
  window::Window,
};
use wry::WebView;

use errors::{BridgeError, WebviewError, catch_panic};
use events::{AppEvent, PROXY, run_event_loop};
use webview::{build_ipc_handler, build_webview};
use window::build_window;

#[pymodule(gil_used = true)]
fn dry(m: &Bound<'_, PyModule>) -> PyResult<()> {
  m.add_function(wrap_pyfunction!(run, m)?)?;
  m.add_function(wrap_pyfunction!(send_event, m)?)?;
  errors::register(m)?;
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
  api: Option<HashMap<String, Py<PyAny>>>,
  dev_tools: bool,
  user_data_folder: String,
}

#[pyfunction]
fn run(py: Python<'_>, settings: Settings) -> PyResult<()> {
  let (event_loop, window, webview) = catch_panic(|| open(py, settings))?;
  run_event_loop(event_loop, window, webview);
  Ok(())
}

/// Everything between the settings and a Webview on screen. Runs inside
/// `catch_panic`, so a panic anywhere down here reaches Python as a
/// `PanicError` instead of killing the interpreter. The event loop itself
/// runs outside it: unwinding through a platform event loop is not something
/// that can be caught safely.
fn open(
  py: Python<'_>, settings: Settings,
) -> PyResult<(EventLoop<AppEvent>, Window, WebView)> {
  if let Some(api) = &settings.api {
    for (name, entry) in api {
      if !entry.bind(py).is_callable() {
        return Err(BridgeError::new_err(format!(
          "Api entry '{name}' is not callable."
        )));
      }
    }
  }

  if settings.html.is_none() && settings.url.is_none() {
    return Err(WebviewError::new_err(
      "The Webview has no Content. Set content to an HTML string, a URL, or a path.",
    ));
  }

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
  .map_err(|err| {
    WebviewError::new_err(format!("The window could not be built: {err}"))
  })?;

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
  .map_err(|err| {
    WebviewError::new_err(format!("The web content could not be built: {err}"))
  })?;

  Ok((event_loop, window, webview))
}

#[pyfunction]
fn send_event(message: &str) -> PyResult<()> {
  if let Some(sender) = &*PROXY
    .get()
    .ok_or_else(|| BridgeError::new_err("The Bridge is not initialized."))?
    .lock()
    .map_err(|_| BridgeError::new_err("The Bridge is poisoned."))?
  {
    sender
      .send_event(AppEvent::FromPython(message.to_string()))
      .map_err(|err| {
        BridgeError::new_err(format!("The Event could not be sent: {err}"))
      })?;
    Ok(())
  } else {
    Err(BridgeError::new_err("The Bridge is not running."))
  }
}
