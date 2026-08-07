//! Where Dry's diagnostics go.
//!
//! Nothing here writes to stdout or stderr. Every record goes through Python's
//! `logging` under the package's own namespace, so an application routes,
//! filters and formats Dry the way it routes any other library. The package
//! installs a `NullHandler` on `dry`, so an application that configures
//! nothing sees nothing.

use pyo3::{
  PyErr, PyResult, Python,
  types::{PyAnyMethods, PyDict, PyDictMethods},
};

/// Opening the Webview, and the window it lives in.
pub const WEBVIEW: &str = "dry.webview";

/// Messages crossing the Bridge, in either direction.
pub const BRIDGE: &str = "dry.bridge";

pub fn debug(logger: &str, message: impl AsRef<str>) {
  record(logger, "debug", message.as_ref(), None);
}

pub fn warning(logger: &str, message: impl AsRef<str>) {
  record(logger, "warning", message.as_ref(), None);
}

pub fn error(logger: &str, message: impl AsRef<str>) {
  record(logger, "error", message.as_ref(), None);
}

/// An error record carrying a Python exception, so the handler that formats
/// it gets the traceback too.
pub fn exception(logger: &str, message: impl AsRef<str>, error: &PyErr) {
  record(logger, "error", message.as_ref(), Some(error));
}

/// A failure to log is not worth failing over, and there is nowhere left to
/// report it to, so it is dropped.
fn record(logger: &str, level: &str, message: &str, error: Option<&PyErr>) {
  Python::attach(|py| {
    let _ = emit(py, logger, level, message, error);
  });
}

fn emit(
  py: Python<'_>, logger: &str, level: &str, message: &str, error: Option<&PyErr>,
) -> PyResult<()> {
  let logger = py.import("logging")?.call_method1("getLogger", (logger,))?;
  match error {
    Some(error) => {
      let keywords = PyDict::new(py);
      keywords.set_item("exc_info", error.value(py))?;
      logger.call_method(level, (message,), Some(&keywords))?;
    },
    None => {
      logger.call_method1(level, (message,))?;
    },
  }
  Ok(())
}
