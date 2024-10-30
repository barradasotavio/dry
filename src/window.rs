use std::str::Split;
use tao::{
    dpi::{LogicalSize, PhysicalSize},
    event_loop::EventLoopProxy,
    window::{ResizeDirection, Window},
};

use crate::UserEvent;

pub const WINDOW_FUNCTIONS_JS: &str = include_str!("js/window_functions.js");
pub const WINDOW_EVENTS_JS: &str = include_str!("js/window_events.js");
pub const WINDOW_BORDERS_JS: &str = include_str!("js/window_borders.js");

#[derive(Debug, PartialEq, Clone)]
pub enum BorderCheck {
    Client,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    NoWhere,
}

impl BorderCheck {
    pub fn drag_resize_window(
        &self,
        window: &Window,
    ) {
        let _ = window.drag_resize_window(match self {
            BorderCheck::Left => ResizeDirection::West,
            BorderCheck::Right => ResizeDirection::East,
            BorderCheck::Top => ResizeDirection::North,
            BorderCheck::Bottom => ResizeDirection::South,
            BorderCheck::TopLeft => ResizeDirection::NorthWest,
            BorderCheck::TopRight => ResizeDirection::NorthEast,
            BorderCheck::BottomLeft => ResizeDirection::SouthWest,
            BorderCheck::BottomRight => ResizeDirection::SouthEast,
            _ => unreachable!(),
        });
    }
}

pub fn run_border_check(
    window_size: PhysicalSize<u32>,
    x: u32,
    y: u32,
    scale: f64,
) -> BorderCheck {
    const BORDERLESS_RESIZE_INSET: f64 = 5.0;

    let window_size_logical: LogicalSize<u32> = window_size.to_logical(scale);
    let inset = (BORDERLESS_RESIZE_INSET * scale).ceil() as u32;

    let left = x < inset;
    let right = x >= window_size_logical.width - inset;
    let top = y < inset;
    let bottom = y >= window_size_logical.height - inset;

    match (left, right, top, bottom) {
        (true, false, true, false) => BorderCheck::TopLeft,
        (false, true, true, false) => BorderCheck::TopRight,
        (true, false, false, true) => BorderCheck::BottomLeft,
        (false, true, false, true) => BorderCheck::BottomRight,
        (true, false, false, false) => BorderCheck::Left,
        (false, true, false, false) => BorderCheck::Right,
        (false, false, true, false) => BorderCheck::Top,
        (false, false, false, true) => BorderCheck::Bottom,
        (false, false, false, false) => BorderCheck::Client,
        _ => BorderCheck::NoWhere,
    }
}

fn parse_coordinates(
    request: &mut Split<[char; 2]>
) -> Result<(u32, u32), &'static str> {
    if let (Some(x_str), Some(y_str)) = (request.next(), request.next()) {
        if let (Ok(x), Ok(y)) = (x_str.parse::<u32>(), y_str.parse::<u32>()) {
            return Ok((x, y));
        }
    }
    Err("Invalid or missing coordinates")
}

pub fn handle_window_requests(
    request_body: &String,
    proxy: &EventLoopProxy<UserEvent>,
) {
    let mut request = request_body.split([':', ',']);
    request.next(); // Skip the "window_control" prefix

    let action = match request.next() {
        Some(action) => action,
        None => {
            eprintln!("Invalid request: {}", request_body);
            return;
        },
    };

    let result = match action {
        "minimize" => proxy.send_event(UserEvent::Minimize),
        "toggle_maximize" => proxy.send_event(UserEvent::Maximize),
        "close" => proxy.send_event(UserEvent::CloseWindow),
        "drag" => proxy.send_event(UserEvent::DragWindow),
        "mouse_down" => match parse_coordinates(&mut request) {
            Ok((x, y)) => proxy.send_event(UserEvent::MouseDown(x, y)),
            Err(e) => {
                eprintln!("Failed to parse coordinates: {}", e);
                return;
            },
        },
        _ => {
            eprintln!("Invalid window control: {}", action);
            return;
        },
    };

    if let Err(e) = result {
        eprintln!("Failed to send event: {:?}", e);
    }
}
