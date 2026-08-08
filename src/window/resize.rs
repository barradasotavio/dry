//! Drag-resize driven from the frontend, for platforms with no native one.
//!
//! `tao` 0.36 implements `Window::drag_resize_window` on its macOS backend as
//! an unconditional `Err(NotSupported)`, so the eight resize edges of an
//! undecorated window did nothing there. Rather than draw handles that cannot
//! work, `dry.resize()` runs the drag itself on macOS and reports the pointer
//! on every move; this module turns those reports into geometry. See ADR-0004.
//!
//! Every report is absolute, never incremental: the geometry it asks for is
//! derived from the pointer and the window's current frame, so a report that
//! is dropped, coalesced or arrives late costs a frame rather than leaving the
//! window permanently out of step with the cursor.

use std::sync::Mutex;
use tao::{
  dpi::{LogicalPosition, LogicalSize},
  window::Window,
};

use crate::logs;

/// The minimum inner size the window currently has, in logical pixels.
///
/// The platform clamps a resize the user drives, but not a `set_inner_size`
/// this module drives, so the clamp has to happen here. Written again whenever
/// `min_size` is assigned on a running Webview, so a drag cannot go on
/// enforcing a minimum the window no longer has.
static MIN_SIZE: Mutex<Option<(f64, f64)>> = Mutex::new(None);

/// What the drag in progress has to remember between reports.
static ANCHOR: Mutex<Option<Anchor>> = Mutex::new(None);

/// What a drag holds still, taken when the edge was grabbed.
///
/// A drag on the west or north edge moves the window as well as resizing it,
/// and the platform is free to refuse the move: macOS will not lift a window's
/// top above the menu bar. Pinning the edges the drag leaves alone means a
/// refused move costs the window nothing, instead of letting it grow away from
/// the cursor by the amount that was refused.
#[derive(Debug)]
struct Anchor {
  /// The edges not under drag, in logical pixels.
  right: f64,
  bottom: f64,
  /// The last top-left corner asked for, so the next report can tell a move
  /// the platform refused from one it granted.
  requested: (f64, f64),
  /// How far up the platform has proved it will let this window go.
  ceiling: f64,
}

pub fn remember_min_size(min_size: (u32, u32)) {
  if let Ok(mut remembered) = MIN_SIZE.lock() {
    *remembered = Some((f64::from(min_size.0), f64::from(min_size.1)));
  }
}

/// One pointer report from a drag on a resize edge.
#[derive(Debug)]
pub struct Drag {
  /// Whether this report is the grab that starts the drag.
  grabbed: bool,
  west: bool,
  east: bool,
  north: bool,
  south: bool,
  /// The pointer, in the window's own client coordinates, logical pixels.
  pointer: (f64, f64),
  /// Where the drag was grabbed, as an offset from the edge it drags, so the
  /// window does not jump by the thickness of the handle on the first move.
  grab: (f64, f64),
}

/// Reads `<phase>:<direction>:<pointer x>:<pointer y>:<grab x>:<grab y>` off
/// the rest of a `window_control:resize_drag:...` request.
pub fn parse<'a>(fields: &mut impl Iterator<Item = &'a str>) -> Option<Drag> {
  let phase = fields.next()?;
  let direction = fields.next()?;
  let mut number = || fields.next()?.parse::<f64>().ok();
  let pointer = (number()?, number()?);
  let grab = (number()?, number()?);

  let grabbed = match phase {
    "grab" => true,
    "move" => false,
    _ => return None,
  };

  let drag = Drag {
    grabbed,
    west: direction.ends_with("west"),
    east: direction.ends_with("east"),
    north: direction.starts_with("north"),
    south: direction.starts_with("south"),
    pointer,
    grab,
  };

  let names_an_edge = drag.west || drag.east || drag.north || drag.south;
  let is_finite = pointer.0.is_finite()
    && pointer.1.is_finite()
    && grab.0.is_finite()
    && grab.1.is_finite();

  (names_an_edge && is_finite).then_some(drag)
}

/// Moves the edges under drag to the pointer, leaving the others where they
/// are. A window that cannot report its position keeps the size it has: a
/// resize is not worth a dead Webview.
pub fn apply(drag: &Drag, window: &Window) {
  let scale_factor = window.scale_factor();
  let position = match window.outer_position() {
    Ok(position) => position.to_logical::<f64>(scale_factor),
    Err(err) => {
      logs::error(
        logs::WEBVIEW,
        format!("The window could not be resized: {err}"),
      );
      return;
    },
  };
  // `inner_size` measures tao's content view, which the Webview replaces as
  // the window's content view, so it reports the size the window was built
  // with forever after. `outer_size` measures the window frame, which is the
  // same thing here: the edges are only drawn on an undecorated window.
  let size = window.outer_size().to_logical::<f64>(scale_factor);

  let mut held = ANCHOR.lock().expect("the resize anchor should be readable");
  if drag.grabbed || held.is_none() {
    *held = Some(Anchor {
      right: position.x + size.width,
      bottom: position.y + size.height,
      requested: (position.x, position.y),
      ceiling: f64::NEG_INFINITY,
    });
  }
  let anchor = held.as_mut().expect("the drag was just anchored");
  let (right, bottom) = (anchor.right, anchor.bottom);

  // The window sitting lower than it was last asked to is the platform saying
  // no. Taking it as the ceiling stops the next report asking again, which is
  // what would walk the pinned bottom edge down the screen.
  if position.y > anchor.requested.1 + 0.5 {
    anchor.ceiling = position.y;
  }

  let (mut x, mut y) = (position.x, position.y);
  let (mut width, mut height) = (size.width, size.height);

  // The pointer is measured against the frame the frontend last saw, and the
  // edges not under drag are pinned, so each edge lands where the cursor is
  // however late, coalesced or refused the reports around it were.
  if drag.west {
    x += drag.pointer.0 - drag.grab.0;
    width = right - x;
  }
  if drag.east {
    width = drag.pointer.0 - drag.grab.0;
  }
  if drag.north {
    y = (y + drag.pointer.1 - drag.grab.1).max(anchor.ceiling);
    height = bottom - y;
  }
  if drag.south {
    height = drag.pointer.1 - drag.grab.1;
  }

  let (min_width, min_height) = MIN_SIZE
    .lock()
    .ok()
    .and_then(|remembered| *remembered)
    .unwrap_or((1.0, 1.0));
  if width < min_width {
    // The pinned edge stays put, so the one under drag stops at the minimum.
    if drag.west {
      x = right - min_width;
    }
    width = min_width;
  }
  if height < min_height {
    if drag.north {
      y = bottom - min_height;
    }
    height = min_height;
  }

  anchor.requested = (x, y);
  drop(held);

  // Position first: setting the size keeps the top-left corner, so moving the
  // corner afterwards would undo it.
  if x != position.x || y != position.y {
    window.set_outer_position(LogicalPosition::new(x, y));
  }
  if width != size.width || height != size.height {
    window.set_inner_size(LogicalSize::new(width, height));
  }
}

#[cfg(test)]
mod tests;
