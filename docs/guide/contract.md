# The Bridge contract

The **Bridge contract** is the closed set of values that may cross the Bridge:
the JSON data model, with `json.dumps` / `json.loads` semantics. A value outside
it raises rather than being converted into something you did not send.

It applies to everything that crosses, in both directions — a Call's arguments
and its return value, an Event's value.

| Python | JavaScript |
| --- | --- |
| `None` | `null` |
| `bool` | `boolean` |
| `int` | `number` |
| `float` | `number` |
| `str` | `string` |
| `list` | `array` |
| `tuple` | `array` |
| `dict` | `object` |

Coming back the other way, a JSON `number` arrives in Python as an `int` if it
is whole and a `float` if it is not, an `array` as a `list`, and an `object` as
a `dict`.

## The consequences worth knowing

- **A `tuple` is written as an array**, so a round trip returns a `list`.
- **A `dict` keeps its insertion order** across the Bridge.
- **Dictionary keys are coerced to strings**, exactly as `json.dumps` coerces
  them, so a round trip returns string keys. Only `str`, `int`, `float`, `bool`
  and `None` may be keys; anything else raises.
- **An `int` outside ±2\*\*53 raises**, in both directions, because JavaScript
  would read it with digits missing. Pass it as a `str` if you need the digits.
- **`NaN` and `Infinity` raise.** JSON has neither.
- **`set`, `frozenset`, `bytes` and `bytearray` raise.** JSON has none of them
  and none survives the round trip. Pass a `list`, or a `str` — base64 if the
  bytes are binary.
- **`datetime`, `Decimal`, `Enum`, dataclasses and everything else raise**
  unless you convert them, which is what the
  [`default=` hook](./default-hook.md) is for.
- A value nested deeper than 128 levels, or holding a circular reference,
  raises rather than recursing.

`True` crosses as `true` and not as `1`. That sounds too obvious to state, and
it is stated because it was not true before 0.4.0.

## What a refusal looks like

The message says what was refused and how to get out of it:

```python
>>> wv.emit('files', {'a', 'b'})
TypeError: set is outside the Bridge contract: JSON has no set, and a set does
not survive the round trip. Pass a list instead.

>>> wv.emit('id', 2**60)
ValueError: 1152921504606846976 is outside the Bridge contract: a JSON number
carries whole numbers only up to ±2**53, and the frontend would read this one
with digits missing.
```

A refusal on the way out of a Call reaches the frontend too: the Promise
rejects with that same `TypeError` rather than hanging.

Why a closed set rather than a best-effort conversion:
[ADR-0002](./decisions/0002.md).
