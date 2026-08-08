# Dry

**One native window rendering a web frontend, and a Bridge to Python.**

Dry is a small, dependency-free library for building a desktop UI out of HTML,
CSS and JavaScript, driven by Python. It is written in
[Rust](https://www.rust-lang.org/) on top of
[Wry](https://github.com/tauri-apps/wry), and renders with the web engine the
operating system already ships — WebView2 on Windows, WKWebView on macOS.

📖 **[Documentation](https://barradasotavio.github.io/dry/)**

## Installation

```bash
pip install dry-webview
```

Requires CPython 3.14 or newer, on Windows or macOS.

## Getting started

```python
from dry import Webview

HTML = """
<button onclick="greet()">Greet</button>
<p id="out"></p>
<script>
  async function greet() {
    out.textContent = await window.dry.api.hello('World');
  }
</script>
"""


def hello(name: str) -> str:
    return f'Hello, {name}!'


wv = Webview(
    app_id='com.example.hello',
    title='Hello',
    html=HTML,
    api={'hello': hello},
)
wv.run()
```

`wv.run()` opens the window and hands it the process: closing the window exits
the interpreter, so nothing written after it runs.

Render an HTML string, a URL, or a directory of compiled assets. Call Python
from JavaScript and get a `Promise` back. Emit Events in either direction.
Drop the native titlebar and draw your own. More examples are in
[`examples/`](https://github.com/barradasotavio/dry/tree/master/examples).

## Documentation

Everything else lives on the [documentation
site](https://barradasotavio.github.io/dry/):

- [Content modes](https://barradasotavio.github.io/dry/content.html) — an HTML
  string, a URL, or a served directory
- [The Bridge](https://barradasotavio.github.io/dry/calls.html) — Calls,
  Events, and the contract on what may cross
- [Custom titlebars](https://barradasotavio.github.io/dry/titlebar.html) and
  [window Events](https://barradasotavio.github.io/dry/window-events.html)
- [Errors and logging](https://barradasotavio.github.io/dry/errors.html)
- [Reference](https://barradasotavio.github.io/dry/reference-webview.html)
- [Architecture decisions](https://barradasotavio.github.io/dry/decisions/0001.html)

## Upgrading from 0.3.x

**0.4.0 is a breaking release.** The JavaScript surface moved under
`window.dry`, content modes are explicit, sizes are logical pixels, the Bridge
contract is now strictly the JSON data model, application data moved to an App
id, and the Python floor is 3.14. Each break, and what to do about it, is in
the [migration
guide](https://barradasotavio.github.io/dry/migration-0.4.html). The
[changelog](https://github.com/barradasotavio/dry/blob/master/CHANGELOG.md) has
the full list.

## Platform support

| Platform | Status |
| --- | --- |
| Windows (x86-64) | Supported. Built and tested on every commit |
| macOS (universal2) | Supported. Built and tested on every commit |
| Linux | Not supported |

| Python | Status |
| --- | --- |
| CPython 3.14 and newer | Supported, via a single `abi3` wheel per platform |
| CPython 3.13 and older | Not supported |
| Free-threaded builds | Not supported until `abi3t` |

## Contributing

Issues and specs live as
[GitHub issues](https://github.com/barradasotavio/dry/issues). Building from
source needs a Rust toolchain and [maturin](https://www.maturin.rs/):

```bash
uv run maturin develop --uv
uv run python -m unittest discover -s tests
cargo test
```

The documentation site is Markdown under `docs/guide`, built with
[mdBook](https://rust-lang.github.io/mdBook/): `mdbook serve docs` previews it.

## License

MIT. See
[LICENSE](https://github.com/barradasotavio/dry/blob/master/LICENSE).
