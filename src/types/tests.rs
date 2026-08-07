//! Tests for the Bridge contract.
//!
//! The contract is the JSON data model, with `json.dumps` / `json.loads`
//! semantics and nothing else (ADR-0002). Every test here is either a value
//! the contract carries, or a value it refuses out loud.

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
    let value = from_python_with(&eval(py, expression), None)
      .expect("the value should be read out of Python");
    to_json(&value).expect("the value should reach the wire")
  })
}

/// Why a Python expression is refused as it is read out of Python.
fn refused_by_python(expression: &str) -> String {
  Python::attach(|py| {
    from_python_with(&eval(py, expression), None)
      .expect_err("the value should be refused")
      .to_string()
  })
}

/// A Python expression carried by a `default=` hook, as the frontend sees it.
fn crosses_through_hook(expression: &str, hook: &str) -> String {
  Python::attach(|py| {
    let hook = eval(py, hook);
    let value = from_python_with(&eval(py, expression), Some(&hook))
      .expect("the value should be read out of Python");
    to_json(&value).expect("the value should reach the wire")
  })
}

/// Why a Python expression is refused even with a `default=` hook in place.
fn refused_through_hook(expression: &str, hook: &str) -> String {
  Python::attach(|py| {
    let hook = eval(py, hook);
    from_python_with(&eval(py, expression), Some(&hook))
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

/// Why a wire value is refused as it is read off the wire.
fn refused_from_javascript(json: &str) -> String {
  from_json(json)
    .expect_err("the value should be refused")
    .to_string()
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
  fn a_boolean_crosses_as_a_boolean() {
    // `bool` is resolved before `int`, so the CPython subclassing does not
    // leak across the Bridge as the numbers 1 and 0.
    assert_eq!(crosses_to_javascript("True"), "true");
    assert_eq!(crosses_to_javascript("False"), "false");
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
    // `json.dumps` writes a tuple as an array, so the contract does too.
    // Nothing distinguishes it on the far side, and a round trip returns a
    // list.
    assert_eq!(crosses_to_javascript("()"), "[]");
    assert_eq!(crosses_to_javascript("(1, 2)"), "[1,2]");
  }

  #[test]
  fn a_set_is_refused() {
    // Outside the contract: JSON has no set, and a set does not survive the
    // round trip.
    let error = refused_by_python("{'only'}");
    assert!(error.starts_with("TypeError: set"), "unexpected: {}", error);
    assert!(
      error.contains("Pass a list instead"),
      "unexpected: {}",
      error
    );
  }

  #[test]
  fn a_frozenset_is_refused() {
    let error = refused_by_python("frozenset({'only'})");
    assert!(
      error.starts_with("TypeError: frozenset"),
      "unexpected: {}",
      error
    );
  }

  #[test]
  fn a_dictionary_crosses_as_an_object() {
    assert_eq!(crosses_to_javascript("{}"), "{}");
    assert_eq!(crosses_to_javascript("{'a': 1}"), r#"{"a":1}"#);
    assert_eq!(
      crosses_to_javascript("{'a': {'b': [True]}}"),
      r#"{"a":{"b":[true]}}"#
    );
  }

  #[test]
  fn a_dictionary_keeps_its_order() {
    assert_eq!(
      crosses_to_javascript("{'b': 1, 'a': 2}"),
      r#"{"b":1,"a":2}"#
    );
  }

  #[test]
  fn an_integer_dictionary_key_crosses_as_a_string() {
    // JSON object keys are strings, so the round trip cannot return the int.
    // `json.dumps` coerces the same way.
    assert_eq!(crosses_to_javascript("{1: 'one'}"), r#"{"1":"one"}"#);
  }

  #[test]
  fn a_boolean_dictionary_key_crosses_as_true_or_false() {
    // `json.dumps({True: 'yes'})` writes "true", not "1".
    assert_eq!(crosses_to_javascript("{True: 'yes'}"), r#"{"true":"yes"}"#);
    assert_eq!(crosses_to_javascript("{False: 'no'}"), r#"{"false":"no"}"#);
  }

  #[test]
  fn a_float_dictionary_key_crosses_as_a_string() {
    assert_eq!(
      crosses_to_javascript("{1.5: 'one and a half'}"),
      r#"{"1.5":"one and a half"}"#
    );
    assert_eq!(crosses_to_javascript("{1.0: 'one'}"), r#"{"1.0":"one"}"#);
  }

  #[test]
  fn a_none_dictionary_key_crosses_as_the_string_null() {
    assert_eq!(
      crosses_to_javascript("{None: 'nothing'}"),
      r#"{"null":"nothing"}"#
    );
  }

  #[test]
  fn a_dictionary_key_outside_the_contract_is_refused() {
    let error = refused_by_python("{(1, 2): 'pair'}");
    assert!(
      error.starts_with("TypeError: A dictionary key of type tuple"),
      "unexpected: {}",
      error
    );
    assert!(
      error.contains("str, int, float, bool or None"),
      "unexpected: {}",
      error
    );
  }

  #[test]
  fn bytes_are_refused() {
    // Outside the contract: JSON has no bytes, and the README once promised
    // an array of numbers for a value the model cannot hold.
    let error = refused_by_python("b'AB'");
    assert!(
      error.starts_with("TypeError: bytes"),
      "unexpected: {}",
      error
    );
    assert!(error.contains("base64"), "unexpected: {}", error);
  }

  #[test]
  fn a_bytearray_is_refused() {
    let error = refused_by_python("bytearray(b'AB')");
    assert!(
      error.starts_with("TypeError: bytearray"),
      "unexpected: {}",
      error
    );
  }

  #[test]
  fn an_integer_at_the_javascript_limit_crosses() {
    assert_eq!(crosses_to_javascript("2**53"), "9007199254740992");
    assert_eq!(crosses_to_javascript("-2**53"), "-9007199254740992");
  }

  #[test]
  fn an_integer_past_the_javascript_limit_is_refused() {
    // i64 would carry it, but the frontend reads every number as a double,
    // so the digits would be gone on arrival.
    let error = refused_by_python("2**53 + 1");
    assert!(
      error.starts_with("ValueError: 9007199254740993 is outside the Bridge contract"),
      "unexpected: {}",
      error
    );
    assert!(error.contains("±2**53"), "unexpected: {}", error);
  }

  #[test]
  fn an_integer_past_i64_is_refused() {
    // The old model let `Float` catch this one and silently rounded it to
    // 1.8446744073709552e19.
    let error = refused_by_python("2**64");
    assert!(
      error.starts_with("ValueError: 18446744073709551616 is outside"),
      "unexpected: {}",
      error
    );
  }

  #[test]
  fn a_negative_integer_past_the_javascript_limit_is_refused() {
    let error = refused_by_python("-2**53 - 1");
    assert!(error.starts_with("ValueError:"), "unexpected: {}", error);
  }

  #[test]
  fn nan_and_infinity_are_refused() {
    // JSON has neither, and `json.dumps` only writes them by breaking the
    // format it claims to produce.
    for expression in ["float('nan')", "float('inf')", "float('-inf')"] {
      let error = refused_by_python(expression);
      assert!(
        error.starts_with("ValueError:") && error.contains("NaN or Infinity"),
        "unexpected: {}",
        error
      );
    }
  }

  #[test]
  fn an_integer_subclass_crosses_as_a_number() {
    // As in `json.dumps`: an `IntEnum` is an int, and crosses as one.
    assert_eq!(
      crosses_to_javascript("__import__('enum').IntEnum('Size', {'BIG': 2}).BIG"),
      "2"
    );
  }

  #[test]
  fn a_circular_reference_is_refused() {
    Python::attach(|py| {
      let list = PyList::empty(py);
      list.append(&list).expect("the list should hold itself");
      let error = from_python_with(list.as_any(), None)
        .expect_err("the value should be refused")
        .to_string();
      assert!(
        error.starts_with("ValueError: Circular reference detected"),
        "unexpected: {}",
        error
      );
    });
  }

  #[test]
  fn a_value_outside_the_contract_is_refused() {
    let error = refused_by_python("object()");
    assert!(
      error.starts_with("TypeError: Object of type object is outside the Bridge"),
      "unexpected error: {}",
      error
    );
    assert!(error.contains("default="), "unexpected error: {}", error);
  }

  #[test]
  fn a_datetime_is_refused_without_a_hook() {
    // ADR-0002: `datetime`, `Decimal`, `Enum` and dataclasses are the
    // developer's job, through the hook.
    let error = refused_by_python("__import__('datetime').date(2026, 8, 7)");
    assert!(
      error.starts_with("TypeError: Object of type date"),
      "unexpected: {}",
      error
    );
  }
}

mod the_default_hook {
  use super::*;

  #[test]
  fn a_datetime_crosses_through_the_hook() {
    assert_eq!(
      crosses_through_hook(
        "__import__('datetime').date(2026, 8, 7)",
        "lambda value: value.isoformat()"
      ),
      r#""2026-08-07""#
    );
  }

  #[test]
  fn a_decimal_crosses_through_the_hook() {
    assert_eq!(
      crosses_through_hook(
        "__import__('decimal').Decimal('1.5')",
        "lambda value: float(value)"
      ),
      "1.5"
    );
  }

  #[test]
  fn an_enum_crosses_through_the_hook() {
    assert_eq!(
      crosses_through_hook(
        "__import__('enum').Enum('Colour', ['RED']).RED",
        "lambda value: value.name"
      ),
      r#""RED""#
    );
  }

  #[test]
  fn a_dataclass_crosses_through_the_hook() {
    assert_eq!(
      crosses_through_hook(
        "__import__('dataclasses').make_dataclass('P', ['x'])(1)",
        "lambda value: __import__('dataclasses').asdict(value)"
      ),
      r#"{"x":1}"#
    );
  }

  #[test]
  fn the_hook_is_left_alone_for_a_value_inside_the_contract() {
    assert_eq!(
      crosses_through_hook("[1, True, 'two']", "lambda value: 'hooked'"),
      r#"[1,true,"two"]"#
    );
  }

  #[test]
  fn the_hook_reaches_a_value_nested_inside_the_contract() {
    assert_eq!(
      crosses_through_hook(
        "{'when': [__import__('datetime').date(2026, 8, 7)]}",
        "lambda value: value.isoformat()"
      ),
      r#"{"when":["2026-08-07"]}"#
    );
  }

  #[test]
  fn what_the_hook_returns_is_read_by_the_contract_too() {
    let error = refused_through_hook("object()", "lambda value: {'a', 'b'}");
    assert!(error.starts_with("TypeError: set"), "unexpected: {}", error);
  }

  #[test]
  fn an_exception_from_the_hook_reaches_the_caller() {
    let error = refused_through_hook("object()", "lambda value: 1 / 0");
    assert!(
      error.starts_with("ZeroDivisionError:"),
      "unexpected: {}",
      error
    );
  }

  #[test]
  fn a_hook_that_never_converts_is_stopped() {
    let error = refused_through_hook("object()", "lambda value: value");
    assert!(
      error.starts_with("ValueError: Circular reference detected"),
      "unexpected: {}",
      error
    );
  }

  #[test]
  fn the_hook_does_not_reach_dictionary_keys() {
    // As in `json.dumps`: `default=` converts values, never keys.
    let error = refused_through_hook("{(1, 2): 'pair'}", "lambda value: str(value)");
    assert!(
      error.starts_with("TypeError: A dictionary key of type tuple"),
      "unexpected: {}",
      error
    );
  }

  #[test]
  fn the_installed_hook_carries_the_call_path() {
    // `from_python` is what `api.rs` reaches for, and it reads the hook
    // installed for the process rather than one passed down the call.
    Python::attach(|py| {
      let value = eval(py, "__import__('decimal').Decimal('1.5')");
      assert!(from_python(&value).is_err());

      let hook = eval(py, "lambda value: str(value)");
      set_default_hook(Some(hook.unbind()));
      let read = from_python(&value).expect("the hook should convert it");
      assert_eq!(
        to_json(&read).expect("it should reach the wire"),
        r#""1.5""#
      );

      set_default_hook(None);
      assert!(from_python(&value).is_err());
    });
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
    // The literal form decides the Python type, as `json.loads` decides it.
    assert_eq!(crosses_to_python("1.0"), "1.0");
    assert_eq!(crosses_to_python("1e2"), "100.0");
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
  fn a_whole_number_at_the_javascript_limit_crosses() {
    assert_eq!(crosses_to_python("9007199254740992"), "9007199254740992");
    assert_eq!(crosses_to_python("-9007199254740992"), "-9007199254740992");
  }

  #[test]
  fn a_whole_number_past_the_javascript_limit_is_refused() {
    // The contract raises in this direction too: the frontend could not have
    // meant a number it cannot itself represent.
    let error = refused_from_javascript("9007199254740993");
    assert!(error.contains("±2**53"), "unexpected: {}", error);
    let error = refused_from_javascript("[-9007199254740993]");
    assert!(error.contains("±2**53"), "unexpected: {}", error);
  }

  #[test]
  fn nan_and_infinity_are_refused() {
    assert!(from_json("NaN").is_err());
    assert!(from_json("Infinity").is_err());
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

mod round_trips {
  use super::*;

  /// A Python expression, as Python sees it after a full round trip.
  fn round_trip(expression: &str) -> String {
    Python::attach(|py| {
      let value = from_python_with(&eval(py, expression), None)
        .expect("the value should be read out of Python");
      let json = to_json(&value).expect("the value should reach the wire");
      let back = from_json(&json).expect("the value should be read back");
      to_python(py, &back)
        .expect("the value should reach Python")
        .repr()
        .expect("the value should have a repr")
        .to_string()
    })
  }

  #[test]
  fn a_boolean_comes_back_a_boolean() {
    assert_eq!(round_trip("True"), "True");
    assert_eq!(round_trip("[False]"), "[False]");
  }

  #[test]
  fn a_tuple_comes_back_a_list() {
    assert_eq!(round_trip("(1, 2)"), "[1, 2]");
  }

  #[test]
  fn a_dictionary_comes_back_with_string_keys() {
    // Intended, and the same coercion `json.dumps` performs.
    assert_eq!(
      round_trip("{2: 'two', True: 'yes', None: 'nothing'}"),
      "{'2': 'two', 'true': 'yes', 'null': 'nothing'}"
    );
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
      result: PythonType::String("hi".to_string()),
      error: None,
    };
    assert_eq!(
      call_result_script(&result).expect("the result should reach the wire"),
      r#"window.dry.resolveCall({"call_id":"abc","result":"hi","error":null})"#
    );
  }

  #[test]
  fn a_failed_call_carries_null_and_a_reason() {
    let result =
      CallResult::failed("abc".to_string(), "Function x not found.".to_string());
    assert_eq!(
      call_result_script(&result).expect("the result should reach the wire"),
      r#"window.dry.resolveCall({"call_id":"abc","result":null,"error":"Function x not found."})"#
    );
  }
}
