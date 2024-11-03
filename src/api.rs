use std::{collections::HashMap, error::Error};

use pyo3::{
    types::{PyFunction, PyTuple},
    FromPyObject, Py, PyObject, Python, ToPyObject,
};
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use tao::event_loop::EventLoopProxy;

use crate::AppEvent;

pub const API_JS: &str = include_str!("js/api.js");

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, FromPyObject)]
#[serde(untagged)]
enum CommonKey {
    Boolean(bool),
    Integer(i64),
    String(String),
}

impl ToPyObject for CommonKey {
    fn to_object(
        &self,
        py: Python,
    ) -> PyObject {
        match self {
            CommonKey::Boolean(value) => value.to_object(py),
            CommonKey::Integer(value) => value.to_object(py),
            CommonKey::String(value) => value.to_object(py),
        }
    }
}

#[derive(FromPyObject, Serialize, Deserialize)]
#[serde(untagged)]
enum CommonType {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<CommonType>),
    Dict(HashMap<CommonKey, CommonType>),
}

impl ToPyObject for CommonType {
    fn to_object(
        &self,
        py: Python,
    ) -> PyObject {
        match self {
            CommonType::Boolean(value) => value.to_object(py),
            CommonType::Integer(value) => value.to_object(py),
            CommonType::Float(value) => value.to_object(py),
            CommonType::String(value) => value.to_object(py),
            CommonType::List(value) => value.to_object(py),
            CommonType::Dict(value) => value.to_object(py),
        }
    }
}

#[derive(Deserialize)]
struct CallRequest {
    call_id: String,
    function: String,
    arguments: Option<Vec<CommonType>>,
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
            let py_args = match &self.arguments {
                Some(args) => PyTuple::new_bound(py, args),
                None => PyTuple::empty_bound(py),
            };
            let py_result: Option<CommonType> =
                py_func.call1(py, py_args)?.extract(py)?;
            Ok(CallResponse {
                call_id: self.call_id.clone(),
                result: py_result,
                error: None,
            })
        })
    }
}

#[derive(Serialize)]
struct CallResponse {
    call_id: String,
    result: Option<CommonType>,
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
                result: None,
                error: Some(err.to_string()),
            }
        },
    };
    call_response.run(&event_loop_proxy)?;
    Ok(())
}
