# Window Events

The window reports what it is doing as ordinary [Events](./events.md) under
names Dry reserves for itself. They travel on the same bus as any Event of your
own, reach both sides, and are subscribed to the same way.

```javascript
window.dry.on('window:maximized', () => icon.src = RESTORE);
window.dry.on('window:resized', ({ width, height }) => show(width, height));
```

```python
wv.on('window:resized', lambda size: print(size['width'], size['height']))
```

| Name | Value |
| --- | --- |
| `window:maximized`, `window:unmaximized` | `null` |
| `window:minimized`, `window:restored` | `null` |
| `window:hidden`, `window:shown` | `null` |
| `window:focused`, `window:blurred` | `null` |
| `window:resized` | `{width, height}` |
| `window:moved` | `{x, y}` |
| `window:close-requested` | `null` |

The names come in opposed pairs rather than one name carrying a boolean,
because a listener should read as the thing that happened rather than unwrap a
value before it knows what it was told.

## What you can rely on

- **Every one of them fires for a change the user made**, not only for one your
  application made: a keyboard shortcut, the window menu, a double-click on the
  titlebar. The window's state is read once per turn of the event loop and
  compared with the last reading, so a change with no platform event of its own
  is still caught.
- **A change your Python made is announced identically.** `wv.maximized = True`
  and a double-click on the titlebar reach a listener as the same
  `window:maximized`, because both are read off the window rather than
  reported by whoever asked — and a change the platform refused announces
  nothing. See [Runtime window control](./runtime-control.md).
- **`window:hidden` and `window:shown` are reachable through `wv.visible`**,
  which is the only thing in Dry that takes a window off the screen without
  closing it. A minimized window is minimized, not hidden.
- **Sizes and positions are logical pixels**, the same unit `size=` and
  `min_size=` are given, so they are the numbers CSS is working in.
- **`window:resized` and `window:moved` are emitted at most once per turn of
  the event loop, and only when the value actually changed.** A drag firing
  hundreds of platform events a second cannot produce more Events than the
  window had turns to draw in, so a listener cannot fall behind, and the last
  turn always carries the final geometry.
- **While the window is minimized or hidden, size, position and maximized state
  hold their last observed values.** The platform's answers there are not about
  the window the user will see again — Windows parks a minimized window at
  -32000 — so a minimize does not report a move to nowhere.
- `window:close-requested` is a **notification, not a vote**. The
  [close hook](./close-hook.md) is the only thing that can refuse a close.

`fullscreen` has no Event. `wv.fullscreen = True` does enter it, but what
arrives is what the platform makes of it — on macOS a `window:maximized` with a
`window:moved` and a `window:resized` behind it — so a name of its own could
not be told from what already comes. Read `wv.fullscreen`, or
`wv.state().fullscreen`, instead of listening for it.

## What is, rather than what changed

An Event only reaches a listener that was registered when it fired. A page that
has just loaded, or a callback that was not listening, has observed nothing and
still has to draw a maximize button one way round:

```javascript
const { maximized } = await window.dry.state();
```

```python
if wv.state().maximized:
    ...
```

Both answer from the last reading the event loop took — the same reading every
Event above was a difference from — so the query and the Events can never
disagree. See
[Runtime window control](./runtime-control.md#reading-the-whole-window-at-once).

## Reserved means reserved

A name beginning with `window:` belongs to Dry. Listening is unrestricted —
that is exactly how these are heard, on both sides — but `wv.emit` and
`window.dry.emit` refuse to emit one. A listener for `window:resized` is
therefore hearing from the window and nothing else.
