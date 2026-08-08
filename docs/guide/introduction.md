# Dry

Dry is a Python library that opens **one native window rendering a web
frontend**, plus a Bridge between that frontend and Python. It is written in
Rust on top of [Wry](https://github.com/tauri-apps/wry), ships as a compiled
extension module, and depends on nothing but the Python standard library.

```python
from dry import Webview

wv = Webview(
    app_id='com.example.hello',
    title='Hello',
    html='<h1>Hello from Dry</h1>',
)
wv.run()
```

`wv.run()` opens the window and does not come back. Closing the window exits
the process, so nothing written after it runs.

## What is in scope

Anything a frontend legitimately needs from its host window: the window itself,
its titlebar and edges, what it renders, and a channel to Python. General
application concerns — filesystem access, databases, HTTP clients — are not.
Those are Python's, and the developer wires them through the Bridge.

## The shape of the library

There is one public object, the `Webview`, and one channel, the Bridge. The
Bridge carries exactly two message shapes, and both travel in both directions:

|  | Frontend to Python | Python to frontend |
| --- | --- | --- |
| **Call** — returns a value | `window.dry.api.name(...)` | *deliberately absent*; use `wv.eval_js` |
| **Event** — returns nothing | `window.dry.emit(name, value)` | `wv.emit(name, value)` |

The missing quadrant is a decision, not an omission: a Python-side `await` on
the frontend never resolves if the page navigates or hangs. When the answer
matters, have the frontend Call Python.

## Where to go next

- [Installing](./installing.md) and [your first Webview](./first-webview.md).
- [Content modes](./content.md) — an HTML string, a URL, or a directory.
- [The Bridge](./calls.md) — Calls, Events, and the contract on what may cross.
- [Migrating to 0.4.0](./migration-0.4.md) if you are on 0.3.x. It is a
  breaking release.

The vocabulary used throughout this site — Webview, Bridge, Call, Event, Api,
Portal, Content, Root, Drag region, Resize edge, Bridge contract, Close hook,
App id — is defined in
[`CONTEXT.md`](https://github.com/barradasotavio/dry/blob/master/CONTEXT.md)
and used with exactly those meanings.
