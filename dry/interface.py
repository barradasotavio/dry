from pathlib import Path
from re import match
from tempfile import gettempdir
from typing import Any, Callable

from . import dry


class Webview:
    """
    A class that provides a simple interface for creating and managing a webview window.

    The Webview class allows you to create a desktop application window that can display
    web content, either from a URL or HTML string. It provides controls for window
    properties like size, title, decorations, and developer tools.

    Attributes:
        title (str): The window title. Defaults to 'My Dry Webview'.
        min_size (tuple[int, int]): Minimum window dimensions (width, height).
        size (tuple[int, int]): Initial window dimensions (width, height).
        decorations (bool): Whether to show window decorations (title bar, borders).
        icon_path (str | None): Path to the window icon file (.ico format).
        content (str): HTML content or URL to display in the window.
        api (dict[str, Callable]): JavaScript-accessible Python functions.
        dev_tools (bool): Whether to enable developer tools.
        user_data_folder (str): Path to store user data. Defaults to temp folder.

    Example:
        >>> wv = Webview()
        >>> wv.title = "My App"
        >>> wv.content = "<h1>Hello World</h1>"
        >>> wv.run()
    """

    _title: str = 'My Dry Webview'
    _min_size: tuple[int, int] = (800, 600)
    _size: tuple[int, int] = (800, 600)
    _decorations: bool = True
    _icon_path: str | None = None
    _html: str | None = None
    _url: str | None = None
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
    def icon_path(self, icon_path: str | None) -> None:
        """
        Set the path to the icon of the webview window (only .ico).
        """
        self._icon_path = icon_path

    @property
    def content(self) -> str | None:
        """
        Get the content of the webview window.
        """
        return self._html or self._url or '<h1>Hello, World!</h1>'

    @content.setter
    def content(self, content: str) -> None:
        """
        Set the content of the webview window, either an HTML, a URL, or a file path.
        """
        is_url = bool(match(r'https?://[a-z0-9.-]+', content))
        if is_url:
            self._url, self._html = content, None
        elif Path(content).is_file():
            # For local files, use a custom protocol with the full path
            # This avoids issues with file:// URIs in Wry
            file_path = Path(content).resolve().as_posix()
            self._url = f"localfile://{file_path}"
            self._html = None
        else:
            self._url, self._html = None, content

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
    def user_data_folder(self, user_data_folder: str) -> None:
        """
        Set the user data folder path.
        """
        self._user_data_folder = user_data_folder

    def run(self):
        """
        Run the webview window, in a blocking loop.
        """
        dry.run(
            {
                'title': self.title,
                'min_size': self.min_size,
                'size': self.size,
                'decorations': self.decorations,
                'icon_path': self.icon_path,
                'html': self._html,
                'url': self._url,
                'api': self.api,
                'dev_tools': self.dev_tools,
                'user_data_folder': self.user_data_folder,
            }
        )
