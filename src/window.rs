use pyo3::{PyResult, pyfunction};
use std::path::Path;
use tao::window::ResizeDirection;
use tao::{
  dpi::{LogicalPosition, LogicalSize},
  error::OsError,
  event_loop::{EventLoop, EventLoopProxy},
  window::{Fullscreen, Icon, Window, WindowBuilder},
};

use crate::{
  errors::WebviewError,
  events::{AppEvent, send_to_event_loop},
  logs,
};

pub mod resize;
pub mod state;

pub const WINDOW_FUNCTIONS_JS: &str = include_str!("js/window_functions.js");
pub const WINDOW_EVENTS_JS: &str = include_str!("js/window_events.js");
pub const WINDOW_BORDERS_JS: &str = include_str!("js/window_borders.js");

pub fn build_window(
  event_loop: &EventLoop<AppEvent>, title: String, min_size: (u32, u32),
  size: (u32, u32), decorations: bool, icon_path: Option<String>,
) -> Result<Window, OsError> {
  resize::remember_min_size(min_size);
  let min_size = LogicalSize::new(min_size.0, min_size.1);
  let size = LogicalSize::new(size.0, size.1);
  let mut window_builder = WindowBuilder::new()
    .with_title(title)
    .with_min_inner_size(min_size)
    .with_inner_size(size)
    .with_decorations(decorations);
  if let Some(icon_path) = icon_path {
    let icon = load_icon(Path::new(&icon_path));
    window_builder = window_builder.with_window_icon(icon);
  }
  let window = window_builder.build(event_loop)?;
  Ok(window)
}

/// An icon that cannot be read is worth a record, not a dead Webview: the
/// window opens with the platform's default icon instead.
fn load_icon(path: &Path) -> Option<Icon> {
  let image = match image::open(path) {
    Ok(image) => image.into_rgba8(),
    Err(err) => {
      logs::warning(
        logs::WEBVIEW,
        format!("The icon at '{}' could not be read: {err}", path.display()),
      );
      return None;
    },
  };
  let (width, height) = image.dimensions();
  Icon::from_rgba(image.into_raw(), width, height).ok()
}

/// One change to a window that is already open.
///
/// Every setting a Webview may still be assigned once it is running arrives
/// here, and it arrives as a message rather than as a call: the window belongs
/// to the thread that draws it, and the Python assigning it is on the portal
/// (ADR-0001). A change is posted, applied on the next turn of the loop, and
/// read back out of the window by the same diff that reports a change the user
/// made — which is why nothing here emits an Event of its own. Emitting one
/// here would either duplicate what the diff is about to emit, or lie about a
/// change the platform refused.
///
/// Sizes and positions are logical pixels, the unit every dimension in the
/// library is in.
#[derive(Debug)]
pub enum Change {
  Title(String),
  Size((u32, u32)),
  MinSize((u32, u32)),
  Decorations(bool),
  Icon(Option<String>),
  Position((i32, i32)),
  Visible(bool),
  Maximized(bool),
  Minimized(bool),
  Fullscreen(bool),
}

/// Applies one change, on the thread that owns the window.
pub fn apply(change: Change, window: &Window) {
  match change {
    Change::Title(title) => window.set_title(&title),
    Change::Size((width, height)) => {
      window.set_inner_size(LogicalSize::new(width, height))
    },
    Change::MinSize(min_size) => {
      resize::remember_min_size(min_size);
      window.set_min_inner_size(Some(LogicalSize::new(min_size.0, min_size.1)));
    },
    Change::Decorations(decorations) => {
      window.set_decorations(decorations);
      // A window that has just lost its titlebar renders into all of itself,
      // so the frame every reported size is taken against has changed.
      state::decorations_changed(window);
    },
    Change::Icon(path) => {
      let icon = path.as_deref().and_then(|path| load_icon(Path::new(path)));
      window.set_window_icon(icon);
    },
    Change::Position((x, y)) => window.set_outer_position(LogicalPosition::new(x, y)),
    Change::Visible(visible) => window.set_visible(visible),
    Change::Maximized(maximized) => window.set_maximized(maximized),
    Change::Minimized(minimized) => window.set_minimized(minimized),
    Change::Fullscreen(fullscreen) => window.set_fullscreen(
      // Borderless on the window's current monitor: the fullscreen a desktop
      // application wants, rather than an exclusive video mode that would
      // change the display's resolution under the user.
      fullscreen.then(|| Fullscreen::Borderless(None)),
    ),
  }
}

/// Posts a change to the running window, or says why there is none.
///
/// The Python side refuses the call before the Webview is running, so this is
/// the backstop for the window that closed between the check and the send.
fn request(change: Change) -> PyResult<()> {
  send_to_event_loop(AppEvent::ChangeWindow(change)).map_err(WebviewError::new_err)
}

/// The Python entry points, one per setting.
///
/// Each is the assignment half of a property on `Webview`, which is where the
/// argument is named and documented; see `dry/interface.py`.
#[pyfunction]
pub fn set_window_title(title: String) -> PyResult<()> {
  request(Change::Title(title))
}

#[pyfunction]
pub fn set_window_size(size: (u32, u32)) -> PyResult<()> {
  request(Change::Size(size))
}

#[pyfunction]
pub fn set_window_min_size(min_size: (u32, u32)) -> PyResult<()> {
  request(Change::MinSize(min_size))
}

#[pyfunction]
pub fn set_window_decorations(decorations: bool) -> PyResult<()> {
  request(Change::Decorations(decorations))
}

#[pyfunction]
pub fn set_window_icon(icon_path: Option<String>) -> PyResult<()> {
  request(Change::Icon(icon_path))
}

#[pyfunction]
pub fn set_window_position(position: (i32, i32)) -> PyResult<()> {
  request(Change::Position(position))
}

#[pyfunction]
pub fn set_window_visible(visible: bool) -> PyResult<()> {
  request(Change::Visible(visible))
}

#[pyfunction]
pub fn set_window_maximized(maximized: bool) -> PyResult<()> {
  request(Change::Maximized(maximized))
}

#[pyfunction]
pub fn set_window_minimized(minimized: bool) -> PyResult<()> {
  request(Change::Minimized(minimized))
}

#[pyfunction]
pub fn set_window_fullscreen(fullscreen: bool) -> PyResult<()> {
  request(Change::Fullscreen(fullscreen))
}

pub fn handle_window_requests(request_body: &String, proxy: &EventLoopProxy<AppEvent>) {
  let mut request = request_body.split([':', ',']);
  request.next(); // Skip the "window_control" prefix

  let action = match request.next() {
    Some(action) => action,
    None => {
      logs::error(
        logs::WEBVIEW,
        format!("Invalid window request: {request_body}"),
      );
      return;
    },
  };

  let result = match action {
    "minimize" => proxy.send_event(AppEvent::MinimizeWindow),
    "toggle_maximize" => proxy.send_event(AppEvent::MaximizeWindow),
    "close" => proxy.send_event(AppEvent::CloseWindow),
    // The frontend half of the state query. It goes round the event loop
    // rather than being answered here because the answer has to be evaluated
    // in the page, which only the thread that owns the WebView may do.
    "state" => proxy.send_event(AppEvent::QueryWindowState),
    "drag" => proxy.send_event(AppEvent::DragWindow),
    "resize" => {
      let direction = match request.next() {
        Some("north") => ResizeDirection::North,
        Some("south") => ResizeDirection::South,
        Some("east") => ResizeDirection::East,
        Some("west") => ResizeDirection::West,
        Some("north-west") => ResizeDirection::NorthWest,
        Some("north-east") => ResizeDirection::NorthEast,
        Some("south-west") => ResizeDirection::SouthWest,
        Some("south-east") => ResizeDirection::SouthEast,
        _ => {
          logs::error(logs::WEBVIEW, "Invalid resize direction.");
          return;
        },
      };
      proxy.send_event(AppEvent::ResizeWindow(direction))
    },
    // A frontend that has no native drag-resize to call runs the drag itself
    // and reports the pointer on every move. See `resize` and ADR-0004.
    "resize_drag" => match resize::parse(&mut request) {
      Some(drag) => proxy.send_event(AppEvent::ResizeDragged(drag)),
      None => {
        logs::error(
          logs::WEBVIEW,
          format!("Invalid resize report: {request_body}"),
        );
        return;
      },
    },
    _ => {
      logs::error(logs::WEBVIEW, format!("Invalid window control: {action}"));
      return;
    },
  };

  if let Err(err) = result {
    logs::error(
      logs::WEBVIEW,
      format!("The window Event could not be sent: {err}"),
    );
  }
}
