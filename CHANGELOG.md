# Changelog

## 0.4.0 (unreleased)

A deliberately breaking release. Every break is covered, with what to do about
it, in the [migration guide](https://barradasotavio.github.io/dry/migration-0.4.html).

### Breaking

- **Python 3.14 is the floor.** Wheels are stable-ABI (`abi3-py314`), one per
  platform; 3.11–3.13 are no longer supported, and free-threaded builds cannot
  install Dry until `abi3t`.
- **The JavaScript surface is namespaced under `window.dry`.** `window.api`,
  `window.minimize`, `window.toggleMaximize`, `window.close`, `window.drag` and
  `window.resize` move under it; `window.ipcCallback` and `window.ipcStore` are
  gone from the public surface. `window.close`, `window.resizeTo` and
  `window.resizeBy` are the browser's own again.
- **Content is explicit.** `wv.content` and its sniffing are replaced by three
  mutually exclusive modes: `html`, `url` and `root`. Declaring two raises;
  declaring none raises at `run()`. There is no single-file mode and no
  built-in placeholder page.
- **`Webview(...)` takes keyword arguments**, and assigning an unknown
  attribute raises. Settings read while the window is built raise if assigned
  after `run()`, naming the setting.
- **An App id decides where data lives**, replacing a folder derived from the
  window title under the temporary directory. Cookies, local storage and cache
  move to the OS application-data directory, and existing sessions do not carry
  over. `title` is now cosmetic.
- **The Bridge contract is the JSON data model**, in both directions. `set`,
  `frozenset`, `bytes` and `bytearray` raise; integers beyond ±2**53, `NaN` and
  `Infinity` raise; booleans arrive as booleans rather than as `1` and `0`;
  dictionary keys are coerced to strings as `json.dumps` coerces them.
- **`size` and `min_size` are logical pixels**, so a window on a scaled display
  opens at the size it declares rather than at that size divided by the scale
  factor.
- **A drag region drags its whole subtree.** Interactive elements inside one
  need `data-no-drag-region`.
- **Failures are exceptions and log records.** `DryError`, `WebviewError`,
  `BridgeError` and `PanicError` replace printed diagnostics and an aborting
  panic; Dry writes nothing to stdout or stderr, logging instead to `dry`,
  `dry.webview` and `dry.bridge`.
- **Api entries must be callable**, checked before the window opens, and a
  Call's arguments are checked against the callable's declared annotations
  before it runs.
- **Callbacks run off the window's thread and concurrently**, so state shared
  between them must be thread-safe.

### Added

- **Events in both directions**: `wv.on`, `wv.off`, `wv.emit` and
  `wv.eval_js` in Python; `window.dry.on`, `off` and `emit` in the frontend.
- **Window Events** under reserved `window:` names — `maximized`,
  `unmaximized`, `minimized`, `restored`, `hidden`, `shown`, `focused`,
  `blurred`, `resized`, `moved`, `close-requested` — delivered on the same bus
  to both sides, and fired for OS-initiated changes as much as for
  library-initiated ones.
- **A Root**: a local directory served over an internal protocol so relative
  assets resolve, with per-extension content types, `index.html` for
  directories, `403` for a path escaping the Root and `404` for a missing file.
- **A close hook**: `on_close`, asked on every route in, able to refuse a close
  by returning `False`, followed by an ordered shutdown that drains in-flight
  Calls and runs `atexit` handlers.
- **A `default=` hook**, the one `json.dumps(default=...)` takes, for
  converting your own types on the way out.
- **`data-no-drag-region`**, opting an element and its subtree out of a drag
  region.
- **macOS support**, built and tested in CI alongside Windows, including resize
  edges on an undecorated window, which tao does not support natively.
- **A documentation site**, and a README that is a README again.

### Fixed

- `run()` no longer holds the GIL, so Python threads keep running for the life
  of the window. A local server in a `threading.Thread` works.
- A failed navigation is diagnosed and reported on `dry.webview` instead of
  leaving a blank window with no explanation.
- A `localfile://` request no longer ignores its path and answers with a
  content type that is not a media type.
- A window declared 800×600 opens at that apparent size on a display of any
  scale factor.

---

Releases before 0.4.0 predate this changelog. Their history is in the
[commit log](https://github.com/barradasotavio/dry/commits/master).
