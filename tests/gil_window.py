"""
The child half of the GIL regression test: one real Webview, one Python thread.

Run as a subprocess by tests/test_gil_released_during_run.py, never imported.
It has to be a subprocess because `run()` never returns — `tao::EventLoop::run`
exits the process from inside itself — so nothing this script does after the
call can be observed, and everything it wants to report has to be on disk
before the window closes.

A daemon thread starts ticking before `run()` and writes every tick to a
journal, flushed each time: the process is killed with `process::exit`, so
anything still sitting in a buffer is lost. The window closes itself from
JavaScript after WINDOW_LIFETIME, which ends the process. The parent then reads
the journal and asks how much of the window's life the thread was awake for.

Usage: python gil_window.py <journal-path>
"""

import os
import sys
import threading
import time

# How long the window stays open before the page closes it. Long enough that a
# frozen thread and a running one cannot be confused, short enough to sit in
# CI.
WINDOW_LIFETIME = 1.5

# The gap between ticks. WINDOW_LIFETIME / TICK_INTERVAL ticks are expected.
TICK_INTERVAL = 0.05

# When the ticker gives up waiting for the window to close itself and ends the
# process. Only reached when the page never runs, and it keeps that failure
# from hanging the parent until its timeout. Far above WINDOW_LIFETIME on
# purpose: the page's timer starts only once the webview has rendered, and a
# cold WebView2 on a Windows runner takes seconds to get there.
BACKSTOP = 30.0

# The exit code the backstop leaves behind, so the parent can tell a window
# that closed itself from one that had to be shot.
BACKSTOP_EXIT = 3

# Every timestamp in the journal is measured from here, so the parent can line
# the ticks up against the moment `run()` was called.
START = time.monotonic()

PAGE = f"""
<!doctype html>
<meta charset="utf-8">
<title>GIL regression</title>
<body>Dry is checking that Python threads keep running.</body>
<script>
  setTimeout(() => window.dry.close(), {int(WINDOW_LIFETIME * 1000)});
</script>
"""


def journal(path: str, event: str) -> None:
    """
    Append one stamped line and flush it. Nothing here survives buffering.
    """
    with open(path, 'a', encoding='utf-8') as file:
        file.write(f'{event} {time.monotonic() - START:.6f}\n')
        file.flush()
        os.fsync(file.fileno())


def tick(path: str) -> None:
    """
    Tick until the process dies under us, or until the backstop fires.

    Deliberately unrelated to the Api and to the portal: this is a plain
    thread a user could have started, and the whole point is that it owes
    nothing to Dry.
    """
    started = time.monotonic()
    while True:
        time.sleep(TICK_INTERVAL)
        journal(path, 'tick')
        if time.monotonic() - started > BACKSTOP:
            journal(path, 'backstop')
            os._exit(BACKSTOP_EXIT)


def main() -> int:
    path = sys.argv[1]

    from dry import Webview

    webview = Webview()
    webview.title = 'Dry GIL regression'
    webview.size = webview.min_size = (320, 240)
    webview.html = PAGE

    threading.Thread(target=tick, args=(path,), daemon=True).start()

    journal(path, 'run')
    webview.run()

    # Unreachable while `run()` owns the process. If it ever does return, the
    # parent should hear about it rather than read a silent pass.
    journal(path, 'returned')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
