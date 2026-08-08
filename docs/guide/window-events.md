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

`fullscreen` has no Event: macOS reports entering it as a `resized` plus a
`moved`, and there is nothing in the current surface that enters it, so the
name would be one that could not be tested.

## Reserved means reserved

A name beginning with `window:` belongs to Dry. Listening is unrestricted —
that is exactly how these are heard, on both sides — but `wv.emit` and
`window.dry.emit` refuse to emit one. A listener for `window:resized` is
therefore hearing from the window and nothing else.
