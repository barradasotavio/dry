//! The Bridge contract: the values that may cross between Python and the
//! frontend, and the conversions that carry them.
//!
//! Every conversion here is a free function over owned data, so a test can
//! exercise both directions without opening a Webview or running an event
//! loop. The call path in `api.rs` holds the GIL and reaches Python; this
//! module only translates.

use pyo3::{
  Borrowed, Bound, FromPyObject, IntoPyObject, PyAny, PyErr, PyResult, Python,
  exceptions::PyTypeError,
  types::{PyAnyMethods, PyNone, PySet, PyTuple, PyTypeMethods},
};
use serde::{Deserialize, Serialize};
use serde_json::{Error as JsonError, from_str, to_string};
use std::{
  collections::HashMap,
  convert::Infallible,
  hash::{Hash, Hasher},
};

#[cfg(test)]
mod tests;

#[derive(Serialize, Deserialize, FromPyObject, IntoPyObject, Clone, Debug)]
pub struct FloatType(f64);

impl Eq for FloatType {}

impl PartialEq for FloatType {
  fn eq(&self, other: &Self) -> bool {
    self.0.to_bits() == other.0.to_bits()
  }
}

impl Hash for FloatType {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.0.to_bits().hash(state)
  }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NoneType;

impl<'a, 'py> FromPyObject<'a, 'py> for NoneType {
  type Error = PyErr;

  fn extract(ob: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
    if ob.is_none() {
      Ok(NoneType)
    } else {
      Err(PyTypeError::new_err(format!(
        "Expected None, found {:?}",
        ob.get_type().to_string()
      )))
    }
  }
}

impl<'py> IntoPyObject<'py> for NoneType {
  type Target = PyNone;
  type Output = Borrowed<'py, 'py, Self::Target>;
  type Error = Infallible;

  fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
    Ok(PyNone::get(py))
  }
}

#[derive(Serialize, Deserialize, IntoPyObject, Clone, Debug)]
pub struct SetType<T>(Vec<T>);

impl<'a, 'py, T> FromPyObject<'a, 'py> for SetType<T>
where
  T: for<'x> FromPyObject<'x, 'py>,
{
  type Error = PyErr;

  fn extract(ob: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
    if ob.get_type().name()? == "set" {
      let set = ob.cast::<PySet>()?;
      let mut items = Vec::new();
      for item in set.try_iter()? {
        items.push(item?.extract().map_err(Into::into)?);
      }
      Ok(SetType(items))
    } else {
      Err(PyTypeError::new_err(format!(
        "Expected Set, found {:?}",
        ob.get_type().to_string()
      )))
    }
  }
}

#[derive(
  Serialize, Deserialize, FromPyObject, IntoPyObject, Eq, PartialEq, Hash, Clone, Debug,
)]
#[serde(untagged)]
pub enum Primitive {
  Integer(i64),
  Float(FloatType),
  Boolean(bool),
  String(String),
}

#[derive(Serialize, Deserialize, FromPyObject, IntoPyObject, Clone, Debug)]
#[serde(untagged)]
pub enum NonPrimitive<T> {
  Sequence(Vec<T>),
  Mapping(HashMap<Primitive, T>),
  Set(SetType<T>),
}

#[derive(Serialize, Deserialize, FromPyObject, IntoPyObject, Clone, Debug)]
#[serde(untagged)]
pub enum PythonType {
  None(NoneType),
  Primitive(Primitive),
  NonPrimitive(NonPrimitive<PythonType>),
}

/// A Call travelling from the frontend to Python.
#[derive(Deserialize, Debug)]
pub struct Call {
  pub call_id: String,
  pub function: String,
  pub arguments: Vec<PythonType>,
}

/// The reply to a Call, travelling from Python back to the frontend.
#[derive(Serialize, Debug)]
pub struct CallResult {
  pub call_id: String,
  pub result: PythonType,
  pub error: Option<String>,
}

impl CallResult {
  /// A reply carrying no value, only the reason the Call did not produce one.
  pub fn failed(call_id: String, error: String) -> Self {
    CallResult {
      call_id,
      result: PythonType::None(NoneType),
      error: Some(error),
    }
  }
}

/// Reads a Call off the wire.
pub fn parse_call(body: &str) -> Result<Call, JsonError> {
  from_str(body)
}

/// Writes the JavaScript that hands a CallResult back to the frontend.
pub fn call_result_script(result: &CallResult) -> Result<String, JsonError> {
  Ok(format!("window.ipcCallback({})", to_string(result)?))
}

/// Reads a Python value into the Bridge contract.
pub fn from_python(value: &Bound<'_, PyAny>) -> PyResult<PythonType> {
  value.extract()
}

/// Writes a value of the Bridge contract back into a Python object.
pub fn to_python<'py>(
  py: Python<'py>, value: &PythonType,
) -> PyResult<Bound<'py, PyAny>> {
  value.clone().into_pyobject(py)
}

/// Writes the arguments of a Call into the tuple a Python callable expects.
pub fn arguments_to_python<'py>(
  py: Python<'py>, arguments: &[PythonType],
) -> PyResult<Bound<'py, PyTuple>> {
  let py_arguments = arguments
    .iter()
    .map(|argument| to_python(py, argument))
    .collect::<PyResult<Vec<_>>>()?;
  PyTuple::new(py, py_arguments)
}

/// Writes a value of the Bridge contract onto the wire. The call path reaches
/// the same serde impls through `call_result_script`, which wraps the value in
/// a CallResult; this is the value alone.
#[cfg_attr(not(test), allow(dead_code))]
pub fn to_json(value: &PythonType) -> Result<String, JsonError> {
  to_string(value)
}

/// Reads a value of the Bridge contract off the wire. The call path reaches
/// the same serde impls through `parse_call`, which reads a whole Call; this
/// is the value alone.
#[cfg_attr(not(test), allow(dead_code))]
pub fn from_json(json: &str) -> Result<PythonType, JsonError> {
  from_str(json)
}
