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
use events::{AppEvent, PROXY, run_event_loop, send_to_event_loop};
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
  default: Option<Py<PyAny>>,
}

/// A Webview open on screen, on its way to the event loop.
///
/// The wrapper exists to cross `Python::detach`, which asks for something
/// `Send`. That bound is not about threads here — `detach` runs the closure on
/// the thread that called it, and none of this may ever leave the main thread
/// anyway. It is how PyO3 keeps a `Python` token from being carried past the
/// point where the GIL is released, and this carries none.
struct Opened {
  event_loop: EventLoop<AppEvent>,
  window: Window,
  webview: WebView,
}

unsafe impl Send for Opened {}

impl Opened {
  /// Hands the main thread to the event loop, for good.
  fn show(self) {
    run_event_loop(self.event_loop, self.window, self.webview);
  }
}

#[pyfunction]
fn run(py: Python<'_>, mut settings: Settings) -> PyResult<()> {
  // The `default=` hook ADR-0002 promises, installed before anything can
  // cross the Bridge.
  types::set_default_hook(settings.default.take());

  let opened = catch_panic(|| open(py, settings))?;

  // The GIL goes back to Python before the event loop takes the main thread,
  // and never comes back: `tao::EventLoop::run` does not return, it exits the
  // process. Holding it would be holding it for the life of the application,
  // and no callback on any other thread could ever run — which is the whole
  // point of the portal. Everything the event loop needs from Python from
  // here on reattaches through `Python::attach`.
  py.detach(move || opened.show());

  Ok(())
}

/// Everything between the settings and a Webview on screen. Runs inside
/// `catch_panic`, so a panic anywhere down here reaches Python as a
/// `PanicError` instead of killing the interpreter. The event loop itself
/// runs outside it: unwinding through a platform event loop is not something
/// that can be caught safely.
fn open(py: Python<'_>, settings: Settings) -> PyResult<Opened> {
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

  Ok(Opened {
    event_loop,
    window,
    webview,
  })
}

#[pyfunction]
fn send_event(message: &str) -> PyResult<()> {
  send_to_event_loop(AppEvent::FromPython(message.to_string()))
    .map_err(BridgeError::new_err)
}
