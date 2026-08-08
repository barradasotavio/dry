//! Tests for the close sequence and the Event bus, run without a window.
//!
//! What the event loop does on a close request is two calls into Python with a
//! decision between them, so that is what is exercised here: the portal is
//! loaded without the `dry` package around it and held up to the same
//! `allowed_by` and `shut_down_through` the event loop uses, in the same
//! order. A refusal, an agreement, a hook that raises, and a Call still
//! running when the close comes are all visible from there.

use pyo3::{
  Bound, Python,
  types::{PyAnyMethods, PyDict, PyDictMethods, PyModule},
};
use std::{
  ffi::{CStr, CString},
  sync::atomic::{AtomicUsize, Ordering},
};

use super::{
  BridgeEvent, allowed_by, deliver_through, emittable, event_script, parse_event,
  shut_down_through,
};
use crate::types::PythonType;

/// What every close test has in front of it: a stand-in for the Completion,
/// and Dry's own logger kept quiet so that a deliberately raising hook does
/// not print through logging's last-resort handler.
const HARNESS: &str = r#"
import asyncio, atexit, logging, threading, time

logging.getLogger('dry').addHandler(logging.NullHandler())

class Answer:
    def __init__(self):
        self.done = threading.Event()
        self.value = None
        self.error = None

    def resolve(self, value):
        self.value = value
        self.done.set()

    def reject(self, error):
        self.error = error
        self.done.set()
"#;

/// Puts `dry/portal.py` inside a package of its own and imports it there.
///
/// The synthetic package carries nothing but a search path pointing at `dry/`,
/// which is all a relative import needs: `from .signature import mismatch`
/// finds the real `dry/signature.py`, and so will every relative import
/// written after it. Importing the actual `dry` package instead would run its
/// `__init__`, which reaches for the extension module — and the extension only
/// exists once maturin has built it, so no `cargo test` can import it. That is
/// the one relative import the portal still cannot make: a sibling that pulls
/// the extension in, as `dry/exceptions.py` does. A sibling of pure Python,
/// which is what the portal reaches for, resolves here exactly as it does in
/// an installed package.
const LOAD_PORTAL: &CStr = cr#"
from importlib.machinery import ModuleSpec
from importlib.util import module_from_spec, spec_from_file_location
from sys import modules

spec = ModuleSpec(package, None, is_package=True)
spec.submodule_search_locations = [directory]
modules[package] = module_from_spec(spec)

spec = spec_from_file_location(f'{package}.portal', f'{directory}/portal.py')
portal = module_from_spec(spec)
modules[spec.name] = portal
spec.loader.exec_module(portal)
"#;

/// The portal, loaded under `package` and answering to nothing else.
///
/// The name is the whole of the isolation. `sys.modules` hands back the module
/// object already registered under a name, and the portal keeps state for the
/// process — a loop, a thread pool, a closed flag — so two tests loading under
/// one name would be running the same portal, and one test's shutdown would be
/// another's.
fn portal_in<'py>(py: Python<'py>, package: &str) -> Bound<'py, PyModule> {
  let globals = PyDict::new(py);
  globals
    .set_item("package", package)
    .expect("the package name should be reachable from the loader");
  globals
    .set_item("directory", concat!(env!("CARGO_MANIFEST_DIR"), "/dry"))
    .expect("the package directory should be reachable from the loader");

  py.run(LOAD_PORTAL, Some(&globals), None)
    .expect("the portal should import");

  globals
    .get_item("portal")
    .expect("the loader should leave the portal")
    .expect("the loader should leave the portal")
    .cast_into::<PyModule>()
    .expect("the portal should be a module")
}

/// Runs a close the way the event loop runs one.
///
/// `setup` prepares the portal — a hook, a Call in flight — and then the
/// close sequence happens from Rust: the application is asked, and only if it
/// agrees does Python shut down. `verdict` inspects what is left, with
/// `allowed` telling it which way the decision went.
///
/// Every test gets a portal of its own, so that one test's shutdown is not
/// another's.
fn through_a_close(setup: &str, verdict: &str) -> String {
  static PORTALS: AtomicUsize = AtomicUsize::new(0);
  let name = format!(
    "dry_portal_closing_{}",
    PORTALS.fetch_add(1, Ordering::Relaxed)
  );

  Python::attach(|py| {
    let portal = portal_in(py, &name);

    let globals = PyDict::new(py);
    globals
      .set_item("portal", portal.clone())
      .expect("the portal should be reachable from the test");

    let arrangement = CString::new(format!("{HARNESS}\n{setup}\n"))
      .expect("the test setup should be readable");
    py.run(&arrangement, Some(&globals), None)
      .expect("the test setup should run");

    let allowed = allowed_by(&portal);
    globals
      .set_item("allowed", allowed)
      .expect("the decision should be reachable from the test");

    if allowed {
      shut_down_through(&portal);
    }

    let question = CString::new(verdict).expect("the test verdict should be readable");
    py.run(&question, Some(&globals), None)
      .expect("the test verdict should run");

    globals
      .get_item("verdict")
      .expect("the test body should leave a verdict")
      .expect("the test body should leave a verdict")
      .extract()
      .expect("the verdict should be a string")
  })
}

#[test]
fn a_webview_with_no_close_hook_closes() {
  assert_eq!(
    through_a_close("", "verdict = 'closed' if allowed else 'kept open'"),
    "closed"
  );
}

#[test]
fn a_hook_that_refuses_keeps_the_window_open() {
  assert_eq!(
    through_a_close(
      r#"
portal.on_close(lambda: False)
"#,
      "verdict = 'closed' if allowed else 'kept open'",
    ),
    "kept open"
  );
}

#[test]
fn a_hook_that_agrees_closes() {
  assert_eq!(
    through_a_close(
      r#"
portal.on_close(lambda: True)
"#,
      "verdict = 'closed' if allowed else 'kept open'",
    ),
    "closed"
  );
}

/// The hook an application actually writes: it saves its state and returns
/// nothing at all. Only the value `False` refuses.
#[test]
fn a_hook_that_returns_nothing_closes() {
  assert_eq!(
    through_a_close(
      r#"
saved = []

def save():
    saved.append('state')

portal.on_close(save)
"#,
      "verdict = f'{saved}, ' + ('closed' if allowed else 'kept open')",
    ),
    "['state'], closed"
  );
}

/// A coroutine hook is awaited on Dry's loop before its answer is read, so an
/// application that has to await its own storage can still refuse.
#[test]
fn an_awaited_hook_can_refuse() {
  assert_eq!(
    through_a_close(
      r#"
async def unsaved():
    await asyncio.sleep(0.01)
    return False

portal.on_close(unsaved)
"#,
      "verdict = 'closed' if allowed else 'kept open'",
    ),
    "kept open"
  );
}

/// The documented reading of an exception: it is logged, and the close goes
/// ahead. A hook that raises made no decision, and the user must not be left
/// with a window that cannot be closed.
#[test]
fn a_hook_that_raises_closes() {
  assert_eq!(
    through_a_close(
      r#"
def broken():
    raise ValueError('the hook is wrong')

portal.on_close(broken)
"#,
      "verdict = 'closed' if allowed else 'kept open'",
    ),
    "closed"
  );
}

/// A refused close leaves everything as it was: the Bridge is still open, and
/// the application goes on running in the window it just kept.
#[test]
fn a_refused_close_leaves_the_bridge_open() {
  assert_eq!(
    through_a_close(
      r#"
portal.on_close(lambda: False)
"#,
      r#"
answer = Answer()
portal.dispatch('after', lambda: 'still here', (), answer)
verdict = answer.value if answer.done.wait(10) else 'no answer'
portal.shutdown()
"#,
    ),
    "still here"
  );
}

/// The order the whole ticket is about: the hook is asked while the Bridge is
/// still open, so a hook can Call into the application it is asking about.
#[test]
fn the_hook_runs_before_anything_shuts_down() {
  assert_eq!(
    through_a_close(
      r#"
def ask():
    answer = Answer()
    portal.dispatch('state', lambda: 'unsaved', (), answer)
    seen.append(answer.value if answer.done.wait(10) else 'no answer')

seen = []
portal.on_close(ask)
"#,
      "verdict = f'{seen}'",
    ),
    "['unsaved']"
  );
}

/// A Call already running when the close comes is waited for, not truncated.
/// The callback is still running when the shutdown starts and has finished by
/// the time it returns, which is the difference between an application that
/// finishes writing its file and one that is cut off mid-write.
#[test]
fn an_in_flight_call_finishes_before_the_process_goes() {
  assert_eq!(
    through_a_close(
      r#"
def slow():
    time.sleep(0.3)
    return 'finished'

answer = Answer()
portal.dispatch('slow', slow, (), answer)
time.sleep(0.05)
assert not answer.done.is_set(), 'the Call should still be running'
"#,
      "verdict = answer.value if answer.done.is_set() else 'cut short'",
    ),
    "finished"
  );
}

/// The Calls that arrive while the door is closing settle rather than hang:
/// the window is going, so no answer could be delivered anyway, and a
/// rejection naming the reason is what the frontend can act on.
#[test]
fn a_call_arriving_after_the_close_is_rejected() {
  assert_eq!(
    through_a_close(
      "",
      r#"
try:
    portal.dispatch('late', lambda: None, (), Answer())
    verdict = 'accepted'
except RuntimeError as error:
    verdict = str(error)
"#,
    ),
    "The Bridge is closed, so the Call cannot be run."
  );
}

/// `tao` exits the process itself, so nothing Python would normally run on the
/// way out runs by itself. A `finally` block waiting on a cancelled coroutine
/// and an `atexit` handler both get their turn because the portal gives it to
/// them.
#[test]
fn the_way_out_runs_what_python_is_owed() {
  assert_eq!(
    through_a_close(
      r#"
portal._SHUTDOWN_TIMEOUT = 0.2

ran = []
atexit.register(lambda: ran.append('atexit'))

async def forever():
    try:
        await asyncio.Event().wait()
    finally:
        ran.append('finally')

answer = Answer()
portal.dispatch('forever', forever, (), answer)
time.sleep(0.05)
"#,
      "verdict = f'{sorted(ran)}'",
    ),
    "['atexit', 'finally']"
  );
}

#[test]
fn an_event_reads_off_the_wire_with_its_value() {
  let event = parse_event(r#"{"name": "saved", "value": {"id": 7}}"#)
    .expect("the Event should read");
  assert_eq!(event.name, "saved");
  assert_eq!(
    event.value,
    PythonType::Object(vec![("id".to_string(), PythonType::Integer(7))])
  );
}

/// `JSON.stringify` drops a key whose value is `undefined`, so
/// `window.dry.emit('saved')` arrives with no value field at all. It is null,
/// not a broken message.
#[test]
fn an_event_with_no_value_carries_null() {
  let event = parse_event(r#"{"name": "saved"}"#).expect("the Event should read");
  assert_eq!(event.value, PythonType::Null);
}

#[test]
fn an_event_travels_to_the_frontend_as_a_delivery() {
  let script = event_script(&BridgeEvent {
    name: "tick".to_string(),
    value: PythonType::Array(vec![PythonType::Boolean(true)]),
  })
  .expect("the Event should be written");
  assert_eq!(
    script,
    r#"window.dry.deliverEvent({"name":"tick","value":[true]})"#
  );
}

/// The mechanism #6 is waiting for. A reserved name is refused at both public
/// doors, so a listener for one is hearing from Dry and nothing else.
#[test]
fn a_reserved_name_cannot_be_emitted_by_an_application() {
  let refusal = emittable("window:maximized").expect_err("a reserved name is refused");
  assert!(refusal.contains("reserved"));
  assert!(emittable("maximized").is_ok(), "an ordinary name is not");
}

#[test]
fn an_event_needs_a_name() {
  assert!(emittable("").is_err());
}

/// Runs a delivery the way the Bridge runs one: listeners are registered in
/// `setup`, the Events in `events` are then delivered from Rust through the
/// same `deliver_through` the IPC handler uses, and `verdict` reports what
/// arrived. Every test gets a portal of its own.
fn through_the_bus(
  setup: &str, events: &[(&str, PythonType)], verdict: &str,
) -> String {
  static PORTALS: AtomicUsize = AtomicUsize::new(0);
  let name = format!(
    "dry_portal_listening_{}",
    PORTALS.fetch_add(1, Ordering::Relaxed)
  );

  Python::attach(|py| {
    let portal = portal_in(py, &name);

    let globals = PyDict::new(py);
    globals
      .set_item("portal", portal.clone())
      .expect("the portal should be reachable from the test");

    let arrangement = CString::new(format!("{HARNESS}\n{setup}\n"))
      .expect("the test setup should be readable");
    py.run(&arrangement, Some(&globals), None)
      .expect("the test setup should run");

    for (event, value) in events {
      deliver_through(&portal, event, value);
    }

    let question = CString::new(verdict).expect("the test verdict should be readable");
    py.run(&question, Some(&globals), None)
      .expect("the test verdict should run");

    let answer = globals
      .get_item("verdict")
      .expect("the test body should leave a verdict")
      .expect("the test body should leave a verdict")
      .extract()
      .expect("the verdict should be a string");

    portal
      .call_method0("shutdown")
      .expect("the portal should shut down");

    answer
  })
}

/// What the whole quadrant is for: one Event, every listener registered for
/// its name, with the value that crossed.
#[test]
fn an_event_reaches_every_listener_registered_for_its_name() {
  assert_eq!(
    through_the_bus(
      r#"
seen = []
arrived = threading.Semaphore(0)

def first(value):
    seen.append(('first', value))
    arrived.release()

def second(value):
    seen.append(('second', value))
    arrived.release()

portal.listen('saved', first)
portal.listen('saved', second)
"#,
      &[("saved", PythonType::Integer(7))],
      r#"
assert all(arrived.acquire(timeout=10) for _ in range(2)), 'a listener never ran'
verdict = f'{sorted(seen)}'
"#,
    ),
    "[('first', 7), ('second', 7)]"
  );
}

/// A frontend announcing something nobody listens for is the ordinary case,
/// not a failure.
#[test]
fn an_event_with_no_listeners_is_not_an_error() {
  assert_eq!(
    through_the_bus(
      "",
      &[("unheard", PythonType::Null)],
      "verdict = 'no listener, no complaint'",
    ),
    "no listener, no complaint"
  );
}

/// An Event has no sender waiting on an answer, so one broken listener is not
/// grounds for silencing the others.
#[test]
fn a_listener_that_raises_does_not_stop_the_others() {
  assert_eq!(
    through_the_bus(
      r#"
seen = []
arrived = threading.Event()

def broken(value):
    raise ValueError('this listener is wrong')

def working(value):
    seen.append(value)
    arrived.set()

portal.listen('saved', broken)
portal.listen('saved', working)
"#,
      &[("saved", PythonType::String("state".to_string()))],
      r#"
assert arrived.wait(10), 'the second listener never ran'
verdict = f'{seen}'
"#,
    ),
    "['state']"
  );
}

/// A listener runs on the portal, never on the thread that draws the window —
/// the same guarantee ADR-0001 gives a Call, for the same reason.
#[test]
fn a_listener_runs_off_the_calling_thread() {
  assert_eq!(
    through_the_bus(
      r#"
here = threading.current_thread().name
seen = []
arrived = threading.Event()

def where(value):
    seen.append(threading.current_thread().name)
    arrived.set()

portal.listen('where', where)
"#,
      &[("where", PythonType::Null)],
      r#"
assert arrived.wait(10), 'the listener never ran'
verdict = 'elsewhere' if seen[0] != here else 'on the calling thread'
"#,
    ),
    "elsewhere"
  );
}

/// A coroutine listener is awaited on Dry's loop, as a coroutine callback is.
#[test]
fn a_coroutine_listener_is_awaited() {
  assert_eq!(
    through_the_bus(
      r#"
seen = []
arrived = threading.Event()

async def slowly(value):
    await asyncio.sleep(0.01)
    seen.append(value)
    arrived.set()

portal.listen('saved', slowly)
"#,
      &[("saved", PythonType::Integer(1))],
      r#"
assert arrived.wait(10), 'the listener never ran'
verdict = f'{seen}'
"#,
    ),
    "[1]"
  );
}

#[test]
fn an_unregistered_listener_stops_hearing() {
  assert_eq!(
    through_the_bus(
      r#"
seen = []

def listener(value):
    seen.append(value)

portal.listen('saved', listener)
portal.unlisten('saved', listener)
portal.unlisten('saved', listener)
portal.unlisten('never', listener)
"#,
      &[("saved", PythonType::Integer(1))],
      r#"
time.sleep(0.05)
verdict = f'{seen}'
"#,
    ),
    "[]"
  );
}
