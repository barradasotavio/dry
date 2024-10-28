# Dry: a tiny webview library for Python

Dry is an attempt to provide a minimalistic webview library for Python, built on top of [Wry](https://github.com/tauri-apps/wry). It is designed to be as simple as possible, with a focus on ease of use and minimal dependencies.

## Installation

Dry can be installed using pip:

```bash
pip install dry-webview
```

## Usage

Here is a simple example of how to use Dry:

```python
from dry import WebView

webview = WebView()
webview.title = "Hello, World!"
webview.content = "<h1>Hello, World!</h1>"
webview.run()
```

A more complete example can be found in the `examples` directory.

## Status

Dry is currently in the early stages of development and it has been tested and built only for Windows. It is not yet feature-complete, and there may be bugs or missing functionality. Breaking changes may occur in future releases. Use at your own risk!