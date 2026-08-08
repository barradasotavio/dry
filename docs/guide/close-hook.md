# Closing the window

The **close hook** is the callable asked before the Webview closes, and the one
thing that can refuse a close.

```python
def on_close():
    if editor.is_dirty():
        return False  # keeps the window open
    editor.save()


wv = Webview(app_id='com.example.myapp', html=HTML, on_close=on_close)
wv.run()
```

Returning `False` — that value, not anything merely falsy — refuses the close.
Anything else, `None` included, lets it go, so a hook that only saves state
does not have to remember to return anything.

An `async def` hook works too, and is awaited on Dry's loop before the answer
is read.

## Every route in is asked

The native titlebar button, the window manager, an OS shortcut and
`window.dry.close()` all go through the same path. A refusal one route could
skip would not be a guarantee.

The hook runs on the thread that owns the window, with the window held still,
which is what makes the decision meaningful — a modal "you have unsaved
changes" prompt is exactly the case it exists for, and it has to be answered
before the close continues. Nothing is timed out: a hook that never returns
keeps the window open, the same as one that refuses.

## A hook that raises does not refuse

The close goes ahead, and the exception is logged on `dry.bridge`. Refusing is
deliberate — it is `False`, returned on purpose — and a hook that raises has not
made a decision. A decision it never made must not be the one that traps the
user in a window that cannot be closed.

## What happens after the hook agrees

In this order:

1. In-flight Calls and Event deliveries are given **up to 5 seconds** to finish
   on their own. A Call halfway through writing a file is what the whole
   sequence exists to protect.
2. What the grace period does not save is cut short, out loud. A coroutine is
   cancelled, so its `finally:` blocks run and its Call rejects with the
   `CancelledError`. A Call that had not started is cancelled the same way. A
   pool thread already inside a callable cannot be interrupted at all — Python
   has no such thing — so it is left, unanswered, with a warning logged.
3. A Call arriving during shutdown is **rejected** with a reason, not left
   hanging: the window is going, so the reply could not be delivered even if
   the callable ran.
4. The asyncio loop is drained and stopped.
5. `atexit` handlers run. Dry runs them by hand, because the platform event
   loop exits the process from under the interpreter and they would otherwise
   never happen.
6. The process exits.

## What still does not run

A `try: ... finally:` wrapped around `wv.run()` **does not run**, because the
event loop exits the process from inside `run()`. That is a consequence of Dry
owning the process ([ADR-0001](./decisions/0001.md)), and the honest workaround
is the close hook or an `atexit` handler, both of which do run.

`finally:` blocks *inside* callbacks do run.

## Setting it later

`on_close` is a property as well as a constructor argument, and like the other
settings read at build time, assigning it after `run()` raises. One Webview has
one hook: setting it again before `run()` replaces the previous one.
