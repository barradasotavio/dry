//! Tests for the window state diff, run without a window.
//!
//! `advance` is the whole of the decision — the event loop only reads the
//! window and posts what comes back out of it — so a reading can be written
//! down here and the Events it produces checked exactly. What cannot be
//! checked here is the reading itself: `read` asks a real window, and whether
//! a platform answers honestly while the window is minimized is a question for
//! `tests/test_window_events_reach_both_sides.py`, which opens one.

use tao::dpi::LogicalSize;

use super::{
  BLURRED, FOCUSED, HIDDEN, MAXIMIZED, MINIMIZED, MOVED, RESIZED, RESTORED, SHOWN,
  UNMAXIMIZED, WindowState, advance, content_size,
};
use crate::{events::RESERVED_PREFIX, types::PythonType};

/// An ordinary window: on screen, focused, not maximized, 800x600 at 100,100.
const OPEN: WindowState = WindowState {
  maximized: false,
  minimized: false,
  fullscreen: false,
  visible: true,
  focused: true,
  size: (800, 600),
  position: (100, 100),
};

/// The names of the Events one reading produced, in the order they were
/// produced.
fn names(before: WindowState, reading: WindowState) -> Vec<&'static str> {
  advance(before, reading)
    .1
    .into_iter()
    .map(|it| it.0)
    .collect()
}

/// The value one Event carried, by name.
fn value(before: WindowState, reading: WindowState, name: &str) -> Option<PythonType> {
  advance(before, reading)
    .1
    .into_iter()
    .find(|it| it.0 == name)
    .map(|it| it.1)
}

fn object(pairs: &[(&str, i64)]) -> PythonType {
  PythonType::Object(
    pairs
      .iter()
      .map(|(key, value)| (key.to_string(), PythonType::Integer(*value)))
      .collect(),
  )
}

#[test]
fn the_content_size_is_the_window_less_its_frame() {
  // A 520x380 window with a 28-pixel titlebar measures 520x408 on the outside
  // and renders into 520x380 — which is the number `size=` was given, and the
  // number the page reads back as window.innerHeight.
  assert_eq!(
    content_size(LogicalSize::new(520.0, 408.0), (0.0, 28.0)),
    (520, 380)
  );
  // Maximized, the same window still owes the same titlebar.
  assert_eq!(
    content_size(LogicalSize::new(1512.0, 948.0), (0.0, 28.0)),
    (1512, 920)
  );
}

#[test]
fn a_content_size_is_never_negative() {
  // A window smaller than its own frame is not a window anyone can render a
  // negative number of pixels into.
  assert_eq!(
    content_size(LogicalSize::new(10.0, 4.0), (0.0, 28.0)),
    (10, 0)
  );
}

#[test]
fn a_window_that_did_not_change_says_nothing() {
  assert_eq!(names(OPEN, OPEN), Vec::<&str>::new());
}

#[test]
fn every_name_is_reserved() {
  // A frontend listener trusts these names because nothing else may emit
  // under them, which only holds while every one of them carries the prefix
  // both public doors refuse.
  for name in [
    MAXIMIZED,
    UNMAXIMIZED,
    MINIMIZED,
    RESTORED,
    HIDDEN,
    SHOWN,
    FOCUSED,
    BLURRED,
    RESIZED,
    MOVED,
    super::CLOSE_REQUESTED,
  ] {
    assert!(
      name.starts_with(RESERVED_PREFIX),
      "'{name}' is not a reserved name, so an application could forge it."
    );
  }
}

#[test]
fn a_maximize_says_so_before_it_says_where() {
  // The coarse change first, then the geometry that explains it: a titlebar
  // that only listens for the first does not have to know the second exists.
  let maximized = WindowState {
    maximized: true,
    size: (1440, 850),
    position: (0, 0),
    ..OPEN
  };
  assert_eq!(names(OPEN, maximized), vec![MAXIMIZED, MOVED, RESIZED]);
}

#[test]
fn leaving_the_maximized_state_says_so() {
  let maximized = WindowState {
    maximized: true,
    ..OPEN
  };
  assert_eq!(names(maximized, OPEN), vec![UNMAXIMIZED]);
}

#[test]
fn each_pair_reports_both_ways() {
  let hidden = WindowState {
    visible: false,
    ..OPEN
  };
  assert_eq!(names(OPEN, hidden), vec![HIDDEN]);
  assert_eq!(names(hidden, OPEN), vec![SHOWN]);

  let minimized = WindowState {
    minimized: true,
    ..OPEN
  };
  assert_eq!(names(OPEN, minimized), vec![MINIMIZED]);
  assert_eq!(names(minimized, OPEN), vec![RESTORED]);

  let blurred = WindowState {
    focused: false,
    ..OPEN
  };
  assert_eq!(names(OPEN, blurred), vec![BLURRED]);
  assert_eq!(names(blurred, OPEN), vec![FOCUSED]);
}

#[test]
fn a_resize_carries_the_new_size_in_logical_pixels() {
  let resized = WindowState {
    size: (1024, 768),
    ..OPEN
  };
  assert_eq!(
    value(OPEN, resized, RESIZED),
    Some(object(&[("width", 1024), ("height", 768)]))
  );
}

#[test]
fn a_move_carries_the_new_position() {
  let moved = WindowState {
    position: (-40, 12),
    ..OPEN
  };
  assert_eq!(
    value(OPEN, moved, MOVED),
    Some(object(&[("x", -40), ("y", 12)]))
  );
}

#[test]
fn a_window_off_the_screen_reports_no_geometry() {
  // Windows parks a minimized window at -32000 and forgets it was maximized.
  // Neither is news about the window the user will see again.
  let parked = WindowState {
    minimized: true,
    maximized: false,
    size: (160, 28),
    position: (-32000, -32000),
    ..OPEN
  };
  assert_eq!(names(OPEN, parked), vec![MINIMIZED]);
}

#[test]
fn a_window_off_the_screen_keeps_the_geometry_it_had() {
  // And so a restore is not followed by a correction: the state carried
  // forward is the one the window comes back to, so the diff on the way back
  // is empty.
  let maximized = WindowState {
    maximized: true,
    size: (1440, 850),
    position: (0, 0),
    ..OPEN
  };
  let parked = WindowState {
    minimized: true,
    maximized: false,
    size: (160, 28),
    position: (-32000, -32000),
    ..OPEN
  };
  let (remembered, _) = advance(maximized, parked);
  assert_eq!(remembered.size, maximized.size);
  assert_eq!(remembered.position, maximized.position);
  assert!(remembered.maximized);

  let (_, back) = advance(remembered, maximized);
  assert_eq!(
    back.into_iter().map(|it| it.0).collect::<Vec<_>>(),
    vec![RESTORED]
  );
}

#[test]
fn a_hidden_window_reports_no_geometry_either() {
  let gone = WindowState {
    visible: false,
    size: (1, 1),
    position: (0, 0),
    ..OPEN
  };
  assert_eq!(names(OPEN, gone), vec![HIDDEN]);
}

#[test]
fn several_changes_in_one_turn_arrive_in_one_fixed_order() {
  // A drag can produce a move and a resize in the same turn of the loop, and
  // a restore can produce all of it at once. The order is the same every
  // time.
  let before = WindowState {
    visible: false,
    minimized: true,
    maximized: false,
    fullscreen: false,
    focused: false,
    size: (800, 600),
    position: (100, 100),
  };
  let after = WindowState {
    visible: true,
    minimized: false,
    maximized: true,
    fullscreen: false,
    focused: true,
    size: (1440, 850),
    position: (0, 0),
  };
  assert_eq!(
    names(before, after),
    vec![SHOWN, RESTORED, MAXIMIZED, FOCUSED, MOVED, RESIZED]
  );
}

#[test]
fn the_state_that_is_remembered_is_the_one_that_was_reported() {
  // Whatever `advance` says, the next diff is taken against the same values,
  // so nothing can be reported twice or dropped between two turns.
  let moved = WindowState {
    position: (7, 9),
    ..OPEN
  };
  let (remembered, _) = advance(OPEN, moved);
  assert_eq!(remembered, moved);
  assert_eq!(names(remembered, moved), Vec::<&str>::new());
}

#[test]
fn the_state_query_answers_with_the_whole_reading() {
  // The shape both sides read: the flags as booleans, and the two values that
  // also travel as Events keeping the shape they have there, so a frontend can
  // hand a `window:resized` value and a queried size to the same code.
  let reading = WindowState {
    maximized: true,
    fullscreen: true,
    ..OPEN
  };
  assert_eq!(
    super::value(&reading),
    PythonType::Object(vec![
      ("maximized".to_string(), PythonType::Boolean(true)),
      ("minimized".to_string(), PythonType::Boolean(false)),
      ("fullscreen".to_string(), PythonType::Boolean(true)),
      ("visible".to_string(), PythonType::Boolean(true)),
      ("focused".to_string(), PythonType::Boolean(true)),
      (
        "size".to_string(),
        object(&[("width", 800), ("height", 600)])
      ),
      ("position".to_string(), object(&[("x", 100), ("y", 100)])),
    ])
  );
}

#[test]
fn the_state_query_reports_what_the_events_report() {
  // Whatever the query says about size and position is the same number the
  // matching Event carries, because it is the same reading — a query in
  // physical pixels and an Event in logical ones would be one library
  // answering the same question two ways.
  let moved = WindowState {
    size: (1024, 768),
    position: (-40, 12),
    ..OPEN
  };
  let PythonType::Object(fields) = super::value(&moved) else {
    panic!("the state query should answer with an object");
  };
  let field = |name: &str| {
    fields
      .iter()
      .find(|(key, _)| key == name)
      .map(|(_, value)| value.clone())
  };
  assert_eq!(field("size"), value(OPEN, moved, RESIZED));
  assert_eq!(field("position"), value(OPEN, moved, MOVED));
}

#[test]
fn a_window_nobody_has_read_yet_has_no_state_to_report() {
  // The query answers from the last reading the event loop took, and before
  // `initial` there is none. Python turns this into the error naming the
  // Webview that is not running, rather than inventing a window at 0,0.
  assert!(super::snapshot().is_none());
}
