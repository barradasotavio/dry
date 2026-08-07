//! Characterisation tests for the Bridge contract.
//!
//! These record what the conversion does *today*, bugs included. Where the
//! behaviour is known to be wrong, the test says so and names the issue that
//! will change it, so that issue has a failing target to flip.

use super::*;
use pyo3::types::PyAnyMethods;
use std::ffi::CString;

fn eval<'py>(py: Python<'py>, expression: &str) -> Bound<'py, PyAny> {
  py.eval(CString::new(expression).unwrap().as_c_str(), None, None)
    .expect("the test expression should evaluate")
}

/// A Python expression, as the frontend sees it after crossing the Bridge.
fn crosses_to_javascript(expression: &str) -> String {
  Python::attach(|py| {
    let value = from_python(&eval(py, expression))
      .expect("the value should be read out of Python");
    to_json(&value).expect("the value should reach the wire")
  })
}

/// Why a Python expression is refused as it is read out of Python.
fn refused_by_python(expression: &str) -> String {
  Python::attach(|py| {
    from_python(&eval(py, expression))
      .expect_err("the value should be refused")
      .to_string()
  })
}

/// A wire value, as Python sees it after crossing the Bridge.
fn crosses_to_python(json: &str) -> String {
  Python::attach(|py| {
    let value = from_json(json).expect("the value should be read off the wire");
    to_python(py, &value)
      .expect("the value should reach Python")
      .repr()
      .expect("the value should have a repr")
      .to_string()
  })
}

mod python_to_javascript {
  use super::*;

  #[test]
  fn none_crosses_as_null() {
    assert_eq!(crosses_to_javascript("None"), "null");
  }

  #[test]
  fn integers_cross_as_numbers() {
    assert_eq!(crosses_to_javascript("0"), "0");
    assert_eq!(crosses_to_javascript("42"), "42");
    assert_eq!(crosses_to_javascript("-7"), "-7");
  }

  #[test]
  fn floats_cross_as_numbers() {
    assert_eq!(crosses_to_javascript("1.5"), "1.5");
    assert_eq!(crosses_to_javascript("-0.25"), "-0.25");
  }

  #[test]
  fn a_whole_float_loses_its_decimal_point() {
    // JSON has one number type, so the frontend cannot tell 1.0 from 1.
    assert_eq!(crosses_to_javascript("1.0"), "1.0");
  }

  #[test]
  fn strings_cross_as_strings() {
    assert_eq!(crosses_to_javascript("''"), r#""""#);
    assert_eq!(crosses_to_javascript("'hello'"), r#""hello""#);
    assert_eq!(crosses_to_javascript(r#"'say "hi"'"#), r#""say \"hi\"""#);
    assert_eq!(crosses_to_javascript("'ação'"), "\"ação\"");
  }

  #[test]
  fn a_boolean_crosses_as_a_number() {
    // WRONG. `Primitive` orders `Integer(i64)` before `Boolean(bool)`, and a
    // Python bool is an int, so it is read as one. Issue #12 makes these
    // `true` and `false`.
    assert_eq!(crosses_to_javascript("True"), "1");
    assert_eq!(crosses_to_javascript("False"), "0");
  }

  #[test]
  fn a_list_crosses_as_an_array() {
    assert_eq!(crosses_to_javascript("[]"), "[]");
    assert_eq!(
      crosses_to_javascript("[1, 'two', None]"),
      r#"[1,"two",null]"#
    );
    assert_eq!(crosses_to_javascript("[[1], [2, [3]]]"), "[[1],[2,[3]]]");
  }

  #[test]
  fn a_tuple_crosses_as_an_array() {
    // Nothing distinguishes a tuple on the far side; it is read as a sequence.
    assert_eq!(crosses_to_javascript("(1, 2)"), "[1,2]");
  }

  #[test]
  fn a_set_crosses_as_an_array() {
    // Issue #12 takes `set` out of the contract: it round-trips to a list.
    assert_eq!(crosses_to_javascript("{'only'}"), r#"["only"]"#);
  }

  #[test]
  fn a_dictionary_crosses_as_an_object() {
    assert_eq!(crosses_to_javascript("{}"), "{}");
    assert_eq!(crosses_to_javascript("{'a': 1}"), r#"{"a":1}"#);
    assert_eq!(
      crosses_to_javascript("{'a': {'b': [True]}}"),
      r#"{"a":{"b":[1]}}"#
    );
  }

  #[test]
  fn an_integer_dictionary_key_crosses_as_a_string() {
    // JSON object keys are strings, so the round trip cannot return the int.
    assert_eq!(crosses_to_javascript("{1: 'one'}"), r#"{"1":"one"}"#);
  }

  #[test]
  fn a_boolean_dictionary_key_crosses_as_a_number_string() {
    // The same read-a-bool-as-an-int bug as above, seen through a key.
    // Issue #12 makes this "true".
    assert_eq!(crosses_to_javascript("{True: 'yes'}"), r#"{"1":"yes"}"#);
  }

  #[test]
  fn a_float_dictionary_key_crosses_as_a_string() {
    assert_eq!(
      crosses_to_javascript("{1.5: 'one and a half'}"),
      r#"{"1.5":"one and a half"}"#
    );
  }

  #[test]
  fn bytes_cross_as_an_array_of_numbers() {
    // Issue #12 takes `bytes` out of the contract: it has no JSON analogue.
    assert_eq!(crosses_to_javascript("b'AB'"), "[65,66]");
  }

  #[test]
  fn an_integer_past_the_javascript_limit_crosses_and_loses_precision() {
    // i64 carries it, but the frontend reads it as a double. Issue #12 raises
    // instead of losing digits.
    assert_eq!(crosses_to_javascript("2**53 + 1"), "9007199254740993");
  }

  #[test]
  fn an_integer_past_i64_crosses_as_a_float() {
    // WRONG. `Integer(i64)` cannot hold it, so `Float` catches it and the
    // digits are gone. Issue #12 raises instead, past ±2^53.
    assert_eq!(crosses_to_javascript("2**64"), "1.8446744073709552e19");
  }

  #[test]
  fn a_value_outside_the_contract_is_refused() {
    let error = refused_by_python("object()");
    assert!(
      error.starts_with("TypeError:"),
      "unexpected error: {}",
      error
    );
  }
}

mod javascript_to_python {
  use super::*;

  #[test]
  fn null_crosses_as_none() {
    assert_eq!(crosses_to_python("null"), "None");
  }

  #[test]
  fn a_boolean_crosses_as_a_boolean() {
    // The Bridge is asymmetric today: a bool survives this direction, because
    // JSON `true` cannot be read as an integer. Issue #12 closes the gap.
    assert_eq!(crosses_to_python("true"), "True");
    assert_eq!(crosses_to_python("false"), "False");
  }

  #[test]
  fn a_whole_number_crosses_as_an_integer() {
    assert_eq!(crosses_to_python("0"), "0");
    assert_eq!(crosses_to_python("-7"), "-7");
  }

  #[test]
  fn a_fractional_number_crosses_as_a_float() {
    assert_eq!(crosses_to_python("1.5"), "1.5");
  }

  #[test]
  fn a_whole_number_written_with_a_decimal_point_crosses_as_a_float() {
    assert_eq!(crosses_to_python("1.0"), "1.0");
  }

  #[test]
  fn a_string_crosses_as_a_string() {
    assert_eq!(crosses_to_python(r#""hello""#), "'hello'");
  }

  #[test]
  fn an_array_crosses_as_a_list() {
    assert_eq!(crosses_to_python("[]"), "[]");
    assert_eq!(crosses_to_python(r#"[1, "two", null]"#), "[1, 'two', None]");
    assert_eq!(crosses_to_python("[[1], [2, [3]]]"), "[[1], [2, [3]]]");
  }

  #[test]
  fn an_object_crosses_as_a_dictionary() {
    assert_eq!(crosses_to_python("{}"), "{}");
    assert_eq!(crosses_to_python(r#"{"a": 1}"#), "{'a': 1}");
    assert_eq!(
      crosses_to_python(r#"{"a": {"b": [true]}}"#),
      "{'a': {'b': [True]}}"
    );
  }

  #[test]
  fn a_numeric_object_key_stays_a_string() {
    assert_eq!(crosses_to_python(r#"{"1": "one"}"#), "{'1': 'one'}");
  }

  #[test]
  fn the_arguments_of_a_call_cross_as_a_tuple() {
    Python::attach(|py| {
      let call = parse_call(
        r#"{"call_id":"abc","function":"greet","arguments":[1,"two",null]}"#,
      )
      .expect("the Call should parse");
      let arguments = arguments_to_python(py, &call.arguments)
        .expect("the arguments should reach Python");
      assert_eq!(
        arguments
          .repr()
          .expect("the tuple should have a repr")
          .to_string(),
        "(1, 'two', None)"
      );
    });
  }
}

mod call_messages {
  use super::*;

  #[test]
  fn a_call_is_read_off_the_wire() {
    let call =
      parse_call(r#"{"call_id":"abc","function":"greet","arguments":["world"]}"#)
        .expect("the Call should parse");
    assert_eq!(call.call_id, "abc");
    assert_eq!(call.function, "greet");
    assert_eq!(call.arguments.len(), 1);
    assert_eq!(
      to_json(&call.arguments[0]).expect("the argument should reach the wire"),
      r#""world""#
    );
  }

  #[test]
  fn a_call_without_arguments_is_read_off_the_wire() {
    let call = parse_call(r#"{"call_id":"abc","function":"now","arguments":[]}"#)
      .expect("the Call should parse");
    assert!(call.arguments.is_empty());
  }

  #[test]
  fn a_malformed_call_is_refused() {
    assert!(parse_call("not json").is_err());
    assert!(parse_call(r#"{"function":"greet","arguments":[]}"#).is_err());
  }

  #[test]
  fn a_result_is_written_as_a_callback() {
    let result = CallResult {
      call_id: "abc".to_string(),
      result: PythonType::Primitive(Primitive::String("hi".to_string())),
      error: None,
    };
    assert_eq!(
      call_result_script(&result).expect("the result should reach the wire"),
      r#"window.ipcCallback({"call_id":"abc","result":"hi","error":null})"#
    );
  }

  #[test]
  fn a_failed_call_carries_null_and_a_reason() {
    let result =
      CallResult::failed("abc".to_string(), "Function x not found.".to_string());
    assert_eq!(
      call_result_script(&result).expect("the result should reach the wire"),
      r#"window.ipcCallback({"call_id":"abc","result":null,"error":"Function x not found."})"#
    );
  }
}
