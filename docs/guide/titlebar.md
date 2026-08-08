# Custom titlebars

An undecorated Webview has no native titlebar and no native frame, and gets two
things in their place: **drag regions**, which move the window, and **resize
edges**, which resize it.

```python
wv = Webview(app_id='com.example.myapp', decorations=False, html=HTML)
```

## Drag regions

An element marked `data-drag-region` moves the window when dragged.

```html
<div data-drag-region>
    <h1>My Application</h1>
    <button data-no-drag-region onclick="window.dry.minimize()">–</button>
    <button data-no-drag-region onclick="window.dry.toggleMaximize()">□</button>
    <button data-no-drag-region onclick="window.dry.close()">×</button>
</div>
```

**The whole subtree drags.** The heading above moves the window exactly as the
bare margin around it does. An element marked `data-no-drag-region` opts itself
and its own subtree back out, which is what the buttons inside a titlebar want:
a click reaches them instead of moving the window.

The nearest marked ancestor wins, so an opt-out nested inside a drag region is
honoured, and a drag region nested inside an opt-out drags again.

A **double click** inside a drag region toggles maximize, as a native titlebar
does. A drag only starts once the pointer has actually moved, so a click inside
a drag region stays a click.

Only the primary mouse button drags.

## Resize edges

An undecorated Webview draws eight thin strips over its own border — 3px along
each side, 7px at each corner — each with the cursor the direction implies. A
grab on one resizes the window from that edge or corner.

They are ordinary fixed-position elements at `z-index: 9999`, and each carries
`data-no-drag-region`, so a grab on the top edge resizes rather than moving the
window even when the titlebar sits right behind it.

The mechanism differs by platform: Windows hands the grab to the operating
system, which takes it over with a modal loop of its own, while macOS has no
such path — tao answers `NotSupported` — so the frontend runs the drag itself
and reports the pointer to Rust on every move. The behaviour is the same either
way. The reasoning is in [ADR-0004](./decisions/0004.md).

## Window controls

```html
<button onclick="window.dry.minimize()">Minimize</button>
<button onclick="window.dry.toggleMaximize()">Maximize</button>
<button onclick="window.dry.close()">Close</button>
```

These work with or without decorations. `window.dry.close()` goes through the
[close hook](./close-hook.md) exactly as the native titlebar button does.

There is also `window.dry.drag()`, which starts a window drag from a handler of
your own, and `window.dry.resize(direction)` for the eight directions —
`'north'`, `'north-east'`, `'east'`, `'south-east'`, `'south'`, `'south-west'`,
`'west'`, `'north-west'` — if you would rather draw your own edges.

## Keeping the titlebar in sync

A titlebar that can only command the window keeps its own guess at whether the
window is maximized, and that guess is wrong the first time the user
double-clicks the bar or reaches for an OS shortcut. Listen instead:

```html
<script>
    window.dry.on('window:maximized', () => icon.src = RESTORE);
    window.dry.on('window:unmaximized', () => icon.src = MAXIMIZE);
</script>
```

A page that has just loaded has heard no Event yet, so ask once on the way in:

```javascript
const { maximized } = await window.dry.state();
icon.src = maximized ? RESTORE : MAXIMIZE;
```

See [Window Events](./window-events.md) and
[Runtime window control](./runtime-control.md). A full example is
[`examples/titlebar.py`](https://github.com/barradasotavio/dry/tree/master/examples/titlebar.py).
