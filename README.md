# Dry: a simple webview library

**Dry** is a tiny, no-dependency webview library that lets you use your web development skills to create user interfaces for your Python applications. Built with [Rust](https://www.rust-lang.org/) on top of [Wry](https://github.com/tauri-apps/wry).

## Why?

-   **Familiar Tech**: Use HTML, CSS and JS to design your UIs!
-   **Explicit Content**: Render an HTML string, a URL, or a directory of files.
-   **Customizable**: Support for borderless windows with custom titlebars!
-   **Callbacks**: Interact with Python from JavaScript!

## Installation

Getting started with Dry is straightforward. Simply use `pip` or `uv` to install:

```bash
pip install dry-webview
uv add dry-webview
```

## Getting Started

Here's a quick example of how to use Dry to create a simple webview:

```python
from dry import Webview

wv = Webview(
    title="My Python App!",
    app_id="com.example.myapp",
    html="<h1>Hello, World!</h1>",
)
wv.run()
```

Every option is a keyword argument, so your editor tells you what there is and a typo raises instead of quietly doing nothing. Each one is also a property, for values you only work out later:

```python
wv = Webview(app_id="com.example.myapp")
wv.html = render_page()
wv.run()
```

For more examples, check out the [examples directory](https://github.com/barradasotavio/dry/tree/master/examples).

## Features

### Explicit Content

A `Webview` renders exactly one content, declared as exactly one of three mutually exclusive options. Declaring more than one, or none, raises.

```python
from dry import Webview
from pathlib import Path

# An HTML string
Webview(html="<h1>Hello, World!</h1>")

# A URL
Webview(url="http://localhost:8000")

# A root: a directory served to the webview, starting at its index.html
Webview(root=Path(__file__).parent / "dist")
```

`wv.root` is what a compiled frontend wants. The directory is served over an internal protocol, so relative assets — `./assets/index.js`, `<img src="logo.png">` — resolve against it, each file with the content type its extension implies. A request that would resolve outside the directory is refused, and a request for a file that is not there returns a 404 your frontend can observe. Both `wv.root` and `wv.icon_path` accept a `str` or any `os.PathLike`.

#### Serving from a local server

Most people reaching for a local server want `wv.root`, which serves the same directory with no server to start and no port to pick. Reach for a server only when you already have one — a dev server with hot reload, or an application that genuinely speaks HTTP.

When you do, run it in a separate **process**, not a thread. `wv.run()` holds the GIL for as long as the window is open, so a server started in a `threading.Thread` never gets to run: it accepts no connection while the Webview is up, and the window simply stays blank.

```python
from multiprocessing import Process
from dry import Webview

def serve_files():
    # Your server logic here, blocking

if __name__ == "__main__":

    server = Process(target=serve_files, daemon=True)
    server.start()

    wv = Webview(url="http://localhost:8000")
    wv.run()
```

A daemon process shuts down along with the parent. See [`examples/server.py`](https://github.com/barradasotavio/dry/tree/master/examples/server.py) for a working one.

A plain `http://` address loads on macOS as it does on Windows — App Transport Security does not stand between a Python process and its own local server. What does fail there, and fails silently, is **HTTPS with a certificate the system does not trust**: WKWebView abandons the navigation with no prompt to accept the certificate and no error you can see, leaving a blank window. Serve local development over plain HTTP on `localhost` rather than behind a self-signed certificate.

### Custom Titlebar

Dry supports custom titlebars, allowing you to create a unique look for your application. You tell the `Webview` class to hide decorations like this:

```python
wv = Webview(decorations=False, html=HTML)
```

And then you can use `data-drag-region` to define the draggable area in your HTML, which will probably be your custom titlebar:

```html
<div data-drag-region>
    <h1>Custom Titlebar</h1>
</div>
```

A window without decorations will automatically be draggable within the `data-drag-region` area, having resize handles automatically positioned at all corners.

The whole subtree under a drag region drags, so the heading above moves the window just like the bare margin around it does. Mark an element with `data-no-drag-region` to opt it and its own subtree out — which is what buttons living inside a titlebar want, so a click reaches them instead of moving the window:

```html
<div data-drag-region>
    <h1>Custom Titlebar</h1>
    <button data-no-drag-region onclick="window.dry.close()">Close</button>
</div>
```

Everything Dry exposes to the frontend lives on a single global, `window.dry`, so it never collides with a standard browser API or with your own globals.

With or without decorations, basic window controls are available from the DOM, allowing you to minimize, maximize and close window. More are to come in the future.

```html
<button onclick="window.dry.minimize()">Minimize</button>
<button onclick="window.dry.toggleMaximize()">Maximize</button>
<button onclick="window.dry.close()">Close</button>
```

#### Window events

A titlebar that only commands the window keeps its own guess at whether the window is maximised, and that guess is wrong the first time someone double-clicks the bar or uses an OS keyboard shortcut. So the window reports what it is doing, as ordinary Events under names Dry reserves for itself:

```html
<script>
    window.dry.on('window:maximized', () => icon.src = RESTORE);
    window.dry.on('window:unmaximized', () => icon.src = MAXIMIZE);
    window.dry.on('window:resized', ({ width, height }) => show(width, height));
</script>
```

| Name | Value |
| --- | --- |
| `window:maximized`, `window:unmaximized` | none |
| `window:minimized`, `window:restored` | none |
| `window:hidden`, `window:shown` | none |
| `window:focused`, `window:blurred` | none |
| `window:resized` | `{width, height}` |
| `window:moved` | `{x, y}` |
| `window:close-requested` | none, and it is a notification rather than a vote — `on_close` is what can refuse a close |

Every one of them fires for a change the user made as much as for one your application made. Sizes and positions are logical pixels, the same unit `size=` and `min_size=` are given, so they are the numbers CSS is working in. `window:resized` and `window:moved` are emitted at most once per turn of the event loop and only when the value actually changed, so a drag does not flood your listeners.

The same names work from Python, on the same bus as any Event of your own:

```python
wv.on('window:resized', lambda size: print(size['width'], size['height']))
```

A name beginning with `window:` belongs to Dry: listen for one as much as you like, but `wv.emit` and `window.dry.emit` refuse it, so a listener for one knows it is hearing from the window and nothing else.

### Callbacks

You can use callbacks to interact with Python from JavaScript. You define them like this:

```python
def hello_world():
    return "Hello, World!"

def dumb_sum(a, b):
    return a + b

api = {
    "helloWorld": hello_world,
    "dumbSum": dumb_sum
}

wv = Webview(api=api, html=HTML)
wv.run()
```

And then you can call them from JavaScript as follows:

```javascript
const hello = await window.dry.api.helloWorld();
const sum = await window.dry.api.dumbSum(1, 2);

console.log(hello); // Hello, World!
console.log(sum); // 3
```

Values crossing the bridge are exactly the JSON data model, with `json.dumps`
and `json.loads` semantics. Anything outside it raises, rather than arriving as
something you did not send:

| Python Type | JavaScript Type |
| ----------- | --------------- |
| None        | null            |
| bool        | boolean         |
| int         | number          |
| float       | number          |
| str         | string          |
| list        | array           |
| tuple       | array           |
| dict        | object          |

A few consequences worth knowing:

-   A `tuple` is written as an array, so a round trip returns a `list`.
-   Dictionary keys are coerced to strings, exactly as `json.dumps` coerces
    them, so a round trip returns string keys. Only `str`, `int`, `float`,
    `bool` and `None` may be keys.
-   An `int` outside ±2^53 raises, because JavaScript would read it with digits
    missing. `NaN` and `Infinity` raise, being outside JSON.
-   `set` and `bytes` raise: JSON has neither, and neither survives the round
    trip. Pass a `list`, or a `str`.
-   `datetime`, `Decimal`, `Enum`, dataclasses and anything else raise unless
    you convert them yourself.

#### Converting your own types

Rather than converting at every call site, hand the `Webview` a `default`, the
same hook `json.dumps(default=...)` takes. It is called with any value outside
the contract and must return one inside it:

```python
from dataclasses import asdict, is_dataclass
from datetime import datetime
from decimal import Decimal

def default(value):
    if isinstance(value, datetime):
        return value.isoformat()
    if isinstance(value, Decimal):
        return float(value)
    if is_dataclass(value):
        return asdict(value)
    raise TypeError(f"{type(value).__name__} is not JSON serializable")

wv = Webview(html=HTML, api=api, default=default)
```

What it returns is checked in turn, so a hook may return another value the hook
itself handles. Raise from it for anything you do not want to convert, and the
Call rejects as it would have without a hook. It is the last thing consulted,
so it is never asked about a `set`, `bytes` or a dictionary key — those are
refused before it, for the reasons above.

### Errors and Logging

Everything Dry can fail at raises a `DryError`, so you can catch what you mean:

```python
from dry import Webview
from dry.exceptions import BridgeError, DryError, PanicError, WebviewError
```

`WebviewError` covers a window or web content that could not be built, `BridgeError` a message that could not cross to or from the frontend, and `PanicError` a bug inside Dry itself. All three are `DryError`.

A callback that raises rejects the JavaScript promise with an `Error` whose `name` is the Python exception's type, so the frontend can tell one failure from another:

```javascript
try {
    await window.dry.api.load_file('missing.txt');
} catch (error) {
    if (error.name === 'FileNotFoundError') { /* ... */ }
}
```

Dry writes to no stream of its own. Its diagnostics go to the `dry` logger, and its children `dry.webview` and `dry.bridge`, and stay silent until your application configures logging:

```python
import logging

logging.basicConfig(level=logging.DEBUG)
```

### Where your application's data lives

Cookies, local storage and cache belong to an **app id** — a stable
reverse-domain identifier such as `com.example.myapp` — not to the window
title. Rename the window and the session survives; two applications that
happen to share a title no longer share a cookie jar; a title containing a
colon no longer produces a path Windows refuses.

```python
wv = Webview(app_id="com.example.myapp", html=HTML)
```

The data lands under the directory the operating system keeps application data
in, so nothing clears it between runs:

| Platform | Location                                                |
| -------- | ------------------------------------------------------- |
| Windows  | `%LOCALAPPDATA%\<app id>`                               |
| macOS    | `~/Library/Application Support/<app id>`                |
| Linux    | `$XDG_DATA_HOME/<app id>`, or `~/.local/share/<app id>` |

Leave `app_id` out and one is derived from your entry-point script, which is
enough to develop against but not something to ship: declare your own before
you release, or the folder moves when your script does. `user_data_folder=`
overrides the location outright, and is rarely what you want.

### Other options

| Option           | Description                                                              |
| ---------------- | ------------------------------------------------------------------------ |
| title            | The window title. Defaults to 'My Dry Webview'.                          |
| size             | Initial window dimensions in logical pixels. Defaults to (800, 600).     |
| min_size         | Minimum window dimensions in logical pixels. Defaults to (800, 600).     |
| decorations      | Whether to show window decorations (title bar, borders).                 |
| icon_path        | Path to the window icon file (.ico format).                              |
| html             | An HTML string to render.                                                |
| url              | A URL to load.                                                           |
| root             | A directory to serve, starting at its index.html.                        |
| api              | JavaScript-accessible Python functions.                                  |
| dev_tools        | Whether to enable developer tools.                                       |
| app_id           | The identifier deciding where this application's data lives.             |
| user_data_folder | Where that data is stored, overriding what the app id chooses.           |
| default          | Converts a value outside the Bridge contract, as `json.dumps` does.      |

Dimensions are **logical pixels**, independent of display scaling: a window
declared as 800 by 600 opens at that apparent size on a display scaled to 150%
as on one scaled to 100%.

The options the `Webview` reads only while it is being built — the content,
`api`, `dev_tools`, `app_id`, `user_data_folder` and `default` — raise if you
assign them after `run()`, naming the one you assigned, rather than silently
doing nothing.

## Current Status

Dry is in its early stages and currently supports Windows only. Expect ongoing development, new features, and potential changes.

## Platform Compatibility

| Platform   | Status     |
| ---------- | ---------- |
| Windows 11 | ✅ Tested  |
| Linux      | ❌ Not Yet |
| macOS      | ❌ Not Yet |

## Python Compatibility

| Python Version | Status    |
| -------------- | --------- |
| CPython 3.11   | ✅ Tested |
| CPython 3.12   | ✅ Tested |
| CPython 3.13   | ✅ Tested |

## License

Dry is distributed under the MIT License. For more details, see the [LICENSE](https://github.com/barradasotavio/dry/blob/master/LICENSE) file.
