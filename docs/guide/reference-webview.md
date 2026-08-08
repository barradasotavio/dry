# The Webview

One native window rendering a web frontend, and the Bridge to it. Dry's one
public object.

```python
from dry import Webview
```

Every option is keyword-only, and every option is also a property.

## Options

| Option | Type | Default | Meaning |
| --- | --- | --- | --- |
| `title` | `str` | `'My Dry Webview'` | The window title. Cosmetic only |
| `size` | `tuple[int, int]` | `(800, 600)` | Initial dimensions, logical pixels |
| `min_size` | `tuple[int, int]` | `(800, 600)` | Minimum dimensions, logical pixels |
| `decorations` | `bool` | `True` | Native titlebar and borders |
| `icon_path` | `str \| os.PathLike \| None` | `None` | Window icon, `.ico`, Windows only |
| `html` | `str \| None` | `None` | Content: an HTML string |
| `url` | `str \| None` | `None` | Content: an address to load |
| `root` | `str \| os.PathLike \| None` | `None` | Content: a directory to serve |
| `api` | `dict[str, Callable] \| None` | `None` | The names the frontend may Call |
| `dev_tools` | `bool` | `False` | Enable the web inspector |
| `app_id` | `str \| None` | derived | Decides where this application's data lives |
| `user_data_folder` | `str \| os.PathLike \| None` | from `app_id` | Overrides that location outright |
| `default` | `Callable[[Any], Any] \| None` | `None` | Converts a value outside the Bridge contract |
| `on_close` | `Callable[[], object] \| None` | `None` | Asked before the window closes |

Exactly one of `html`, `url` and `root` must be declared. Declaring a second
raises immediately; declaring none raises at `run()`.

Assigning an attribute that is not one of these raises `AttributeError`.
Assigning any of them except `title`, `size`, `min_size`, `decorations` and
`icon_path` after `run()` raises `RuntimeError` — see
[Window options](./window-options.md#what-can-change-after-run).

## Methods

| Method | Does |
| --- | --- |
| `run()` | Opens the window and hands it the process. Never returns |
| `on(name, listener)` | Registers a listener for an Event, and returns the listener |
| `off(name, listener)` | Takes one registration off |
| `emit(name, value=None)` | Emits an Event to the frontend |
| `eval_js(script)` | Evaluates a script in the page, reading nothing back |

`on` and `off` work before `run()`. `emit` and `eval_js` need a running window
and raise a `BridgeError` without one.

Runtime control of the open window — moving it, resizing it, hiding it,
querying its state — is [not in the library yet](./runtime-control.md).

## Read-only behaviour worth knowing

- `wv.root` reads back a resolved `pathlib.Path`, whatever you assigned.
- `wv.icon_path` reads back a POSIX-style `str`.
- `wv.user_data_folder` reads back the folder in use, whether it came from the
  App id or from an override.

## Exceptions

```python
from dry import BridgeError, DryError, PanicError, WebviewError
```

See [Errors and logging](./errors.md).
