# Your first Webview

```python
from dry import Webview

wv = Webview(
    app_id='com.example.hello',
    title='Hello',
    size=(900, 600),
    html='<h1>Hello from Dry</h1>',
)
wv.run()
```

Three things are worth knowing before you write anything longer than this.

## Every option is a keyword argument

`Webview()` takes keyword arguments only. Your editor lists what there is, and
a typo raises instead of quietly creating an attribute that never applies:

```python
wv = Webview(html='<h1>Hi</h1>')
wv.titel = 'My App'
# AttributeError: 'Webview' object has no attribute 'titel' and no __dict__
# for setting new attributes. Did you mean: 'title'?
```

Every option is also a property, for the values you only work out later:

```python
wv = Webview(app_id='com.example.hello')
wv.title = f'Report — {report.name}'
wv.html = render_page(report)
wv.run()
```

Most settings are read once, while the window is being built. Assigning one of
those after `run()` raises a `RuntimeError` naming it rather than doing
nothing — see [Window options](./window-options.md) for which is which.

## `run()` never returns

`wv.run()` hands the main thread to the platform's event loop, which does not
give it back: closing the window exits the process from inside that loop.

```python
wv.run()
print('goodbye')  # never printed
```

Nothing after `run()` executes, and a `finally:` wrapped around it does not run
either. Work that has to happen on the way out belongs in a
[close hook](./close-hook.md), in an `atexit` handler — Dry runs those itself
before the process goes — or in a `finally:` inside a callback.

The corollary is that an application cannot make `asyncio.run(main())` its
entry point. Dry runs an asyncio loop of its own, on a background thread, and
your `async def` code lives inside Api callables and Event listeners. See
[The Portal](./portal.md) and [ADR-0001](./decisions/0001.md).

## Give it an App id

```python
wv = Webview(app_id='com.example.hello', html=HTML)
```

The App id decides where cookies, local storage and cache are kept. Leave it
out and one is derived from your entry-point script, which is enough to develop
against but moves when the script moves. Declare your own before you ship:
[Where your data lives](./app-data.md).

## Talking to Python

Give the Webview an `api`, and the frontend can Call it:

```python
from dry import Webview

HTML = """
<button onclick="greet()">Greet</button>
<p id="out"></p>
<script>
  async function greet() {
    out.textContent = await window.dry.api.hello('World');
  }
</script>
"""


def hello(name: str) -> str:
    return f'Hello, {name}!'


wv = Webview(app_id='com.example.hello', html=HTML, api={'hello': hello})
wv.run()
```

That is the whole of the Bridge's Call half. [Calls](./calls.md) covers what
happens when the callable is slow, raises, or is declared `async def`.
