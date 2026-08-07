use std::{
  borrow::Cow,
  collections::HashMap,
  fs,
  path::{Path, PathBuf},
};

use pyo3::{Py, PyAny};
use tao::{event_loop::EventLoopProxy, window::Window};
use wry::{
  Error as WryError, WebContext, WebView, WebViewBuilder,
  http::{Request, header::CONTENT_TYPE, response::Response},
};

use crate::{
  api::{API_JS, handle_api_requests},
  events::AppEvent,
  logs,
  window::{
    WINDOW_BORDERS_JS, WINDOW_EVENTS_JS, WINDOW_FUNCTIONS_JS, handle_window_requests,
  },
};

#[cfg(test)]
mod tests;

pub const NAMESPACE_JS: &str = include_str!("js/namespace.js");

/// The scheme of the internal protocol a Root is served over.
const ROOT_SCHEME: &str = "localfile";

/// The prefix Python uses to hand a Root down as a URL. Everything after it is
/// the absolute path of the directory to serve.
const ROOT_URL_PREFIX: &str = "localfile://";

/// Where the Webview starts when the Content is a Root. Every relative asset a
/// page requests resolves against this origin and lands in the Root.
const ROOT_ORIGIN: &str = "localfile://localhost/";

/// The file a request for a directory is served from.
const ROOT_INDEX: &str = "index.html";

pub fn build_webview(
  window: &Window, ipc_handler: impl Fn(Request<String>) + 'static,
  html: Option<String>, url: Option<String>, decorations: bool, api: bool,
  dev_tools: bool, udf: String,
) -> Result<WebView, WryError> {
  let data_directory = PathBuf::from(udf);
  let mut web_context = WebContext::new(Some(data_directory));

  let mut builder = WebViewBuilder::new_with_web_context(&mut web_context)
    .with_initialization_script(NAMESPACE_JS)
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
    (None, Some(url)) => match url.strip_prefix(ROOT_URL_PREFIX) {
      Some(directory) => {
        let root = Root::new(PathBuf::from(directory));

        builder = builder
          .with_custom_protocol(ROOT_SCHEME.into(), move |_webview_id, request| {
            root.serve(request.uri().path())
          })
          .with_url(ROOT_ORIGIN);

        builder.build(window)?
      },
      None => builder.with_url(url).build(window)?,
    },
    (None, None) => panic!("No content provided."),
  };

  Ok(webview)
}

/// A Root: a local directory served to the Webview, one file per request.
///
/// The directory is canonicalised once, up front, so every resolved path can be
/// tested against it without walking symlinks again on each request.
struct Root {
  directory: PathBuf,
}

/// Why a request did not reach a file inside the Root.
#[derive(Debug, PartialEq, Eq)]
enum Rejection {
  /// The request resolved outside the Root, or tried to.
  Outside,
  /// The request resolved inside the Root but there is no file there.
  NotFound,
}

impl Root {
  fn new(directory: PathBuf) -> Self {
    let directory = directory.canonicalize().unwrap_or(directory);
    Root { directory }
  }

  /// Answers one request for a path beneath the Root.
  fn serve(&self, request_path: &str) -> Response<Cow<'static, [u8]>> {
    match self.resolve(request_path) {
      Ok(path) => match fs::read(&path) {
        Ok(content) => respond(200, content_type(&path), Cow::Owned(content)),
        Err(_) => respond(404, "text/plain; charset=utf-8", not_found(request_path)),
      },
      Err(Rejection::NotFound) => {
        respond(404, "text/plain; charset=utf-8", not_found(request_path))
      },
      Err(Rejection::Outside) => respond(
        403,
        "text/plain; charset=utf-8",
        Cow::Owned(format!("Outside the root: {request_path}").into_bytes()),
      ),
    }
  }

  /// Turns a requested path into the file it names inside the Root.
  ///
  /// A path escaping the Root is refused twice over: `..` and anything that
  /// would be read as a separator or a drive are rejected before the join, and
  /// the canonical result is then required to sit beneath the Root, which
  /// catches a symlink pointing out of it.
  fn resolve(&self, request_path: &str) -> Result<PathBuf, Rejection> {
    let decoded = percent_decode(request_path);

    let mut candidate = self.directory.clone();
    for component in decoded.split('/') {
      if component.is_empty() || component == "." {
        continue;
      }
      if component == ".."
        || component.contains('\\')
        || component.contains(':')
        || component.contains('\0')
      {
        return Err(Rejection::Outside);
      }
      candidate.push(component);
    }

    if candidate.is_dir() {
      candidate.push(ROOT_INDEX);
    }

    let resolved = candidate.canonicalize().map_err(|_| Rejection::NotFound)?;

    if !resolved.starts_with(&self.directory) {
      return Err(Rejection::Outside);
    }

    if !resolved.is_file() {
      return Err(Rejection::NotFound);
    }

    Ok(resolved)
  }
}

fn respond(
  status: u16, content_type: &str, body: Cow<'static, [u8]>,
) -> Response<Cow<'static, [u8]>> {
  Response::builder()
    .status(status)
    .header(CONTENT_TYPE, content_type)
    .body(body)
    .expect("Failed to build response")
}

fn not_found(request_path: &str) -> Cow<'static, [u8]> {
  Cow::Owned(format!("Not found: {request_path}").into_bytes())
}

/// The content type of a file, read from its extension. An extension we do not
/// know gets `application/octet-stream` rather than a guess at the bytes.
fn content_type(path: &Path) -> &'static str {
  let extension = path
    .extension()
    .and_then(|extension| extension.to_str())
    .unwrap_or_default()
    .to_ascii_lowercase();

  match extension.as_str() {
    "html" | "htm" => "text/html; charset=utf-8",
    "js" | "mjs" => "text/javascript; charset=utf-8",
    "css" => "text/css; charset=utf-8",
    "json" | "map" => "application/json; charset=utf-8",
    "txt" => "text/plain; charset=utf-8",
    "csv" => "text/csv; charset=utf-8",
    "xml" => "application/xml; charset=utf-8",
    "wasm" => "application/wasm",
    "pdf" => "application/pdf",
    "svg" => "image/svg+xml",
    "png" => "image/png",
    "jpg" | "jpeg" => "image/jpeg",
    "gif" => "image/gif",
    "webp" => "image/webp",
    "avif" => "image/avif",
    "bmp" => "image/bmp",
    "ico" => "image/x-icon",
    "woff" => "font/woff",
    "woff2" => "font/woff2",
    "ttf" => "font/ttf",
    "otf" => "font/otf",
    "mp3" => "audio/mpeg",
    "wav" => "audio/wav",
    "ogg" | "oga" => "audio/ogg",
    "mp4" => "video/mp4",
    "webm" => "video/webm",
    _ => "application/octet-stream",
  }
}

/// Reads the percent-escapes out of a request path, so a file whose name holds
/// a space or a non-ASCII character is found on disk.
fn percent_decode(input: &str) -> String {
  fn hex(byte: u8) -> Option<u8> {
    match byte {
      b'0'..=b'9' => Some(byte - b'0'),
      b'a'..=b'f' => Some(byte - b'a' + 10),
      b'A'..=b'F' => Some(byte - b'A' + 10),
      _ => None,
    }
  }

  let bytes = input.as_bytes();
  let mut decoded = Vec::with_capacity(bytes.len());
  let mut index = 0;

  while index < bytes.len() {
    if bytes[index] == b'%'
      && let Some(&high) = bytes.get(index + 1)
      && let Some(&low) = bytes.get(index + 2)
      && let (Some(high), Some(low)) = (hex(high), hex(low))
    {
      decoded.push(high * 16 + low);
      index += 3;
      continue;
    }
    decoded.push(bytes[index]);
    index += 1;
  }

  String::from_utf8_lossy(&decoded).into_owned()
}

pub fn build_ipc_handler(
  api: Option<HashMap<String, Py<PyAny>>>, event_loop_proxy: EventLoopProxy<AppEvent>,
) -> impl Fn(Request<String>) + 'static {
  move |request| {
    let request_body = request.body();

    if request_body.starts_with("window_control") {
      handle_window_requests(request_body, &event_loop_proxy);
      return;
    }

    if let Some(api) = &api
      && let Err(err) = handle_api_requests(request_body, api, &event_loop_proxy)
    {
      logs::error(
        logs::BRIDGE,
        format!("The Call could not be handled: {err}"),
      );
    }
  }
}
