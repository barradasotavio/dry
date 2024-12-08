use pyo3::{
    exceptions::PyTypeError,
    types::{PyAnyMethods, PyNone, PySet, PyTypeMethods},
    Borrowed, Bound, FromPyObject, IntoPyObject, PyAny, PyResult, Python,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    convert::Infallible,
    hash::{Hash, Hasher},
};

#[derive(Serialize, Deserialize, FromPyObject, IntoPyObject, Clone)]
pub struct FloatType(f64);

impl Eq for FloatType {}

impl PartialEq for FloatType {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Hash for FloatType {
    fn hash<H: Hasher>(
        &self,
        state: &mut H,
    ) {
        self.0.to_bits().hash(state)
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NoneType;

impl<'py> FromPyObject<'py> for NoneType {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
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

    fn into_pyobject(
        self,
        py: Python<'py>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(PyNone::get(py))
    }
}

#[derive(Serialize, Deserialize, IntoPyObject, Clone)]
pub struct SetType<T>(Vec<T>);

impl<'py, T: FromPyObject<'py>> FromPyObject<'py> for SetType<T> {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        if ob.get_type().name()? == "set" {
            let set = ob.downcast::<PySet>()?;
            let mut items = Vec::new();
            for item in set.try_iter()? {
                items.push(item?.extract()?);
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
    Serialize,
    Deserialize,
    FromPyObject,
    IntoPyObject,
    Eq,
    PartialEq,
    Hash,
    Clone,
)]
#[serde(untagged)]
pub enum Primitive {
    Integer(i64),
    Float(FloatType),
    Boolean(bool),
    String(String),
}

#[derive(Serialize, Deserialize, FromPyObject, IntoPyObject, Clone)]
#[serde(untagged)]
pub enum NonPrimitive<T> {
    Sequence(Vec<T>),
    Mapping(HashMap<Primitive, T>),
    Set(SetType<T>),
}

#[derive(Serialize, Deserialize, FromPyObject, IntoPyObject, Clone)]
#[serde(untagged)]
pub enum PythonType {
    None(NoneType),
    Primitive(Primitive),
    NonPrimitive(NonPrimitive<PythonType>),
}
