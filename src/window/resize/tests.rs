//! Tests for the pointer reports a frontend drag sends.
//!
//! `apply` needs a real window and is verified by driving one; what is worth
//! testing here is that a report names the edges it claims to name, and that
//! a malformed one is refused rather than moving the window somewhere absurd.

use super::*;

fn parse_request(request: &str) -> Option<Drag> {
  let mut fields = request.split([':', ',']);
  fields.next(); // Skip the "window_control" prefix
  fields.next(); // Skip the "resize_drag" action
  parse(&mut fields)
}

fn edges_of(request: &str) -> (bool, bool, bool, bool) {
  let drag = parse_request(request).expect("the report should be read");
  (drag.west, drag.east, drag.north, drag.south)
}

#[test]
fn a_single_edge_drags_alone() {
  assert_eq!(
    edges_of("window_control:resize_drag:move:west:10:20:3:0"),
    (true, false, false, false)
  );
  assert_eq!(
    edges_of("window_control:resize_drag:move:south:10:20:0:3"),
    (false, false, false, true)
  );
}

#[test]
fn a_corner_drags_two_edges() {
  assert_eq!(
    edges_of("window_control:resize_drag:move:north-west:10:20:3:3"),
    (true, false, true, false)
  );
  assert_eq!(
    edges_of("window_control:resize_drag:move:south-east:10:20:3:3"),
    (false, true, false, true)
  );
}

#[test]
fn a_report_carries_the_pointer_and_the_grab() {
  let drag = parse_request("window_control:resize_drag:move:east:640.5:-12:-3:0")
    .expect("the report should be read");
  assert_eq!(drag.pointer, (640.5, -12.0));
  assert_eq!(drag.grab, (-3.0, 0.0));
}

#[test]
fn the_grab_is_told_from_the_moves_that_follow_it() {
  let grab = parse_request("window_control:resize_drag:grab:west:3:20:3:0")
    .expect("the report should be read");
  assert!(grab.grabbed);
  let moved = parse_request("window_control:resize_drag:move:west:9:20:3:0")
    .expect("the report should be read");
  assert!(!moved.grabbed);
}

#[test]
fn a_report_naming_no_phase_is_refused() {
  assert!(parse_request("window_control:resize_drag:west:10:20:3:0").is_none());
}

#[test]
fn a_direction_naming_no_edge_is_refused() {
  assert!(
    parse_request("window_control:resize_drag:move:sideways:10:20:0:0").is_none()
  );
}

#[test]
fn a_report_missing_a_number_is_refused() {
  assert!(parse_request("window_control:resize_drag:move:west:10:20").is_none());
}

#[test]
fn a_report_with_an_unreadable_number_is_refused() {
  assert!(parse_request("window_control:resize_drag:move:west:ten:20:0:0").is_none());
}

#[test]
fn a_report_that_is_not_a_number_at_all_is_refused() {
  assert!(parse_request("window_control:resize_drag:move:west:NaN:20:0:0").is_none());
  assert!(parse_request("window_control:resize_drag:move:west:inf:20:0:0").is_none());
}
