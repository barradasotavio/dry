from __future__ import annotations
from re import match
from . import dry
from typing import Callable, Mapping, Union, TypeAlias

DryType: TypeAlias = Union[
    bool,
    int,
    float,
    str,
    list[bool],
    list[int],
    list[float],
    list[str],
    list[bool | int],
    list[bool | float],
    list[bool | str],
    list[int | float],
    list[int | str],
    list[float | str],
    list[bool | int | float],
    list[bool | int | str],
    list[bool | float | str],
    list[int | float | str],
    list[bool | int | float | str],
    list['DryType'],
    Mapping[bool, 'DryType'],
    Mapping[int, 'DryType'],
    Mapping[str, 'DryType'],
    Mapping[bool | int, 'DryType'],
    Mapping[bool | str, 'DryType'],
    Mapping[int | str, 'DryType'],
]

DryFunction: TypeAlias = Callable[..., DryType]


class Webview:
    _title: str
    _min_size: tuple[int, int]
    _size: tuple[int, int]
    _html: str
    _url: str | None
    _api: Mapping[str, DryFunction]

    def __init__(
        self,
        title: str = 'Webview Window',
        min_size: tuple[int, int] = (1152, 720),
        size: tuple[int, int] = (1280, 800),
        content: str = '<h1>Hello World</h1>',
    ):
        self.title = title
        self.min_size = (min_size[0], min_size[1])
        self.size = size
        self.content = content

    @property
    def title(self):
        """
        Get the title of the webview window.
        """
        return self._title

    @title.setter
    def title(self, title: str):
        """
        Set the title of the webview window.
        """
        self._title = title

    @property
    def min_size(self):
        """
        Get the minimum size of the webview window.
        """
        return self._min_size

    @min_size.setter
    def min_size(self, width_and_height: tuple[int, int]):
        """
        Set the minimum size of the webview window.
        """
        self._min_size = width_and_height

    @property
    def size(self):
        """
        Get the size of the webview window.
        """
        return self._size

    @size.setter
    def size(self, width_and_height: tuple[int, int]):
        """
        Set the size of the webview window.
        """
        self._size = width_and_height

    @property
    def content(self):
        """
        Get the content of the webview window.
        """
        return self._html or self._url

    @content.setter
    def content(self, content: str):
        """
        Set the content of the webview window, either an HTML or a URL.
        """
        is_url = match(r'https?://[a-z0-9.-]+', content)
        if is_url:
            raise ValueError('Setting url is not supported yet.')
            self._url = content
            self._html = None
        else:
            self._url = None
            self._html = content

    @property
    def api(self):
        """
        Get the functions being passed down to the webview window.
        """
        return self._api

    @api.setter
    def api(self, api: Mapping[str, DryFunction]) -> None:
        """
        Set the functions being passed down to the webview window.
        """
        self._api = api

    def run(self):
        """
        Run the webview window, in a blocking loop.
        """
        dry.run(
            title=self.title,
            min_size=self.min_size,
            size=self.size,
            html=self._html,
            api=self._api,
        )
