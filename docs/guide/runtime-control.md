# Runtime window control

The window can be commanded from Python while it is open, and read back the
same way. Everything here is a **property**, so changing the window and asking
what it is doing are the same word:

```python
wv.title = f'{filename} — My App'
wv.size = (1080, 720)
wv.maximized = True
```

Since `run()` never returns, "while it is open" means from inside a callback:
an Api callable, an Event listener or the [close hook](./close-hook.md).

```python
from dry import Webview


def open_settings() -> None:
    wv.size = (1080, 720)
    wv.title = 'My App — Settings'


wv = Webview(
    app_id='com.example.myapp',
    title='My App',
    html=HTML,
    api={'open_settings': open_settings},
)
wv.run()
```

`open_settings` names `wv` above the line that creates it, which is fine:
Python looks a global up when the function runs, not when it is defined, and a
callback cannot run until `run()` has the window open. Calling `open_settings()`
yourself in between is the only way to reach a name that is not there yet, and
nothing in the Bridge does that.

## Settings that keep applying

`title`, `size`, `min_size`, `decorations` and `icon_path` are constructor
arguments that go on working afterwards. Assigning one before `run()` decides
what the window opens as; assigning it afterwards changes the open window.
Every other
constructor argument is read once while the Webview is built and raises if
assigned later — see
[Window options](./window-options.md#what-can-change-after-run).

| Setting | Assigned while running |
| --- | --- |
| `title` | Retitles the window |
| `size` | Resizes it, logical pixels |
| `min_size` | Sets the floor, and grows the window to meet it |
| `decorations` | Adds or removes the native titlebar and borders |
| `icon_path` | Replaces the icon; `None` restores the platform default |

`size` reads back what the window currently **measures**, not the last number
you gave it, so a window the user dragged reports the size it was dragged to. A
`min_size` larger than the window resizes the window up to it:

```python
wv.min_size = (900, 600)
wv.size  # (900, 600)
```

## States a window only has once it is open

`position`, `visible`, `maximized`, `minimized` and `fullscreen` are not
settings — a window that does not exist has no corner to sit in and no screen
to fill. They are not constructor arguments, and touching one before `run()`
raises rather than quietly storing a value that would never be applied:

```python
wv = Webview(app_id='com.example.myapp', html=HTML)
wv.maximized = True
# RuntimeError: maximized belongs to a window that is on screen, and this
# Webview has not opened one yet. Ask for it once run() has, from inside an Api
# callable, an Event listener or the close hook.
```

| State | Assigned | Read |
| --- | --- | --- |
| `position` | Moves the window, logical pixels | Where its top-left corner is, decorations included |
| `visible` | Takes it off the screen, or puts it back | Whether it is on screen |
| `maximized` | Maximizes, or restores the previous size | Whether it fills its screen |
| `minimized` | Minimizes to the dock or taskbar, or restores | Whether it is minimized |
| `fullscreen` | Takes over the screen, or comes back | Whether it has |

A platform that refuses part of what you asked for simply reports where the
window actually went — macOS will not lift a window above the menu bar, and
`wv.position` afterwards is the corner it settled on, not the one you named.

**A minimized window is still `visible`.** The user can see it in the dock or
the taskbar and put it back; only `visible = False` takes it off the screen
altogether.

`fullscreen` is borderless fullscreen on the window's current monitor, which is
what a desktop application wants: it does not change the display's resolution
under the user.

## Reading the whole window at once

`wv.state()` answers with a `WindowState`, a `NamedTuple` of everything the
window is doing, taken as one reading:

```python
state = wv.state()
# WindowState(maximized=False, minimized=False, fullscreen=False, visible=True,
#             focused=True, size=(640, 480), position=(436, 144))

if not state.maximized:
    wv.maximized = True
```

Reading the properties one at a time is the same information, but a reading is
taken at one instant, so its fields cannot contradict each other the way two
properties read a moment apart can. `size` and `position` are pairs of logical
pixels; `size` is the area the frontend renders into — the same measurement
`size=` sets, and the same number the page reads back as `window.innerWidth`.

`focused` is in the reading and is not a property of its own: Dry reports which
window has the keyboard, through `window:focused` and `window:blurred` as much
as here, but does not take focus for you.

Like the five states, `state()` raises before `run()`: answering with the
settings the window will be built from would be a guess dressed as a
measurement.

## Asking from the frontend

The same reading is a Promise in the page. A titlebar that has just loaded has
observed no [window Event](./window-events.md) at all and still has to draw its
maximize button one way round:

```javascript
const { maximized, size } = await window.dry.state();
icon.src = maximized ? RESTORE : MAXIMIZE;
```

The frontend gets the shape the window Events use — `size` is
`{width, height}` and `position` is `{x, y}` — so a value from
`window.dry.on('window:resized', ...)` and a value from `dry.state()` can go to
the same code. Python prefers a pair, and that is the only difference between
the two sides.

Every call waiting on the same trip resolves with the same reading, so a page
polling the query costs one trip either way.

The frontend commands the window with `window.dry.minimize()`,
`window.dry.toggleMaximize()` and `window.dry.close()`, which are the three a
custom titlebar needs and are covered in
[Custom titlebars](./titlebar.md#window-controls). Anything beyond those three
is Python's: give the page an Api callable that does it.

## Every change announces itself

A change made from Python is announced exactly as a change the user made. Dry
reads the window once per turn of its event loop and emits the `window:` Events
for whatever moved, so `wv.maximized = True` and a double-click on the titlebar
reach a listener identically, and a change the platform refused announces
nothing:

```python
wv.on('window:hidden', lambda _: tray.show_restore_item())
wv.on('window:shown', lambda _: tray.hide_restore_item())
```

`window:hidden` and `window:shown` are reachable exactly because `visible` is
assignable — nothing else in Dry hides a window.

`fullscreen` is the one state with no Event of its own: macOS reports entering
it as the platform's own mix of a maximize, a `window:moved` and a
`window:resized`, so a name for it could not be told from what already arrives.
Read `wv.fullscreen`, or `wv.state().fullscreen`, instead of listening for it.

## The reading can be one turn old

An assignment crosses to the thread that draws the window and is applied on its
next turn of the event loop. The state query answers from the reading taken at
the **last** turn — the same reading every `window:` Event was a difference
from — so a query and an Event can never disagree, but a reading taken in the
same breath as an assignment is still the state before it:

```python
wv.maximized = True
wv.state().maximized  # False — the window has not had its turn yet
```

That is not a race to be won by sleeping. If the order matters, wait for the
Event that announces the change:

```python
maximized = threading.Event()
wv.on('window:maximized', lambda _: maximized.set())

wv.maximized = True
maximized.wait(timeout=5)
```

## `decorations` at runtime does not add resize edges

An undecorated Webview draws its own
[resize edges](./titlebar.md#resize-edges), and the script that draws them is
installed **when the Webview is built**, from the `decorations` the constructor
was given. Assigning `decorations` later moves the native titlebar and borders
and nothing else:

- A window built with `decorations=True` and undecorated at runtime has no
  resize edges, and reloading the Content does not produce them. It can still
  be resized from Python and from `window.dry.resize(direction)`, but there is
  nothing on its border to grab.
- A window built with `decorations=False` keeps its eight edge strips even
  after `decorations = True`, sitting just inside the native frame.

So a window that means to toggle its titlebar should be **built undecorated**
and draw its own, which is what [Custom titlebars](./titlebar.md) covers
anyway. `data-drag-region` is unaffected either way: it is installed for every
window.

## Hiding a window instead of closing it

`visible = False` takes the window off the screen without closing it: no
titlebar button, no dock entry, no close hook. The Webview goes on running —
the event loop still turns, the Bridge still carries Calls and Events, and the
page keeps its state — which is what makes a tray application possible, where
closing the window would end the process.

```python
def on_close() -> bool:
    wv.visible = False
    return False  # refuses the close; the window is hidden, not gone


wv = Webview(app_id='com.example.myapp', html=HTML, on_close=on_close)
```

Something else then has to bring it back with `wv.visible = True` — a tray
icon, a global shortcut, a second instance handing over. A window hidden with
nothing left to show it again is a process the user cannot see or quit.

## Everything is logical pixels

Every dimension and coordinate here — `size`, `min_size`, `position`, and both
values inside a `WindowState` — is in **logical pixels**, independent of
display scaling, the same unit the constructor takes and the same unit
[window Events](./window-events.md) report. They are the numbers CSS is working
in, so a window told to be 640 wide reports 640 from `window.innerWidth` on a
display at any scale factor.
