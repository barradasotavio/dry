use pyo3::{Py, PyAny, Python};
use std::{collections::HashMap, error::Error};
use tao::event_loop::EventLoopProxy;

use crate::{
  events::AppEvent,
  types::{
    Call, CallResult, arguments_to_python, call_result_script, from_python, parse_call,
  },
};

pub const API_JS: &str = include_str!("js/api.js");

/// Runs a Call against the Api, holding the GIL for exactly as long as the
/// Python callable does. Everything either side of it is conversion, and lives
/// in `types.rs` where a test can reach it.
fn run_call(
  call: &Call, api: &HashMap<String, Py<PyAny>>,
) -> Result<CallResult, Box<dyn Error>> {
  let py_func = api
    .get(&call.function)
    .ok_or(format!("Function {} not found.", call.function))?;
  Python::attach(|py| {
    let py_args = arguments_to_python(py, &call.arguments)?;
    match py_func.call1(py, py_args) {
      Ok(py_result) => Ok(CallResult {
        call_id: call.call_id.clone(),
        result: from_python(py_result.bind(py))?,
        error: None,
      }),
      Err(py_err) => {
        py_err.display(py);
        Ok(CallResult::failed(call.call_id.clone(), py_err.to_string()))
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
      eprintln!("{:?}", err);
      CallResult::failed(call.call_id, err.to_string())
    },
  };
  event_loop_proxy
    .send_event(AppEvent::RunJavascript(call_result_script(&call_result)?))?;
  Ok(())
}
