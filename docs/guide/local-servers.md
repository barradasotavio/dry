# Loading a URL from a local server

Most people reaching for a local server want a [Root](./root.md), which serves
the same directory with no server to start and no port to pick. Reach for a
server when you already have one: a dev server with hot reload, or an
application that genuinely speaks HTTP.

```python
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from multiprocessing import Process
from pathlib import Path

from dry import Webview

ROOT = Path(__file__).parent / 'dist'
PORT = 8000


def serve() -> None:
    handler = partial(SimpleHTTPRequestHandler, directory=str(ROOT))
    ThreadingHTTPServer(('127.0.0.1', PORT), handler).serve_forever()


if __name__ == '__main__':
    server = Process(target=serve, daemon=True)
    server.start()

    wv = Webview(app_id='com.example.myapp', url=f'http://localhost:{PORT}')
    wv.run()
```

A daemon process goes down with its parent, so closing the window stops the
server. The full file is
[`examples/server.py`](https://github.com/barradasotavio/dry/tree/master/examples/server.py).

## A thread works too, since 0.4.0

`run()` releases the GIL before the event loop takes the main thread and never
takes it back, so ordinary Python threads keep running for the whole life of
the window — there is a regression test that opens a real window and counts a
thread's ticks inside it.

Before 0.4.0 that was not true: `run()` held the GIL, every Python thread in
the process stopped, and a server started in a `threading.Thread` accepted no
connection while the window was up. If you carried a `multiprocessing.Process`
around to work past that, a `threading.Thread` is now enough:

```python
from threading import Thread

Thread(target=serve, daemon=True).start()
```

A separate process still buys isolation — a crashing server does not take the
window with it — which is why the shipped example uses one.

## Serve plain HTTP, not a self-signed HTTPS

Plain `http://` loads on macOS exactly as it does on Windows. App Transport
Security does not stand between a Python process and its own local server: an
interpreter run from a terminal has no app bundle and no `Info.plist`, so there
is no policy to apply. This was measured, including against a real DNS hostname
rather than only `localhost` and raw IPs.

**HTTPS with a certificate the system does not trust is what fails.** WKWebView
abandons the navigation with no prompt and no visible error. Dry now notices —
after five seconds without a page it diagnoses the address from Python and
writes what it found to the `dry.webview` logger — but the window is still
blank. Serve local development over plain HTTP on `localhost`.

See [Errors and logging](./errors.md) for how to see that diagnosis.
