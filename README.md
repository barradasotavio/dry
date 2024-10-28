# Dry: a tiny webview library for Python

Dry is an attempt to provide a minimalistic webview library for Python, designed to be as simple as possible and to have no dependencies other than its binary. It is powered by [Rust](https://www.rust-lang.org/), [Maturin](https://github.com/PyO3/maturin) and [PyO3](https://github.com/PyO3/pyo3), being built on top of [Wry](https://github.com/tauri-apps/wry) and [Tao](https://github.com/tauri-apps/tao).

## Installation

Dry installation has been tested with pip and uv:

```bash
pip install dry-webview
uv add dry-webview
```
## Usage

Here is a simple example of how to use Dry:

```python
from dry import Webview

wv = Webview()
wv.title = "Hello, World!"
wv.content =  "https://www.example.com" or "<h1>Hello, World!</h1>"
wv.run()
```

A more complete example can be found in the [`examples`](https://github.com/barradasotavio/dry/tree/master/examples) directory.

## Status

Dry is in early stages of development and it has been tested and compiled only for Windows. There may be bugs or missing functionality. Breaking changes may occur in future releases.

## Roadmap

*Legend*:  
🟢 Already implemented — 🟡 In progress — 🔴 Not started

### Features
- 🟢 Render HTML content
- 🟢 Load HTTP/HTTPS content
- 🟢 Call Python functions from JavaScript
- 🟢 Enable dev mode (dev tools and auto reload)
- 🟡 Customize the title bar icon
- 🔴 Set a custom taskbar icon
- 🔴 Enable JavaScript calls from Python
- 🔴 Allow usage of custom titlebar
- 🔴 Store and manage global state in Python
- 🔴 Support PyInstaller

### Platform Compatibility
- 🟢 Windows support
- 🔴 Linux support
- 🔴 MacOS support

### Python Compatibility
- 🔴 CPython 3.13
- 🟢 CPython 3.12
- 🔴 CPython 3.11