"""
The child half of the failed-navigation test: one real Webview, one bad URL.

Run as a subprocess by tests/test_navigation_failures_are_reported.py, never
imported. It has to be a subprocess for the same reason the GIL test's child
does: `run()` never returns, so everything worth reporting has to be on disk
before the process ends.

The report Dry makes about a navigation that never arrived is a `logging`
record on `dry.webview`, so this child configures `logging` to a file and lets
the journal *be* the log. Its own notes go to a `test` logger in the same file,
so one read gives the parent both the record and the run around it.

The level is DEBUG on purpose, because the three outcomes have to be told
apart and only the level tells them apart:

  * `ERROR dry.webview:` — the failure was diagnosed and reported. The point.
  * `DEBUG dry.webview:` — the watchdog ran and Python reached the address
    anyway, so Dry declined to accuse it. A disagreement between the webview's
    network stack and Python's, not a suppressed report.
  * neither — the watchdog never diagnosed anything, because it stood down on
    the page-load handler calling the failure an arrival. This is the WebView2
    error-page case: the report is suppressed and the blank window is silent
    again.

Usage: python navigation_window.py <journal-path> <url>
"""

import logging
import os
import sys
import threading
import time

# How long to keep the window open after Dry has said something, so a second
# record still lands in the journal.
GRACE = 1.0

# When the watchdog gives up and ends the process because Dry said nothing at
# all. Comfortably past the 5s navigation timeout plus the 5s probe, with room
# for a cold WebView2 taking its time to open in the first place.
HARD_CAP = 45.0

# The exit code a run that heard nothing leaves behind, so the parent can tell
# silence from a child that died some other way.
CAP_EXIT = 3

FORMAT = '%(levelname)s %(name)s: %(message)s'

note = logging.getLogger('test')


class Heard(logging.Handler):
    """
    Fires when `dry.webview` says something about *this* address.

    Attached to `dry.webview` rather than to the root, so the child's own notes
    do not look like Dry speaking, and matched against the URL because Dry has
    other things to say — it records the window opening at debug — and none of
    those are the verdict this test is waiting for.
    """

    def __init__(self, url: str) -> None:
        super().__init__(level=logging.DEBUG)
        self.url = url
        self.happened = threading.Event()

    def emit(self, record: logging.LogRecord) -> None:
        if self.url in record.getMessage():
            self.happened.set()


def watchdog(heard: Heard) -> None:
    """
    Ends the process once Dry has spoken, or once it is clear it will not.

    `run()` never returns, so nothing else can end this process, and the
    parent's verdict is read off the journal either way.
    """
    started = time.monotonic()
    while True:
        if heard.happened.wait(timeout=0.25):
            time.sleep(GRACE)
            note.info('heard')
            os._exit(0)
        if time.monotonic() - started > HARD_CAP:
            note.info('silent')
            os._exit(CAP_EXIT)


def main() -> int:
    path, url = sys.argv[1], sys.argv[2]

    # `filemode='w'` so a rerun cannot read the last run's journal. Records are
    # flushed by `StreamHandler.emit`, which matters: this process ends with
    # `os._exit` and nothing buffered would survive it.
    logging.basicConfig(
        filename=path,
        filemode='w',
        level=logging.DEBUG,
        format=FORMAT,
        force=True,
    )

    heard = Heard(url)
    logging.getLogger('dry.webview').addHandler(heard)

    from dry import Webview

    webview = Webview(
        title='Dry failed navigation',
        size=(420, 260),
        min_size=(420, 260),
        url=url,
    )

    threading.Thread(target=watchdog, args=(heard,), daemon=True).start()

    note.info('run %s', url)
    webview.run()

    note.info('returned')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
