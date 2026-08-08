//! Tests for the Call path, on both sides of it.
//!
//! The Completion is exercised straight from Rust: it is the piece that
//! decides whether a Call still has an answer owing, and none of that needs a
//! window. The portal is loaded without the `dry` package around it, so the
//! dispatch that keeps a callback off the event-loop thread can be watched
//! happening — a slow callable handed back immediately, two of them
//! overlapping, a coroutine awaited — again with no window anywhere.

use pyo3::{
  Bound, Python,
  types::{PyAnyMethods, PyDict, PyDictMethods, PyModule},
};
use std::{
  ffi::{CStr, CString},
  sync::atomic::{AtomicUsize, Ordering},
};

use super::Completion;

/// Dry's own logger, kept quiet: a test that deliberately fails a Call would
/// otherwise print the record through logging's last-resort handler.
fn silence_logs(py: Python<'_>) {
  py.run(
    c"import logging; logging.getLogger('dry').addHandler(logging.NullHandler())",
    None,
    None,
  )
  .expect("logging should be configurable");
}

fn evaluate<'py>(
  py: Python<'py>, expression: &std::ffi::CStr,
) -> Bound<'py, pyo3::PyAny> {
  py.eval(expression, None, None)
    .expect("the test expression should evaluate")
}

#[test]
fn a_call_is_answered_only_once() {
  let completion = Completion::new("call-1".to_string());
  assert_eq!(completion.claim(), Some("call-1".to_string()));
  assert_eq!(completion.claim(), None);
}

#[test]
fn a_returned_value_answers_the_call() {
  Python::attach(|py| {
    silence_logs(py);
    let completion = Completion::new("call-2".to_string());
    completion
      .resolve(&evaluate(py, c"[1, 2, 3]"))
      .expect("a value inside the Bridge contract should be accepted");
    assert_eq!(completion.claim(), None, "the Call is answered");
  });
}

#[test]
fn a_value_outside_the_bridge_contract_leaves_the_call_unanswered() {
  Python::attach(|py| {
    silence_logs(py);
    let completion = Completion::new("call-3".to_string());
    let refusal = completion
      .resolve(&evaluate(py, c"{1, 2}"))
      .expect_err("a set should be refused");
    assert!(refusal.to_string().contains("Bridge contract"));
    assert_eq!(
      completion.claim(),
      Some("call-3".to_string()),
      "the Call is still owed an answer, so the refusal can be rejected"
    );
  });
}

#[test]
fn an_exception_answers_the_call() {
  Python::attach(|py| {
    silence_logs(py);
    let completion = Completion::new("call-4".to_string());
    completion.reject(&evaluate(py, c"PermissionError('denied')"));
    assert_eq!(completion.claim(), None, "the Call is answered");
  });
}

/// What every portal test has in front of it: a stand-in for the Completion
/// that records the answer and lets the test wait for it.
const HARNESS: &str = r#"
import asyncio, logging, threading, time

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

    def wait(self):
        assert self.done.wait(10), 'the Call was never answered'
        return self
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

/// Loads a portal of its own and runs `body` against it. The body leaves what
/// it found in `verdict`.
fn through_the_portal(body: &str) -> String {
  static PORTALS: AtomicUsize = AtomicUsize::new(0);
  let name = format!(
    "dry_portal_tested_{}",
    PORTALS.fetch_add(1, Ordering::Relaxed)
  );

  Python::attach(|py| {
    let portal = portal_in(py, &name);

    let globals = PyDict::new(py);
    globals
      .set_item("portal", portal)
      .expect("the portal should be reachable from the test");

    let script = CString::new(format!("{HARNESS}\n{body}\nportal.shutdown()\n"))
      .expect("the test body should be readable");
    py.run(&script, Some(&globals), None)
      .expect("the test body should run");

    globals
      .get_item("verdict")
      .expect("the test body should leave a verdict")
      .expect("the test body should leave a verdict")
      .extract()
      .expect("the verdict should be a string")
  })
}

#[test]
fn a_coroutine_callback_is_awaited() {
  assert_eq!(
    through_the_portal(
      r#"
async def doubled(number):
    await asyncio.sleep(0.01)
    return number * 2

answer = Answer()
portal.dispatch('doubled', doubled, (21,), answer)
verdict = repr(answer.wait().value)
"#
    ),
    "42"
  );
}

#[test]
fn a_callable_returning_an_awaitable_is_awaited_too() {
  assert_eq!(
    through_the_portal(
      r#"
class Doubler:
    async def __call__(self, number):
        await asyncio.sleep(0.01)
        return number * 2

answer = Answer()
portal.dispatch('doubled', Doubler(), (21,), answer)
verdict = repr(answer.wait().value)
"#
    ),
    "42"
  );
}

#[test]
fn a_plain_callback_runs_off_the_calling_thread() {
  assert_eq!(
    through_the_portal(
      r#"
here = threading.current_thread().name

def which_thread():
    return threading.current_thread().name

answer = Answer()
portal.dispatch('which_thread', which_thread, (), answer)
verdict = 'elsewhere' if answer.wait().value != here else 'on the calling thread'
"#
    ),
    "elsewhere"
  );
}

/// The whole point of the ticket, in the one place a test can see it: the
/// thread that took the Call is free again long before the callback ends. On
/// a real Webview that thread is the one drawing the window.
#[test]
fn a_slow_callback_hands_the_calling_thread_straight_back() {
  assert_eq!(
    through_the_portal(
      r#"
def slow():
    time.sleep(0.5)
    return 'done'

answer = Answer()
started = time.monotonic()
portal.dispatch('slow', slow, (), answer)
handed_back = time.monotonic() - started
answer.wait()
verdict = 'free' if handed_back < 0.1 else f'blocked for {handed_back:.2f}s'
"#
    ),
    "free"
  );
}

#[test]
fn two_slow_calls_overlap_instead_of_queueing() {
  assert_eq!(
    through_the_portal(
      r#"
def slow():
    time.sleep(0.3)
    return 'done'

first, second = Answer(), Answer()
started = time.monotonic()
portal.dispatch('slow', slow, (), first)
portal.dispatch('slow', slow, (), second)
first.wait()
second.wait()
elapsed = time.monotonic() - started
verdict = 'overlapped' if elapsed < 0.5 else f'serialised over {elapsed:.2f}s'
"#
    ),
    "overlapped"
  );
}

#[test]
fn a_plain_callback_that_raises_rejects_its_call() {
  assert_eq!(
    through_the_portal(
      r#"
def refuse():
    raise PermissionError('denied')

answer = Answer()
portal.dispatch('refuse', refuse, (), answer)
error = answer.wait().error
verdict = f'{type(error).__name__}: {error}'
"#
    ),
    "PermissionError: denied"
  );
}

#[test]
fn a_coroutine_callback_that_raises_rejects_its_call() {
  assert_eq!(
    through_the_portal(
      r#"
async def refuse():
    await asyncio.sleep(0.01)
    raise PermissionError('denied')

answer = Answer()
portal.dispatch('refuse', refuse, (), answer)
error = answer.wait().error
verdict = f'{type(error).__name__}: {error}'
"#
    ),
    "PermissionError: denied"
  );
}

/// The refusal comes from `dry/signature.py`, reached by the relative import
/// the portal is loaded inside its package for: the wording is `mismatch`'s
/// own, so a Call refused before it ran is what this asserts, not a callback
/// that ran and raised `TypeError` by itself.
#[test]
fn a_call_with_the_wrong_arguments_rejects_rather_than_hangs() {
  assert_eq!(
    through_the_portal(
      r#"
async def needs_two(first, second):
    return first + second

answer = Answer()
portal.dispatch('needs_two', needs_two, (1,), answer)
error = answer.wait().error
verdict = f'{type(error).__name__}: {error}'
"#
    ),
    "TypeError: needs_two takes 2 arguments, received 1. second was not passed."
  );
}

/// The isolation every portal test rests on, checked rather than assumed: two
/// loads are two modules, with two sets of the state the portal keeps for the
/// process. Shutting one down leaves the other open for business.
#[test]
fn each_load_is_a_portal_of_its_own() {
  Python::attach(|py| {
    silence_logs(py);
    let first = portal_in(py, "dry_portal_isolated_a");
    let second = portal_in(py, "dry_portal_isolated_b");

    assert!(!first.is(&second), "two loads are two modules");

    let globals = PyDict::new(py);
    globals
      .set_item("first", &first)
      .expect("the first portal should be reachable from the test");
    globals
      .set_item("second", &second)
      .expect("the second portal should be reachable from the test");

    let script = CString::new(format!(
      "{HARNESS}
first.dispatch('name', lambda: 'answered', (), Answer())
first.shutdown()

answer = Answer()
second.dispatch('name', lambda: 'answered', (), answer)
verdict = answer.wait().value
second.shutdown()
"
    ))
    .expect("the test body should be readable");
    py.run(&script, Some(&globals), None)
      .expect("the test body should run");

    let verdict: String = globals
      .get_item("verdict")
      .expect("the test body should leave a verdict")
      .expect("the test body should leave a verdict")
      .extract()
      .expect("the verdict should be a string");
    assert_eq!(
      verdict, "answered",
      "a closed portal did not close the other"
    );
  });
}

#[test]
fn a_closed_portal_refuses_a_new_call() {
  assert_eq!(
    through_the_portal(
      r#"
portal.shutdown()
try:
    portal.dispatch('anything', lambda: None, (), Answer())
    verdict = 'accepted'
except RuntimeError as error:
    verdict = str(error)
"#
    ),
    "The Bridge is closed, so the Call cannot be run."
  );
}
