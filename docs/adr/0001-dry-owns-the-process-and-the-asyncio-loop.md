# Dry owns the process and the asyncio loop

The OS GUI event loop must own the main thread — on macOS this is an AppKit requirement, not a convention — and `tao::EventLoop::run` never returns, exiting the process directly. Dry therefore runs the GUI loop on the main thread and starts its own asyncio loop on a daemon child thread; `async def` callbacks are scheduled onto it and plain `def` callbacks go to a thread pool, so neither blocks the window. The consequence a reader will trip over: an application cannot make `asyncio.run(main())` its entry point — Dry owns the process and the developer's async code lives inside callbacks.

## Considered Options

- **An asyncio loop on the main thread, GUI on a child thread** — impossible on macOS.
- **Interleaving both loops** via `run_return` and `ControlFlow::Poll` driven from an asyncio callback — burns CPU and is fragile.
- **Accepting a user-supplied loop** (`Webview(loop=...)`) — deferred, not rejected; it can be added later without breaking the owned-loop default.

## Consequences

Callbacks now run concurrently, so user code must be thread-safe. Because `tao` exits the process directly, ordered shutdown is Dry's responsibility: `CloseRequested` is intercepted so Python close hooks run and the asyncio loop closes before exit.
