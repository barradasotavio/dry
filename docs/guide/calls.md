# Calls: the frontend asks Python

A **Call** is a Bridge message that returns a value. The frontend Calls a name
in the Api, and the Promise it gets back resolves with what the Python callable
returned.

```python
from dry import Webview


def hello(name: str) -> str:
    return f'Hello, {name}!'


def add(a: int, b: int) -> int:
    return a + b


wv = Webview(app_id='com.example.myapp', html=HTML, api={'hello': hello, 'add': add})
wv.run()
```

```javascript
const greeting = await window.dry.api.hello('World');  // "Hello, World!"
const sum = await window.dry.api.add(1, 2);            // 3
```

The **Api** is the mapping of names to Python callables the frontend may Call.
The name on the JavaScript side is the key in the dictionary, not the Python
function's own name, so `{'helloWorld': hello_world}` lets each side keep its
own conventions.

Every entry must be callable. One that is not is refused before a window is
built:

```python
Webview(api={'total': 42}, html=HTML).run()
# dry.exceptions.BridgeError: Api entry 'total' is not callable.
```

`functools.partial`, an object with `__call__`, a bound method and most
built-ins all work.

## Sync, async, and why nothing freezes

A callable declared `async def` is scheduled onto the asyncio loop Dry owns.
Anything else runs in a thread pool. Either way it is off the thread drawing
the window, so a slow Call does not freeze the UI and two Calls do not queue
behind one another.

```python
import asyncio


async def fetch_report(id: str) -> dict[str, object]:
    await asyncio.sleep(2)
    return {'id': id, 'rows': []}
```

The consequence is that **your callables run concurrently**, and any state they
share is yours to make thread-safe. See [The Portal](./portal.md) and
[ADR-0001](./decisions/0001.md).

## Arguments are checked against your annotations

A Call arrives as JSON and lands on a callable you wrote. Before it runs, Dry
compares what arrived with what the callable declared:

```
TypeError: save_file expects str for path, received number instead.
TypeError: save_file takes 2 arguments, received 1. data was not passed.
```

The name in the message is the Api key the frontend Called. The expected type
is your annotation as written; the received type is named in JSON's own
vocabulary, because JSON is what the frontend wrote — with Python's name added
where JSON is too coarse to explain the refusal, as when an `int` is handed
`1.5`.

The check is deliberately **shallow**: arity, and the top level of each
argument. `list[int]` asks whether an array arrived, not what is in it. Turning
a dictionary into a dataclass is validation, which is not this library's job.

It is also deliberately **timid**. An annotation Dry cannot resolve, or can
resolve but cannot express in the JSON data model, leaves its parameter
unchecked — including your own classes and dataclasses, and forward references,
and parameters with no annotation at all. A Call wrongly refused would be a bug
in code you cannot reach; a Call wrongly let through is a bug in code you can
see. Every uncertainty resolves towards letting the Call through.

Two judgements worth knowing: `float` accepts an integer, because JSON has one
number type, while `int` does not accept a float; and `bool` is not `int` in
either direction, despite `issubclass(bool, int)`.

## When a callable raises

The Promise rejects with an `Error` whose `name` is the Python exception's
type, so the frontend can tell one failure from another:

```javascript
try {
    await window.dry.api.loadFile('missing.txt');
} catch (error) {
    if (error.name === 'FileNotFoundError') { /* ... */ }
}
```

The exception is also logged, with its traceback, on the `dry.bridge` logger.

A Call always settles. A callable that returns a value outside the [Bridge
contract](./contract.md) rejects with the `TypeError` explaining the way out
rather than leaving the Promise hanging, and a Call that arrives while the
window is closing is rejected rather than left unanswered.

## Python does not Call the frontend

There is no `await wv.call_js(...)`. A Python-side await on the frontend never
resolves if the page navigates away or hangs, and Dry owns the process, so the
consequence would be an application that cannot be closed. When the answer
matters, have the frontend Call Python.

For the cases where a script simply has to run in the page, `wv.eval_js(script)`
evaluates one and reads nothing back.
