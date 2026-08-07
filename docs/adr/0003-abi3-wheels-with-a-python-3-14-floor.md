# abi3 wheels with a Python 3.14 floor

Dry ships stable-ABI (`abi3-py314`) wheels, two per release — `win_amd64` and `macosx_universal2` — rather than a matrix of one build per Python version. A single wheel then imports on every future CPython, so a new October release needs no action; the previous per-version approach left the library claiming 3.13 support while its own author ran 3.14.

The floor sits at 3.14 rather than 3.11 for two reasons. Modern typing without `typing_extensions` keeps the zero-dependency promise intact, and PEP 649's deferred annotations are what make reading a callback's declared signature at runtime tractable — which is how Dry reports "`save_file` expects `str` for `path`, received `number`" instead of a raw `TypeError` from inside the bridge.

## Consequences

Users on 3.11–3.13 are excluded, which is most installed CPython today. This is accepted because it is cheap to undo: lowering the floor later is purely additive — a `cp311-abi3` wheel serves everyone the current one serves, and breaks nobody.

`abi3` does not cover free-threaded builds, so `python3.14t` cannot install Dry. `abi3t` (PEP 803) arrives with Python 3.15 and can be published alongside without touching the existing wheel.

The limited API excludes some PyO3 types, `PyFunction` among them. The replacement — `Py<PyAny>` plus a `callable()` check — is wanted anyway, since it accepts `functools.partial`, objects with `__call__`, and built-in functions that the old signature rejected.
