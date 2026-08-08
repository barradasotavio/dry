//! Tests for the Call path, on both sides of it.
//!
//! The Completion is exercised straight from Rust: it is the piece that
//! decides whether a Call still has an answer owing, and none of that needs a
//! window. The portal is loaded on its own, without the package around it, so
//! the dispatch that keeps a callback off the event-loop thread can be watched
//! happening — a slow callable handed back immediately, two of them
//! overlapping, a coroutine awaited — again with no window anywhere.

use pyo3::{
  Bound, Python,
  types::{PyAnyMethods, PyDict, PyDictMethods, PyModule},
};
use std::{
  ffi::CString,
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

/// Loads `dry/portal.py` by itself, without the package that would drag the
/// extension module in with it, and runs `body` against it. The body leaves
/// what it found in `verdict`.
///
/// Every test gets a portal of its own — a loop, a thread pool and a name
/// nothing else answers to — so that one test's shutdown is not another's.
fn through_the_portal(body: &str) -> String {
  static PORTALS: AtomicUsize = AtomicUsize::new(0);
  let name = CString::new(format!(
    "dry_portal_tested_{}",
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

#[test]
fn a_call_with_the_wrong_arguments_rejects_rather_than_hangs() {
  assert_eq!(
    through_the_portal(
      r#"
async def needs_two(first, second):
    return first + second

answer = Answer()
portal.dispatch('needs_two', needs_two, (1,), answer)
verdict = type(answer.wait().error).__name__
"#
    ),
    "TypeError"
  );
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
