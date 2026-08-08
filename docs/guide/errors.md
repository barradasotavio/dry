# Errors and logging

## The exception hierarchy

Everything Dry can fail at raises a `DryError`, so you can catch what you mean:

```python
from dry import BridgeError, DryError, PanicError, Webview, WebviewError
```

(They are also importable from `dry.exceptions`.)

| Exception | Raised for |
| --- | --- |
| `WebviewError` | A window or web content that could not be built |
| `BridgeError` | A message that could not cross to or from the frontend |
| `PanicError` | A bug inside Dry itself, carrying the file and line |

All three are `DryError`, which is an `Exception`.

```python
from dry import Webview, WebviewError

try:
    Webview(html='<h1>Hi</h1>').run()
except WebviewError as error:
    fall_back(error)
```

A `PanicError` means a Rust panic was caught on its way out and turned into
something you can handle rather than an aborted process. The event loop itself
is deliberately outside that net: unwinding through a platform event loop is
not safe to catch.

Values refused by the [Bridge contract](./contract.md) raise the ordinary
`TypeError` and `ValueError` you would expect from `json.dumps`, not a
`BridgeError`.

## A callable that raises

The frontend's Promise rejects with an `Error` whose `name` is the Python
exception's type:

```javascript
try {
    await window.dry.api.loadFile('missing.txt');
} catch (error) {
    if (error.name === 'FileNotFoundError') { /* ... */ }
}
```

## Logging

**Dry writes to no stream of its own.** No `print`, nothing on stdout or
stderr. Its diagnostics go to the `dry` logger and its children, and a
`NullHandler` keeps them silent until your application configures logging:

```python
import logging

logging.basicConfig(level=logging.DEBUG)
```

| Logger | Carries |
| --- | --- |
| `dry` | The parent. Configure this one to catch everything |
| `dry.webview` | The window and the web content: a failed navigation, an unreadable icon |
| `dry.bridge` | Messages crossing the Bridge: a Call that raised, a listener that raised, a close hook that raised, Calls cut short by a close |

## The blank window

A URL Content that does not arrive within five seconds is diagnosed and
reported on `dry.webview`, naming an untrusted certificate, a refused
connection or an unresolvable host.

Read that report for what it is: **a heuristic**. wry has no failed-navigation
hook, so Dry diagnoses the address from Python with `urllib` and `ssl` after
the fact. Python and the platform's web engine have different network stacks,
different certificate stores and different proxy handling, so they can disagree.
Nothing is logged above debug unless the diagnosis reproduces a concrete
failure, so a page that is merely slow is never accused.

A [Root](./root.md) is not watched: it is served from inside this process and
answers its own failures with a `403` or a `404` you can see.
