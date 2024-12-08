use pyo3::{
    types::{PyFunction, PyTuple},
    Py, Python,
};
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use std::{collections::HashMap, error::Error};
use tao::event_loop::EventLoopProxy;

use crate::{
    events::AppEvent,
    types::{NoneType, PythonType},
};

pub const API_JS: &str = include_str!("js/api.js");

#[derive(Deserialize)]
struct CallRequest {
    call_id: String,
    function: String,
    arguments: Vec<PythonType>,
}

impl CallRequest {
    fn run(
        &self,
        api: &HashMap<String, Py<PyFunction>>,
    ) -> Result<CallResponse, Box<dyn Error>> {
        let py_func = api
            .get(&self.function)
            .ok_or(format!("Function {} not found.", self.function))?;
        Python::with_gil(|py| {
            let py_args = PyTuple::new(py, self.arguments.clone())?;
            match py_func.call1(py, py_args) {
                Ok(py_result) => Ok(CallResponse {
                    call_id: self.call_id.clone(),
                    result: py_result.extract(py)?,
                    error: None,
                }),
                Err(py_err) => {
                    py_err.display(py);
                    Ok(CallResponse {
                        call_id: self.call_id.clone(),
                        result: PythonType::None(NoneType),
                        error: Some(py_err.to_string()),
                    })
                },
            }
        })
    }
}

#[derive(Serialize)]
struct CallResponse {
    call_id: String,
    result: PythonType,
    error: Option<String>,
}

impl CallResponse {
    fn run(
        &self,
        event_loop_proxy: &EventLoopProxy<AppEvent>,
    ) -> Result<(), Box<dyn Error>> {
        let response = format!("window.ipcCallback({})", to_string(self)?);
        event_loop_proxy.send_event(AppEvent::RunJavascript(response))?;
        Ok(())
    }
}

pub fn handle_api_requests(
    request_body: &String,
    api: &HashMap<String, Py<PyFunction>>,
    event_loop_proxy: &EventLoopProxy<AppEvent>,
) -> Result<(), Box<dyn Error>> {
    let call_request: CallRequest = from_str(request_body)?;
    let call_response = match call_request.run(api) {
        Ok(call_response) => call_response,
        Err(err) => {
            eprintln!("{:?}", err);
            CallResponse {
                call_id: call_request.call_id,
                result: PythonType::None(NoneType),
                error: Some(err.to_string()),
            }
        },
    };
    call_response.run(&event_loop_proxy)?;
    Ok(())
}
