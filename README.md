# Dry: a simple webview library

**Dry** is a tiny, no-dependency webview library that lets you use your web development skills to create user interfaces for your Python applications. Built with [Rust](https://www.rust-lang.org/) on top of [Wry](https://github.com/tauri-apps/wry).

## Why?

-   **Familiar Tech**: Use HTML, CSS and JS to design your UIs!
-   **Flexible Content**: Render from an HTML string or from a URL.
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

wv = Webview()
wv.title = "My Python App!"
wv.content = "<h1>Hello, World!</h1>"
wv.run()
```

For more examples, check out the [examples directory](https://github.com/barradasotavio/dry/tree/master/examples).

## Features

### Flexible Content

The `Webview` class supports loading content from a string containing HTML, from a URL or using the file URI scheme. You could, for example, compile your HTML, CSS and JS into a single file and load it into the webview.

```python
from dry import Webview
from pathlib import Path

# Using file URI scheme
html = Path(__file__).parent / "index.html"
wv.content = html.resolve().as_uri()

# Using a URL
wv.content = "http://localhost:8000"

# Using a string
wv.content = "<h1>Hello, World!</h1>"
```

If your UI needs to come from a server, know that `wv.run()` blocks the main thread. Consider running the server from a separate thread (preferably a daemon one, which will shutdown along with the main thread).

```python
import threading
from dry import Webview

def serve_files():
    # Your server logic here

if __name__ == "__main__":

    thread = threading.Thread(target=serve_files, daemon=True)
    thread.start()

    wv = Webview()
    wv.content = "http://localhost:8000"
    wv.run()
```

### Custom Titlebar

Dry supports custom titlebars, allowing you to create a unique look for your application. You tell the `Webview` class to hide decorations like this:

```python
wv.decorations = False
```

And then you can use `data-drag-region` to define the draggable area in your HTML, which will probably be your custom titlebar:

```html
<div data-drag-region>
    <h1>Custom Titlebar</h1>
</div>
```

A window without decorations will automatically be draggable within the `data-drag-region` area, having resize handles automatically positioned at all corners.

With or without decorations, basic window controls are available from the DOM, allowing you to minimize, maximize and close window. More are to come in the future.

```html
<button onclick="window.minimize()">Minimize</button>
<button onclick="window.toggleMaximize()">Maximize</button>
<button onclick="window.close()">Close</button>
```

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

wv = Webview()
wv.api = api
wv.run()
```

And then you can call them from JavaScript as follows:

```javascript
const hello = await window.api.helloWorld();
const sum = await window.api.dumbSum(1, 2);

console.log(hello); // Hello, World!
console.log(sum); // 3
```

Be aware of the supported data types for function arguments and return values:

| Python Type | JavaScript Type |
| ----------- | --------------- |
| None        | null            |
| bool        | boolean         |
| int         | number          |
| float       | number          |
| str         | string          |
| list        | array           |
| tuple       | array           |
| set         | array           |
| dict        | object          |
| bytes       | number[]        |

### Other

The `Webview` class has a few options you can set through its properties:

| Property         | Description                                              |
| ---------------- | -------------------------------------------------------- |
| title            | The window title. Defaults to 'My Dry Webview'.          |
| min_size         | Minimum window dimensions (width, height).               |
| size             | Initial window dimensions (width, height).               |
| decorations      | Whether to show window decorations (title bar, borders). |
| icon_path        | Path to the window icon file (.ico format).              |
| content          | HTML string or file:///, http:// and https:// locations  |
| api              | JavaScript-accessible Python functions.                  |
| dev_tools        | Whether to enable developer tools.                       |
| user_data_folder | Path to store user data. Defaults to temp folder.        |

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
