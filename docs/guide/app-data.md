# Where your data lives

Cookies, local storage, IndexedDB and cache belong to an **App id** — a stable
reverse-domain identifier such as `com.example.myapp` — not to the window title.

```python
wv = Webview(app_id='com.example.myapp', html=HTML)
```

Rename the window and the session survives. Two applications that happen to
share a title no longer share a cookie jar. A title containing a colon no
longer produces a path Windows refuses.

The data lands under the directory the operating system keeps application data
in, so nothing clears it between runs:

| Platform | Location |
| --- | --- |
| Windows | `%LOCALAPPDATA%\<app id>` |
| macOS | `~/Library/Application Support/<app id>` |
| Linux | `$XDG_DATA_HOME/<app id>`, or `~/.local/share/<app id>` |

## What an App id may look like

One path segment: letters, digits, dots, dashes and underscores, starting with
a letter or a digit. That constraint is deliberate — nothing you pass can
escape into a parent directory or name a drive:

```python
Webview(app_id='../../etc')
# ValueError: app_id must be one path segment of letters, digits, dots, dashes
# and underscores, starting with a letter or a digit, such as com.example.myapp.
# Got: '../../etc'.
```

## Leaving it out

An App id is derived from your entry-point script:
`dry.<script-stem>.<8 hex characters of the script's absolute path>`. The digest
keeps two different `main.py` files from sharing a cookie jar.

That is enough to develop against and not something to ship: **the folder moves
when the script moves**. Declare your own before you release.

## Overriding the location outright

```python
Webview(app_id='com.example.myapp', user_data_folder='/var/tmp/myapp')
```

`user_data_folder` takes the location out of the App id's hands entirely. It is
rarely what you want; a portable application that keeps its state beside itself
is the case it exists for. `~` is expanded, and the directory is created if it
is not there.

`wv.user_data_folder` reads back the folder in use either way.

Both settings are read while the Webview is being built, so assigning either
after `run()` raises.
