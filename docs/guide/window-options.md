# Window options

```python
from dry import Webview

wv = Webview(
    app_id='com.example.myapp',
    title='My App',
    size=(1080, 720),
    min_size=(800, 600),
    decorations=True,
    icon_path='assets/app.ico',
    dev_tools=True,
    html=HTML,
)
wv.run()
```

## Sizes are logical pixels

`size` and `min_size` are **logical pixels**, independent of display scaling: a
window declared 800 by 600 opens at that apparent size on a display scaled to
150% as on one scaled to 100%, and the page's own `window.innerWidth` reports
the same 800. They are the numbers CSS is working in.

Every size and position Dry reports back — in
[Window Events](./window-events.md) — is in the same unit, for the same reason.

## `decorations`

`decorations=False` removes the native titlebar and borders, and the Webview
draws its own [resize edges](./titlebar.md#resize-edges) instead. `data-drag-region`
is what then moves the window.

## `icon_path`

A path to an `.ico` file — a `str` or any `os.PathLike`. ICO is the only format
the build decodes.

The icon is a Windows feature. macOS has no per-window icon, so the setting has
no effect there.

An icon that cannot be read is a warning on the `dry.webview` logger, not a
failure: the window opens with the platform's default icon.

## `dev_tools`

`dev_tools=True` enables the platform's web inspector. Leave it off in a
release build.

## `title`

Purely cosmetic since 0.4.0. It no longer decides where your data is stored, so
it may contain any character — see [Where your data lives](./app-data.md).

## What can change after `run()`

Every option is also a property, and most of them are read once, while the
Webview is being built. Assigning one of those after `run()` raises a
`RuntimeError` naming the setting, rather than silently doing nothing:

```python
wv.api = {'hello': hello}
# RuntimeError: api is fixed at construction and the Webview is already
# running, so assigning it now would change nothing. Pass api to Webview(...)
# instead.
```

| Setting | After `run()` |
| --- | --- |
| `html`, `url`, `root` | raises |
| `api`, `default` | raises |
| `dev_tools` | raises |
| `app_id`, `user_data_folder` | raises |
| `on_close` | raises |
| `title`, `size`, `min_size`, `decorations`, `icon_path` | assignable, but does not yet move the window — see [Runtime window control](./runtime-control.md) |

Since `run()` never returns, "after `run()`" means from inside a callback: an
Api callable, an Event listener or a close hook.
