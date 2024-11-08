from __future__ import annotations

from re import match
from typing import Any, Callable

from . import dry


class Webview:
    _title: str
    _min_size: tuple[int, int]
    _size: tuple[int, int]
    _decorations: bool | None = None
    _icon_path: str | None = None
    _html: str | None = None
    _url: str | None = None
    _api: dict[str, Callable[..., Any]] | None = None
    _dev_tools: bool | None = None

    def __init__(
        self,
        title: str = 'My Dry Webview',
        min_size: tuple[int, int] = (1152, 720),
        size: tuple[int, int] = (1280, 800),
        decorations: bool | None = True,
        icon_path: str | None = None,
        content: str = '<h1>Hello, World!</h1>',
        api: dict[str, Callable[..., Any]] | None = None,
        dev_tools: bool | None = False,
    ):
        """
        Initialize the webview window.

        :param title: The title of the webview window.
        :param min_size: The minimum size of the webview window.
        :param size: The size of the webview window.
        :param decorations: Whether window decorations are enabled.
        :param icon_path: The path to the icon of the webview window (only .ico).
        :param content: The content of the webview window, either an HTML or a URL.
        :param api: The functions being passed down to the webview window.
        :param dev_tools: Whether the developer tools are enabled.

        :type title: str
        :type min_size: tuple[int, int]
        :type size: tuple[int, int]
        :type decorations: bool | None
        :type icon_path: str | None
        :type content: str
        :type api: Mapping[str, Callable[..., Any]] | None
        :type dev_tools: bool | None

        :return: Webview
        """
        self.title = title
        self.min_size = min_size
        self.size = size
        self.decorations = decorations
        self.content = content
        self.api = api
        self.dev_tools = dev_tools
        self.icon_path = icon_path

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
    def decorations(self, decorations: bool | None) -> None:
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

    @property
    def content(self) -> str | None:
        """
        Get the content of the webview window.
        """
        return self._html or self._url

    @content.setter
    def content(self, content: str) -> None:
        """
        Set the content of the webview window, either an HTML or a URL.
        """
        is_url = bool(match(r'https?://[a-z0-9.-]+', content))
        self._url, self._html = (content, None) if is_url else (None, content)

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
    def dev_tools(self, dev_tools: bool | None) -> None:
        """
        Set whether the developer tools are enabled.
        """
        self._dev_tools = dev_tools

    def run(self):
        """
        Run the webview window, in a blocking loop.
        """
        dry.run(
            title=self.title,
            min_size=self.min_size,
            size=self.size,
            decorations=self.decorations,
            icon_path=self.icon_path,
            html=self._html,
            url=self._url,
            api=self.api,
            dev_tools=self.dev_tools,
        )
