"""
Where a Call and an Event run, and where the process is put down.

The GUI event loop owns the main thread — on macOS that is an AppKit
requirement, not a preference — so a callback that runs there holds the window
still for its whole duration: no repaint, no input, no second Call. The portal
takes every Call off that thread. A coroutine callback is scheduled onto an
asyncio loop Dry owns on a daemon thread, and a plain callback goes to a thread
pool. Both answer through the same Completion, which carries the reply back
across the Bridge.

An Event listener takes the same road, minus the Completion: it is a Bridge
message that returns nothing, so it is handed over and never answered. What it
returns is dropped, and what it raises is logged.

Two consequences, recorded in ADR-0001. Dry owns the process, so an application
cannot make `asyncio.run(main())` its entry point — the developer's async code
lives inside callbacks, on the loop this module runs. And callbacks now run
concurrently, so an Api whose callables share state must make that state
thread-safe itself.

The same module closes the window down, because closing is the moment all of
this has to end in the right order. `tao` exits the process itself, so nothing
Python would normally do on the way out happens by itself: the close hook, the
draining of in-flight Calls and the interpreter's own exit handlers are all run
from here, by the event-loop thread, before it lets the process go.

Standard library only: depending on anyio would buy trio support this project
does not need, at the cost of the zero-dependency promise.
"""

from asyncio import (
    AbstractEventLoop,
    all_tasks,
    current_task,
    gather,
    new_event_loop,
    run_coroutine_threadsafe,
    set_event_loop,
)
from atexit import _run_exitfuncs  # pyright: ignore[reportPrivateUsage]
from concurrent.futures import Future, ThreadPoolExecutor, wait
from inspect import isawaitable, iscoroutinefunction
from logging import getLogger
from threading import Lock, Thread
from typing import Any, Awaitable, Callable, Protocol

_LOGGER = getLogger('dry.bridge')

# How long a closing portal waits, at each step, before giving up on it.
_SHUTDOWN_TIMEOUT = 5.0


class Completion(Protocol):
    """
    How a Call is answered.

    Exactly one of these two lands, exactly once. `resolve` carries a value
    that must be inside the Bridge contract, and raises if it is not, leaving
    the Call unanswered so that the failure can be rejected instead.
    """

    def resolve(self, value: object, /) -> None: ...

    def reject(self, error: BaseException, /) -> None: ...


CloseHook = Callable[[], object]
"""
What an application registers to be asked before its Webview closes.

Takes nothing. Returning `False` — that value, not anything falsy — refuses the
close and the window stays open. Anything else, `None` included, lets it go
ahead, so a hook that only saves state does not have to remember to return
anything. A coroutine function works too: it is awaited on Dry's loop before
the answer is read.
"""

Listener = Callable[[Any], object]
"""
What an application registers for the name of an Event.

Takes the Event's value, one argument, and returns nothing that anybody reads:
an Event has no return path, so whatever a listener returns is dropped. A
coroutine function works too, awaited on Dry's loop.
"""

_lock = Lock()
_loop: AbstractEventLoop | None = None
_thread: Thread | None = None
_executor: ThreadPoolExecutor | None = None
_closed = False
_hook: CloseHook | None = None

# Every Call that has been handed over and not yet answered. A closing portal
# waits on these, which is the difference between an application that finishes
# writing its file and one that is cut off mid-write.
_pending: set[Future[Any]] = set()

# Every Python listener, by the name of the Event it is registered for. The
# lists are kept in registration order and never mutated in place once handed
# out, so a listener registering or unregistering another from inside its own
# delivery cannot disturb the delivery already under way.
_listeners: dict[str, list[Listener]] = {}


def dispatch(
    name: str,
    function: Callable[..., object],
    arguments: tuple[object, ...],
    completion: Completion,
) -> None:
    """
    Runs one Call off the event-loop thread and answers it when it finishes.

    Returns as soon as the work is handed over, so the caller — the GUI thread
    — is free again. A coroutine function is scheduled onto the loop; anything
    else goes to the thread pool, and if what it returns turns out to be
    awaitable it finishes on the loop too, which is what makes a callable
    object with an `async def __call__` work like an `async def`.
    """
    loop, executor = _running()

    if iscoroutinefunction(function):
        try:
            coroutine = function(*arguments)
        except BaseException as error:
            _reject(name, error, completion)
            return
        _on_loop(name, coroutine, loop, completion)
        return

    _answer(name, executor.submit(function, *arguments), loop, completion)


def listen(name: str, listener: Listener) -> None:
    """
    Registers a listener for the Event of that name. Registering the same
    listener twice registers it twice, and it is then delivered to twice — the
    register is a list, not a set, because two identical closures are not the
    same subscription.
    """
    with _lock:
        _listeners.setdefault(name, []).append(listener)


def unlisten(name: str, listener: Listener) -> None:
    """
    Takes one registration of a listener off the name, the earliest one.
    Removing a listener that was never registered does nothing.
    """
    with _lock:
        registered = _listeners.get(name)
        if registered is None:
            return
        remaining = list(registered)
        try:
            remaining.remove(listener)
        except ValueError:
            return
        if remaining:
            _listeners[name] = remaining
        else:
            del _listeners[name]


def deliver(name: str, value: object) -> None:
    """
    Hands one Event to every listener registered for its name, and returns.

    Called by Rust on the thread that owns the window, so nothing is run here:
    each listener goes to the portal exactly as a Call does, and the window
    carries on drawing. An Event with no listeners is a no-op, which is what
    makes it safe for a frontend to announce something nobody is waiting for.

    Listeners are handed over in the order they registered, and that is the
    only ordering an application may rely on. They run concurrently, on the
    loop or in the pool, so they finish in whatever order they finish in, and
    two listeners that share state must make that state thread-safe — the same
    consequence ADR-0001 records for an Api.

    A listener that raises is logged and the rest are delivered to anyway: an
    Event has no sender waiting on an answer, so one broken listener is not
    grounds for silencing the others.
    """
    with _lock:
        listeners = _listeners.get(name)

    if not listeners:
        return

    try:
        loop, executor = _running()
    except RuntimeError:
        # The window is going. There is nobody to tell, and nothing a listener
        # could usefully do from inside a closing process.
        _LOGGER.debug(
            "The Bridge is closed, so the Event '%s' reached no listener.", name
        )
        return

    for listener in listeners:
        _hand_over(name, listener, value, loop, executor)


def on_close(hook: CloseHook | None) -> None:
    """
    Registers the callable asked before the Webview closes, or clears it with
    `None`. One Webview, one hook: registering again replaces it.
    """
    global _hook

    with _lock:
        _hook = hook


def closing() -> bool:
    """
    Asks the application whether its Webview may close, and answers for it when
    it has nothing to say.

    Runs on the thread that owns the window, so the window is held still while
    the hook decides — which is what makes the decision meaningful: a modal
    "you have unsaved changes" prompt is exactly the case this exists for, and
    it has to be answered before the close continues. Nothing is timed out
    here; a hook that never returns keeps the window open, the same as a hook
    that refuses, and that is the application's own doing.

    A hook that raises lets the close go ahead, and the exception is logged. It
    is the less obvious of the two readings — refusing would look safer — but a
    hook that raises has not made a decision, and a decision it never made must
    not be the one that traps the user in a window that cannot be closed.
    Refusing a close is deliberate: it is the value `False`, returned on
    purpose.
    """
    with _lock:
        if _closed:
            return True
        hook = _hook

    if hook is None:
        return True

    try:
        verdict = hook()
        if isawaitable(verdict):
            verdict = _awaited_here(verdict)
    except BaseException:
        _LOGGER.exception('The close hook raised, so the close goes ahead.')
        return True

    return verdict is not False


def closed() -> None:
    """
    Everything Python is owed on the way out, in the order it is owed.

    The portal shuts down first and the interpreter's exit handlers run after
    it, which is the order CPython itself uses at the end of a normal run:
    threads first, `atexit` last. Dry has to run it by hand because `tao` exits
    the process from under the interpreter — see ADR-0001 — and an application
    that only ever flushed its file from an `atexit` handler would otherwise
    lose it every single time.
    """
    shutdown()
    _run_exit_handlers()


def shutdown() -> None:
    """
    Closes the portal: no further Call is accepted, the Calls already in flight
    are given until `_SHUTDOWN_TIMEOUT` to finish on their own, whatever is
    still running on the loop is then cancelled and awaited, and the loop
    stops.

    In-flight work is waited for rather than cancelled outright. A Call that is
    halfway through writing a file is the case the whole close sequence exists
    to protect, and a callback is user code Dry cannot judge. The wait is
    bounded all the same, because an unbounded one would hand any slow callback
    the power to make the window unclosable.

    What the grace period does not save is cut short, on purpose and out loud:
    a coroutine is cancelled, so its `finally` blocks run and its Call is
    rejected with the `CancelledError`, and a thread-pool Call that has not
    started is cancelled the same way. A pool thread already inside a callback
    cannot be interrupted at all — Python has no such thing — so it is left,
    unanswered, to the exiting process.
    """
    global _loop, _thread, _executor, _closed

    with _lock:
        loop, thread, executor = _loop, _thread, _executor
        _loop, _thread, _executor = None, None, None
        _closed = True
        running = set(_pending)
        _pending.clear()

    if running:
        _, unfinished = wait(running, timeout=_SHUTDOWN_TIMEOUT)
        if unfinished:
            _LOGGER.warning(
                '%d Call(s) had not finished after %.0fs and were cut short.',
                len(unfinished),
                _SHUTDOWN_TIMEOUT,
            )

    if executor is not None:
        executor.shutdown(wait=False, cancel_futures=True)

    if loop is not None:
        try:
            run_coroutine_threadsafe(_drained(), loop).result(_SHUTDOWN_TIMEOUT)
        except BaseException:
            _LOGGER.exception('The asyncio loop could not be drained.')
        loop.call_soon_threadsafe(loop.stop)

    if thread is not None:
        thread.join(_SHUTDOWN_TIMEOUT)

    if loop is not None:
        loop.close()


def _awaited_here(awaitable: Awaitable[object]) -> object:
    """
    Runs an awaited close hook to its end on Dry's loop, and blocks the thread
    that owns the window until it has an answer. A hook is one decision, and a
    decision cannot be answered later.
    """
    loop, _ = _running()
    return run_coroutine_threadsafe(_awaited(awaitable), loop).result()


def _run_exit_handlers() -> None:
    """
    Runs, once, the `atexit` handlers the exiting process will never reach.
    CPython empties the registry as it goes, so a later exit that does reach
    them finds nothing left to run twice.
    """
    try:
        _run_exitfuncs()
    except BaseException:
        _LOGGER.exception('An exit handler raised on the way out.')


async def _drained() -> None:
    """
    Ends whatever the loop is still holding, on the loop itself, so that
    nothing is left half-run when it closes. A coroutine cancelled here answers
    its Call with the CancelledError, which is more than it would get from a
    process that simply exited underneath it.
    """
    pending = [task for task in all_tasks() if task is not current_task()]
    for task in pending:
        _ = task.cancel()
    _ = await gather(*pending, return_exceptions=True)


def _running() -> tuple[AbstractEventLoop, ThreadPoolExecutor]:
    """
    The loop and the thread pool, started on the first Call that needs them.

    An application whose Webview has no Api never starts either.

    A Call that arrives once the portal is closed is refused here, and the
    refusal travels back as the rejection of its Promise. Nothing else it could
    be given would be true: the window is going, so an answer could no longer
    be delivered even if the callable ran. A frontend mid-close therefore sees
    its last Calls fail, by name and with a reason, instead of hanging.
    """
    global _loop, _thread, _executor

    with _lock:
        if _closed:
            raise RuntimeError('The Bridge is closed, so the Call cannot be run.')

        if _loop is None:
            _loop = new_event_loop()
            _thread = Thread(target=_serve, args=(_loop,), name='dry-loop', daemon=True)
            _thread.start()

        if _executor is None:
            _executor = ThreadPoolExecutor(thread_name_prefix='dry-call')

        return _loop, _executor


def _serve(loop: AbstractEventLoop) -> None:
    """
    The loop's whole life, on a thread of its own.
    """
    set_event_loop(loop)
    loop.run_forever()


def _on_loop(
    name: str,
    awaitable: Awaitable[object],
    loop: AbstractEventLoop,
    completion: Completion,
) -> None:
    """
    Hands an awaitable to the loop and answers the Call with what it returns.

    A loop that has already stopped will not take it, and the Call is rejected
    with the reason instead: a rejection the frontend can see beats a Promise
    that never settles.
    """
    try:
        scheduled = run_coroutine_threadsafe(_awaited(awaitable), loop)
    except BaseException as error:
        _reject(name, error, completion)
        return

    _answer(name, scheduled, loop, completion)


async def _awaited(awaitable: Awaitable[object]) -> object:
    return await awaitable


def _answer(
    name: str,
    future: Future[Any],
    loop: AbstractEventLoop,
    completion: Completion,
) -> None:
    """
    Watches one step of a Call and answers it, keeping the step on the list a
    closing portal waits for.
    """

    def answered(future: Future[Any]) -> None:
        with _lock:
            _pending.discard(future)

        try:
            value = future.result()
        except BaseException as error:
            _reject(name, error, completion)
            return

        if isawaitable(value):
            _on_loop(name, value, loop, completion)
            return

        _resolve(name, value, completion)

    with _lock:
        _pending.add(future)

    future.add_done_callback(answered)


def _resolve(name: str, value: object, completion: Completion) -> None:
    """
    Answers a Call with the value its callable returned.

    A value outside the Bridge contract is refused as the Completion converts
    it, and the Call is rejected with that refusal instead — the frontend gets
    the TypeError explaining the way out, rather than a Promise that never
    settles.
    """
    try:
        completion.resolve(value)
    except BaseException as error:
        _LOGGER.error(
            "The Call to '%s' returned a value that cannot cross the Bridge.",
            name,
            exc_info=error,
        )
        _answer_with(name, error, completion)


def _reject(name: str, error: BaseException, completion: Completion) -> None:
    """
    Answers a Call with the exception that ended it. The exception's type name
    travels with it, so JavaScript can tell a ValueError from a
    PermissionError.
    """
    _LOGGER.error("The Call to '%s' raised.", name, exc_info=error)
    _answer_with(name, error, completion)


def _answer_with(name: str, error: BaseException, completion: Completion) -> None:
    try:
        completion.reject(error)
    except BaseException:
        _LOGGER.exception("The Call to '%s' could not be answered.", name)


def _hand_over(
    name: str,
    listener: Listener,
    value: object,
    loop: AbstractEventLoop,
    executor: ThreadPoolExecutor,
) -> None:
    """
    Runs one listener off the event-loop thread. The same choice `dispatch`
    makes for a Call — a coroutine function onto the loop, anything else into
    the pool — without the Completion, because there is nothing to answer.
    """
    if iscoroutinefunction(listener):
        try:
            coroutine = listener(value)
        except BaseException as error:
            _listener_failed(name, error)
            return
        _delivered(name, _scheduled(name, coroutine, loop), loop)
        return

    _delivered(name, executor.submit(listener, value), loop)


def _scheduled(
    name: str, awaitable: Awaitable[object], loop: AbstractEventLoop
) -> Future[Any] | None:
    try:
        return run_coroutine_threadsafe(_awaited(awaitable), loop)
    except BaseException as error:
        _listener_failed(name, error)
        return None


def _delivered(
    name: str, future: Future[Any] | None, loop: AbstractEventLoop
) -> None:
    """
    Watches one step of a delivery. The step joins the list a closing portal
    waits on, so a listener saving state on the way out gets the same grace
    period an in-flight Call gets. What it returns is dropped — an Event has no
    return path — except that an awaitable is finished first, so a callable
    with an `async def __call__` listens as an `async def` does.
    """
    if future is None:
        return

    def finished(future: Future[Any]) -> None:
        with _lock:
            _pending.discard(future)

        try:
            value = future.result()
        except BaseException as error:
            _listener_failed(name, error)
            return

        if isawaitable(value):
            _delivered(name, _scheduled(name, value, loop), loop)

    with _lock:
        _pending.add(future)

    future.add_done_callback(finished)


def _listener_failed(name: str, error: BaseException) -> None:
    """
    Records a listener that raised, with its traceback, and lets the delivery
    carry on to the others.
    """
    _LOGGER.error(
        "A listener for the Event '%s' raised.", name, exc_info=error
    )
