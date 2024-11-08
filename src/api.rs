use std::{collections::HashMap, error::Error};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, Utc};
use pyo3::{
    types::{
        PyAnyMethods, PyBool, PyBytes, PyFloat, PyFunction, PyLong, PySet,
        PyTuple, PyTypeMethods,
    },
    Bound, FromPyObject, Py, PyAny, PyErr, PyObject, Python, ToPyObject,
};
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use tao::event_loop::EventLoopProxy;

use crate::AppEvent;

pub const API_JS: &str = include_str!("js/api.js");

#[derive(Debug, Clone, Copy)]
struct ExactFloat(f64);

impl PartialEq for ExactFloat {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for ExactFloat {}

impl std::hash::Hash for ExactFloat {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        self.0.to_bits().hash(state);
    }
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Hash)]
#[serde(untagged)]
enum CommonKey {
    Boolean(bool),
    Integer(i64),
    Float(ExactFloat),
    String(String),
    #[serde(serialize_with = "serialize_none")]
    None,
}

fn serialize_none<S>(serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_none()
}

impl<'source> FromPyObject<'source> for CommonKey {
    fn extract_bound(ob: &Bound<'source, PyAny>) -> Result<Self, PyErr> {
        if ob.get_type().is_subclass_of::<PyBool>()? {
            return Ok(CommonKey::Boolean(ob.extract()?));
        }

        if ob.is_none() {
            return Ok(CommonKey::None);
        }

        if ob.get_type().is_subclass_of::<PyLong>()? {
            return Ok(CommonKey::Integer(ob.extract()?));
        }

        if ob.get_type().is_subclass_of::<PyFloat>()? {
            return Ok(CommonKey::Float(ExactFloat(ob.extract()?)));
        }

        if let Ok(val) = ob.extract::<String>() {
            return Ok(CommonKey::String(val));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Unsupported type for dictionary keys: {:?}",
            ob.get_type().name()?.to_string()
        )))
    }
}

impl ToPyObject for CommonKey {
    fn to_object(
        &self,
        py: Python,
    ) -> PyObject {
        match self {
            CommonKey::Boolean(value) => value.to_object(py),
            CommonKey::Integer(value) => value.to_object(py),
            CommonKey::Float(value) => value.0.to_object(py),
            CommonKey::String(value) => value.to_object(py),
            CommonKey::None => py.None(),
        }
    }
}

impl Serialize for ExactFloat {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ExactFloat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        f64::deserialize(deserializer).map(ExactFloat)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum CommonType {
    #[serde(serialize_with = "serialize_none")]
    None,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Tuple(Vec<CommonType>),
    Set(Vec<CommonType>),
    List(Vec<CommonType>),
    Dict(HashMap<CommonKey, CommonType>),
    Bytes(String),
    DateTime(DateTime<Utc>),
    Date(NaiveDate),
    Time(NaiveTime),
    Complex {
        real: f64,
        imag: f64,
    },
}

impl<'source> FromPyObject<'source> for CommonType {
    fn extract_bound(ob: &Bound<'source, PyAny>) -> Result<Self, PyErr> {
        let type_name = ob.get_type().name()?.to_string();

        match type_name.as_str() {
            "bool" if ob.get_type().is_subclass_of::<PyBool>()? => {
                return Ok(CommonType::Boolean(ob.extract()?));
            },
            "int" if ob.get_type().is_subclass_of::<PyLong>()? => {
                return Ok(CommonType::Integer(ob.extract()?));
            },
            "float" if ob.get_type().is_subclass_of::<PyFloat>()? => {
                return Ok(CommonType::Float(ob.extract()?));
            },
            "bytes" | "bytearray" => {
                let bytes: Vec<u8> = ob.extract()?;
                return Ok(CommonType::Bytes(BASE64.encode(bytes)));
            },
            "datetime" => {
                let timestamp: f64 =
                    ob.call_method0("timestamp")?.extract()?;
                return Ok(CommonType::DateTime(
                    DateTime::from_timestamp(timestamp as i64, 0)
                        .unwrap_or_default(),
                ));
            },
            "date" => {
                let year: i32 = ob.getattr("year")?.extract()?;
                let month: u32 = ob.getattr("month")?.extract()?;
                let day: u32 = ob.getattr("day")?.extract()?;
                return Ok(CommonType::Date(
                    NaiveDate::from_ymd_opt(year, month, day)
                        .unwrap_or_default(),
                ));
            },
            "time" => {
                let hour: u32 = ob.getattr("hour")?.extract()?;
                let minute: u32 = ob.getattr("minute")?.extract()?;
                let second: u32 = ob.getattr("second")?.extract()?;
                return Ok(CommonType::Time(
                    NaiveTime::from_hms_opt(hour, minute, second)
                        .unwrap_or_default(),
                ));
            },
            "complex" => {
                let real: f64 = ob.getattr("real")?.extract()?;
                let imag: f64 = ob.getattr("imag")?.extract()?;
                return Ok(CommonType::Complex { real, imag });
            },
            _ => {},
        }

        if ob.is_none() {
            return Ok(CommonType::None);
        }

        if let Ok(val) = ob.extract::<String>() {
            return Ok(CommonType::String(val));
        }

        if ob.hasattr("items")? {
            let dict: HashMap<CommonKey, CommonType> = ob.extract()?;
            return Ok(CommonType::Dict(dict));
        }

        if ob.hasattr("__iter__")? && !ob.hasattr("items")? {
            return match type_name.as_str() {
                "tuple" => Ok(CommonType::Tuple(ob.extract()?)),
                "set" => {
                    let set = ob.downcast::<PySet>()?;
                    let mut items = Vec::new();
                    for item in set.iter()? {
                        items.push(item?.extract()?);
                    }
                    Ok(CommonType::Set(items))
                },
                _ => Ok(CommonType::List(ob.extract()?)),
            };
        }

        Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Unsupported type for function returns: {:?}",
            type_name
        )))
    }
}

impl ToPyObject for CommonType {
    fn to_object(
        &self,
        py: Python,
    ) -> PyObject {
        match self {
            CommonType::Complex { real, imag } => {
                let args = PyTuple::new_bound(py, [real, imag]);
                py.import_bound("complex")
                    .unwrap()
                    .call1(args)
                    .unwrap()
                    .into()
            },
            CommonType::DateTime(dt) => {
                let datetime = py.import_bound("datetime").unwrap();
                datetime
                    .getattr("datetime")
                    .unwrap()
                    .call_method1("fromtimestamp", (dt.timestamp(),))
                    .unwrap()
                    .into()
            },
            CommonType::Date(d) => {
                let datetime = py.import_bound("datetime").unwrap();
                datetime
                    .getattr("date")
                    .unwrap()
                    .call_method1("fromordinal", (d.ordinal() as i32,))
                    .unwrap()
                    .into()
            },
            CommonType::Time(t) => {
                let datetime = py.import_bound("datetime").unwrap();
                datetime
                    .getattr("time")
                    .unwrap()
                    .call_method1(
                        "fromisoformat",
                        (t.format("%H:%M:%S").to_string(),),
                    )
                    .unwrap()
                    .into()
            },

            CommonType::Bytes(value) => BASE64
                .decode(value)
                .map(|bytes| PyBytes::new_bound(py, &bytes).into())
                .unwrap_or_else(|_| py.None()),

            CommonType::Dict(value) => value.to_object(py),
            CommonType::List(value) => value.to_object(py),
            CommonType::Tuple(value) => value.to_object(py),
            CommonType::Set(value) => value.to_object(py),

            CommonType::Boolean(value) => value.to_object(py),
            CommonType::Integer(value) => value.to_object(py),
            CommonType::Float(value) => value.to_object(py),
            CommonType::String(value) => value.to_object(py),
            CommonType::None => py.None(),
        }
    }
}

#[derive(Deserialize)]
struct CallRequest {
    call_id: String,
    function: String,
    arguments: Vec<CommonType>,
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
            let py_args = PyTuple::new_bound(py, &self.arguments);
            let py_result: CommonType =
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
    result: CommonType,
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
                result: CommonType::None,
                error: Some(err.to_string()),
            }
        },
    };
    call_response.run(&event_loop_proxy)?;
    Ok(())
}
