use pyo3::{Py, PyAny, Python};
use std::{collections::HashMap, error::Error};
use tao::event_loop::EventLoopProxy;

use crate::{
  errors::{catch_panic, py_error_message},
  events::AppEvent,
  logs,
  types::{
    Call, CallResult, arguments_to_python, call_result_script, from_python, parse_call,
  },
};

pub const API_JS: &str = include_str!("js/api.js");

/// Runs a Call against the Api, holding the GIL for exactly as long as the
/// Python callable does. Everything either side of it is conversion, and lives
/// in `types.rs` where a test can reach it.
///
/// A callable that raises is not a failure of the Call: the exception rides
/// back to the frontend inside the CallResult, type name first, so JavaScript
/// can tell a `ValueError` from a `PermissionError`.
fn run_call(
  call: &Call, api: &HashMap<String, Py<PyAny>>,
) -> Result<CallResult, Box<dyn Error>> {
  let py_func = api
    .get(&call.function)
    .ok_or(format!("Function {} not found.", call.function))?;
  Python::attach(|py| {
    let py_args = arguments_to_python(py, &call.arguments)?;
    match catch_panic(|| py_func.call1(py, py_args)) {
      Ok(py_result) => Ok(CallResult {
        call_id: call.call_id.clone(),
        result: from_python(py_result.bind(py))?,
        error: None,
      }),
      Err(py_err) => {
        logs::exception(
          logs::BRIDGE,
          format!("The Call to '{}' raised.", call.function),
          &py_err,
        );
        Ok(CallResult::failed(
          call.call_id.clone(),
          py_error_message(py, &py_err),
        ))
      },
    }
  })
}

pub fn handle_api_requests(
  request_body: &String, api: &HashMap<String, Py<PyAny>>,
  event_loop_proxy: &EventLoopProxy<AppEvent>,
) -> Result<(), Box<dyn Error>> {
  let call = parse_call(request_body)?;
  let call_result = match run_call(&call, api) {
    Ok(call_result) => call_result,
    Err(err) => {
      logs::error(logs::BRIDGE, format!("The Call could not be run: {err}"));
      CallResult::failed(call.call_id, err.to_string())
    },
  };
  event_loop_proxy
    .send_event(AppEvent::RunJavascript(call_result_script(&call_result)?))?;
  Ok(())
}
