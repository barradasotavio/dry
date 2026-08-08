# Events: both directions

An **Event** is a Bridge message that returns nothing. It carries a name and a
value, and every listener registered for that name receives it. Both directions
work, and they are the same bus.

## Python to the frontend

```python
wv.emit('progress', {'done': 12, 'total': 40})
```

```javascript
window.dry.on('progress', ({ done, total }) => bar.value = done / total);
```

`wv.emit` returns as soon as the Event is on its way, and returns nothing —
that is exactly what separates it from a Call. An Event nobody is listening for
is a no-op, not an error. It is safe from any thread and from inside any
callback. Before `run()` there is no frontend to reach, so it raises a
`BridgeError`.

The value crosses under the [Bridge contract](./contract.md), `default=` hook
included, so anything outside it raises at the `emit` rather than arriving
mangled.

## The frontend to Python

```javascript
window.dry.emit('form-dirty', { form: 'invoice' });
```

```python
def remember(value):
    dirty.add(value['form'])


wv.on('form-dirty', remember)
```

`wv.on(name, listener)` returns the listener it was given, so it can be used
inline: `remember = wv.on('form-dirty', remember)`.

Registering costs nothing and needs no window, so listeners may be registered
before `run()`. `wv.off(name, listener)` takes one registration off; taking off
a listener that was never registered is not an error. Registering the same
listener twice registers it twice, and it is then delivered to twice — two
identical closures are not the same subscription.

On the JavaScript side, `window.dry.on` returns an unsubscribe function, which
is what a component wants to hold on to:

```javascript
const stop = window.dry.on('progress', update);
// later
stop();
```

## How a listener runs

A listener takes the Event's value — one argument — and returns nothing that
anybody reads. An Event has no return path, so whatever it returns is dropped.

Listeners are handed over in the order they registered, and **that is the only
ordering you may rely on**. Each runs off the thread drawing the window, on
Dry's loop if it is an `async def` and in the thread pool otherwise, so they
overlap and finish in any order. Two listeners sharing state must make that
state thread-safe, exactly as an Api must.

A listener that raises is logged with its traceback — on `dry.bridge` in
Python, on the console in the frontend — and the other listeners still get
theirs.

## Reserved names

A name beginning with `window:` belongs to Dry. Listen for one as much as you
like; `wv.emit` and `window.dry.emit` refuse to emit one:

```python
wv.emit('window:resized', {'width': 10, 'height': 10})
# dry.exceptions.BridgeError: 'window:resized' is a reserved Event name: a name
# starting with 'window:' belongs to Dry's own window Events. Listen for it as
# much as you like, but emit under a name of your own.
```

That is what makes a listener for one trustworthy: it is hearing from the
window and nothing else. See [Window Events](./window-events.md).

## Running a script in the page

```python
wv.eval_js('document.title = "Saved"')
```

`eval_js` evaluates a script in the page and reads nothing back. It is the
escape hatch for the one quadrant the Bridge deliberately does not have — see
[Calls](./calls.md#python-does-not-call-the-frontend). An Event is almost
always the better answer, because the frontend decides what to do with it.
