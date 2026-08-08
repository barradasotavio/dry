use std::{
  borrow::Cow,
  collections::HashMap,
  ffi::CStr,
  fs,
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
  time::Duration,
};

use pyo3::{
  Py, PyAny, PyResult, Python,
  types::{PyAnyMethods, PyDict, PyDictMethods},
};
use tao::{event_loop::EventLoopProxy, window::Window};
use wry::{
  Error as WryError, PageLoadEvent, WebContext, WebView, WebViewBuilder,
  http::{Request, header::CONTENT_TYPE, response::Response},
};

use crate::{
  api::{API_JS, handle_api_requests},
  errors::WebviewError,
  events::{AppEvent, EVENT_PREFIX, EVENTS_JS, handle_event_request},
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
    // Always, Api or no Api: an Event needs no Api to cross, and the window
    // Events Dry emits itself have to have somewhere to land.
    .with_initialization_script(EVENTS_JS)
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
      None => {
        let arrived = Arc::new(AtomicBool::new(false));
        let watched = Arc::clone(&arrived);

        builder = builder.with_on_page_load_handler(move |event, at| {
          if matches!(event, PageLoadEvent::Finished) && at != BLANK_PAGE {
            watched.store(true, Ordering::Relaxed);
          }
        });

        let webview = builder.with_url(&url).build(window)?;
        watch_navigation(url, arrived);
        webview
      },
    },
    (None, None) => panic!("No content provided."),
  };

  Ok(webview)
}

/// How long a URL Content is given to arrive before Dry goes and finds out why
/// it has not. Long enough that an ordinary page beats it, short enough that a
/// developer staring at a blank window has not yet started guessing.
const NAVIGATION_TIMEOUT: Duration = Duration::from_secs(5);

/// How long the diagnosis itself may take before it gives up.
const PROBE_TIMEOUT: f64 = 5.0;

/// The schemes Dry can say anything useful about. A Root is served from inside
/// this process and answers its own failures with a 403 or a 404 the developer
/// can read in the window, so it is not watched.
const PROBED_SCHEMES: [&str; 3] = ["http://", "https://", "file://"];

/// What the Webview settles on when a navigation never got off the ground, and
/// exactly what the developer is looking at: the blank window. It is not an
/// arrival, whatever the page-load handler calls it.
const BLANK_PAGE: &str = "about:blank";

/// Whether the page-load handler's account of an arrival can be believed.
///
/// On macOS it can. WKWebView commits `about:blank` when a navigation fails,
/// so a Finished anywhere else really is a page, and the watchdog can stand
/// down without asking the address anything.
///
/// On Windows it cannot, and this was measured: wry raises Finished from
/// WebView2's `NavigationCompleted` without consulting the `IsSuccess` on that
/// event, and the URL it reports is `Source()`, which still names the address
/// that failed. WebView2 commits an error page of its own rather than a blank
/// one, so **every** failed navigation there is indistinguishable from an
/// arrival — a refused connection, an unresolvable host and an untrusted
/// certificate alike, all of them silent. Windows therefore asks the address
/// whatever the handler said. That costs a page that did load one extra
/// request, five seconds in; it does not cost it a false report, because
/// nothing is logged unless Python reproduces a concrete failure.
const PAGE_LOAD_REPORTS_ARRIVAL: bool = cfg!(target_os = "macos");

/// Watches one navigation, and only the first.
///
/// wry has no failed-navigation hook. On macOS the failure reaches
/// WKWebView's `didFailProvisionalNavigation:`, which wry's navigation
/// delegate does not implement at all, so nothing is forwarded. What wry does
/// forward, `with_on_page_load_handler`, is measurably useless on its own:
/// traced against a refused connection it reports Started **and Finished**, at
/// `about:blank` — the load "succeeds" onto the blank page. Traced against an
/// unresolvable host it reports nothing whatsoever. So neither the presence of
/// Finished nor its absence decides anything, and both have to be read
/// together: an arrival is a Finished at a page that is not `about:blank`, and
/// a timer covers the navigation that never reports at all.
///
/// A page that has not arrived is then diagnosed from Python, because the
/// answer lives in a library the process already has. `urllib` and `ssl` name
/// an untrusted certificate, a refused connection and an unresolvable host
/// apart, which is precisely what the developer needs and what no webview
/// callback here offers — and it costs no dependency.
///
/// Nothing is reported unless the diagnosis reproduces a concrete failure. A
/// page that is merely slow, or that loaded and rendered nothing, stays out of
/// the log at anything above debug.
fn watch_navigation(url: String, arrived: Arc<AtomicBool>) {
  if !PROBED_SCHEMES.iter().any(|scheme| url.starts_with(scheme)) {
    return;
  }

  thread::spawn(move || {
    thread::sleep(NAVIGATION_TIMEOUT);

    let arrived = arrived.load(Ordering::Relaxed);
    if PAGE_LOAD_REPORTS_ARRIVAL && arrived {
      return;
    }

    match diagnose(&url) {
      Some(reason) => logs::error(
        logs::WEBVIEW,
        format!("The Webview could not load '{url}': {reason}"),
      ),
      // Reachable, and the page-load handler never said otherwise: the page is
      // slow or empty, which is not worth accusing anybody of.
      None if !arrived => logs::debug(
        logs::WEBVIEW,
        format!(
          "The Webview has not finished loading '{url}', but the address \
           answered when Dry asked. The page itself may be slow or empty."
        ),
      ),
      // Reachable, and the page did load. Nothing happened.
      None => {},
    }
  });
}

/// Why an address did not load, in words, or `None` when Dry reached it and
/// has nothing to accuse.
fn diagnose(url: &str) -> Option<String> {
  let (kind, detail) = Python::attach(|py| probe(py, url).ok())?;
  let headline = headline(&kind)?;
  if detail.is_empty() {
    return Some(headline.to_string());
  }
  Some(format!("{headline}. {detail}"))
}

/// The sentence a developer reads first. Every kind the probe can return is
/// named here, and the three the issue turns on — an untrusted certificate, a
/// refused connection, an unresolvable host — read differently enough to tell
/// apart from the log line alone.
fn headline(kind: &str) -> Option<&'static str> {
  match kind {
    "certificate" => Some("the server's TLS certificate is not trusted"),
    "tls" => Some("the TLS connection failed"),
    "host" => Some("the host could not be resolved"),
    "refused" => Some("the connection was refused"),
    "timeout" => Some("the connection timed out"),
    "missing" => Some("there is no file at that path"),
    "failed" => Some("the connection failed"),
    // `reachable`, and anything a later probe learns to say that this build
    // does not understand. Silence beats a guess.
    _ => None,
  }
}

/// Asks the address the same question a developer would, with the standard
/// library they already have, and names what came back.
const PROBE: &CStr = cr#"
import socket
import ssl
import urllib.error
import urllib.request


def classify(reason):
    detail = f'{type(reason).__name__}: {reason}'
    if isinstance(reason, ssl.SSLCertVerificationError):
        return ('certificate', detail)
    if isinstance(reason, ssl.SSLError):
        return ('tls', detail)
    if isinstance(reason, socket.gaierror):
        return ('host', detail)
    if isinstance(reason, ConnectionRefusedError):
        return ('refused', detail)
    if isinstance(reason, TimeoutError):
        return ('timeout', detail)
    if isinstance(reason, FileNotFoundError):
        return ('missing', detail)
    return ('failed', detail)


def probe(url, timeout):
    try:
        urllib.request.urlopen(url, timeout=timeout).close()
    except urllib.error.HTTPError as error:
        # The address answered, just not with a 200. Whatever left the window
        # blank, it was not the connection.
        return ('reachable', f'HTTP {error.code}')
    except urllib.error.URLError as error:
        return classify(error.reason)
    except Exception as error:
        return classify(error)
    return ('reachable', '')
"#;

/// Runs the probe in a scope of its own, so nothing of Dry's diagnosis is left
/// behind in `sys.modules` for an application to trip over.
fn probe(py: Python<'_>, url: &str) -> PyResult<(String, String)> {
  let scope = PyDict::new(py);
  py.run(PROBE, Some(&scope), None)?;
  scope
    .get_item("probe")?
    .ok_or_else(|| WebviewError::new_err("The probe did not define `probe`."))?
    .call1((url, PROBE_TIMEOUT))?
    .extract::<(String, String)>()
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

/// The one handler every message from the frontend arrives at, on the thread
/// that owns the window. Neither branch blocks: a window request is a message
/// to the event loop, and a Call is handed to the portal and left to finish on
/// its own thread.
pub fn build_ipc_handler(
  api: Option<HashMap<String, Py<PyAny>>>, event_loop_proxy: EventLoopProxy<AppEvent>,
) -> impl Fn(Request<String>) + 'static {
  move |request| {
    let request_body = request.body();

    if request_body.starts_with("window_control") {
      handle_window_requests(request_body, &event_loop_proxy);
      return;
    }

    // An Event, unlike a Call, reaches Python whether or not there is an Api.
    if let Some(event) = request_body.strip_prefix(EVENT_PREFIX) {
      handle_event_request(event);
      return;
    }

    if let Some(api) = &api
      && let Err(err) = handle_api_requests(request_body, api)
    {
      logs::error(
        logs::BRIDGE,
        format!("The Call could not be handled: {err}"),
      );
    }
  }
}
