# Runtime window control

> **Not in the library yet.** This page is a placeholder for the surface that
> changes the window after it has opened — size, position, visibility,
> maximize, minimize, fullscreen, and a query for the current state. It is
> tracked as
> [issue #7](https://github.com/barradasotavio/dry/issues/7). Nothing described
> as missing below should be read as available.

## What exists today

From the frontend, three controls, covered in
[Custom titlebars](./titlebar.md#window-controls):

```javascript
window.dry.minimize();
window.dry.toggleMaximize();
window.dry.close();
```

From Python, the window can be **observed** but not commanded. Every state
change arrives as a [Window Event](./window-events.md), on both sides:

```python
wv.on('window:resized', lambda size: remember(size['width'], size['height']))
```

## What is not there yet

`title`, `size`, `min_size`, `decorations` and `icon_path` remain assignable
after `run()` — they are the settings that do not raise — but the assignment
only changes the stored value. **It does not move, resize or retitle the open
window.** That wiring is #7's, and it is the one place in this documentation
where a property exists whose runtime effect does not.

Also absent until then: setting position or visibility from Python, entering
fullscreen, and a state query that answers with the current maximized,
minimized, focused and fullscreen state along with size and position.

`window:hidden` and `window:shown` are implemented and diffed, but nothing in
the current surface hides a window, so in practice you will not see them yet.

## For whoever fills this in

When #7 lands, this page is where its surface is documented, and these are the
other places that will need a line:

- [Window options](./window-options.md#what-can-change-after-run) — the table
  row that currently reads "does not yet move the window".
- [Window Events](./window-events.md) — `window:hidden` / `window:shown` become
  reachable.
- [The Webview reference](./reference-webview.md) — the methods table.
- [Changelog](./changelog.md).
