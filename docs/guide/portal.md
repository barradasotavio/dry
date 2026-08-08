# The Portal

The **Portal** is where Python code that Dry calls actually runs: an asyncio
loop on a daemon thread, beside a thread pool, off the thread drawing the
window. Every Api callable and every Event listener crosses it.

You never touch it. What it decides, you have to know.

## Why it exists

The GUI event loop must own the main thread — on macOS that is an AppKit
requirement, not a preference — and `tao::EventLoop::run` never returns, exiting
the process from inside itself. A callback that ran on that thread would hold
the window still for its whole duration: no repaint, no input, no second Call.

So Dry takes every Call and every Event delivery off that thread. An `async def`
is scheduled onto the loop; anything else goes into the pool.

## The two consequences

**Your callbacks run concurrently.** Two Calls overlap, and finish in whatever
order they finish in. State shared between Api callables, or between Event
listeners, is yours to make thread-safe — a `threading.Lock`, or a design that
does not share.

The single ordering guarantee: listeners for one Event are *handed over* in the
order they registered. Nothing guarantees they finish in that order.

**Your application cannot make `asyncio.run(main())` its entry point.** Dry owns
the process and owns the loop. Your async code lives inside callbacks, and is
awaited on Dry's loop:

```python
import asyncio

import httpx


async def fetch(url: str) -> str:
    async with httpx.AsyncClient() as client:
        return (await client.get(url)).text


wv = Webview(app_id='com.example.myapp', html=HTML, api={'fetch': fetch})
wv.run()
```

Work that has to start before the window opens and keep running can go on a
thread you start yourself: since 0.4.0 `run()` releases the GIL, so ordinary
Python threads keep running for the life of the window.

The full reasoning, and the alternatives that were rejected, is in
[ADR-0001](./decisions/0001.md).

## Started lazily, shut down in order

Neither the loop nor the pool exists until the first Call or Event that needs
one, so a Webview with no Api and no listeners never starts either.

Both are shut down when the window closes, in the order described in
[Closing the window](./close-hook.md).

## The stdlib only

The Portal is `dry/portal.py`: `asyncio`, `concurrent.futures`, `threading`,
`inspect` and `logging`. Depending on `anyio` would buy trio support this
project does not need, at the cost of the zero-dependency promise. Dry
therefore installs with no transitive dependencies at all.
