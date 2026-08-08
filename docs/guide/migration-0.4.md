# Migrating to 0.4.0

0.4.0 is deliberately breaking. It renames the JavaScript surface, replaces
content sniffing with explicit modes, closes the set of values that may cross
the Bridge, moves where your data is stored, and raises the Python floor.

Everything here is a change a working 0.3.x application can trip over. Each
section says what to do.

---

## 1. Python 3.14 is the floor

`requires-python` is now `>=3.14`, and the wheels are stable-ABI
(`abi3-py314`), one per platform. 3.11, 3.12 and 3.13 are no longer supported;
`pip` on those will not find a 0.4.0 wheel.

Free-threaded builds (`python3.14t`) cannot install Dry either — `abi3` does not
cover them until `abi3t` arrives with Python 3.15.

**What to do:** upgrade the interpreter. Why the floor sits there, and why
lowering it later would break nobody: [ADR-0003](./decisions/0003.md).

---

## 2. The JavaScript surface moved to `window.dry`

Everything Dry injects now hangs off a single global, so nothing collides with
a standard browser API or with your own.

| 0.3.x | 0.4.0 |
| --- | --- |
| `window.api.<name>(...)` | `window.dry.api.<name>(...)` |
| `window.minimize()` | `window.dry.minimize()` |
| `window.toggleMaximize()` | `window.dry.toggleMaximize()` |
| `window.close()` | `window.dry.close()` |
| `window.drag()` | `window.dry.drag()` |
| `window.resize(direction)` | `window.dry.resize(direction)` |
| `window.ipcCallback(...)` | gone from the public surface |
| `window.ipcStore` | gone; the pending-call store is a closure-scoped `Map` |

`window.close`, `window.resize`, `window.resizeTo` and `window.resizeBy` are the
browser's own again — the hijacking is gone. Note that the DOM defines no
`window.resize` method at all, so `window.resize` is now `undefined` and the
`resize` event fires normally. **A page that called `window.close()` meaning
Dry's close now calls the browser's.**

`window.dry` and each of its members are non-writable and non-configurable, so a
page script cannot replace them.

**What to do:** prefix every call with `dry.`. The full surface is in
[the `window.dry` reference](./reference-javascript.md).

---

## 3. Content is explicit: `html`, `url` or `root`

`wv.content` is gone. It sniffed: a string matching `https?://` was a URL, a
string that happened to name an existing file was a file, and anything else was
markup. Now you say which mode you mean.

| 0.3.x | 0.4.0 |
| --- | --- |
| `wv.content = '<h1>Hi</h1>'` | `wv.html = '<h1>Hi</h1>'` |
| `wv.content = 'http://localhost:8000'` | `wv.url = 'http://localhost:8000'` |
| `wv.content = 'C:/app/index.html'` | `wv.html = Path('C:/app/index.html').read_text()`, or `wv.root = 'C:/app'` |

Assigning `wv.content` now raises `AttributeError`, not silently nothing.

There is no single-file mode. `root` takes a **directory** and serves it,
starting at its `index.html`, so relative assets resolve — which is what a
compiled frontend needs and what the old single-file path could not do. A file
passed to `root` raises `NotADirectoryError` naming the alternative.

Declaring two modes raises immediately; declaring none raises at `run()`. In
0.3.x an empty `content` rendered a built-in `<h1>Hello, World!</h1>` — that
placeholder is gone.

**What to do:** [The three Content modes](./content.md),
[Serving a Root](./root.md).

---

## 4. The constructor is keyword-only, and typos raise

0.3.x had no constructor arguments; everything was assigned afterwards. That
still works, and now the same options can be passed to `Webview(...)`:

```python
wv = Webview(title='My App', app_id='com.example.myapp', html=HTML)
```

Two consequences:

- **Positional arguments are refused.** `Webview('My App')` raises `TypeError`.
- **Assigning an unknown attribute raises `AttributeError`** instead of quietly
  creating one that never applies. If you were setting a name Dry never read,
  you will hear about it now.

And a third: **settings read while the window is being built now raise if you
assign them afterwards**, naming the setting, rather than doing nothing. That
is `html`, `url`, `root`, `api`, `dev_tools`, `default`, `app_id`,
`user_data_folder` and `on_close`. See
[Window options](./window-options.md#what-can-change-after-run).

---

## 5. Your data moved, and the title no longer decides where

In 0.3.x the user data folder defaulted to `<temp>/<window title>`. Cookies,
local storage and cache therefore lived in the temporary directory — cleared by
the system whenever it felt like it — renaming the window lost the session, two
applications sharing a title shared a cookie jar, and a title containing a
colon produced a path Windows refuses.

0.4.0 keys the data to an **App id** under the OS's application-data directory:

| Platform | 0.4.0 location |
| --- | --- |
| Windows | `%LOCALAPPDATA%\<app id>` |
| macOS | `~/Library/Application Support/<app id>` |
| Linux | `$XDG_DATA_HOME/<app id>`, or `~/.local/share/<app id>` |

```python
wv = Webview(app_id='com.example.myapp', html=HTML)
```

**Existing sessions do not carry over.** Whatever was in the temporary folder
stays there, and your users log in again once. That is the cost of the fix.

Leave `app_id` out and one is derived from your entry-point script, which is
fine while developing and moves when the script moves. Declare your own before
you ship. `user_data_folder=` still overrides the location outright, and
`wv.title` is now purely cosmetic and may contain any character.

**What to do:** [Where your data lives](./app-data.md).

---

## 6. The Bridge contract refuses what JSON has no room for

The set of values that may cross is now the JSON data model, in both
directions, and a value outside it raises rather than arriving as something you
did not send.

| Value | 0.3.x | 0.4.0 |
| --- | --- | --- |
| `set`, `frozenset` | crossed as an array | raises `TypeError` — pass a `list` |
| `bytes`, `bytearray` | crossed as `number[]` | raises `TypeError` — decode, or base64 to a `str` |
| `True` / `False` | **arrived as `1` / `0`** | arrive as `true` / `false` |
| `int` beyond ±2\*\*53 | crossed, silently losing digits | raises `ValueError`, in both directions |
| `NaN`, `Infinity` | crossed | raise `ValueError` |
| `datetime`, `Decimal`, `Enum`, dataclasses | raised | raise, unless a `default=` hook converts them |
| a `dict` key that is not a `str` | — | coerced to a string exactly as `json.dumps` does |

The boolean row is the one to look for in a working application: a frontend
written against 0.3.x may be comparing `=== 1`, or relying on a number where a
boolean now arrives.

**What to do:** [The Bridge contract](./contract.md). For your own types, hand
the Webview a [`default=` hook](./default-hook.md) — the same one
`json.dumps(default=...)` takes — rather than converting at every call site.

---

## 7. Window sizes are logical pixels

`size` and `min_size` were physical pixels. They are now **logical** pixels,
independent of display scaling, which is the unit CSS works in.

On a display scaled to 200%, `size=(800, 600)` used to give the page a 400×300
CSS viewport. It now gives it 800×600, so **the window will look twice the size
it used to** on a scaled display. Divide your old numbers by the scale factor
you were developing at, or — more usefully — pick the numbers you actually want
the user to see.

---

## 8. A drag region now drags its whole subtree

`data-drag-region` used to match only the element that was clicked, so the
README's own example — a heading inside a drag region — did not drag; only the
bare margin around it did.

Now the whole subtree drags. **A button sitting inside your titlebar will move
the window instead of being clicked** unless you opt it out:

```html
<div data-drag-region>
    <h1>My Application</h1>
    <button data-no-drag-region onclick="window.dry.close()">×</button>
</div>
```

**What to do:** add `data-no-drag-region` to every interactive element inside a
drag region. [Custom titlebars](./titlebar.md).

---

## 9. Failures are exceptions and log records, not printed text

0.3.x printed diagnostics to stdout and stderr, and a Rust panic aborted the
process.

0.4.0 raises `DryError` and its three children — `WebviewError`, `BridgeError`,
`PanicError` — and writes everything else to the `dry` logger and its children
`dry.webview` and `dry.bridge`, silenced by a `NullHandler` until your
application configures logging. **Dry now writes nothing to stdout or stderr at
all**, so anything you were reading off the terminal needs
`logging.basicConfig(...)`.

A callable that raises now rejects the frontend's Promise with an `Error` whose
`name` is the Python exception's type.

**What to do:** [Errors and logging](./errors.md).

---

## 10. Api callables are checked, twice

**Before the window opens:** every entry in `api` must be callable, or
`run()` raises `BridgeError: Api entry '<name>' is not callable.`

**Before each Call runs:** the arguments are checked against the callable's
declared annotations, and a mismatch is refused with a message naming the
parameter — `save_file expects str for path, received number instead.` A 0.3.x
frontend that was passing a string where the Python side declared `int`, and
getting away with it, now gets a rejected Promise.

The check is shallow and timid — arity and the top level of each argument, with
anything it cannot resolve left unchecked — so it will not refuse a Call it
merely does not understand. Details, including how `float`, `int` and `bool`
behave: [Calls](./calls.md#arguments-are-checked-against-your-annotations).

---

## 11. Callbacks run concurrently, off the window's thread

In 0.3.x a callback ran on the thread drawing the window, one at a time, and
the window froze for its duration.

Now an `async def` callable is scheduled onto an asyncio loop Dry owns and
anything else goes into a thread pool. The window stays responsive and two
Calls overlap — which means **state shared between your callables is yours to
make thread-safe**. Code that was implicitly serialised by the old model is not
any more.

Two related changes:

- `run()` no longer holds the GIL, so ordinary Python threads keep running for
  the life of the window. If you moved a local server into a
  `multiprocessing.Process` to work around that, a thread is enough again.
- An application still cannot make `asyncio.run(main())` its entry point. Dry
  owns the process and the loop; your async code lives inside callbacks.

**What to do:** [The Portal](./portal.md), [ADR-0001](./decisions/0001.md).

---

## 12. Closing is ordered, and can be refused

New in 0.4.0, and worth adopting rather than migrating to:

```python
wv = Webview(app_id='com.example.myapp', html=HTML, on_close=save_or_refuse)
```

The close hook is asked on every route in, including `window.dry.close()`, and
returning `False` keeps the window open. After it agrees, in-flight Calls get up
to five seconds, the loop is drained, and **`atexit` handlers now run** — in
0.3.x they never did.

Still true, and still a consequence of Dry owning the process: a `finally:`
around `wv.run()` does not run. [Closing the window](./close-hook.md).

---

## Also new, breaking nothing

- **Events, in both directions.** `wv.on` / `wv.off` / `wv.emit` in Python,
  `window.dry.on` / `off` / `emit` in the frontend. [Events](./events.md)
- **Window Events.** `window:maximized`, `window:resized` and the rest, on the
  same bus, so a custom titlebar can stop guessing.
  [Window Events](./window-events.md)
- **`root`**, a directory served over an internal protocol.
  [Serving a Root](./root.md)
- **Resize edges on macOS**, which tao does not support natively.
  [ADR-0004](./decisions/0004.md)
- **macOS support**, built and tested in CI alongside Windows.
- **A failed navigation is reported** instead of leaving you staring at a blank
  window. [Errors and logging](./errors.md#the-blank-window)
