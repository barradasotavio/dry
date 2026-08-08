"""
Where a Call runs.

The GUI event loop owns the main thread — on macOS that is an AppKit
requirement, not a preference — so a callback that runs there holds the window
still for its whole duration: no repaint, no input, no second Call. The portal
takes every Call off that thread. A coroutine callback is scheduled onto an
asyncio loop Dry owns on a daemon thread, and a plain callback goes to a thread
pool. Both answer through the same Completion, which carries the reply back
across the Bridge.

Two consequences, recorded in ADR-0001. Dry owns the process, so an application
cannot make `asyncio.run(main())` its entry point — the developer's async code
lives inside callbacks, on the loop this module runs. And callbacks now run
concurrently, so an Api whose callables share state must make that state
thread-safe itself.

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
from concurrent.futures import Future, ThreadPoolExecutor
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


_lock = Lock()
_loop: AbstractEventLoop | None = None
_thread: Thread | None = None
_executor: ThreadPoolExecutor | None = None
_closed = False


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


def shutdown() -> None:
    """
    Closes the portal: no further Call is accepted, the Calls already running
    in the thread pool are waited for, and the loop stops.

    Because `tao` exits the process directly, an orderly close is Dry's to
    arrange — see ADR-0001. Nothing calls this yet; the close hook that will is
    its own piece of work.
    """
    global _loop, _thread, _executor, _closed

    with _lock:
        loop, thread, executor = _loop, _thread, _executor
        _loop, _thread, _executor = None, None, None
        _closed = True

    if executor is not None:
        executor.shutdown(wait=True)

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
    """
    _answer(name, run_coroutine_threadsafe(_awaited(awaitable), loop), loop, completion)


async def _awaited(awaitable: Awaitable[object]) -> object:
    return await awaitable


def _answer(
    name: str,
    future: Future[Any],
    loop: AbstractEventLoop,
    completion: Completion,
) -> None:
    def answered(future: Future[Any]) -> None:
        try:
            value = future.result()
        except BaseException as error:
            _reject(name, error, completion)
            return

        if isawaitable(value):
            _on_loop(name, value, loop, completion)
            return

        _resolve(name, value, completion)

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
