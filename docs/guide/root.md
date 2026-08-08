# Serving a Root

A **Root** is a local directory served to the Webview over an internal
protocol, so that relative assets resolve. It is the Content mode a compiled
frontend wants, and it needs no server, no port and no second process.

```python
from pathlib import Path

from dry import Webview

wv = Webview(
    app_id='com.example.myapp',
    root=Path(__file__).parent / 'dist',
)
wv.run()
```

The Webview starts at the directory's `index.html`, and everything that page
names relatively — `./assets/index.js`, `<img src="logo.png">`,
`@font-face { src: url(fonts/inter.woff2) }` — is fetched back out of the same
directory.

`root` accepts a `str` or any `os.PathLike`, expands `~`, and is stored
resolved. It is checked when you assign it, not at `run()`:

```python
Webview(root='./does-not-exist')
# FileNotFoundError: root does not exist: does-not-exist

Webview(root='./index.html')
# NotADirectoryError: root must be a directory, not a file: index.html.
# To render a single file, read it and set webview.html instead.
```

## What the internal protocol answers

| Request | Answer |
| --- | --- |
| A file inside the Root | `200`, with the content type its extension implies |
| A directory inside the Root | its `index.html` |
| A path that resolves outside the Root | `403 Outside the root: <path>` |
| A path inside the Root with no file there | `404 Not found: <path>` |

Escaping is refused twice over: a `..`, a backslash, a colon or a NUL in any
path component is rejected before the path is joined, and the canonicalised
result must still sit beneath the Root — which also catches a symlink pointing
out of the tree. Both refusals are ordinary HTTP statuses your frontend can
observe from `fetch`, not a blank window.

Percent-escapes are decoded, so a file whose name holds a space or a non-ASCII
character is found.

## Content types

Extensions are mapped to types explicitly; text types carry
`; charset=utf-8`, without which WebKit guesses the encoding of CSS and
JavaScript.

`html`, `htm`, `js`, `mjs`, `css`, `json`, `map`, `txt`, `csv`, `xml`, `wasm`,
`pdf`, `svg`, `png`, `jpg`, `jpeg`, `gif`, `webp`, `avif`, `bmp`, `ico`,
`woff`, `woff2`, `ttf`, `otf`, `mp3`, `wav`, `ogg`, `oga`, `mp4`, `webm`.

An extension not on that list is served as `application/octet-stream` rather
than guessed at from the bytes.

## Notes for a bundler

- Build with **relative** asset paths. A frontend that emits `/assets/app.js`
  is asking for the root of the origin, which is the Root's own top level; that
  happens to work here, but relative paths are what survive being served from
  anywhere. In Vite, `base: './'`.
- Client-side routing that relies on a server rewriting unknown paths to
  `index.html` will get a `404` instead. Hash routing works as it stands.
- A working example is
  [`examples/root.py`](https://github.com/barradasotavio/dry/tree/master/examples/root.py).
