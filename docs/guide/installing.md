# Installing

```bash
pip install dry-webview
```

```bash
uv add dry-webview
```

Dry has no Python dependencies. The wheel carries the compiled extension
module, so there is no Rust toolchain to install and nothing to build.

## Python

**CPython 3.14 or newer.** Dry ships stable-ABI (`abi3-py314`) wheels, one per
platform rather than one per Python version, so the same wheel keeps working on
every later CPython. The floor buys two things the library relies on: modern
typing with no `typing_extensions` dependency, and PEP 649 deferred
annotations, which are what let Dry read a callback's declared signature and
report `save_file expects str for path, received number instead` rather than a
raw failure from inside the Bridge. See
[ADR-0003](./decisions/0003.md).

Free-threaded builds (`python3.14t`) cannot install Dry: `abi3` does not cover
them. That changes when `abi3t` arrives with Python 3.15.

## Platforms

| Platform | Wheel | Status |
| --- | --- | --- |
| Windows (x86-64) | `win_amd64` | Built and tested on every commit |
| macOS (Intel and Apple silicon) | `macosx_universal2` | Built and tested on every commit |
| Linux | none | Not supported |

Both platforms run the full test suite in CI, including tests that open a real
window and drive it. Linux has no wheel, no CI job and no backend work: it is
not that it is untested, it is that it is not there.

Dry uses the platform's own web engine — WebView2 on Windows, WKWebView on
macOS — so the rendering engine is the one the operating system ships and
updates.

## From source

Building needs a Rust toolchain and [maturin](https://www.maturin.rs/):

```bash
git clone https://github.com/barradasotavio/dry
cd dry
uv run maturin develop --uv
```

That compiles the extension module and installs it into the environment, after
which `python examples/minimal.py` opens a window.
