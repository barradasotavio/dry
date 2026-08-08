from os import PathLike
from pathlib import Path
from tempfile import gettempdir
from typing import Any, Callable

from . import dry

StrPath = str | PathLike[str]

# The prefix the Rust side reads a Root off. Everything after it is the absolute
# path of the directory to serve.
_ROOT_URL_PREFIX = 'localfile://'


class Webview:
    """
    A class that provides a simple interface for creating and managing a webview window.

    The Webview renders exactly one Content, declared as exactly one of three
    mutually exclusive modes: an HTML string (`html`), a URL (`url`), or a Root
    (`root`) — a local directory served to the Webview so that relative assets
    resolve. Declaring more than one, or none, raises.

    Attributes:
        title (str): The window title. Defaults to 'My Dry Webview'.
        min_size (tuple[int, int]): Minimum window dimensions (width, height).
        size (tuple[int, int]): Initial window dimensions (width, height).
        decorations (bool): Whether to show window decorations (title bar, borders).
        icon_path (str | PathLike[str] | None): Path to the window icon (.ico format).
        html (str | None): An HTML string to render.
        url (str | None): A URL to load.
        root (Path | None): A local directory to serve, starting at its index.html.
        api (dict[str, Callable]): JavaScript-accessible Python functions.
        dev_tools (bool): Whether to enable developer tools.
        user_data_folder (str): Path to store user data. Defaults to temp folder.

    Example:
        >>> wv = Webview()
        >>> wv.title = "My App"
        >>> wv.html = "<h1>Hello World</h1>"
        >>> wv.run()

        >>> wv = Webview()
        >>> wv.root = "./dist"
        >>> wv.run()
    """

    _title: str = 'My Dry Webview'
    _min_size: tuple[int, int] = (800, 600)
    _size: tuple[int, int] = (800, 600)
    _decorations: bool = True
    _icon_path: str | None = None
    _html: str | None = None
    _url: str | None = None
    _root: Path | None = None
    _api: dict[str, Callable[..., Any]] | None = None
    _dev_tools: bool = False
    _user_data_folder: str | None = None

    @property
    def title(self) -> str:
        """
        Get the title of the webview window.
        """
        return self._title

    @title.setter
    def title(self, title: str) -> None:
        """
        Set the title of the webview window.
        """
        self._title = title

    @property
    def min_size(self) -> tuple[int, int]:
        """
        Get the minimum size of the webview window.
        """
        return self._min_size

    @min_size.setter
    def min_size(self, width_and_height: tuple[int, int]) -> None:
        """
        Set the minimum size of the webview window.
        """
        self._min_size = width_and_height

    @property
    def size(self) -> tuple[int, int]:
        """
        Get the size of the webview window.
        """
        return self._size

    @size.setter
    def size(self, width_and_height: tuple[int, int]) -> None:
        """
        Set the size of the webview window.
        """
        self._size = width_and_height

    @property
    def decorations(self) -> bool | None:
        """
        Get whether window decorations are enabled.
        """
        return self._decorations

    @decorations.setter
    def decorations(self, decorations: bool) -> None:
        """
        Set whether window decorations are enabled.
        """
        self._decorations = decorations

    @property
    def icon_path(self) -> str | None:
        """
        Get the path to the icon of the webview window.
        """
        return self._icon_path

    @icon_path.setter
    def icon_path(self, icon_path: StrPath | None) -> None:
        """
        Set the path to the icon of the webview window (only .ico).
        """
        self._icon_path = None if icon_path is None else Path(icon_path).as_posix()

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
        Get the HTML string the webview window renders, if that is its Content.
        """
        return self._html

    @html.setter
    def html(self, html: str | None) -> None:
        """
        Render an HTML string. Refused if a url or a root is already declared.
        """
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
        Get the URL the webview window loads, if that is its Content.
        """
        return self._url

    @url.setter
    def url(self, url: str | None) -> None:
        """
        Load a URL. Refused if an html or a root is already declared.
        """
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
        Get the directory served to the webview window, if that is its Content.
        """
        return self._root

    @root.setter
    def root(self, root: StrPath | None) -> None:
        """
        Serve a local directory, starting at its index.html, so that relative
        assets resolve. Refused if an html or a url is already declared.
        """
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
        Get the functions being passed down to the webview window.
        """
        return self._api

    @api.setter
    def api(self, api: dict[str, Callable[..., Any]] | None) -> None:
        """
        Set the functions being passed down to the webview window.
        """
        self._api = api

    @property
    def dev_tools(self) -> bool | None:
        """
        Get whether the developer tools are enabled.
        """
        return self._dev_tools

    @dev_tools.setter
    def dev_tools(self, dev_tools: bool) -> None:
        """
        Set whether the developer tools are enabled.
        """
        self._dev_tools = dev_tools

    @property
    def user_data_folder(self) -> str:
        """
        Get the user data folder path.
        """
        if self._user_data_folder is None:
            self._user_data_folder = str(Path(gettempdir()) / self.title)
        return self._user_data_folder

    @user_data_folder.setter
    def user_data_folder(self, user_data_folder: StrPath) -> None:
        """
        Set the user data folder path.
        """
        self._user_data_folder = Path(user_data_folder).as_posix()

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
                'root: set webview.html to an HTML string, webview.url to a URL, or '
                'webview.root to a directory.'
            )

        if len(declared) > 1:
            raise ValueError(
                f'Content is declared more than once, as {" and ".join(declared)}. '
                f'A Webview renders exactly one of html, url or root.'
            )

        if self._root is not None:
            return None, f'{_ROOT_URL_PREFIX}{self._root.as_posix()}'

        return self._html, self._url

    def run(self):
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

        dry.run(
            {
                'title': self.title,
                'min_size': self.min_size,
                'size': self.size,
                'decorations': self.decorations,
                'icon_path': self.icon_path,
                'html': html,
                'url': url,
                'api': self.api,
                'dev_tools': self.dev_tools,
                'user_data_folder': self.user_data_folder,
            }
        )
