use tao::{event_loop::EventLoopProxy, window::ResizeDirection};

use crate::AppEvent;

pub const WINDOW_FUNCTIONS_JS: &str = include_str!("js/window_functions.js");
pub const WINDOW_EVENTS_JS: &str = include_str!("js/window_events.js");
pub const WINDOW_BORDERS_JS: &str = include_str!("js/window_borders.js");

pub fn handle_window_requests(
    request_body: &String,
    proxy: &EventLoopProxy<AppEvent>,
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
        "minimize" => proxy.send_event(AppEvent::MinimizeWindow),
        "toggle_maximize" => proxy.send_event(AppEvent::MaximizeWindow),
        "close" => proxy.send_event(AppEvent::CloseWindow),
        "drag" => proxy.send_event(AppEvent::DragWindow),
        "resize" => {
            let direction = match request.next() {
                Some("north") => ResizeDirection::North,
                Some("south") => ResizeDirection::South,
                Some("east") => ResizeDirection::East,
                Some("west") => ResizeDirection::West,
                Some("north-west") => ResizeDirection::NorthWest,
                Some("north-east") => ResizeDirection::NorthEast,
                Some("south-west") => ResizeDirection::SouthWest,
                Some("south-east") => ResizeDirection::SouthEast,
                _ => {
                    eprintln!("Invalid resize direction");
                    return;
                },
            };
            proxy.send_event(AppEvent::ResizeWindow(direction))
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
