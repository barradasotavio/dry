use pyo3::{
  PyResult, Python,
  exceptions::{PyPermissionError, PyValueError},
};

use super::{
  BridgeError, DryError, PanicError, WebviewError, catch_panic, panic_description,
  panic_payload_message, py_error_message,
};

#[test]
fn reads_a_panic_with_a_literal_message() {
  let payload = catch_panic(|| -> PyResult<()> { panic!("No Content provided.") });
  let message = payload.unwrap_err();
  Python::attach(|py| {
    assert!(message.is_instance_of::<PanicError>(py));
    assert!(message.to_string().contains("No Content provided."));
    assert!(message.to_string().contains(file!()));
  });
}

#[test]
fn reads_a_panic_with_a_formatted_message() {
  let payload: Box<dyn std::any::Any + Send> = Box::new("literal".to_string());
  assert_eq!(panic_payload_message(&*payload), "literal");
}

#[test]
fn reads_a_panic_with_an_unknown_payload() {
  let payload: Box<dyn std::any::Any + Send> = Box::new(42_u8);
  assert_eq!(
    panic_payload_message(&*payload),
    "Panicked with an unknown payload."
  );
}

#[test]
fn describes_where_a_panic_happened() {
  let payload: Box<dyn std::any::Any + Send> = Box::new("boom".to_string());
  assert_eq!(
    panic_description(&*payload, Some("src/window.rs:31")),
    "Dry panicked at src/window.rs:31: boom"
  );
  assert_eq!(panic_description(&*payload, None), "Dry panicked: boom");
}

#[test]
fn passes_a_value_through_when_nothing_panics() {
  let value = catch_panic(|| Ok(7)).unwrap();
  assert_eq!(value, 7);
}

#[test]
fn leaves_an_error_alone_when_nothing_panics() {
  let error = catch_panic(|| -> PyResult<()> {
    Err(WebviewError::new_err("The Webview has no Content."))
  })
  .unwrap_err();
  Python::attach(|py| assert!(error.is_instance_of::<WebviewError>(py)));
}

#[test]
fn carries_the_exception_type_name_alongside_its_message() {
  Python::attach(|py| {
    let error = PyValueError::new_err("Not a number.");
    assert_eq!(py_error_message(py, &error), "ValueError: Not a number.");

    let error = PyPermissionError::new_err("Denied.");
    assert_eq!(py_error_message(py, &error), "PermissionError: Denied.");
  });
}

#[test]
fn carries_the_exception_type_name_alone_when_there_is_no_message() {
  Python::attach(|py| {
    let error = PyValueError::new_err("");
    assert_eq!(py_error_message(py, &error), "ValueError");
  });
}

#[test]
fn every_failure_is_a_dry_error() {
  Python::attach(|py| {
    for failure in [
      WebviewError::new_err("."),
      BridgeError::new_err("."),
      PanicError::new_err("."),
    ] {
      assert!(failure.is_instance_of::<DryError>(py));
    }
  });
}
