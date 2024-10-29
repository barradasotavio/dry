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

pub fn handle_window_requests(
    request_body: &String,
    proxy: &EventLoopProxy<UserEvent>,
) {
    let mut request = request_body.split([':', ',']);
    match request.next().unwrap() {
        "minimize" => {
            let _ = proxy.send_event(UserEvent::Minimize);
        },
        "maximize" => {
            let _ = proxy.send_event(UserEvent::Maximize);
        },
        "drag_window" => {
            let _ = proxy.send_event(UserEvent::DragWindow);
        },
        "close" => {
            let _ = proxy.send_event(UserEvent::CloseWindow);
        },
        "mousedown" => {
            let x = request.next().unwrap().parse().unwrap();
            let y = request.next().unwrap().parse().unwrap();
            let _ = proxy.send_event(UserEvent::MouseDown(x, y));
        },
        "mousemove" => {
            let x = request.next().unwrap().parse().unwrap();
            let y = request.next().unwrap().parse().unwrap();
            let _ = proxy.send_event(UserEvent::MouseMove(x, y));
        },
        _ => {},
    }
}

pub const WINDOW_SCRIPT: &str = r#"
document.addEventListener('mousemove', (e) => window.ipc.postMessage(`mousemove:${e.clientX},${e.clientY}`))
document.addEventListener('mousedown', (e) => {
    if (e.target.hasAttribute('data-drag-region') && e.button === 0) {
        e.detail === 2
            ? window.ipc.postMessage('maximize')
            : window.ipc.postMessage('drag_window');
    } else {
    window.ipc.postMessage(`mousedown:${e.clientX},${e.clientY}`);
    }
})
document.addEventListener('touchstart', (e) => {
    if (e.target.hasAttribute('data-drag-region')) {
        window.ipc.postMessage('drag_window');
    }
})
"#;
