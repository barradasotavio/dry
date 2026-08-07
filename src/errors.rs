//! The failures Dry reports to Python.
//!
//! Every way this library can fail arrives in Python as a `DryError`, so an
//! application can catch what it means rather than catching everything. The
//! hierarchy is deliberately shallow and named after the domain: a Webview
//! that could not be opened, a Bridge that could not carry a message, and a
//! Rust panic that would otherwise have taken the process with it.
//!
//! The formatting helpers here are free functions over owned data, so a test
//! can exercise them without opening a Webview or running an event loop.

use std::{
  any::Any,
  panic::{AssertUnwindSafe, catch_unwind, set_hook, take_hook},
  sync::{Arc, Mutex},
};

use pyo3::{
  Bound, PyErr, PyResult, Python, create_exception,
  exceptions::PyException,
  types::{PyAnyMethods, PyModule, PyModuleMethods, PyTypeMethods},
};

#[cfg(test)]
mod tests;

create_exception!(
  dry.exceptions,
  DryError,
  PyException,
  "Base class for every failure Dry reports."
);

create_exception!(
  dry.exceptions,
  WebviewError,
  DryError,
  "The Webview could not be opened, or its Content could not be resolved."
);

create_exception!(
  dry.exceptions,
  BridgeError,
  DryError,
  "A message could not cross the Bridge."
);

create_exception!(
  dry.exceptions,
  PanicError,
  DryError,
  "Rust panicked. A bug in Dry, not in the calling application."
);

/// Puts the hierarchy on the extension module, so `dry/exceptions.py` has
/// something to re-export.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
  let py = module.py();
  module.add("DryError", py.get_type::<DryError>())?;
  module.add("WebviewError", py.get_type::<WebviewError>())?;
  module.add("BridgeError", py.get_type::<BridgeError>())?;
  module.add("PanicError", py.get_type::<PanicError>())?;
  Ok(())
}

/// Runs `body`, turning a panic into a `PanicError` rather than letting it
/// unwind into the interpreter. The default panic hook is silenced for the
/// duration, so nothing reaches stderr uninvited; the location it would have
/// printed is folded into the exception message instead.
///
/// Anything that can panic and is reachable from Python belongs inside this,
/// and nothing that unwinds through a platform event loop does.
pub fn catch_panic<T>(body: impl FnOnce() -> PyResult<T>) -> PyResult<T> {
  let location: Arc<Mutex<Option<String>>> = Arc::default();
  let captured = Arc::clone(&location);

  let previous_hook = take_hook();
  set_hook(Box::new(move |info| {
    if let Ok(mut slot) = captured.lock() {
      *slot = info
        .location()
        .map(|at| format!("{}:{}", at.file(), at.line()));
    }
  }));
  let outcome = catch_unwind(AssertUnwindSafe(body));
  set_hook(previous_hook);

  match outcome {
    Ok(result) => result,
    Err(payload) => {
      let at = location.lock().ok().and_then(|slot| slot.clone());
      Err(PanicError::new_err(panic_description(
        &*payload,
        at.as_deref(),
      )))
    },
  }
}

/// Reads whatever `panic!` was handed, which is a `&str` for a literal and a
/// `String` for a formatted message. Anything else is a payload from a
/// `panic_any` this library never calls.
pub fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
  if let Some(message) = payload.downcast_ref::<&str>() {
    (*message).to_string()
  } else if let Some(message) = payload.downcast_ref::<String>() {
    message.clone()
  } else {
    "Panicked with an unknown payload.".to_string()
  }
}

/// The message a `PanicError` carries: what panicked, and where.
pub fn panic_description(payload: &(dyn Any + Send), at: Option<&str>) -> String {
  let message = panic_payload_message(payload);
  match at {
    Some(at) => format!("Dry panicked at {at}: {message}"),
    None => format!("Dry panicked: {message}"),
  }
}

/// The way an exception crosses the Bridge: its type name, then its message.
///
/// The frontend gets a `TypeError` it can tell apart from a `PermissionError`
/// instead of one opaque string, and `api.js` splits this back into the
/// `name` and `message` of the JavaScript `Error` it rejects with.
pub fn py_error_message(py: Python<'_>, error: &PyErr) -> String {
  let name = error
    .get_type(py)
    .name()
    .map(|name| name.to_string())
    .unwrap_or_else(|_| "Exception".to_string());
  let message = error
    .value(py)
    .str()
    .map(|message| message.to_string())
    .unwrap_or_default();
  if message.is_empty() {
    name
  } else {
    format!("{name}: {message}")
  }
}
