//! Where Dry's diagnostics go.
//!
//! Nothing here writes to stdout or stderr. Every record goes through Python's
//! `logging` under the package's own namespace, so an application routes,
//! filters and formats Dry the way it routes any other library. The package
//! installs a `NullHandler` on `dry`, so an application that configures
//! nothing sees nothing.

use pyo3::{PyResult, Python, types::PyAnyMethods};

/// Opening the Webview, and the window it lives in.
pub const WEBVIEW: &str = "dry.webview";

/// Messages crossing the Bridge, in either direction.
pub const BRIDGE: &str = "dry.bridge";

pub fn debug(logger: &str, message: impl AsRef<str>) {
  record(logger, "debug", message.as_ref());
}

pub fn warning(logger: &str, message: impl AsRef<str>) {
  record(logger, "warning", message.as_ref());
}

pub fn error(logger: &str, message: impl AsRef<str>) {
  record(logger, "error", message.as_ref());
}

/// A failure to log is not worth failing over, and there is nowhere left to
/// report it to, so it is dropped.
fn record(logger: &str, level: &str, message: &str) {
  Python::attach(|py| {
    let _ = emit(py, logger, level, message);
  });
}

fn emit(py: Python<'_>, logger: &str, level: &str, message: &str) -> PyResult<()> {
  py.import("logging")?
    .call_method1("getLogger", (logger,))?
    .call_method1(level, (message,))?;
  Ok(())
}
