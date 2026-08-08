//! Tests for the close sequence, run without a window.
//!
//! What the event loop does on a close request is two calls into Python with a
//! decision between them, so that is what is exercised here: the portal is
//! loaded on its own and held up to the same `allowed_by` and
//! `shut_down_through` the event loop uses, in the same order. A refusal, an
//! agreement, a hook that raises, and a Call still running when the close
//! comes are all visible from there.

use pyo3::{
  Python,
  types::{PyAnyMethods, PyDict, PyDictMethods, PyModule},
};
use std::{
  ffi::CString,
  sync::atomic::{AtomicUsize, Ordering},
};

use super::{allowed_by, shut_down_through};

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
  let name = CString::new(format!(
    "dry_portal_closing_{}",
    PORTALS.fetch_add(1, Ordering::Relaxed)
  ))
  .expect("the module name should be readable");

  Python::attach(|py| {
    let source = CString::new(include_str!("../../dry/portal.py"))
      .expect("the portal should be readable");
    let portal = PyModule::from_code(py, &source, c"portal.py", &name)
      .expect("the portal should import");

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
