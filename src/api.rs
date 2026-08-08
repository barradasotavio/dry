//! The Call path: from a message arriving over the Bridge to the reply that
//! settles the Promise that started it.
//!
//! Nothing here waits for a callable to return. A Call is read off the wire,
//! its arguments are converted, and it is handed to the portal — Dry's asyncio
//! loop and thread pool, in `dry/portal.py` — which answers it whenever it
//! finishes. The thread that took the message is the thread that draws the
//! window, and it gets back to drawing immediately.

use pyo3::{
  Bound, Py, PyAny, PyErr, PyResult, Python, pyclass, pymethods, types::PyAnyMethods,
};
use std::{collections::HashMap, error::Error, sync::Mutex};

use crate::{
  errors::py_error_message,
  events::{AppEvent, send_to_event_loop},
  logs,
  types::{
    Call, CallResult, arguments_to_python, call_result_script, from_python, parse_call,
  },
};

pub const API_JS: &str = include_str!("js/api.js");

#[cfg(test)]
mod tests;

/// The module that owns the asyncio loop and the thread pool.
const PORTAL: &str = "dry.portal";

/// One Call's right to an answer, handed to Python along with the callable.
///
/// A Promise settles once, so a Completion answers once: the call id is taken
/// out on the way past, and a second answer finds nothing left to send. The id
/// survives a `resolve` that fails, though — a return value outside the Bridge
/// contract is refused before the id is taken, so the portal can turn round
/// and reject the Call with that refusal.
#[pyclass(frozen)]
pub struct Completion {
  call_id: Mutex<Option<String>>,
}

impl Completion {
  pub fn new(call_id: String) -> Self {
    Completion {
      call_id: Mutex::new(Some(call_id)),
    }
  }

  /// Takes the right to answer, or `None` if this Call is already answered.
  fn claim(&self) -> Option<String> {
    self.call_id.lock().ok().and_then(|mut slot| slot.take())
  }

  /// A reply that cannot be sent leaves a Promise pending, and there is
  /// nothing further to do about it but say so.
  fn send(&self, result: CallResult) {
    let script = match call_result_script(&result) {
      Ok(script) => script,
      Err(err) => {
        logs::error(
          logs::BRIDGE,
          format!("The reply to a Call could not be written: {err}"),
        );
        return;
      },
    };
    if let Err(err) = send_to_event_loop(AppEvent::RunJavascript(script)) {
      logs::error(
        logs::BRIDGE,
        format!("The reply to a Call could not be sent: {err}"),
      );
    }
  }
}

#[pymethods]
impl Completion {
  /// Answers with the value the callable returned. Raises if that value is
  /// outside the Bridge contract, leaving the Call unanswered.
  fn resolve(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
    let result = from_python(value)?;
    if let Some(call_id) = self.claim() {
      self.send(CallResult {
        call_id,
        result,
        error: None,
      });
    }
    Ok(())
  }

  /// Answers with the exception that ended the Call, type name first, so the
  /// frontend rejects with an `Error` it can tell apart.
  fn reject(&self, error: &Bound<'_, PyAny>) {
    if let Some(call_id) = self.claim() {
      let message = py_error_message(error.py(), &PyErr::from_value(error.clone()));
      self.send(CallResult::failed(call_id, message));
    }
  }
}

/// Reads one message off the Bridge and starts the Call it carries.
///
/// A Call that cannot even be started still settles its Promise: the reason
/// goes back as a rejection rather than leaving the frontend waiting.
pub fn handle_api_requests(
  request_body: &str, api: &HashMap<String, Py<PyAny>>,
) -> Result<(), Box<dyn Error>> {
  let call = parse_call(request_body)?;

  if let Err(reason) = dispatch_call(&call, api) {
    logs::error(
      logs::BRIDGE,
      format!("The Call to '{}' could not be run: {reason}", call.function),
    );
    let script = call_result_script(&CallResult::failed(call.call_id, reason))?;
    send_to_event_loop(AppEvent::RunJavascript(script))?;
  }

  Ok(())
}

/// Hands a Call to the portal and returns. The GIL is held for the lookup and
/// the argument conversion only; the callable itself runs on a thread of the
/// portal's choosing, long after this has returned.
fn dispatch_call(call: &Call, api: &HashMap<String, Py<PyAny>>) -> Result<(), String> {
  let function = api
    .get(&call.function)
    .ok_or_else(|| format!("Function {} not found.", call.function))?;

  Python::attach(|py| {
    let start = || -> PyResult<()> {
      let arguments = arguments_to_python(py, &call.arguments)?;
      let completion = Py::new(py, Completion::new(call.call_id.clone()))?;
      py.import(PORTAL)?.call_method1(
        "dispatch",
        (&call.function, function, arguments, completion),
      )?;
      Ok(())
    };
    start().map_err(|err| py_error_message(py, &err))
  })
}
