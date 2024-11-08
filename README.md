# Dry: give a UI to your Python app using web tech

**Dry** is a minimal, no-dependency webview library for Python that lets you use your web development skills to create user interfaces for your Python applications. Built with [Rust](https://www.rust-lang.org/) and leveraging [Wry](https://github.com/tauri-apps/wry) and [Tao](https://github.com/tauri-apps/tao), Dry is designed to be simple, lightweight, and easy to integrate—no need for extra packages.

## Why Choose Dry?

- **Familiar Tech**: Use HTML, CSS, and JavaScript to design your UIs!
- **Simple API**: wv = Webview(), set a few things up, wv.run()—that's it!
- **Flexible Content**: Load via HTTP/HTTPS or render HTML without a server.
- **Customizable**: Borderless windows with custom titlebars are supported.
- **Small Size**: Binaries ~800 KB, compressed ~300 KB with PyInstaller/UPX.

## Installation

Getting started with Dry is straightforward. Simply use `pip` or `uv` to install:

```bash
pip install dry-webview
uv add dry-webview
```

## Simple, Intuitive Usage

Here's a quick example of how to use Dry to create a simple webview:

```python
from dry import Webview

wv = Webview()
wv.title = "My Python App!"
wv.content = "<h1>Hello, World!</h1>"
wv.run()
```

For more examples, check out the [examples directory](https://github.com/barradasotavio/dry/tree/master/examples).

## Current Status

Dry is in its early stages and currently supports Windows. Linux and macOS support are planned. Expect ongoing development, new features, and potential changes as the library evolves.

## Roadmap

| Feature                             | Status      |
| ----------------------------------- | ----------- |
| Rendering HTML                      | ✅ Completed |
| Loading HTTP/HTTPS                  | ✅ Completed |
| Calling Python from JavaScript      | ✅ Completed |
| Browser Developer Tools             | ✅ Completed |
| Custom Titlebars                    | ✅ Completed |
| Custom Icons                        | ✅ Completed |
| PyInstaller Support                 | ✅ Completed |
| Calling JavaScript from Python      | ❌ Not Yet   |
| Touch Support in Borderless Windows | ❌ Not Yet   |

## Platform Compatibility

| Platform   | Status    |
| ---------- | --------- |
| Windows 11 | ✅ Tested  |
| Linux      | ❌ Not Yet |
| macOS      | ❌ Not Yet |

## Python Compatibility

| Python Version | Status    |
| -------------- | --------- |
| CPython 3.11   | ❌ Not Yet |
| CPython 3.12   | ✅ Tested  |
| CPython 3.13   | ❌ Not Yet |

## License

Dry is distributed under the MIT License. For more details, see the [LICENSE](https://github.com/barradasotavio/dry/blob/master/LICENSE) file.
