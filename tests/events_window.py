"""
The child half of the Event test: one real Webview, Events both ways.

Run as a subprocess by tests/test_events_cross_the_bridge.py, never imported.
It has to be a subprocess for the same reason the GIL test's child does:
`run()` never returns, so everything worth reporting has to be on disk before
the window closes.

The round the window runs, in order:

  1. the page emits `ping` with a value, from JavaScript
  2. three Python listeners are registered for it — one that raises, and two
     that record — and all three are delivered to
  3. the first recording listener emits `pong` back from the callback it is
     already on, the second emits `pong-thread` from a thread it starts
  4. two page listeners for `pong` and one for `pong-thread` record what
     arrived and emit `done` back to Python
  5. Python's `done` listener writes the whole page's account to the journal
     and the page closes the window

Nothing in that round returns a value to its sender. Everything travels as an
Event, and everything crossing is checked against the Bridge contract on the
way, `default=` hook included: the value the page sends back carries a
`datetime` converted by the hook.

Usage: python events_window.py <journal-path>
"""

import os
import sys
import threading
import time
from datetime import datetime, timezone
from typing import Any

# How long the page waits before giving up on the round trip and closing
# anyway, so a failure reads as a missing journal line rather than a hang.
PAGE_PATIENCE = 8000

# When the watchdog thread ends the process because the window never closed
# itself. Only reached when the page never ran at all.
BACKSTOP = 40.0

# The exit code the backstop leaves behind, so the parent can tell a window
# that closed itself from one that had to be shot.
BACKSTOP_EXIT = 3

PAGE = f"""
<!doctype html>
<meta charset="utf-8">
<title>Events</title>
<body>Dry is checking that Events cross both ways.</body>
<script>
  const seen = [];

  let reported = false;
  const report = () => {{
    if (reported) return;
    reported = true;
    window.dry.emit('done', seen);
  }};

  // The two answers race — they come from two Python listeners for the same
  // Event, and those run concurrently — so the page waits for both.
  const waiting = new Set(['pong', 'pong-thread']);
  const arrived = (name) => {{
    waiting.delete(name);
    if (waiting.size === 0) report();
  }};

  // Two listeners for one name, so the page proves the same thing Python
  // proves: an Event reaches every listener registered for its name.
  window.dry.on('pong', (value) => seen.push(['pong-a', value.echo.n]));
  window.dry.on('pong', (value) => seen.push(['pong-b', value.echo.deep]));

  // A listener that throws must not rob the ones after it of theirs.
  window.dry.on('pong', () => {{ throw new Error('this listener is wrong'); }});
  window.dry.on('pong', (value) => {{
    seen.push(['pong-c', typeof value.when]);
    arrived('pong');
  }});

  // A registration taken off again hears nothing.
  const deaf = () => seen.push(['deaf', true]);
  const unsubscribe = window.dry.on('pong', deaf);
  unsubscribe();

  // A reserved name belongs to Dry, and the page may not emit under it.
  try {{
    window.dry.emit('window:maximized', null);
    seen.push(['reserved', 'allowed']);
  }} catch (error) {{
    seen.push(['reserved', 'refused']);
  }}

  // An Event nobody listens for is a no-op, not an error.
  window.dry.emit('nobody-is-listening', 1);

  window.dry.on('pong-thread', (value) => {{
    seen.push(['pong-thread', value]);
    arrived('pong-thread');
  }});

  window.dry.emit('ping', {{ n: 1, deep: [true, null, 'x'] }});

  setTimeout(report, {PAGE_PATIENCE});
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


def main() -> int:
    path = sys.argv[1]

    from dry import Webview

    # The `default=` hook of ADR-0002, so the value an Event carries back to
    # the page is checked by the same contract a Call's return value is.
    def default(value: Any) -> Any:
        if isinstance(value, datetime):
            return value.isoformat()
        raise TypeError(f'{type(value).__name__} cannot cross the Bridge.')

    webview = Webview(
        title='Dry Events',
        size=(360, 240),
        min_size=(360, 240),
        html=PAGE,
        default=default,
    )

    def broken(value: Any) -> None:
        raise ValueError('this listener is wrong')

    def answer(value: Any) -> None:
        journal(path, f'ping {value}')
        # Emitted from inside the callback the Event arrived on.
        webview.emit('pong', {'echo': value, 'when': datetime.now(timezone.utc)})

    def answer_from_a_thread(value: Any) -> None:
        journal(path, 'ping-again')
        threading.Thread(
            target=lambda: webview.emit('pong-thread', 'from a thread'),
            daemon=True,
        ).start()

    # The raising listener goes on first, so the two after it prove that one
    # broken listener does not silence the rest.
    _ = webview.on('ping', broken)
    _ = webview.on('ping', answer)
    _ = webview.on('ping', answer_from_a_thread)

    # Registered and then taken off again, so the Event the page emits under
    # this name reaches nobody at all — which must be a no-op, not an error.
    def unregistered(value: Any) -> None:
        journal(path, 'still-listening')

    _ = webview.on('nobody-is-listening', unregistered)
    webview.off('nobody-is-listening', unregistered)

    def finish(value: Any) -> None:
        journal(path, f'done {value}')
        webview.eval_js('window.dry.close()')

    _ = webview.on('done', finish)

    try:
        webview.emit('too-early', 1)
        journal(path, 'early-emit allowed')
    except Exception as error:
        journal(path, f'early-emit {type(error).__name__}')

    try:
        webview.emit('window:maximized', None)
        journal(path, 'reserved allowed')
    except Exception as error:
        journal(path, f'reserved {type(error).__name__}')

    threading.Thread(target=watchdog, args=(path,), daemon=True).start()

    journal(path, 'run')
    webview.run()

    journal(path, 'returned')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
