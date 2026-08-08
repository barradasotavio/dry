# The `window.dry` namespace

Everything Dry exposes to the frontend hangs off one global, `window.dry`, so
nothing the library injects can collide with a standard browser API or with
your own globals.

`window.dry` and each of its members are defined non-writable and
non-configurable: a page script cannot replace them.

## The Bridge

| Member | Signature | Does |
| --- | --- | --- |
| `dry.api` | `dry.api.<name>(...args) -> Promise` | Calls the Python callable registered under `<name>` |
| `dry.on` | `dry.on(name, listener) -> () => void` | Registers a listener; returns an unsubscribe function |
| `dry.off` | `dry.off(name, listener)` | Takes one registration off |
| `dry.emit` | `dry.emit(name, value)` | Emits an Event to Python's listeners |

`dry.api` is a Proxy: any property read returns a function, so an unknown name
fails when Python is asked, as a rejected Promise, not at the property access.

`dry.emit` refuses a name starting with `window:` with a `TypeError`, and
`dry.on` and `dry.emit` refuse an empty name or a non-function listener the
same way.

Values crossing in either direction obey the
[Bridge contract](./contract.md).

## Window controls

| Member | Does |
| --- | --- |
| `dry.minimize()` | Minimizes the window |
| `dry.toggleMaximize()` | Maximizes, or restores if already maximized |
| `dry.close()` | Asks to close, through the close hook |
| `dry.drag()` | Starts a window drag from your own handler |
| `dry.resize(direction)` | Starts a resize drag from an edge or corner |

`direction` is one of `'north'`, `'north-east'`, `'east'`, `'south-east'`,
`'south'`, `'south-west'`, `'west'`, `'north-west'`. `dry.resize` is meant to
be called from a `mousedown` handler: with no button down there is no grab, and
it starts nothing.

## HTML attributes

| Attribute | Does |
| --- | --- |
| `data-drag-region` | The element and its subtree drag the window; a double click toggles maximize |
| `data-no-drag-region` | The element and its subtree opt back out |

See [Custom titlebars](./titlebar.md).

## Reserved Event names

Names beginning with `window:` are Dry's own. Listen for them freely; emitting
one is refused. The full list is in [Window Events](./window-events.md).

## Two members that are not yours

`dry.resolveCall` and `dry.deliverEvent` exist, non-enumerable, and are how
Rust settles a Promise and hands an Event to your listeners. They are
implementation, not surface: do not call them.

The pending-call store and the listener register live in closures, so no page
script can read or tamper with another script's in-flight Calls or listeners.
