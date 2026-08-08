"""
The child half of the drag-resize test: one undecorated window, one edge drag.

Run as a subprocess by tests/test_resize_edges_drag_the_window.py, never
imported. It has to be a subprocess for the same reason the GIL test's child
does: `run()` never returns, so everything worth reporting has to be on disk
before the window closes.

Two things are measured, and the second is measured differently on each
platform because the two platforms resize by different machinery:

  * What `navigator.platform` and `navigator.userAgent` actually are, and which
    way the guard in src/js/window_functions.js resolves on them. The guard
    decides whether the drag runs in JavaScript or is handed to the platform,
    and a webview that misreports itself sends the whole drag down the wrong
    path silently.

  * That the east edge of an undecorated window really moves when it is
    dragged. On macOS the drag *is* JavaScript, so dispatching the mouse events
    at the edge drives exactly the code a user drives. On Windows the grab is
    handed to the platform, which runs a modal loop of its own reading the real
    cursor, so nothing short of real mouse input reaches it: the drag is
    injected with `SetCursorPos` and `mouse_event` and the window is measured
    through `GetWindowRect`, neither of which goes through Dry at all.

The page reports `isTrusted` on every mousedown it sees, so a run where
injected input never reached the webview is distinguishable from one where it
did and the window did not move.

Usage: python resize_window.py <journal-path>
"""

import json
import os
import sys
import threading
import time
from typing import Any

TITLE = 'Dry drag resize'

# The window the drag starts from, and the floor it may not be dragged below.
SIZE = (420, 300)
MIN_SIZE = (240, 180)

# How far the east edge is dragged outwards. Large enough that no rounding,
# handle thickness or one-pixel disagreement can be mistaken for it, small
# enough to stay on any runner's screen.
GROWTH = 120

# How long the window waits after the drag before it is measured. The drag
# reports cross to Rust over IPC and come back as a resize, so the answer is
# never in the same frame.
SETTLE = 1.0

# When the watchdog ends the process because the window never closed itself.
# Reached when the page never ran, or when a native modal resize loop never
# ended, which would leave the event loop unable to hear anything.
BACKSTOP = 90.0

# The exit code the backstop leaves behind.
BACKSTOP_EXIT = 3

PAGE = f"""
<!doctype html>
<meta charset="utf-8">
<title>{TITLE}</title>
<body style="margin:0;font:14px sans-serif">Dry is dragging its own edge.</body>
<script>
  const guard = /Mac/i.test(navigator.platform || navigator.userAgent);

  // The size the window last settled at, in CSS pixels. Read from the resize
  // event rather than sampled, so the value is whatever the window ended on.
  let size = [window.innerWidth, window.innerHeight];
  window.addEventListener('resize', () => {{
    size = [window.innerWidth, window.innerHeight];
  }});

  // Whether a mousedown was the platform's or this script's. On Windows the
  // drag is injected as real input, and this is how the test tells input that
  // never arrived from input that arrived and did nothing.
  window.addEventListener('mousedown', event => {{
    window.dry.emit('mousedown', {{
      x: event.clientX,
      y: event.clientY,
      trusted: event.isTrusted,
      target: event.target.className || event.target.tagName,
    }});
  }}, true);

  // Where the east edge is grabbed, and what is sitting there. The handles
  // src/js/window_borders.js draws are 3px wide and fixed to the frame, so a
  // grab two pixels in from the right lands on one whatever the window's size.
  const grabX = () => window.innerWidth - 2;
  const grabY = () => Math.round(window.innerHeight / 2);
  const eastHandle = () => document.elementFromPoint(grabX(), grabY());
  const nameOf = (node) => node ? (node.className || node.tagName) : null;

  // The drag macOS runs here. The mousedown goes to whatever element sits on
  // the east edge, so the round trip under test starts where a user's would.
  const dragTheEastEdge = () => {{
    const x = grabX();
    const y = grabY();
    const handle = eastHandle();

    const at = (type, target, clientX) => target.dispatchEvent(new MouseEvent(type, {{
      clientX, clientY: y, bubbles: true, cancelable: true, button: 0, buttons: 1,
    }}));

    at('mousedown', handle || window, x);
    for (let step = 1; step <= 12; step++) {{
      at('mousemove', window, x + Math.round(step * {GROWTH} / 12));
    }}
    at('mouseup', window, x + {GROWTH});
  }};

  window.dry.on('drag-here', dragTheEastEdge);
  window.dry.on('measure', () => window.dry.emit('measured', size));

  // Reported on load, not at parse time: the resize handles are appended on
  // DOMContentLoaded, so before that there is nothing on the east edge to name.
  window.addEventListener('load', () => window.dry.emit('report', {{
    platform: navigator.platform,
    userAgent: navigator.userAgent,
    guard,
    size,
    handle: nameOf(eastHandle()),
  }}));
</script>
"""


def journal(path: str, line: str) -> None:
    """
    Append one line and flush it. Nothing here survives buffering: the process
    is killed with `process::exit` when the window goes.
    """
    with open(path, 'a', encoding='utf-8') as file:
        file.write(f'{line}\n')
        file.flush()
        os.fsync(file.fileno())


def watchdog(path: str) -> None:
    started = time.monotonic()
    while True:
        time.sleep(0.25)
        if time.monotonic() - started > BACKSTOP:
            journal(path, 'backstop')
            os._exit(BACKSTOP_EXIT)


def inject_the_drag(path: str) -> None:
    """
    Drag the east edge with real mouse input, and measure the window without
    asking Dry.

    Windows hands the grab to `DefWindowProc`, which enters a modal sizing loop
    reading the real cursor and ending on the real button going up. Synthetic
    DOM events cannot reach that loop, and neither can anything in the page:
    the button has to be genuinely down before `dry.resize()` is called, and
    genuinely up afterwards, or the loop never starts or never ends.
    """
    import ctypes
    from ctypes import wintypes

    class Rect(ctypes.Structure):
        _fields_ = [
            ('left', wintypes.LONG),
            ('top', wintypes.LONG),
            ('right', wintypes.LONG),
            ('bottom', wintypes.LONG),
        ]

    user32 = ctypes.WinDLL('user32', use_last_error=True)
    user32.FindWindowW.argtypes = [wintypes.LPCWSTR, wintypes.LPCWSTR]
    user32.FindWindowW.restype = wintypes.HWND
    user32.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(Rect)]
    user32.SetCursorPos.argtypes = [ctypes.c_int, ctypes.c_int]

    left_down, left_up = 0x0002, 0x0004

    def rect_of(handle: int) -> tuple[int, int, int, int]:
        rect = Rect()
        user32.GetWindowRect(handle, ctypes.byref(rect))
        return (rect.left, rect.top, rect.right, rect.bottom)

    handle = 0
    deadline = time.monotonic() + 20
    while not handle and time.monotonic() < deadline:
        handle = user32.FindWindowW(None, TITLE) or 0
        if not handle:
            time.sleep(0.2)

    if not handle:
        journal(path, 'native-missing []')
        return

    before = rect_of(handle)
    x, y = before[2] - 2, (before[1] + before[3]) // 2
    journal(path, f'native-before {json.dumps(before)}')
    journal(path, f'native-grab {json.dumps([x, y])}')

    user32.SetForegroundWindow(handle)
    time.sleep(0.5)
    user32.SetCursorPos(x, y)
    time.sleep(0.3)

    # The press has to land on the page, be answered with dry.resize('east'),
    # cross the IPC and reach the event loop before the pointer starts moving.
    user32.mouse_event(left_down, 0, 0, 0, 0)
    time.sleep(0.8)

    for step in range(1, 13):
        user32.SetCursorPos(x + round(step * GROWTH / 12), y)
        time.sleep(0.04)

    time.sleep(0.3)
    user32.mouse_event(left_up, 0, 0, 0, 0)
    time.sleep(0.6)

    after = rect_of(handle)
    journal(path, f'native-after {json.dumps(after)}')


def main() -> int:
    path = sys.argv[1]

    from dry import Webview

    webview = Webview(
        title=TITLE,
        size=SIZE,
        min_size=MIN_SIZE,
        decorations=False,
        html=PAGE,
    )

    def report(value: Any) -> None:
        journal(path, f'report {json.dumps(value)}')
        threading.Thread(target=drive, daemon=True).start()

    def drive() -> None:
        """
        Run the drag the way this platform has to be driven, then ask the page
        what the window ended up as.
        """
        if sys.platform == 'win32':
            inject_the_drag(path)
        else:
            webview.emit('drag-here')
        time.sleep(SETTLE)
        webview.emit('measure')

    def mousedown(value: Any) -> None:
        journal(path, f'mousedown {json.dumps(value)}')

    def measured(value: Any) -> None:
        journal(path, f'measured {json.dumps(value)}')
        webview.eval_js('window.dry.close()')

    _ = webview.on('report', report)
    _ = webview.on('mousedown', mousedown)
    _ = webview.on('measured', measured)

    threading.Thread(target=watchdog, args=(path,), daemon=True).start()

    journal(path, f'run {sys.platform}')
    webview.run()

    journal(path, 'returned')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
