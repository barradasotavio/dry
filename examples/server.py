"""Renders a URL served by a local HTTP server.

The server runs in its own *process*, which keeps a crash in the server from
taking the window with it. A `threading.Thread` also works: `run()` releases the
GIL before the event loop takes the main thread, so Python threads keep running
for the life of the window. That was not true before 0.4.0, when `run()` held
the GIL and a threaded server accepted no connection at all.

Most local content wants `wv.root` instead — see `root.py` — and needs no server
at all.
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

    wv = Webview(
        title='Server Example',
        app_id='com.example.dry.server',
        size=(1080, 720),
        min_size=(1080, 720),
        icon_path=ICON_PATH,
        url=f'http://localhost:{PORT}',
        dev_tools=True,
    )
    wv.run()
