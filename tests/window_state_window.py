"""
The child half of the window Event test: one real Webview, driven through its
own states.

Run as a subprocess by tests/test_window_events_reach_both_sides.py, never
imported. It has to be a subprocess for the same reason the other windowed
children do: `run()` never returns, so everything worth reporting has to be on
disk before the window closes.

The round the window runs, in order. Each step is started by the Python
listener for the Event the step before it produced, so the round is itself the
proof that the Events arrive — a step that never runs is a step whose Event
never came:

  1. the page registers a frontend listener for every `window:` name and emits
     `ready`
  2. `ready` maximizes the window; `window:maximized` unmaximizes it again
  3. `window:unmaximized` minimizes it; `window:minimized` restores it
  4. `window:restored` asks the window to close, which the close hook refuses
     the first time
  5. `window:close-requested` reaches the page while the window is still there
     to hear it, and the page reports everything its listeners saw
  6. the report closes the window for real, and the close hook lets it go

Two things are checked in the page rather than in Python. A frontend listener
for `window:resized` compares the size it was handed with `window.innerWidth`
and `window.innerHeight`, which are CSS pixels — so a size in physical pixels
would be double on any 2x display and the comparison would fail. And the page
tries to emit a `window:` name of its own, which must be refused.

A stall watchdog closes the window if the round stops advancing, so a state
the platform will not enter costs one failing assertion instead of the whole
file.

Usage: python window_state_window.py <journal-path>
"""

import json
import os
import sys
import threading
import time
from typing import Any

# How long a window state is given to finish arriving before the round moves
# on from it. A platform that animates a maximize reports the new state before
# the window has reached the new size.
SETTLE = 1.5

# How long the round may stop advancing before the window is closed anyway, so
# a platform that refuses to minimize reads as a missing journal line rather
# than a hang.
STALL_PATIENCE = 15.0

# When the watchdog thread ends the process because the window never closed
# itself. Only reached when the page never ran at all.
BACKSTOP = 60.0

# The exit code the backstop leaves behind, so the parent can tell a window
# that closed itself from one that had to be shot.
BACKSTOP_EXIT = 3

# Every name Dry reserves for itself, less the two the round cannot produce:
# `hidden` and `shown` need a way to take the window off the screen, which is
# runtime window management and does not exist yet.
WATCHED = [
    'maximized',
    'unmaximized',
    'minimized',
    'restored',
    'hidden',
    'shown',
    'focused',
    'blurred',
    'moved',
]

PAGE = """
<!doctype html>
<meta charset="utf-8">
<title>Window Events</title>
<body>Dry is checking that window Events reach both sides.</body>
<script>
  const seen = [];

  for (const name of %s) {
    window.dry.on('window:' + name, (value) => seen.push([name, value]));
  }

  // A resize carries the new size, and it carries it in logical pixels. CSS
  // pixels are logical pixels, so the page can check that for itself: on a 2x
  // display a physical size would be twice what the page renders into.
  window.dry.on('window:resized', (value) => {
    const width = Math.abs(value.width - window.innerWidth);
    const height = Math.abs(value.height - window.innerHeight);
    seen.push(['resized', value, width <= 2 && height <= 2]);
  });

  // Listening for a reserved name is allowed. Emitting one is not.
  try {
    window.dry.emit('window:maximized', null);
    seen.push(['forged', true]);
  } catch (error) {
    seen.push(['forged', false]);
  }

  // The close the hook is about to refuse. The page is still there to hear it,
  // which is the only moment a frontend can observe a close request at all.
  let reported = false;
  window.dry.on('window:close-requested', (value) => {
    seen.push(['close-requested', value]);
    if (reported) return;
    reported = true;
    window.dry.emit('report', JSON.stringify(seen));
  });

  window.dry.emit('ready', null);
</script>
""" % json.dumps(WATCHED)


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


def main() -> int:
    path = sys.argv[1]

    from dry import Webview

    webview = Webview(
        title='Dry Window Events',
        size=(520, 380),
        min_size=(300, 200),
        html=PAGE,
    )

    # The last time the round moved. The stall watchdog reads it.
    advanced = [time.monotonic()]

    def step(line: str, script: str | None = None) -> None:
        advanced[0] = time.monotonic()
        journal(path, line)
        if script is not None:
            webview.eval_js(script)

    # Each of these listeners proves its own Event reached Python, and starts
    # the step that produces the next one.

    def ready(value: Any) -> None:
        step('ready', 'window.dry.toggleMaximize()')

    def maximized(value: Any) -> None:
        # macOS animates a zoom, and reports `is_maximized` the instant it is
        # asked to — several turns of the event loop before the window has
        # actually got there. Unmaximizing straight away would put the window
        # back before it ever reached the new size, and there would be no
        # resize to observe. The listener runs off the event-loop thread
        # (ADR-0001), so waiting here does not stop the window.
        step(f'maximized {value}')
        time.sleep(SETTLE)
        webview.eval_js('window.dry.toggleMaximize()')

    def unmaximized(value: Any) -> None:
        step(f'unmaximized {value}', 'window.dry.minimize()')

    def minimized(value: Any) -> None:
        step(f'minimized {value}', 'window.dry.minimize()')

    def restored(value: Any) -> None:
        step(f'restored {value}', 'window.dry.close()')

    def resized(value: Any) -> None:
        # Not a step: a resize arrives several times over the round, and only
        # the last one is worth writing down. The page checks the value.
        advanced[0] = time.monotonic()
        journal(path, f'resized {value}')

    def moved(value: Any) -> None:
        advanced[0] = time.monotonic()
        journal(path, f'moved {value}')

    def close_requested(value: Any) -> None:
        step(f'close-requested {value}')

    def report(value: Any) -> None:
        step('report ' + str(value), 'window.dry.close()')

    _ = webview.on('ready', ready)
    _ = webview.on('report', report)
    _ = webview.on('window:maximized', maximized)
    _ = webview.on('window:unmaximized', unmaximized)
    _ = webview.on('window:minimized', minimized)
    _ = webview.on('window:restored', restored)
    _ = webview.on('window:resized', resized)
    _ = webview.on('window:moved', moved)
    _ = webview.on('window:close-requested', close_requested)

    # Two listeners for one window Event, because a window Event is an Event
    # like any other and reaches every listener registered for its name.
    def maximized_again(value: Any) -> None:
        journal(path, 'maximized-again')

    _ = webview.on('window:maximized', maximized_again)

    # The first close is refused, so the page is still there when
    # `window:close-requested` is delivered to it. The second is let go.
    refusals = [0]

    def on_close() -> bool:
        refusals[0] += 1
        journal(path, f'hook {refusals[0]}')
        return refusals[0] > 1

    webview.on_close = on_close

    def stall_watchdog() -> None:
        while True:
            time.sleep(0.5)
            if time.monotonic() - advanced[0] > STALL_PATIENCE:
                journal(path, 'stalled')
                webview.eval_js('window.dry.close()')
                return

    threading.Thread(target=watchdog, args=(path,), daemon=True).start()
    threading.Thread(target=stall_watchdog, daemon=True).start()

    journal(path, 'run')
    webview.run()

    journal(path, 'returned')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
