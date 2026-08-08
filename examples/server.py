"""Renders a URL served by a local HTTP server.

The server runs in its own *process*, not a thread: `wv.run()` holds the GIL for
as long as the window is open, so a server in a `threading.Thread` would never
accept a single connection. Most local content wants `wv.root` instead — see
`root.py` — and needs no server at all.
"""

from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from multiprocessing import Process
from pathlib import Path

from dry import Webview

ICON_PATH = Path(__file__).parent / 'icon.ico'
ROOT_PATH = Path(__file__).parent / 'root'
PORT = 8000


def serve() -> None:
    handler = partial(SimpleHTTPRequestHandler, directory=str(ROOT_PATH))
    ThreadingHTTPServer(('127.0.0.1', PORT), handler).serve_forever()


if __name__ == '__main__':
    server = Process(target=serve, daemon=True)
    server.start()

    wv = Webview()
    wv.title = 'Server Example'
    wv.size = wv.min_size = (1080, 720)
    wv.icon_path = ICON_PATH
    wv.url = f'http://localhost:{PORT}'
    wv.dev_tools = True
    wv.run()
