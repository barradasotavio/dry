use std::str::Split;

use tao::{
    dpi::{LogicalSize, PhysicalSize},
    event_loop::EventLoopProxy,
    window::{CursorIcon, ResizeDirection, Window},
};

use crate::UserEvent;

#[derive(Debug)]
pub enum HitTestResult {
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

impl HitTestResult {
    pub fn drag_resize_window(
        &self,
        window: &Window,
    ) {
        let _ = window.drag_resize_window(match self {
            HitTestResult::Left => ResizeDirection::West,
            HitTestResult::Right => ResizeDirection::East,
            HitTestResult::Top => ResizeDirection::North,
            HitTestResult::Bottom => ResizeDirection::South,
            HitTestResult::TopLeft => ResizeDirection::NorthWest,
            HitTestResult::TopRight => ResizeDirection::NorthEast,
            HitTestResult::BottomLeft => ResizeDirection::SouthWest,
            HitTestResult::BottomRight => ResizeDirection::SouthEast,
            _ => unreachable!(),
        });
    }

    pub fn change_cursor(
        &self,
        window: &Window,
    ) {
        window.set_cursor_icon(match self {
            HitTestResult::Left => CursorIcon::WResize,
            HitTestResult::Right => CursorIcon::EResize,
            HitTestResult::Top => CursorIcon::NResize,
            HitTestResult::Bottom => CursorIcon::SResize,
            HitTestResult::TopLeft => CursorIcon::NwResize,
            HitTestResult::TopRight => CursorIcon::NeResize,
            HitTestResult::BottomLeft => CursorIcon::SwResize,
            HitTestResult::BottomRight => CursorIcon::SeResize,
            _ => CursorIcon::Default,
        });
    }
}

pub fn hit_test(
    window_size: PhysicalSize<u32>,
    x: u32,
    y: u32,
    scale: f64,
) -> HitTestResult {
    const BORDERLESS_RESIZE_INSET: f64 = 5.0;

    const CLIENT: isize = 0b0000;
    const LEFT: isize = 0b0001;
    const RIGHT: isize = 0b0010;
    const TOP: isize = 0b0100;
    const BOTTOM: isize = 0b1000;
    const TOPLEFT: isize = TOP | LEFT;
    const TOPRIGHT: isize = TOP | RIGHT;
    const BOTTOMLEFT: isize = BOTTOM | LEFT;
    const BOTTOMRIGHT: isize = BOTTOM | RIGHT;

    let window_size: LogicalSize<u32> = window_size.to_logical(scale);

    let (top, left) = (0, 0);
    let (bottom, right) = (window_size.height, window_size.width);

    let inset = (BORDERLESS_RESIZE_INSET * scale) as u32;

    #[rustfmt::skip]
    let result =
        (LEFT * (if x < (left + inset) { 1 } else { 0 }))
        | (RIGHT * (if x >= (right - inset) { 1 } else { 0 }))
        | (TOP * (if y < (top + inset) { 1 } else { 0 }))
        | (BOTTOM * (if y >= (bottom - inset) { 1 } else { 0 }));

    match result {
        CLIENT => HitTestResult::Client,
        LEFT => HitTestResult::Left,
        RIGHT => HitTestResult::Right,
        TOP => HitTestResult::Top,
        BOTTOM => HitTestResult::Bottom,
        TOPLEFT => HitTestResult::TopLeft,
        TOPRIGHT => HitTestResult::TopRight,
        BOTTOMLEFT => HitTestResult::BottomLeft,
        BOTTOMRIGHT => HitTestResult::BottomRight,
        _ => HitTestResult::NoWhere,
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

    if request.next() != Some("window_control") {
        return;
    }

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
        "mouse_move" | "mouse_down" => match parse_coordinates(&mut request) {
            Ok((x, y)) => match action {
                "mouse_move" => proxy.send_event(UserEvent::MouseMove(x, y)),
                "mouse_down" => proxy.send_event(UserEvent::MouseDown(x, y)),
                _ => unreachable!(),
            },
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

pub const WINDOW_SCRIPT: &str = r#"
Object.assign(window, {
    messageMouseMove: (x, y) => window.ipc.postMessage(`window_control:mouse_move:${x},${y}`),
    messageMouseDown: (x, y) => window.ipc.postMessage(`window_control:mouse_down:${x},${y}`),
    drag: () => window.ipc.postMessage('window_control:drag'),
    minimize: () => window.ipc.postMessage('window_control:minimize'),
    toggleMaximize: () => window.ipc.postMessage('window_control:toggle_maximize'),
    close: () => window.ipc.postMessage('window_control:close'),
});

document.addEventListener('mousemove', (e) => {
    window.messageMouseMove(e.clientX, e.clientY);
})

document.addEventListener('mousedown', (e) => {
    const isMainMouseButton = e.button === 0;
    if (!isMainMouseButton) { return; }

    const isDragRegion = e.target.hasAttribute('data-drag-region');
    if (!isDragRegion) { window.messageMouseDown(e.clientX, e.clientY); return; }

    const isDoubleClick = e.detail === 2;
    if (isDoubleClick) { window.toggleMaximize(); }
    else { window.drag(); }
})

document.addEventListener('touchstart', (e) => {
    const isDragRegion = e.target.hasAttribute('data-drag-region');
    if (isDragRegion) window.drag();
})
"#;
