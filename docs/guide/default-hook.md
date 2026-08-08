# Converting your own types

Rather than converting at every call site, hand the Webview a `default` — the
same hook `json.dumps(default=...)` takes. It is called with any value outside
the [Bridge contract](./contract.md) and must return one inside it.

```python
from dataclasses import asdict, is_dataclass
from datetime import datetime
from decimal import Decimal

from dry import Webview


def default(value):
    if isinstance(value, datetime):
        return value.isoformat()
    if isinstance(value, Decimal):
        return float(value)
    if is_dataclass(value):
        return asdict(value)
    raise TypeError(f'{type(value).__name__} is not JSON serializable')


wv = Webview(app_id='com.example.myapp', html=HTML, api=api, default=default)
wv.run()
```

What the hook returns is checked in turn, so it may return another value the
hook itself handles — a dataclass holding a `Decimal` converts in two steps.

Raise from it for anything you do not want to convert, and the Call rejects, or
the `emit` raises, exactly as it would have without a hook.

## It is consulted last

The hook is the final step, reached only once every other rule has declined the
value. It is therefore **never asked about**:

- a `set`, `frozenset`, `bytes` or `bytearray` — those are refused before it,
  because a silent conversion is what the contract exists to prevent;
- an `int` outside ±2\*\*53, `NaN` or `Infinity`;
- a dictionary **key**. `json.dumps` does not pass keys to `default` either.
  Convert the keys yourself before the dictionary crosses.

## One hook, for everything

`default=` is read while the Webview is being built and applies to every value
Dry sends: Call return values, Event values, in every direction. Assigning it
after `run()` raises.

It applies on the way **out** only. Values arriving from the frontend are
already inside the contract by construction — JSON has nothing else in it.
