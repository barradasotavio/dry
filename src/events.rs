//! The event loop that owns the window, and the Event half of the Bridge.
//!
//! Two things share this module because they are the same road. An AppEvent is
//! how any other thread reaches the thread that draws the window; a Bridge
//! Event is a message with a name and a value that returns nothing, and every
//! one of them travelling towards the frontend becomes an AppEvent on the way.
//!
//! An Event is not a Call and has no return path. Nothing here waits, nothing
//! here answers, and a listener's return value is dropped on both sides. A
//! Python listener is handed to the portal exactly as a Call is, so it runs off
//! the thread that draws the window; see `dry/portal.py` and ADR-0001.

use pyo3::{
  Bound, PyAny, PyResult, Python,
  types::{PyAnyMethods, PyModule},
};
use serde::{Deserialize, Serialize};
use serde_json::{Error as JsonError, from_str, to_string};
use std::sync::{Mutex, OnceLock};
use tao::{
  event::{Event, StartCause, WindowEvent},
  event_loop::{ControlFlow, EventLoop, EventLoopProxy},
  window::{ResizeDirection, Window},
};
use wry::WebView;

use crate::{
  errors::BridgeError,
  logs,
  types::{PythonType, from_python, to_python},
  window::{self, resize, state},
};

pub const EVENTS_JS: &str = include_str!("js/events.js");

#[cfg(test)]
mod tests;

pub static PROXY: OnceLock<Mutex<Option<EventLoopProxy<AppEvent>>>> = OnceLock::new();

/// The module that answers for Python on the way out: it holds the close hook,
/// the asyncio loop and the thread pool.
const PORTAL: &str = "dry.portal";

/// Hands an AppEvent to the running event loop, from whichever thread holds
/// it. Every reply to a Call travels this way: a callback finishes on a pool
/// thread or on the asyncio loop, and the JavaScript that settles its Promise
/// has to be evaluated back on the thread that owns the window.
pub fn send_to_event_loop(event: AppEvent) -> Result<(), String> {
  let proxy = PROXY.get().ok_or("The Bridge is not initialized.")?;
  let guard = proxy.lock().map_err(|_| "The Bridge is poisoned.")?;
  let sender = guard.as_ref().ok_or("The Bridge is not running.")?;
  sender
    .send_event(event)
    .map_err(|err| format!("The Event could not be sent: {err}"))
}

#[derive(Debug)]
pub enum AppEvent {
  RunJavascript(String),
  DragWindow,
  MinimizeWindow,
  MaximizeWindow,
  CloseWindow,
  ResizeWindow(ResizeDirection),
  ResizeDragged(resize::Drag),
  /// One setting assigned on a Webview that is already running. Applied to
  /// the window here; the Event that says it happened comes from the state
  /// diff, like any other change to the window.
  ChangeWindow(window::Change),
  /// The frontend asked what the window is doing, and is holding a Promise.
  QueryWindowState,
}

/// What the frontend puts in front of an Event on the wire, so the one IPC
/// handler can tell an Event from a Call without parsing it first.
pub const EVENT_PREFIX: &str = "dry_event:";

/// The mark of a name Dry keeps for itself.
///
/// An Event named `window:maximized` is an ordinary Event on the ordinary bus —
/// the only thing reserved about it is who may emit it. Both public doors,
/// `dry.emit_event` from Python and `window.dry.emit` from the frontend, refuse
/// a name starting with this, so a listener for one is hearing from Dry and
/// nothing else. Listening is not restricted: a reserved name is exactly as
/// listenable as any other, on both sides.
///
/// Dry's own side emits one through `emit_reserved`, which does not check.
pub const RESERVED_PREFIX: &str = "window:";

/// One Event on the wire: a name, and a value inside the Bridge contract.
///
/// There is no id and no reply field, because there is nothing to reply to.
/// The same shape travels in both directions.
#[derive(Debug, Deserialize, Serialize)]
pub struct BridgeEvent {
  pub name: String,
  /// Absent is `null`: `JSON.stringify` drops a key whose value is
  /// `undefined`, so `window.dry.emit('saved')` arrives with no value at all.
  #[serde(default = "nothing")]
  pub value: PythonType,
}

fn nothing() -> PythonType {
  PythonType::Null
}

/// Reads one Event off the wire.
pub fn parse_event(body: &str) -> Result<BridgeEvent, JsonError> {
  from_str(body)
}

/// Writes the JavaScript that hands an Event to the frontend's listeners.
pub fn event_script(event: &BridgeEvent) -> Result<String, JsonError> {
  Ok(format!("window.dry.deliverEvent({})", to_string(event)?))
}

/// Refuses a name the application may not emit under, and an empty one.
fn emittable(name: &str) -> Result<(), String> {
  if name.is_empty() {
    return Err("An Event needs a name.".to_string());
  }
  if name.starts_with(RESERVED_PREFIX) {
    return Err(format!(
      "'{name}' is a reserved Event name: a name starting with '{RESERVED_PREFIX}' \
       belongs to Dry's own window Events. Listen for it as much as you like, but \
       emit under a name of your own."
    ));
  }
  Ok(())
}

/// Emits an Event from Python to the frontend.
///
/// Fire and forget, by definition: this returns once the message is on its way
/// to the thread that owns the window, and nothing comes back. A value outside
/// the Bridge contract raises here, through the same `default=` hook a Call's
/// return value goes through.
#[pyo3::pyfunction]
pub fn emit_event(name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
  emittable(name).map_err(BridgeError::new_err)?;
  let value = from_python(value)?;
  deliver_to_frontend(name, &value).map_err(BridgeError::new_err)
}

/// The escape hatch ADR-0001's reasoning leaves in place of a Python-to-
/// frontend Call: raw script, evaluated in the page, returning nothing. A Call
/// with a return value would be an await on the Python side that never
/// resolves if the page navigates away.
#[pyo3::pyfunction]
pub fn eval_js(script: &str) -> PyResult<()> {
  send_to_event_loop(AppEvent::RunJavascript(script.to_string()))
    .map_err(BridgeError::new_err)
}

/// Hands an Event to the frontend's listeners.
pub fn deliver_to_frontend(name: &str, value: &PythonType) -> Result<(), String> {
  let event = BridgeEvent {
    name: name.to_string(),
    value: value.clone(),
  };
  let script = event_script(&event)
    .map_err(|err| format!("The Event could not be written: {err}"))?;
  send_to_event_loop(AppEvent::RunJavascript(script))
}

/// Hands an Event to Python's listeners, through the portal.
///
/// Called on the thread that draws the window, and returns as soon as the
/// portal has taken the listeners off it. Nothing here can fail in a way the
/// sender could act on — an Event has no sender waiting — so a failure is
/// logged and dropped.
pub fn deliver_to_python(name: &str, value: &PythonType) {
  Python::attach(|py| match py.import(PORTAL) {
    Ok(portal) => deliver_through(&portal, name, value),
    Err(err) => {
      logs::error(
        logs::BRIDGE,
        format!("The Event '{name}' could not reach Python: {err}"),
      );
    },
  });
}

/// The half of `deliver_to_python` a test can hold a portal up to.
fn deliver_through(portal: &Bound<'_, PyModule>, name: &str, value: &PythonType) {
  let deliver = || -> PyResult<()> {
    let value = to_python(portal.py(), value)?;
    portal.call_method1("deliver", (name, value))?;
    Ok(())
  };
  if let Err(err) = deliver() {
    logs::error(
      logs::BRIDGE,
      format!("The Event '{name}' could not be delivered to Python: {err}"),
    );
  }
}

/// Emits an Event under a reserved name, to both sides at once.
///
/// The door Dry's own Events come through: a window event is an Event like any
/// other, so it reaches every Python listener and every frontend listener
/// registered for its name, and neither side can forge one. The window state
/// in `window::state` comes through here.
pub fn emit_reserved(name: &str, value: PythonType) {
  if let Err(err) = deliver_to_frontend(name, &value) {
    logs::error(
      logs::BRIDGE,
      format!("The Event '{name}' could not be delivered to the frontend: {err}"),
    );
  }
  deliver_to_python(name, &value);
}

/// Emits a reserved Event to both sides without going round the event loop.
///
/// `emit_reserved` posts the frontend's copy as an AppEvent, which the loop
/// picks up on a later turn. That is right for everything except the last
/// Event the window ever sends: `window:close-requested` is emitted from
/// inside the turn that may end the loop, and a script queued for a turn that
/// never comes is a script the page never sees. So the caller that already
/// holds the WebView on the thread that draws it evaluates the script here and
/// now, and the frontend hears the request before the close hook is asked.
pub fn emit_reserved_now(webview: &WebView, name: &str, value: PythonType) {
  let event = BridgeEvent {
    name: name.to_string(),
    value,
  };
  match event_script(&event) {
    Ok(script) => run_javascript(webview, &script),
    Err(err) => logs::error(
      logs::BRIDGE,
      format!("The Event '{name}' could not be written: {err}"),
    ),
  }
  deliver_to_python(name, &event.value);
}

/// Reads one Event off the Bridge and hands it to Python.
///
/// A frontend may not emit under a reserved name, so a page cannot forge the
/// window event a listener trusts Dry for.
pub fn handle_event_request(body: &str) {
  let event = match parse_event(body) {
    Ok(event) => event,
    Err(err) => {
      logs::error(logs::BRIDGE, format!("The Event could not be read: {err}"));
      return;
    },
  };

  if let Err(reason) = emittable(&event.name) {
    logs::error(logs::BRIDGE, format!("The Event was refused: {reason}"));
    return;
  }

  deliver_to_python(&event.name, &event.value);
}

pub fn run_event_loop(
  event_loop: EventLoop<AppEvent>, window: Window, webview: WebView,
) {
  let mut webview = webview;
  // What the window looked like on the turn before this one. Every window
  // Event is the difference between this and a fresh reading; see
  // `window::state`.
  let mut window_state = state::initial(&window);
  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => {
        logs::debug(
          logs::WEBVIEW,
          format!("Webview '{}' started.", window.title()),
        );
      },
      Event::WindowEvent { event, .. } => {
        handle_window_event(event, &mut webview, control_flow)
      },
      Event::UserEvent(app_event) => {
        handle_app_event(app_event, &window, &mut webview, control_flow)
      },
      // The platform has handed over everything it had this turn, so the
      // window is read once here rather than at each of the events that might
      // have changed it. That is what coalesces a drag into one Event per turn
      // of the loop, and it is also how a change no WindowEvent announces —
      // a maximize from an OS keyboard shortcut, a hide from the app menu —
      // still reaches a listener.
      Event::MainEventsCleared => state::sync(&window, &mut window_state),
      _ => (),
    }
  });
}

fn handle_window_event(
  event: WindowEvent, webview: &mut WebView, control_flow: &mut ControlFlow,
) {
  match event {
    WindowEvent::CloseRequested => request_close(webview, control_flow),
    _ => (),
  }
}

/// Announces the close and then runs it.
///
/// Every route in comes through here, so a frontend listening for
/// `window:close-requested` hears the titlebar button, the window manager and
/// `window.dry.close()` alike. The Event is a notification and not a vote: the
/// one thing that can refuse a close is the close hook, which is asked next.
fn request_close(webview: &mut WebView, control_flow: &mut ControlFlow) {
  emit_reserved_now(webview, state::CLOSE_REQUESTED, PythonType::Null);
  close(webview, control_flow);
}

fn handle_app_event(
  event: AppEvent, window: &Window, webview: &mut WebView,
  control_flow: &mut ControlFlow,
) {
  match event {
    AppEvent::RunJavascript(js) => run_javascript(webview, &js),
    AppEvent::CloseWindow => request_close(webview, control_flow),
    AppEvent::MinimizeWindow => toggle_minimize(window),
    AppEvent::MaximizeWindow => toggle_maximize(window),
    AppEvent::DragWindow => drag(window),
    AppEvent::ResizeWindow(direction) => {
      if let Err(err) = window.drag_resize_window(direction) {
        logs::error(
          logs::WEBVIEW,
          format!("The window could not be resized: {err}"),
        );
      }
    },
    AppEvent::ResizeDragged(drag) => resize::apply(&drag, window),
    // Nothing is emitted here. The change lands on the window, and the
    // reading taken at the end of this same turn of the loop is what tells
    // both sides about it — so a size Python asked for and a size the user
    // dragged arrive as the same Event, and a change the platform refused
    // announces nothing.
    AppEvent::ChangeWindow(change) => window::apply(change, window),
    AppEvent::QueryWindowState => answer_state_query(webview),
  }
}

/// Resolves the Promise the frontend's `window.dry.state()` is holding.
///
/// Every waiting caller is resolved with the one reading, because a reading
/// taken now is the current answer to every question asked since the last one.
///
/// Nothing here can reach a page before the window has been read: the first
/// reading is taken before the loop starts, and this runs on a turn of it.
fn answer_state_query(webview: &WebView) {
  let Some(reading) = state::snapshot() else {
    logs::error(
      logs::BRIDGE,
      "The window state was asked for before the window had been read.",
    );
    return;
  };
  match to_string(&state::value(&reading)) {
    Ok(value) => run_javascript(webview, &format!("window.dry.resolveState({value})")),
    Err(err) => logs::error(
      logs::BRIDGE,
      format!("The window state could not be written: {err}"),
    ),
  }
}

fn run_javascript(webview: &WebView, js: &str) {
  if let Err(err) = webview.evaluate_script(js) {
    logs::error(
      logs::BRIDGE,
      format!("The JavaScript could not be evaluated: {err}"),
    );
  }
}

/// Every way the Webview can be closed comes through here — the titlebar
/// button, the window manager, and `window.dry.close()` from the frontend
/// alike — because a close the application is allowed to refuse is not much of
/// a guarantee if one of the routes in skips the asking.
///
/// The order is the whole point. The application is asked first, while the
/// window is still there to be kept; only then does Python shut down, with the
/// Calls still in flight given their chance to finish; and only then does the
/// event loop exit, which on this platform means the process goes with it.
fn close(webview: &mut WebView, control_flow: &mut ControlFlow) {
  if !closing_allowed() {
    logs::debug(
      logs::WEBVIEW,
      "The close was refused by the close hook, so the window stays open.",
    );
    return;
  }

  shut_down_python();
  exit_app(webview, control_flow);
}

/// Asks Python whether the Webview may close.
///
/// A portal that cannot even be reached cannot refuse: an unreachable module
/// has registered no hook, and a window nothing can close is worse than one
/// that closes without asking.
fn closing_allowed() -> bool {
  Python::attach(|py| match py.import(PORTAL) {
    Ok(portal) => allowed_by(&portal),
    Err(err) => {
      logs::error(
        logs::BRIDGE,
        format!("The close hook could not be reached: {err}"),
      );
      true
    },
  })
}

/// The half of `closing_allowed` a test can hold a portal up to.
fn allowed_by(portal: &Bound<'_, PyModule>) -> bool {
  match portal.call_method0("closing").and_then(|it| it.extract()) {
    Ok(allowed) => allowed,
    Err(err) => {
      logs::error(
        logs::BRIDGE,
        format!("The close hook could not be asked: {err}"),
      );
      true
    },
  }
}

/// Shuts Python down and waits for it, holding the event loop here until it is
/// done. The wait is Python's to bound — the portal gives an in-flight Call a
/// grace period and cuts it short after that — and the GIL is released for the
/// whole of it, so the Calls being waited for can actually run.
fn shut_down_python() {
  Python::attach(|py| match py.import(PORTAL) {
    Ok(portal) => shut_down_through(&portal),
    Err(err) => {
      logs::error(
        logs::BRIDGE,
        format!("Python could not be shut down: {err}"),
      );
    },
  });
}

/// The half of `shut_down_python` a test can hold a portal up to.
fn shut_down_through(portal: &Bound<'_, PyModule>) {
  if let Err(err) = portal.call_method0("closed") {
    logs::error(
      logs::BRIDGE,
      format!("Python did not shut down cleanly: {err}"),
    );
  }
}

fn exit_app(webview: &mut WebView, control_flow: &mut ControlFlow) {
  let mut webview = Some(webview);
  webview.take();
  *control_flow = ControlFlow::Exit;
}

fn toggle_minimize(window: &Window) {
  let minimized = window.is_minimized();
  window.set_minimized(!minimized);
}

fn toggle_maximize(window: &Window) {
  let is_maximized = window.is_maximized();
  window.set_maximized(!is_maximized);
}

fn drag(window: &Window) {
  if let Err(err) = window.drag_window() {
    logs::error(
      logs::WEBVIEW,
      format!("The window could not be dragged: {err}"),
    );
  }
}
