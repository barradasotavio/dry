from hashlib import sha256
from os import PathLike, environ
from pathlib import Path
from re import compile as compile_pattern
from sys import argv, executable, platform
from typing import Any, Callable, NamedTuple

from . import dry, portal
from .exceptions import WebviewError
from .portal import CloseHook, Listener

StrPath = str | PathLike[str]

# The prefix the Rust side reads a Root off. Everything after it is the absolute
# path of the directory to serve.
_ROOT_URL_PREFIX = 'localfile://'

# What an App id may look like. One path segment, starting with a letter or a
# digit, so it is a legal directory name on every platform Dry supports.
_APP_ID = compile_pattern(r'^[A-Za-z0-9][A-Za-z0-9._-]*$')

# Anything that is not safe inside a derived App id.
_NOT_IN_SLUG = compile_pattern(r'[^A-Za-z0-9]+')


def _user_data_directory() -> Path:
    """
    The directory the operating system keeps application data in.

    Not the temporary directory: a login session, a cookie jar and local
    storage should not live somewhere the system may clear between runs.
    """
    if platform == 'win32':
        local_app_data = environ.get('LOCALAPPDATA')
        if local_app_data:
            return Path(local_app_data)
        return Path.home() / 'AppData' / 'Local'
    if platform == 'darwin':
        return Path.home() / 'Library' / 'Application Support'
    xdg_data_home = environ.get('XDG_DATA_HOME')
    if xdg_data_home:
        return Path(xdg_data_home)
    return Path.home() / '.local' / 'share'


def _derive_app_id() -> str:
    """
    An App id for an application that did not declare one.

    Derived from the entry-point script rather than the window title, so that
    renaming the window keeps the session, and stable across runs of the same
    script. The short digest of the script's absolute path keeps two different
    `main.py` files from sharing a cookie jar.
    """
    script = argv[0] if argv else ''
    if script and not script.startswith('-'):
        path = Path(script).resolve()
        name, seed = path.stem, str(path)
    else:
        name, seed = 'python', executable or 'python'

    slug = _NOT_IN_SLUG.sub('-', name).strip('-').lower() or 'app'
    digest = sha256(seed.encode('utf-8')).hexdigest()[:8]
    return f'dry.{slug}.{digest}'


class WindowState(NamedTuple):
    """
    What the window is doing, in one reading.

    The answer `Webview.state()` gives. Every field is also a property of its
    own, and reading them one at a time is the same information — but a
    reading is taken at one instant, so the fields of a `WindowState` cannot
    contradict each other the way two properties read a moment apart can.

    `size` and `position` are logical pixels, the unit every dimension in Dry
    is in, and `size` is the area the frontend renders into — the same
    measurement `size=` sets, and the same number the page reads back as
    `window.innerWidth`.
    """

    maximized: bool
    minimized: bool
    fullscreen: bool
    visible: bool
    focused: bool
    size: tuple[int, int]
    position: tuple[int, int]


def _read_state(reading: dict[str, Any]) -> WindowState:
    """
    The mapping the extension module answers with, as the public reading.

    The two values that also travel as Events keep the shape they have there —
    `{width, height}` and `{x, y}` — because that shape is what a frontend
    reads. Python prefers a pair, so this is where they become one.
    """
    size: dict[str, Any] = reading['size']
    position: dict[str, Any] = reading['position']
    return WindowState(
        maximized=bool(reading['maximized']),
        minimized=bool(reading['minimized']),
        fullscreen=bool(reading['fullscreen']),
        visible=bool(reading['visible']),
        focused=bool(reading['focused']),
        size=(int(size['width']), int(size['height'])),
        position=(int(position['x']), int(position['y'])),
    )


class Webview:
    """
    One native window rendering a web frontend, and the Bridge to it.

    Every option is a keyword argument, and every one of them is also a
    property, because some are genuinely computed after construction:

        wv = Webview(title='My App', html='<h1>Hello</h1>')
        wv.run()

    The Webview renders exactly one Content, declared as exactly one of three
    mutually exclusive modes: an HTML string (`html`), a URL (`url`), or a Root
    (`root`) — a local directory served to the Webview so that relative assets
    resolve. Declaring more than one, or none, raises.

    The Bridge to that frontend carries two shapes, both ways. A Call returns a
    value: the frontend Calls a name in `api` and awaits what Python returns. An
    Event returns nothing and goes to every listener registered for its name:
    `wv.emit(name, value)` reaches the frontend's `window.dry.on(name, ...)`,
    and the page's `window.dry.emit(name, value)` reaches every listener
    registered through `wv.on(name, ...)`. Python does not Call the frontend —
    `eval_js` covers the rare case where a script has to run there.

    Assigning an attribute that is not a setting raises, so a typo cannot
    silently create one that never applies. So does assigning a setting that
    the Webview reads only while it is being built, once `run()` has been
    called and reading it again is no longer possible.

    The settings that are not fixed that way — `title`, `size`, `min_size`,
    `decorations` and `icon_path` — apply to the window as they are assigned,
    running or not. Beside them are the five states a window only has once it
    is on screen: `position`, `visible`, `maximized`, `minimized` and
    `fullscreen`, which raise before `run()` rather than pretending to be
    settings. Every one of these is a property, so changing the window and
    reading it back are the same word:

        wv.maximized = True
        wv.title = f'{document} — My App'
        wv.visible = False

    A change made this way is announced exactly as a change the user made:
    Dry reads the window once per turn of its event loop and emits the
    `window:` Events for whatever moved, so a frontend stays in sync without
    caring who asked. `wv.state()` answers the same reading in one piece for
    a caller that needs the current value rather than the change.

    Args:
        title: The window title. Purely cosmetic — it no longer decides where
            data is stored, so it may contain any character.
        size: The window dimensions in logical pixels, independent of display
            scaling. Assignable afterwards, which resizes the window.
        min_size: Minimum window dimensions in logical pixels.
        decorations: Whether to show the native title bar and borders. Without
            them the Webview draws its own resize edges, on every platform it
            supports.
        icon_path: Path to the window icon (.ico format).
        html: An HTML string to render.
        url: A URL to load.
        root: A local directory to serve, starting at its index.html.
        api: The names the frontend may Call, mapped to Python callables.
        dev_tools: Whether to enable the developer tools.
        app_id: A stable reverse-domain identifier, such as
            `com.example.myapp`, deciding where cookies, local storage and
            cache live. Derived from the entry-point script when not given.
        user_data_folder: Where that data is stored, overriding the location
            the App id chooses. Rarely needed.
        default: Called with any value outside the Bridge contract, exactly as
            `json.dumps(default=...)` calls it, and must return something
            inside the contract. Raises if it does not.

    Example:
        >>> wv = Webview(title='My App', html='<h1>Hello World</h1>')
        >>> wv.run()

        >>> wv = Webview(app_id='com.example.myapp', root='./dist')
        >>> wv.run()
    """

    __slots__ = (
        '_api',
        '_app_id',
        '_decorations',
        '_default',
        '_dev_tools',
        '_html',
        '_icon_path',
        '_min_size',
        '_on_close',
        '_root',
        '_running',
        '_size',
        '_title',
        '_url',
        '_user_data_folder',
    )

    def __init__(
        self,
        *,
        title: str = 'My Dry Webview',
        size: tuple[int, int] = (800, 600),
        min_size: tuple[int, int] = (800, 600),
        decorations: bool = True,
        icon_path: StrPath | None = None,
        html: str | None = None,
        url: str | None = None,
        root: StrPath | None = None,
        api: dict[str, Callable[..., Any]] | None = None,
        dev_tools: bool = False,
        app_id: str | None = None,
        user_data_folder: StrPath | None = None,
        default: Callable[[Any], Any] | None = None,
        on_close: CloseHook | None = None,
    ) -> None:
        self._running = False

        self._html: str | None = None
        self._url: str | None = None
        self._root: Path | None = None
        self._user_data_folder: str | None = None

        self.title = title
        self.size = size
        self.min_size = min_size
        self.decorations = decorations
        self.icon_path = icon_path
        if html is not None:
            self.html = html
        if url is not None:
            self.url = url
        if root is not None:
            self.root = root
        self.api = api
        self.dev_tools = dev_tools
        self.app_id = app_id if app_id is not None else _derive_app_id()
        if user_data_folder is not None:
            self.user_data_folder = user_data_folder
        self.default = default
        self.on_close = on_close

    def _refuse_late_assignment(self, setting: str) -> None:
        """
        Refuse a setting the Webview has already been built from.
        """
        if self._running:
            raise RuntimeError(
                f'{setting} is fixed at construction and the Webview is already '
                f'running, so assigning it now would change nothing. Pass '
                f'{setting} to Webview(...) instead.'
            )

    def _require_running(self, name: str) -> None:
        """
        Refuse a question only a window on screen can answer.
        """
        if not self._running:
            raise RuntimeError(
                f'{name} belongs to a window that is on screen, and this Webview '
                f'has not opened one yet. Ask for it once run() has, from inside '
                f'an Api callable, an Event listener or the close hook.'
            )

    def _reading(self) -> WindowState | None:
        """
        What the window is doing, or None when there is no window to ask.

        The window is opened by `run()` and read a moment later, so a callback
        on another thread can be quick enough to ask before the first reading
        exists. That is not worth an exception on a getter that has a
        perfectly good answer of its own to fall back on.
        """
        if not self._running:
            return None
        try:
            return _read_state(dry.window_state())
        except WebviewError:
            return None

    def state(self) -> WindowState:
        """
        What the window is doing right now, in one reading:

            if not wv.state().maximized:
                wv.maximized = True

        The complement to the window Events. They say what changed, and a
        listener that was not registered when it changed heard nothing; this
        says what is, for a frontend or a callback that has to draw a maximize
        button one way round before anybody touches the window.

        Answers from the last reading the event loop took, which is the reading
        every `window:` Event was a difference from, so the query and the
        Events can never disagree. A change assigned here applies on the next
        turn of that loop, so a reading taken in the same breath as an
        assignment may still be the state before it — wait for the Event that
        announces the change if the order matters.

        Raises if the Webview is not running: a window that does not exist has
        no size, and answering with the settings it will be built from would be
        a guess dressed as a measurement.
        """
        self._require_running('state()')
        return _read_state(dry.window_state())

    @property
    def title(self) -> str:
        """
        The window title.

        Assignable while the Webview is running, and the window is retitled.
        """
        return self._title

    @title.setter
    def title(self, title: str) -> None:
        if self._running:
            dry.set_window_title(title)
        self._title = title

    @property
    def min_size(self) -> tuple[int, int]:
        """
        The minimum window dimensions, in logical pixels.

        Assignable while the Webview is running. A minimum larger than the
        window grows the window to meet it, on every platform Dry supports.
        """
        return self._min_size

    @min_size.setter
    def min_size(self, width_and_height: tuple[int, int]) -> None:
        if self._running:
            dry.set_window_min_size(width_and_height)
        self._min_size = width_and_height

    @property
    def size(self) -> tuple[int, int]:
        """
        The window dimensions, in logical pixels.

        The size the window was built with until it is running, and what it
        currently measures afterwards — including a size the user dragged it
        to, because a property that reported only the last size Python asked
        for would go stale the first time anybody touched the window.

        Assigning it resizes the window, which reaches every listener of
        `window:resized` exactly as a drag does.
        """
        reading = self._reading()
        return self._size if reading is None else reading.size

    @size.setter
    def size(self, width_and_height: tuple[int, int]) -> None:
        if self._running:
            dry.set_window_size(width_and_height)
        self._size = width_and_height

    @property
    def decorations(self) -> bool:
        """
        Whether the native title bar and borders are shown.

        Assignable while the Webview is running. The resize edges an
        undecorated Webview draws over its own border are injected into the
        page as it loads, so a window that opened decorated and was undecorated
        afterwards has no resize edges until the Content is loaded again.
        """
        return self._decorations

    @decorations.setter
    def decorations(self, decorations: bool) -> None:
        if self._running:
            dry.set_window_decorations(decorations)
        self._decorations = decorations

    @property
    def icon_path(self) -> str | None:
        """
        The path to the window icon, if there is one.

        Assignable while the Webview is running. `None` puts the platform's
        default icon back, and an icon that cannot be read is logged rather
        than raised, exactly as at construction.
        """
        return self._icon_path

    @icon_path.setter
    def icon_path(self, icon_path: StrPath | None) -> None:
        resolved = None if icon_path is None else Path(icon_path).as_posix()
        if self._running:
            dry.set_window_icon(resolved)
        self._icon_path = resolved

    @property
    def position(self) -> tuple[int, int]:
        """
        Where the window sits on the desktop, in logical pixels.

        The outer position — the top-left corner of the window itself,
        decorations included — measured the same way `window:moved` reports it.

        Unlike `size`, there is no setting to fall back on: a window has no
        position until the platform gives it one, so reading this before
        `run()` raises rather than inventing a corner. Assigning it moves the
        window, and a platform that refuses part of the move (macOS will not
        lift a window above the menu bar) simply reports where the window
        actually went.
        """
        self._require_running('position')
        return self.state().position

    @position.setter
    def position(self, x_and_y: tuple[int, int]) -> None:
        self._require_running('position')
        dry.set_window_position(x_and_y)

    @property
    def visible(self) -> bool:
        """
        Whether the window is on screen.

        Setting it to `False` takes the window off the screen without closing
        it — no titlebar button, no dock entry, no close hook — and `True` puts
        it back. The Webview goes on running either way: the event loop still
        turns, the Bridge still carries Calls and Events, and the page keeps
        its state. That is what makes a tray application possible, where
        closing the window would end the process.

        Hiding and showing reach `window:hidden` and `window:shown`, whoever
        asked for it. A minimized window is minimized, not hidden: it is still
        `visible`, because the user can still see it in the dock or the
        taskbar and put it back.
        """
        self._require_running('visible')
        return self.state().visible

    @visible.setter
    def visible(self, visible: bool) -> None:
        self._require_running('visible')
        dry.set_window_visible(visible)

    @property
    def maximized(self) -> bool:
        """
        Whether the window fills the screen it is on, keeping its decorations.

        Assigning `False` restores the size the window had before it was
        maximized. Reaches `window:maximized` and `window:unmaximized`.
        """
        self._require_running('maximized')
        return self.state().maximized

    @maximized.setter
    def maximized(self, maximized: bool) -> None:
        self._require_running('maximized')
        dry.set_window_maximized(maximized)

    @property
    def minimized(self) -> bool:
        """
        Whether the window is minimized to the dock or the taskbar.

        Assigning `False` restores it. Reaches `window:minimized` and
        `window:restored`.
        """
        self._require_running('minimized')
        return self.state().minimized

    @minimized.setter
    def minimized(self, minimized: bool) -> None:
        self._require_running('minimized')
        dry.set_window_minimized(minimized)

    @property
    def fullscreen(self) -> bool:
        """
        Whether the window has taken over the screen it is on.

        Borderless fullscreen on the window's current monitor, which is the
        one a desktop application wants — it does not change the display's
        resolution under the user.

        This is the one window state with no Event of its own: macOS reports
        entering fullscreen as a resize and a move, so a name for it could not
        be told from what the platform already sends. Read it here, or read
        `state().fullscreen` alongside everything else.
        """
        self._require_running('fullscreen')
        return self.state().fullscreen

    @fullscreen.setter
    def fullscreen(self, fullscreen: bool) -> None:
        self._require_running('fullscreen')
        dry.set_window_fullscreen(fullscreen)

    def _refuse_second_mode(self, mode: str) -> None:
        """
        Refuse a Content mode when another one is already declared.
        """
        declared = {
            'html': self._html is not None,
            'url': self._url is not None,
            'root': self._root is not None,
        }
        conflict = next(
            (other for other, is_set in declared.items() if is_set and other != mode),
            None,
        )
        if conflict is not None:
            raise ValueError(
                f'Content is already declared as {conflict}, so it cannot also be '
                f'declared as {mode}. A Webview renders exactly one of html, url or '
                f'root. Set webview.{conflict} = None first.'
            )

    @property
    def html(self) -> str | None:
        """
        The HTML string the Webview renders, if that is its Content.
        """
        return self._html

    @html.setter
    def html(self, html: str | None) -> None:
        self._refuse_late_assignment('html')
        if html is None:
            self._html = None
            return
        if not isinstance(html, str):  # pyright: ignore[reportUnnecessaryIsInstance]
            raise TypeError(f'html must be a str, got {type(html).__name__}.')
        self._refuse_second_mode('html')
        self._html = html

    @property
    def url(self) -> str | None:
        """
        The URL the Webview loads, if that is its Content.
        """
        return self._url

    @url.setter
    def url(self, url: str | None) -> None:
        self._refuse_late_assignment('url')
        if url is None:
            self._url = None
            return
        if not isinstance(url, str):  # pyright: ignore[reportUnnecessaryIsInstance]
            raise TypeError(f'url must be a str, got {type(url).__name__}.')
        self._refuse_second_mode('url')
        self._url = url

    @property
    def root(self) -> Path | None:
        """
        The directory served to the Webview, if that is its Content.
        """
        return self._root

    @root.setter
    def root(self, root: StrPath | None) -> None:
        self._refuse_late_assignment('root')
        if root is None:
            self._root = None
            return
        directory = Path(root).expanduser()
        if not directory.exists():
            raise FileNotFoundError(f'root does not exist: {directory}')
        if not directory.is_dir():
            raise NotADirectoryError(
                f'root must be a directory, not a file: {directory}. To render a '
                f'single file, read it and set webview.html instead.'
            )
        self._refuse_second_mode('root')
        self._root = directory.resolve()

    @property
    def api(self) -> dict[str, Callable[..., Any]] | None:
        """
        The names the frontend may Call, mapped to Python callables.

        A Call's arguments are checked against the callable's declared
        annotations before it runs, so a frontend that passes the wrong type is
        told which parameter and what arrived. An unannotated parameter, or one
        whose annotation cannot be resolved, is left unchecked.
        """
        return self._api

    @api.setter
    def api(self, api: dict[str, Callable[..., Any]] | None) -> None:
        self._refuse_late_assignment('api')
        self._api = api

    @property
    def dev_tools(self) -> bool:
        """
        Whether the developer tools are enabled.
        """
        return self._dev_tools

    @dev_tools.setter
    def dev_tools(self, dev_tools: bool) -> None:
        self._refuse_late_assignment('dev_tools')
        self._dev_tools = dev_tools

    @property
    def default(self) -> Callable[[Any], Any] | None:
        """
        The hook called for a value outside the Bridge contract.
        """
        return self._default

    @default.setter
    def default(self, default: Callable[[Any], Any] | None) -> None:
        self._refuse_late_assignment('default')
        if default is not None and not callable(default):  # pyright: ignore[reportUnnecessaryComparison]
            raise TypeError(f'default must be callable, got {type(default).__name__}.')
        self._default = default

    @property
    def app_id(self) -> str:
        """
        The stable identifier deciding where this application's data lives.
        """
        return self._app_id

    @app_id.setter
    def app_id(self, app_id: str) -> None:
        self._refuse_late_assignment('app_id')
        if not isinstance(app_id, str):  # pyright: ignore[reportUnnecessaryIsInstance]
            raise TypeError(f'app_id must be a str, got {type(app_id).__name__}.')
        if not _APP_ID.match(app_id):
            raise ValueError(
                f'app_id must be one path segment of letters, digits, dots, '
                f'dashes and underscores, starting with a letter or a digit, '
                f'such as com.example.myapp. Got: {app_id!r}.'
            )
        self._app_id = app_id

    @property
    def user_data_folder(self) -> str:
        """
        Where cookies, local storage and cache are kept.

        The App id under the operating system's user-data directory, unless an
        explicit folder was given.
        """
        if self._user_data_folder is not None:
            return self._user_data_folder
        return str(_user_data_directory() / self._app_id)

    @user_data_folder.setter
    def user_data_folder(self, user_data_folder: StrPath) -> None:
        self._refuse_late_assignment('user_data_folder')
        self._user_data_folder = str(Path(user_data_folder).expanduser())

    def _content(self) -> tuple[str | None, str | None]:
        """
        Resolve the declared Content into the html and url the Rust side reads.

        A Root travels as a url under the internal protocol, carrying the
        directory to serve.
        """
        declared = [
            mode
            for mode, value in (
                ('html', self._html),
                ('url', self._url),
                ('root', self._root),
            )
            if value is not None
        ]

        if not declared:
            raise ValueError(
                'No content declared. A Webview renders exactly one of html, url or '
                'root: pass html=, url= or root= to Webview(...), or set the '
                'matching property before run().'
            )

        if len(declared) > 1:
            raise ValueError(
                f'Content is declared more than once, as {" and ".join(declared)}. '
                f'A Webview renders exactly one of html, url or root.'
            )

        if self._root is not None:
            return None, f'{_ROOT_URL_PREFIX}{self._root.as_posix()}'

        return self._html, self._url

    @property
    def on_close(self) -> CloseHook | None:
        """
        The hook asked before the Webview closes.

        Returning `False` refuses the close and leaves the window open, so an
        application can prompt about unsaved work. Anything else allows it, and
        a hook that only saves state need return nothing. A hook that raises has
        not made a decision, so the close goes ahead and the exception is logged.

        The hook runs on the event-loop thread while the window is held still. A
        coroutine function works too, awaited on Dry's loop.
        """
        return self._on_close

    @on_close.setter
    def on_close(self, on_close: CloseHook | None) -> None:
        self._refuse_late_assignment('on_close')
        if on_close is not None and not callable(on_close):  # pyright: ignore[reportUnnecessaryComparison]
            raise TypeError(
                f'on_close must be callable, got {type(on_close).__name__}.'
            )
        self._on_close = on_close

    def on(self, name: str, listener: Listener) -> Listener:
        """
        Register a listener for the Event of that name, and return it:

            wv.on('form-dirty', remember)

        Every listener registered for a name receives every Event carrying it,
        in the order they registered — but each runs off the thread that draws
        the window, on Dry's loop or in its pool, so they overlap and finish in
        any order. Two listeners sharing state must make that state
        thread-safe, exactly as an Api must (ADR-0001).

        A listener takes the Event's value and returns nothing anybody reads:
        an Event has no return path. One that raises is logged on `dry.bridge`
        with its traceback, and the others still get theirs.

        Registering costs nothing and needs no window, so a listener may be
        registered before `run()`. A name beginning with `window:` is Dry's
        own — listening for one is exactly how the window Events are heard.
        """
        portal.listen(name, listener)
        return listener

    def off(self, name: str, listener: Listener) -> None:
        """
        Take one registration of a listener off that name. A listener that was
        never registered is not an error.
        """
        portal.unlisten(name, listener)

    def emit(self, name: str, value: Any = None) -> None:
        """
        Emit an Event to the frontend, where every listener registered for the
        name through `window.dry.on` receives it.

        Returns as soon as the Event is on its way, and returns nothing: that
        is what separates an Event from a Call. An Event nobody is listening
        for is a no-op, not an error.

        The value crosses under the Bridge contract, `default=` hook included,
        so anything outside it raises here rather than arriving mangled. Safe
        from any thread and from inside any callback; before `run()` there is
        no frontend to reach, and it raises.

        A name beginning with `window:` is reserved for Dry's own window
        Events and raises: listen for those, do not emit them.
        """
        dry.emit_event(name, value)

    def eval_js(self, script: str) -> None:
        """
        Evaluate a script in the page, and read nothing back.

        The escape hatch for the one quadrant the Bridge deliberately does not
        have: a Python-to-frontend Call with a return value would be an await
        on this side that never resolves if the page navigates or hangs. When
        the answer matters, have the frontend Call Python instead.
        """
        dry.eval_js(script)

    def run(self) -> None:
        """
        Run the webview window, in a blocking loop.

        This call owns the process: the GUI event loop takes the main thread and
        does not give it back, and closing the window exits the interpreter, so
        nothing after `run()` executes. An application therefore cannot make
        `asyncio.run(main())` its entry point — Dry starts an asyncio loop of its
        own on a background thread, and the developer's async code lives inside
        Api callbacks, which are awaited on that loop.

        A callback declared with `async def` is scheduled onto that loop, and any
        other callback runs in a thread pool, so a slow one no longer freezes the
        window. Both consequences follow: callbacks run concurrently, and an Api
        whose callables share state must make that state thread-safe. See
        ADR-0001.
        """
        html, url = self._content()

        user_data_folder = self.user_data_folder
        Path(user_data_folder).mkdir(parents=True, exist_ok=True)

        # From here the settings below have been read, and reading them again
        # is not something that happens: the event loop takes the main thread
        # and does not give it back. The flag stands until the window fails to
        # open, which is the only way this call returns.
        portal.on_close(self._on_close)

        self._running = True
        try:
            dry.run(
                {
                    'title': self._title,
                    'min_size': self._min_size,
                    'size': self._size,
                    'decorations': self._decorations,
                    'icon_path': self._icon_path,
                    'html': html,
                    'url': url,
                    'api': self._api,
                    'dev_tools': self._dev_tools,
                    'user_data_folder': user_data_folder,
                    'default': self._default,
                }
            )
        finally:
            self._running = False
