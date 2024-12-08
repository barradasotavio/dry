use std::path::Path;
use tao::window::ResizeDirection;
use tao::{
    dpi::PhysicalSize,
    error::OsError,
    event_loop::{EventLoop, EventLoopProxy},
    window::{Icon, Window, WindowBuilder},
};

use crate::events::AppEvent;

pub const WINDOW_FUNCTIONS_JS: &str = include_str!("js/window_functions.js");
pub const WINDOW_EVENTS_JS: &str = include_str!("js/window_events.js");
pub const WINDOW_BORDERS_JS: &str = include_str!("js/window_borders.js");

pub fn build_window(
    event_loop: &EventLoop<AppEvent>,
    title: String,
    min_size: (u32, u32),
    size: (u32, u32),
    decorations: bool,
    icon_path: Option<String>,
) -> Result<Window, OsError> {
    let min_size = PhysicalSize::new(min_size.0, min_size.1);
    let size = PhysicalSize::new(size.0, size.1);
    let mut window_builder = WindowBuilder::new()
        .with_title(title)
        .with_min_inner_size(min_size)
        .with_inner_size(size)
        .with_decorations(decorations);
    if let Some(icon_path) = icon_path {
        let icon = load_icon(Path::new(&icon_path));
        window_builder = window_builder.with_window_icon(icon);
    }
    let window = window_builder.build(event_loop)?;
    Ok(window)
}

fn load_icon(path: &Path) -> Option<Icon> {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    Icon::from_rgba(icon_rgba, icon_width, icon_height).ok()
}

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
