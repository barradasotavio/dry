//! The Bridge contract: the values that may cross between Python and the
//! frontend, and the conversions that carry them.
//!
//! The contract is the JSON data model, with `json.dumps` / `json.loads`
//! semantics and nothing else. A value outside it raises rather than being
//! bent into the nearest shape — see ADR-0002.
//!
//! Every conversion here is a free function over owned data, so a test can
//! exercise both directions without opening a Webview or running an event
//! loop. The call path in `api.rs` holds the GIL and reaches Python; this
//! module only translates.

use pyo3::{
  Bound, IntoPyObject, Py, PyAny, PyResult, Python,
  exceptions::{PyTypeError, PyValueError},
  types::{
    PyAnyMethods, PyBool, PyByteArray, PyBytes, PyDict, PyDictMethods, PyFloat,
    PyFrozenSet, PyInt, PyList, PyListMethods, PySet, PyString, PyTuple, PyTypeMethods,
  },
};
use serde::{
  Deserialize, Deserializer, Serialize, Serializer,
  de::{MapAccess, SeqAccess, Visitor},
  ser::{SerializeMap, SerializeSeq},
};
use serde_json::{Error as JsonError, from_str, to_string};
use std::{fmt, sync::RwLock};

#[cfg(test)]
mod tests;

/// The largest integer a JSON number carries without losing digits, because
/// the frontend reads every number as a double.
const INTEGER_LIMIT: i64 = 9_007_199_254_740_992; // 2**53

/// How deep a value may nest before we call it a cycle. `json.dumps` detects
/// the cycle itself; we detect the runaway recursion it causes. The figure is
/// serde_json's own recursion limit, so the two directions refuse the same
/// depths.
const DEPTH_LIMIT: usize = 128;

/// The `default=` hook, held for the lifetime of the process because the Api
/// is registered once. `None` means a value outside the contract raises.
static DEFAULT_HOOK: RwLock<Option<Py<PyAny>>> = RwLock::new(None);

/// A value of the Bridge contract: exactly the JSON data model.
#[derive(Clone, Debug, PartialEq)]
pub enum PythonType {
  Null,
  Boolean(bool),
  Integer(i64),
  Float(f64),
  String(String),
  Array(Vec<PythonType>),
  /// Insertion-ordered, as a JSON object is written and as `dict` is ordered.
  /// Keys are already coerced to strings, exactly as `json.dumps` coerces
  /// them, so nothing downstream has to think about key types.
  Object(Vec<(String, PythonType)>),
}

impl Serialize for PythonType {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    match self {
      PythonType::Null => serializer.serialize_unit(),
      PythonType::Boolean(value) => serializer.serialize_bool(*value),
      PythonType::Integer(value) => serializer.serialize_i64(*value),
      PythonType::Float(value) => serializer.serialize_f64(*value),
      PythonType::String(value) => serializer.serialize_str(value),
      PythonType::Array(items) => {
        let mut sequence = serializer.serialize_seq(Some(items.len()))?;
        for item in items {
          sequence.serialize_element(item)?;
        }
        sequence.end()
      },
      PythonType::Object(entries) => {
        let mut map = serializer.serialize_map(Some(entries.len()))?;
        for (key, value) in entries {
          map.serialize_entry(key, value)?;
        }
        map.end()
      },
    }
  }
}

struct PythonTypeVisitor;

impl<'de> Visitor<'de> for PythonTypeVisitor {
  type Value = PythonType;

  fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str("a JSON value")
  }

  fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
    Ok(PythonType::Null)
  }

  fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
    Ok(PythonType::Null)
  }

  fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> {
    Ok(PythonType::Boolean(value))
  }

  fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
    if value.abs() > INTEGER_LIMIT {
      return Err(E::custom(format!(
        "{value} is outside the Bridge contract: a JSON number carries whole \
         numbers only up to ±2**53"
      )));
    }
    Ok(PythonType::Integer(value))
  }

  fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
    if value > INTEGER_LIMIT as u64 {
      return Err(E::custom(format!(
        "{value} is outside the Bridge contract: a JSON number carries whole \
         numbers only up to ±2**53"
      )));
    }
    Ok(PythonType::Integer(value as i64))
  }

  fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
    Ok(PythonType::Float(value))
  }

  fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
    Ok(PythonType::String(value.to_string()))
  }

  fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
    Ok(PythonType::String(value))
  }

  fn visit_seq<A: SeqAccess<'de>>(
    self, mut access: A,
  ) -> Result<Self::Value, A::Error> {
    let mut items = Vec::new();
    while let Some(item) = access.next_element()? {
      items.push(item);
    }
    Ok(PythonType::Array(items))
  }

  fn visit_map<A: MapAccess<'de>>(
    self, mut access: A,
  ) -> Result<Self::Value, A::Error> {
    let mut entries = Vec::new();
    while let Some((key, value)) = access.next_entry::<String, PythonType>()? {
      entries.push((key, value));
    }
    Ok(PythonType::Object(entries))
  }
}

impl<'de> Deserialize<'de> for PythonType {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    deserializer.deserialize_any(PythonTypeVisitor)
  }
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
      result: PythonType::Null,
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
  Ok(format!("window.dry.resolveCall({})", to_string(result)?))
}

/// Installs the `default=` hook, the one escape hatch the contract offers.
/// It has the signature `json.dumps` gives it: called with the value that is
/// outside the contract, it returns one that is inside, or raises.
///
/// Passing `None` takes the hook away, and a value outside the contract
/// raises again.
#[cfg_attr(not(test), allow(dead_code))]
pub fn set_default_hook(hook: Option<Py<PyAny>>) {
  if let Ok(mut slot) = DEFAULT_HOOK.write() {
    *slot = hook;
  }
}

/// Reads a Python value into the Bridge contract, through the installed
/// `default=` hook.
pub fn from_python(value: &Bound<'_, PyAny>) -> PyResult<PythonType> {
  let hook = DEFAULT_HOOK
    .read()
    .ok()
    .and_then(|slot| slot.as_ref().map(|hook| hook.clone_ref(value.py())));
  match &hook {
    Some(hook) => from_python_with(value, Some(hook.bind(value.py()))),
    None => from_python_with(value, None),
  }
}

/// Reads a Python value into the Bridge contract, with the `default=` hook
/// given explicitly rather than taken from the installed one.
pub fn from_python_with<'py>(
  value: &Bound<'py, PyAny>, default: Option<&Bound<'py, PyAny>>,
) -> PyResult<PythonType> {
  read(value, default, 0)
}

fn read<'py>(
  value: &Bound<'py, PyAny>, default: Option<&Bound<'py, PyAny>>, depth: usize,
) -> PyResult<PythonType> {
  if depth > DEPTH_LIMIT {
    return Err(PyValueError::new_err(format!(
      "Circular reference detected, or a value nested deeper than \
       {DEPTH_LIMIT} levels."
    )));
  }

  if value.is_none() {
    return Ok(PythonType::Null);
  }

  // Before the integer arm, always: in CPython `bool` is a subclass of `int`,
  // and reading a bool as an integer is what sent `True` across as `1`.
  if value.is_instance_of::<PyBool>() {
    return Ok(PythonType::Boolean(value.extract()?));
  }

  if value.is_instance_of::<PyInt>() {
    return read_integer(value);
  }

  if value.is_instance_of::<PyFloat>() {
    let number: f64 = value.extract()?;
    if !number.is_finite() {
      return Err(PyValueError::new_err(format!(
        "{} is outside the Bridge contract: JSON has no NaN or Infinity.",
        value.repr()?
      )));
    }
    return Ok(PythonType::Float(number));
  }

  if value.is_instance_of::<PyString>() {
    return Ok(PythonType::String(value.extract()?));
  }

  // `json.dumps` writes a tuple as an array, so the Bridge does too. The far
  // side sees a JavaScript array, and a round trip returns a `list`.
  if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
    let mut items = Vec::new();
    for item in value.try_iter()? {
      items.push(read(&item?, default, depth + 1)?);
    }
    return Ok(PythonType::Array(items));
  }

  if value.is_instance_of::<PyDict>() {
    let dictionary = value.cast::<PyDict>()?;
    let mut entries = Vec::with_capacity(dictionary.len());
    for (key, item) in dictionary.iter() {
      entries.push((read_key(&key)?, read(&item, default, depth + 1)?));
    }
    return Ok(PythonType::Object(entries));
  }

  if value.is_instance_of::<PySet>() || value.is_instance_of::<PyFrozenSet>() {
    return Err(PyTypeError::new_err(format!(
      "{} is outside the Bridge contract: JSON has no set, and a set does not \
       survive the round trip. Pass a list instead.",
      value.get_type().name()?
    )));
  }

  if value.is_instance_of::<PyBytes>() || value.is_instance_of::<PyByteArray>() {
    return Err(PyTypeError::new_err(format!(
      "{} is outside the Bridge contract: JSON has no bytes. Decode it to a \
       str, or encode it with base64 and pass the str.",
      value.get_type().name()?
    )));
  }

  // The one escape hatch, with the signature `json.dumps` gives it.
  if let Some(default) = default {
    let replacement = default.call1((value,))?;
    return read(&replacement, Some(default), depth + 1);
  }

  Err(PyTypeError::new_err(format!(
    "Object of type {} is outside the Bridge contract, and no default= hook \
     was given to convert it.",
    value.get_type().name()?
  )))
}

fn read_integer(value: &Bound<'_, PyAny>) -> PyResult<PythonType> {
  if let Ok(number) = value.extract::<i64>()
    && number.abs() <= INTEGER_LIMIT
  {
    return Ok(PythonType::Integer(number));
  }
  Err(PyValueError::new_err(format!(
    "{} is outside the Bridge contract: a JSON number carries whole numbers \
     only up to ±2**53, and the frontend would read this one with digits \
     missing.",
    value.repr()?
  )))
}

/// Coerces a dictionary key to a string exactly as `json.dumps` coerces it,
/// so a round trip returns string keys. The `default=` hook does not reach
/// here, again as in `json.dumps`.
fn read_key(key: &Bound<'_, PyAny>) -> PyResult<String> {
  if key.is_instance_of::<PyString>() {
    return key.extract();
  }

  if key.is_instance_of::<PyBool>() {
    return Ok(match key.extract::<bool>()? {
      true => "true".to_string(),
      false => "false".to_string(),
    });
  }

  if key.is_instance_of::<PyInt>() {
    return Ok(key.str()?.to_string());
  }

  if key.is_instance_of::<PyFloat>() {
    let number: f64 = key.extract()?;
    if !number.is_finite() {
      return Err(PyValueError::new_err(format!(
        "The dictionary key {} is outside the Bridge contract: JSON has no \
         NaN or Infinity.",
        key.repr()?
      )));
    }
    return Ok(key.repr()?.to_string());
  }

  if key.is_none() {
    return Ok("null".to_string());
  }

  Err(PyTypeError::new_err(format!(
    "A dictionary key of type {} is outside the Bridge contract: keys must be \
     str, int, float, bool or None.",
    key.get_type().name()?
  )))
}

/// Writes a value of the Bridge contract back into a Python object.
pub fn to_python<'py>(
  py: Python<'py>, value: &PythonType,
) -> PyResult<Bound<'py, PyAny>> {
  Ok(match value {
    PythonType::Null => py.None().into_bound(py),
    PythonType::Boolean(item) => PyBool::new(py, *item).to_owned().into_any(),
    PythonType::Integer(item) => item.into_pyobject(py)?.into_any(),
    PythonType::Float(item) => PyFloat::new(py, *item).into_any(),
    PythonType::String(item) => PyString::new(py, item).into_any(),
    PythonType::Array(items) => {
      let list = PyList::empty(py);
      for item in items {
        list.append(to_python(py, item)?)?;
      }
      list.into_any()
    },
    PythonType::Object(entries) => {
      let dictionary = PyDict::new(py);
      for (key, item) in entries {
        dictionary.set_item(key, to_python(py, item)?)?;
      }
      dictionary.into_any()
    },
  })
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
