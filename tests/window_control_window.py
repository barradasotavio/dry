"""
The child half of the runtime window control test: one real Webview, resized,
moved, retitled and maximized while it is running.

Run as a subprocess by tests/test_window_control_applies_at_runtime.py, never
imported. It has to be a subprocess for the same reason the other windowed
children do: `run()` never returns, so everything worth reporting has to be on
disk before the window closes.

What is being proved is that a setting assigned after the window opened
reaches the window — the thing that did nothing at all before. So every step
assigns a property and then reads the window back, twice over:

  * `webview.state()`, Dry's own reading, which is what a Python callback sees
  * `window.innerWidth` and `window.innerHeight` in the page, which are CSS
    pixels — so a size that arrived in physical pixels would be double on any
    2x display and the two readings would not match

The round is driven by one thread rather than by chaining Event listeners the
way tests/window_state_window.py does, because what is under test here is the
assignment and not the Event. A step waits for the window to reach the state
it asked for, gives up after a moment, and writes down what it saw either way,
so a platform that refuses one step still lets the rest of the round run.

Visibility is not exercised here: `window:hidden` and `window:shown` are the
proof that `visible` applies, and they are checked in
tests/test_window_events_reach_both_sides.py.

Usage: python window_control_window.py <journal-path>
"""

import json
import os
import sys
import threading
import time
from typing import Any, Callable

# How long a step waits for the window to reach what it was asked for. The
# assignment crosses to the thread that draws the window and is applied on its
# next turn, and a platform that animates the change takes longer still.
PATIENCE = 8.0

# How often the window is asked whether it got there yet.
POLL = 0.05

# How long a change the platform animates is given to finish after it has
# already reported itself done. macOS reports the new state several turns of
# the event loop before the window has actually got there.
SETTLE = 1.0

# When the watchdog thread ends the process because the window never closed
# itself. Only reached when the round never ran at all.
BACKSTOP = 90.0

# The exit code the backstop leaves behind, so the parent can tell a window
# that closed itself from one that had to be shot.
BACKSTOP_EXIT = 3

# What the window opens as, and what every later size is a change from.
OPENING_SIZE = (520, 380)

PAGE = """
<!doctype html>
<meta charset="utf-8">
<title>Window Control</title>
<body>Dry is checking that window settings apply at runtime.</body>
<script>
  // The frontend half of the state query. A page that has just loaded has
  // observed no window Event at all and still has to know whether to draw its
  // maximize button one way round.
  window.dry.state().then((state) => window.dry.emit('page-state', state));

  // What the page renders into, beside what Dry says the window is. CSS
  // pixels are logical pixels, so a size that arrived in physical pixels
  // would be double on any 2x display and the two would not match.
  window.dry.on('measure', async () => {
    const state = await window.dry.state();
    window.dry.emit('measured', {
      inner: [window.innerWidth, window.innerHeight],
      state: state,
    });
  });

  window.dry.emit('ready', null);
</script>
"""


# The round writes from its own thread while listeners write from the Portal,
# so two writers meet here. Appending is not atomic on Windows — `O_APPEND`
# there seeks to the end and then writes, so two writers can land on the same
# offset and one line is simply lost, which looks exactly like a step that
# never ran.
WRITING = threading.Lock()


def journal(path: str, line: str) -> None:
    """
    Append one line and flush it. Nothing here survives buffering: the process
    is killed with `process::exit` when the window goes.
    """
    with WRITING, open(path, 'a', encoding='utf-8') as file:
        file.write(f'{line}\n')
        file.flush()
        os.fsync(file.fileno())


def record(path: str, name: str, **fields: Any) -> None:
    """
    One step of the round, written as a name and the readings it took, so the
    parent can assert on numbers rather than on prose.
    """
    journal(path, f'{name} {json.dumps(fields)}')


def watchdog(path: str) -> None:
    started = time.monotonic()
    while True:
        time.sleep(0.25)
        if time.monotonic() - started > BACKSTOP:
            journal(path, 'backstop')
            os._exit(BACKSTOP_EXIT)


def until(condition: Callable[[], bool]) -> bool:
    """
    Wait for the window to have got there, for as long as it is worth waiting.
    """
    deadline = time.monotonic() + PATIENCE
    while time.monotonic() < deadline:
        if condition():
            return True
        time.sleep(POLL)
    return condition()


def near(measured: tuple[int, int], asked: tuple[int, int]) -> bool:
    """
    Whether a window landed where it was asked to, allowing a pixel of
    rounding on each axis. A window that arrived in physical pixels on a 2x
    display is not near anything.
    """
    return abs(measured[0] - asked[0]) <= 2 and abs(measured[1] - asked[1]) <= 2


def main() -> int:
    path = sys.argv[1]

    from dry import Webview

    webview = Webview(
        title='Dry Window Control',
        size=OPENING_SIZE,
        min_size=(300, 200),
        html=PAGE,
    )

    opened = threading.Event()
    measured: list[Any] = []
    reported = threading.Event()

    def ready(value: Any) -> None:
        opened.set()

    def page_state(value: Any) -> None:
        # What the page was told when it asked, before it had heard a single
        # window Event — which is the case the query exists for.
        record(path, 'page-state', reading=value)

    def on_measured(value: Any) -> None:
        measured.append(value)
        reported.set()

    _ = webview.on('ready', ready)
    _ = webview.on('page-state', page_state)
    _ = webview.on('measured', on_measured)

    def ask_the_page() -> Any:
        """
        What the page renders into, and what it is told when it asks.
        """
        reported.clear()
        webview.emit('measure', None)
        if not reported.wait(PATIENCE):
            return None
        return measured[-1]

    def round_of_changes() -> None:
        if not opened.wait(PATIENCE * 2):
            journal(path, 'the page never loaded')
            webview.eval_js('window.dry.close()')
            return

        opening = webview.state()
        record(path, 'opened', size=opening.size, position=opening.position)

        # Size. The window is asked for a size it does not have, and both the
        # state query and the page have to agree that it got it.
        asked = (640, 480)
        webview.size = asked
        _ = until(lambda: near(webview.state().size, asked))
        record(
            path,
            'resized',
            asked=asked,
            state=webview.state().size,
            # `size` reads back what the window measures, not what it was last
            # told, so these two are the same number.
            prop=webview.size,
            page=ask_the_page(),
        )

        # Position. There is no setting to fall back on, so this is also the
        # proof that a window can be read back at all.
        asked_position = (140, 120)
        webview.position = asked_position
        _ = until(lambda: near(webview.position, asked_position))
        record(path, 'moved', asked=asked_position, state=webview.position)

        # The settings that change the window without moving it. Nothing here
        # can be read back through any API either platform offers — the proof
        # that they applied is the window itself, which is why this test is
        # run by a human at least once.
        webview.title = 'Renamed while running'
        webview.min_size = (200, 150)
        webview.icon_path = None
        record(path, 'assigned', title=webview.title, min_size=webview.min_size)

        # Maximize, from Python rather than from the frontend or the user.
        webview.maximized = True
        _ = until(lambda: webview.state().maximized)
        record(
            path,
            'maximized',
            state=webview.state().maximized,
            page=ask_the_page(),
        )

        webview.maximized = False
        _ = until(lambda: not webview.state().maximized)
        record(path, 'unmaximized', state=webview.state().maximized)

        # Minimize and restore, the same way round.
        webview.minimized = True
        _ = until(lambda: webview.state().minimized)
        record(path, 'minimized', state=webview.state().minimized)
        webview.minimized = False
        _ = until(lambda: not webview.state().minimized)
        record(path, 'restored', state=webview.state().minimized)

        # Fullscreen, the one window state with no Event of its own — so the
        # state query is the only way to see it happen at all.
        webview.fullscreen = True
        _ = until(lambda: webview.state().fullscreen)
        record(path, 'fullscreen', state=webview.state().fullscreen)
        webview.fullscreen = False
        _ = until(lambda: not webview.state().fullscreen)
        record(path, 'windowed', state=webview.state().fullscreen)

        # Decorations last: the window loses its titlebar, and what it renders
        # into changes with it, so nothing else is measured after this.
        webview.decorations = False
        time.sleep(SETTLE)
        record(
            path,
            'undecorated',
            state=webview.state().size,
            page=ask_the_page(),
        )

        journal(path, 'done')
        webview.eval_js('window.dry.close()')

    def refuse_nothing() -> bool:
        journal(path, 'hook')
        return True

    webview.on_close = refuse_nothing

    threading.Thread(target=watchdog, args=(path,), daemon=True).start()
    threading.Thread(target=round_of_changes, daemon=True).start()

    journal(path, 'run')
    webview.run()

    journal(path, 'returned')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
