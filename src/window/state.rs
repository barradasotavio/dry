//! What the window is doing, turned into Events the frontend can listen for.
//!
//! A custom titlebar can already command the window but cannot see it, so it
//! keeps its own guess at whether the window is maximised — and that guess is
//! wrong the moment the user double-clicks the titlebar or reaches for an OS
//! keyboard shortcut. This module is how the window answers back.
//!
//! ## Reserved names
//!
//! Every name here starts with `window:`, the prefix both public doors refuse
//! (see `events::RESERVED_PREFIX`), so a listener for one of these is hearing
//! from Dry and nothing else. They are ordinary Events on the ordinary bus:
//! `wv.on('window:resized', ...)` in Python and
//! `window.dry.on('window:resized', ...)` in the frontend, and every one of
//! them reaches both sides.
//!
//! The names come in opposed pairs — `maximized`/`unmaximized`,
//! `minimized`/`restored`, `hidden`/`shown`, `focused`/`blurred` — because a
//! frontend that wants to track a boolean wants two names, not one name with a
//! boolean inside it: `on('window:maximized', ...)` reads as the thing that
//! happened, where `on('window:maximize', it => ...)` makes every listener
//! unwrap a value before it knows what it was told. The two that are not
//! booleans, `resized` and `moved`, carry the new value instead, because a
//! resize without the new size would send every listener straight back to ask
//! for it.
//!
//! ## Sizes are logical pixels
//!
//! `size=` and `min_size=` on the Webview are logical pixels (#1), so these
//! are too — a frontend told its window is 1600 wide when CSS says 800 would
//! be a silent inconsistency across one library. Physical pixels are converted
//! by the window's current scale factor and rounded to whole logical pixels,
//! which is also what keeps a drag across a fractional scale factor from
//! reporting a new size on every frame.
//!
//! ## Coalescing
//!
//! `moved` and `resized` are emitted **at most once per turn of the event
//! loop, and only when the value actually changed**. Nothing here listens for
//! `WindowEvent::Resized` or `WindowEvent::Moved`; the window is read once at
//! `MainEventsCleared`, after the platform has handed over everything it had,
//! and the reading is compared with the last one.
//!
//! That is the coalescing decision, and the reason for it is that a turn of
//! the event loop is the natural rate limit: a drag that fires a hundred
//! platform events in a second cannot produce more Events than the window got
//! turns to draw in, so a listener can never fall behind the loop feeding it.
//! It also needs no timer. A throttle on a clock would have to put a deadline
//! on a loop that otherwise sleeps in `ControlFlow::Wait`, and would have to
//! grow a trailing flush to avoid leaving the frontend holding a stale size
//! after the drag stopped — which is exactly the bug it would be introduced to
//! avoid. Comparing against the last reading gives the trailing edge for free:
//! the last turn of the loop always carries the final geometry.
//!
//! ## Asking instead of listening
//!
//! The Events say what changed; the state query says what is. A frontend that
//! has only just loaded, or a Python callback that was not listening, has
//! observed nothing and still has to draw a maximize button one way round.
//! `window_state()` answers from the last reading the loop took — the same
//! reading the diff was taken against — so the query and the Events can never
//! disagree about the window, and no second, slower source of truth exists to
//! disagree with.
//!
//! ## A window that is not on screen has nothing to report
//!
//! While the window is minimized or hidden, the platform's answers for size,
//! position and maximized state are not about the window the user will see
//! again — Windows parks a minimized window at -32000. Those three are left at
//! their last observed values until the window comes back, so a minimize does
//! not fire a spurious `moved`, and a restore does not have to correct one.

use pyo3::{Bound, PyAny, PyResult, Python};
use std::sync::Mutex;
use tao::{
  dpi::{LogicalPosition, LogicalSize},
  window::Window,
};

use crate::{
  errors::WebviewError,
  events,
  types::{PythonType, to_python},
};

#[cfg(test)]
mod tests;

/// The window was maximized, by the frontend, by Python or by the user.
pub const MAXIMIZED: &str = "window:maximized";
/// The window left the maximized state for its previous size.
pub const UNMAXIMIZED: &str = "window:unmaximized";
/// The window was minimized to the dock or the taskbar.
pub const MINIMIZED: &str = "window:minimized";
/// The window came back from being minimized.
pub const RESTORED: &str = "window:restored";
/// The window was taken off the screen without being closed.
pub const HIDDEN: &str = "window:hidden";
/// The window was put back on the screen.
pub const SHOWN: &str = "window:shown";
/// The window became the one receiving keyboard input.
pub const FOCUSED: &str = "window:focused";
/// The window stopped being the one receiving keyboard input.
pub const BLURRED: &str = "window:blurred";
/// The window's inner size changed. Carries `{width, height}`.
pub const RESIZED: &str = "window:resized";
/// The window's outer position changed. Carries `{x, y}`.
pub const MOVED: &str = "window:moved";
/// Something asked for the window to close, before the close hook is asked.
pub const CLOSE_REQUESTED: &str = "window:close-requested";

/// Everything about the window that is worth an Event, in one reading.
///
/// Sizes and positions are whole logical pixels, so two readings that differ
/// only below a pixel are the same reading and emit nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowState {
  pub maximized: bool,
  pub minimized: bool,
  /// Whether the window has taken over a whole screen. Carried in the state
  /// query and in nothing else: a fullscreen has no Event of its own, because
  /// macOS reports one as a resize and a move and there would be no second
  /// route to test the name against.
  pub fullscreen: bool,
  pub visible: bool,
  pub focused: bool,
  /// Inner size — the area the frontend renders into, the same measurement
  /// `size=` sets.
  pub size: (i64, i64),
  /// Outer position — where the window sits on the desktop, decorations
  /// included.
  pub position: (i64, i64),
}

/// What the window measures on the outside minus what the frontend renders
/// into, in logical pixels: the titlebar, and any border.
///
/// Measured from the window as it was built, and then held — because that is
/// the only moment tao can be asked. On macOS wry takes the window's content
/// view for its own, which leaves the view tao measures orphaned and its frame
/// frozen at the size the window opened with, so `inner_size()` goes on
/// reporting that size through every maximize and every drag. `outer_size()`
/// reads the window itself and stays honest, so the content size is taken from
/// there instead.
///
/// A logical inset rather than a physical one, because a titlebar is the same
/// number of logical pixels on every display and a window that moves to a
/// screen of another scale factor must not report a different content size for
/// the same window.
///
/// Not fixed for the life of the window, because `decorations` may be assigned
/// on a running Webview and a window that has just lost its titlebar renders
/// into all of itself. See `decorations_changed`.
static FRAME_INSET: Mutex<Option<(f64, f64)>> = Mutex::new(None);

/// The inset measured while the window still had its decorations.
///
/// Kept so that decorations turned off and on again report the same content
/// size as before, on the platform where the frame cannot be measured twice.
static DECORATED_INSET: Mutex<Option<(f64, f64)>> = Mutex::new(None);

fn frame_inset() -> (f64, f64) {
  FRAME_INSET
    .lock()
    .ok()
    .and_then(|inset| *inset)
    .unwrap_or((0.0, 0.0))
}

fn set_frame_inset(inset: (f64, f64), decorated: bool) {
  if let Ok(mut current) = FRAME_INSET.lock() {
    *current = Some(inset);
  }
  if decorated && let Ok(mut decorated_inset) = DECORATED_INSET.lock() {
    *decorated_inset = Some(inset);
  }
}

/// The frame the window has now, measured or remembered.
///
/// Windows answers `inner_size` honestly, so the frame is measured again
/// whenever it changes — which matters there, because an undecorated tao
/// window keeps its resize frame and is still larger than the page it shows.
///
/// macOS cannot be asked twice, so the two states are known rather than
/// measured: an undecorated window there is borderless and owes nothing, and a
/// decorated one owes the titlebar measured while the window was still tao's.
/// A window built undecorated and decorated afterwards is the one case with
/// nothing to remember; it reports the outer size until it is undecorated
/// again, which is a titlebar's worth of error on a window nobody has yet
/// asked for.
fn measure_inset(window: &Window) -> (f64, f64) {
  let decorated = window.is_decorated();

  #[cfg(not(target_os = "macos"))]
  {
    let scale = window.scale_factor();
    let outer: LogicalSize<f64> = window.outer_size().to_logical(scale);
    let inner: LogicalSize<f64> = window.inner_size().to_logical(scale);
    let _ = decorated;
    (
      (outer.width - inner.width).max(0.0),
      (outer.height - inner.height).max(0.0),
    )
  }

  #[cfg(target_os = "macos")]
  {
    if !decorated {
      return (0.0, 0.0);
    }
    DECORATED_INSET
      .lock()
      .ok()
      .and_then(|inset| *inset)
      .unwrap_or((0.0, 0.0))
  }
}

/// The window's decorations were turned on or off, so the frame it owes has
/// changed and the next reading has to be taken against the new one.
pub fn decorations_changed(window: &Window) {
  set_frame_inset(measure_inset(window), window.is_decorated());
}

/// The last reading the event loop took, for whoever asks between two turns.
///
/// The state query answers from here rather than from the window, and that is
/// deliberate: the window may only be touched on the thread that draws it,
/// while a Python listener asking what the window is doing runs on the portal
/// (ADR-0001). Answering from the reading the diff was taken against also
/// means the query and the Events can never disagree — a listener told
/// `window:maximized` and asking immediately afterwards is told the same
/// thing, because it is the same reading.
///
/// At most one turn of the event loop out of date, and a turn is exactly how
/// long it takes for anything the caller just asked for to be true anyway.
static CURRENT: Mutex<Option<WindowState>> = Mutex::new(None);

/// Puts a reading where the state query can find it.
fn remember(state: WindowState) {
  if let Ok(mut current) = CURRENT.lock() {
    *current = Some(state);
  }
}

/// The last reading, or `None` before the window has been read at all.
pub fn snapshot() -> Option<WindowState> {
  CURRENT.lock().ok().and_then(|current| *current)
}

/// The first reading, taken before the loop starts.
///
/// It emits nothing: a window that has just opened has not changed, and a
/// frontend that has not loaded yet has nobody registered to hear about it
/// anyway. Its job is to give the first diff something honest to be a
/// difference from, and to measure the frame while the window is still the
/// size it was built at.
pub fn initial(window: &Window) -> WindowState {
  let scale = window.scale_factor();
  let outer: LogicalSize<f64> = window.outer_size().to_logical(scale);
  let inner: LogicalSize<f64> = window.inner_size().to_logical(scale);
  set_frame_inset(
    (
      (outer.width - inner.width).max(0.0),
      (outer.height - inner.height).max(0.0),
    ),
    window.is_decorated(),
  );

  let state = read(
    window,
    &WindowState {
      maximized: false,
      minimized: false,
      fullscreen: false,
      visible: true,
      focused: false,
      size: (0, 0),
      position: (0, 0),
    },
  );
  remember(state);
  state
}

/// The area the frontend renders into, from what the window measures on the
/// outside. Whole logical pixels, and never negative.
fn content_size(outer: LogicalSize<f64>, inset: (f64, f64)) -> (i64, i64) {
  (
    (outer.width - inset.0).max(0.0).round() as i64,
    (outer.height - inset.1).max(0.0).round() as i64,
  )
}

/// Reads the window as it is now.
///
/// `previous` is only reached for when the platform refuses to say where the
/// window is; a position nobody can name has not changed as far as anyone
/// listening is concerned.
pub fn read(window: &Window, previous: &WindowState) -> WindowState {
  let scale = window.scale_factor();
  let outer: LogicalSize<f64> = window.outer_size().to_logical(scale);
  let size = content_size(outer, frame_inset());
  let position = match window.outer_position() {
    Ok(position) => {
      let position: LogicalPosition<f64> = position.to_logical(scale);
      (position.x.round() as i64, position.y.round() as i64)
    },
    Err(_) => previous.position,
  };
  let minimized = window.is_minimized();
  WindowState {
    maximized: window.is_maximized(),
    minimized,
    fullscreen: window.fullscreen().is_some(),
    // macOS answers `is_visible` with false for a window that is only
    // miniaturized, so a minimize there would otherwise arrive as
    // `window:hidden` and `window:minimized` together — two names for one
    // thing, and a frontend told the window was hidden when the dock is
    // showing it. A minimized window is minimized, not hidden.
    visible: window.is_visible() || minimized,
    focused: window.is_focused(),
    size,
    position,
  }
}

/// Whether a reading of size, position and maximized state is about a window
/// anyone can see.
fn on_screen(state: &WindowState) -> bool {
  state.visible && !state.minimized
}

/// Turns one reading into the state to remember and the Events to emit.
///
/// Pure, and the whole of the decision: the event loop only reads the window
/// and posts what comes back out of here.
///
/// The order within one turn is fixed — visibility, minimize, maximize, focus,
/// move, resize — so that a listener seeing several at once sees the coarse
/// change before the geometry it explains. A maximize arrives as
/// `window:maximized` and then the `window:moved` and `window:resized` that
/// say where it went.
pub fn advance(
  before: WindowState, reading: WindowState,
) -> (WindowState, Vec<(&'static str, PythonType)>) {
  let after = if on_screen(&reading) {
    reading
  } else {
    WindowState {
      maximized: before.maximized,
      size: before.size,
      position: before.position,
      ..reading
    }
  };

  let mut events = Vec::new();
  let mut flag = |changed: bool, on: &'static str, off: &'static str, now: bool| {
    if changed {
      events.push((if now { on } else { off }, PythonType::Null));
    }
  };

  flag(
    after.visible != before.visible,
    SHOWN,
    HIDDEN,
    after.visible,
  );
  flag(
    after.minimized != before.minimized,
    MINIMIZED,
    RESTORED,
    after.minimized,
  );
  flag(
    after.maximized != before.maximized,
    MAXIMIZED,
    UNMAXIMIZED,
    after.maximized,
  );
  flag(
    after.focused != before.focused,
    FOCUSED,
    BLURRED,
    after.focused,
  );

  if after.position != before.position {
    events.push((MOVED, position_value(after.position)));
  }
  if after.size != before.size {
    events.push((RESIZED, size_value(after.size)));
  }

  (after, events)
}

/// `{x, y}` in logical pixels. An object rather than a two-element array
/// because `event.x` survives a reader who has forgotten the order.
fn position_value(position: (i64, i64)) -> PythonType {
  PythonType::Object(vec![
    ("x".to_string(), PythonType::Integer(position.0)),
    ("y".to_string(), PythonType::Integer(position.1)),
  ])
}

/// `{width, height}` in logical pixels.
fn size_value(size: (i64, i64)) -> PythonType {
  PythonType::Object(vec![
    ("width".to_string(), PythonType::Integer(size.0)),
    ("height".to_string(), PythonType::Integer(size.1)),
  ])
}

/// One reading as the state query hands it over, to Python and to the
/// frontend alike.
///
/// The same shape on both sides, and the two values that also travel as
/// Events keep the shape they have there: `size` is `{width, height}` and
/// `position` is `{x, y}`, so a frontend can hand what it was given by
/// `window:resized` and what it read here to the same code.
pub fn value(state: &WindowState) -> PythonType {
  PythonType::Object(vec![
    (
      "maximized".to_string(),
      PythonType::Boolean(state.maximized),
    ),
    (
      "minimized".to_string(),
      PythonType::Boolean(state.minimized),
    ),
    (
      "fullscreen".to_string(),
      PythonType::Boolean(state.fullscreen),
    ),
    ("visible".to_string(), PythonType::Boolean(state.visible)),
    ("focused".to_string(), PythonType::Boolean(state.focused)),
    ("size".to_string(), size_value(state.size)),
    ("position".to_string(), position_value(state.position)),
  ])
}

/// What the window is doing, for a caller who has not been listening.
///
/// The complement to the window Events: a frontend that has just loaded has
/// observed no change at all, and a titlebar has to draw the maximize button
/// one way or the other before anybody touches the window. Answers from the
/// last reading the event loop took, which is the reading every window Event
/// was a difference from.
#[pyo3::pyfunction]
pub fn window_state(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
  let state = snapshot().ok_or_else(|| {
    WebviewError::new_err(
      "The window is not open yet, so it has no state to report. Ask for it \
       from inside a callback, once run() has the window on screen.",
    )
  })?;
  to_python(py, &value(&state))
}

/// Reads the window, remembers it, and emits what changed to both sides.
///
/// Called once per turn of the event loop. When nothing changed this reads six
/// values and posts nothing, which is what makes it cheap enough to call that
/// often.
pub fn sync(window: &Window, state: &mut WindowState) {
  let reading = read(window, state);
  let (after, events) = advance(*state, reading);
  *state = after;
  // Before the Events, so a listener that asks what the window is doing the
  // moment it hears one is told the state that Event announced, not the one
  // before it.
  remember(after);
  for (name, value) in events {
    events::emit_reserved(name, value);
  }
}
