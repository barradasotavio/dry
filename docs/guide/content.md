# The three Content modes

A Webview renders exactly one Content, declared as exactly one of three
mutually exclusive modes.

```python
from pathlib import Path

from dry import Webview

# An HTML string
Webview(html='<h1>Hello, World!</h1>')

# A URL
Webview(url='http://localhost:8000')

# A Root: a local directory, served starting at its index.html
Webview(root=Path(__file__).parent / 'dist')
```

There is no sniffing. Dry does not look at a string and decide whether it is
markup, a path or an address; you say which it is, and the mode you named is
the mode you get.

## Declaring two, or none

Declaring a second mode raises as soon as you do it, naming the conflict and
how to resolve it:

```python
wv = Webview(html='<h1>Hi</h1>')
wv.url = 'https://example.com'
# ValueError: Content is already declared as html, so it cannot also be
# declared as url. A Webview renders exactly one of html, url or root.
# Set webview.html = None first.
```

Setting a mode back to `None` clears it, so switching is a two-step move on
purpose:

```python
wv.html = None
wv.url = 'https://example.com'
```

Declaring none is caught at `run()`, because a Webview may legitimately be
built empty and filled in later:

```python
Webview().run()
# ValueError: No content declared. A Webview renders exactly one of html, url
# or root: pass html=, url= or root= to Webview(...), or set the matching
# property before run().
```

## `html`

An HTML string, loaded as the document. Anything relative inside it —
`<img src="logo.png">`, `<script src="./app.js">` — has no directory to resolve
against, so `html` is for self-contained pages: markup, inline styles, inline
scripts, and absolute URLs. The moment you have files beside your page, you
want a [Root](./root.md).

## `url`

Any address the platform's web engine can load: a remote site, or a local
server of your own. See
[Loading a URL from a local server](./local-servers.md).

## `root`

A local directory served to the Webview over Dry's own internal protocol, so
relative assets resolve. This is the mode a compiled frontend wants — the
output directory of a Vite, esbuild or Parcel build. See [Serving a
Root](./root.md).

## Content is fixed once the window opens

`html`, `url` and `root` are read while the Webview is being built. Assigning
one after `run()` raises a `RuntimeError` naming the setting, rather than
silently doing nothing. To change what the frontend shows while it is running,
change it from the frontend — that is what `wv.emit` and `wv.eval_js` are for.
