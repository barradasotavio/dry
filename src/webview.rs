use std::{
  borrow::Cow,
  collections::HashMap,
  fs,
  path::{Path, PathBuf},
};

use pyo3::{types::PyFunction, Py};
use tao::{event_loop::EventLoopProxy, window::Window};
use wry::{
  http::{header::CONTENT_TYPE, response::Response, Request},
  Error as WryError, WebContext, WebView, WebViewBuilder,
};

use crate::{
  api::{handle_api_requests, API_JS},
  events::AppEvent,
  window::{
    handle_window_requests, WINDOW_BORDERS_JS, WINDOW_EVENTS_JS, WINDOW_FUNCTIONS_JS,
  },
};

pub fn build_webview(
  window: &Window, ipc_handler: impl Fn(Request<String>) + 'static,
  html: Option<String>, url: Option<String>, decorations: bool, api: bool,
  dev_tools: bool, udf: String,
) -> Result<WebView, WryError> {
  let data_directory = PathBuf::from(udf);
  let mut web_context = WebContext::new(Some(data_directory));

  let mut builder = WebViewBuilder::with_web_context(&mut web_context)
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

  let webview = match (html, url) {
    (Some(html), _) => builder.with_html(html).build(window)?,
    (None, Some(url)) => {
      if url.starts_with("localfile://") {
        let file_path = url.trim_start_matches("localfile://").to_string();

        builder = builder.with_custom_protocol(
          "localfile".into(),
          move |_webview_id, _request| {
            handle_file_request(&file_path)
          },
        );

        builder = builder.with_url("localfile://localhost/");
        builder.build(window)?
      } else {
        builder.with_url(url).build(window)?
      }
    }
    (None, None) => panic!("No html or url provided."),
  };

  Ok(webview)
}

fn handle_file_request(file_path: &str) -> Response<Cow<'static, [u8]>> {
  let path = Path::new(file_path);

  if let Ok(content) = fs::read(path) {
    let mime_type = match path.extension().and_then(|ext| ext.to_str()) {
      Some("html") => "text/html",
      Some("js") => "text/javascript",
      Some("css") => "text/css",
      Some("png") | Some("jpg") | Some("jpeg") | Some("gif") | Some("svg") => "image/",
      _ => "application/octet-stream",
    };

    Response::builder()
      .header(CONTENT_TYPE, mime_type)
      .body(Cow::Owned(content))
      .unwrap()
  } else {
    Response::builder()
      .header(CONTENT_TYPE, "text/plain")
      .status(404)
      .body(Cow::Owned(b"File not found".to_vec()))
      .unwrap()
  }
}

pub fn build_ipc_handler(
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
      if let Err(err) = handle_api_requests(request_body, api, &event_loop_proxy) {
        eprintln!("{:?}", err);
      }
    }
  }
}
