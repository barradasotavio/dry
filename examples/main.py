from datetime import datetime as dt
from pathlib import Path
from random import randint as random_integer
from typing import Any, Callable

from dry import Webview

ICON_PATH = Path(__file__).parent / 'icon.ico'
HTML_PATH = Path(__file__).parent / 'main.html'

with open(HTML_PATH, encoding='utf-8') as f:
    HTML = f.read()


def hello(name: str) -> str:
    hour = dt.now().hour
    time_of_day_greeting = (
        'Good morning'
        if 5 <= hour < 12
        else 'Good afternoon'
        if 12 <= hour < 18
        else 'Good evening'
        if 18 <= hour < 22
        else 'Good night'
    )
    greeting = ['Hello', 'Hi', 'Hey', 'Greetings', time_of_day_greeting][
        random_integer(0, 4)
    ]
    message = f'{greeting} from Python, {name}!'
    return message


def returns_none() -> None:
    return None


def returns_bytes() -> bytes:
    return b'Hello, World!'


def returns_dict():
    return {'a': 1, 'b': 2, 'c': 3}


def returns_list():
    return [1, 2, 3]


def returns_tuple():
    return (1, 2, 3)


def returns_set():
    return {1, 2, 3}


def returns_float():
    return 3.14


def returns_int():
    return 42


def returns_str():
    return 'Hello, World!'


def raises_exception():
    raise Exception('This is an exception')


api: dict[str, Callable[..., Any]] = {
    'hello': hello,
    'returnsNone': returns_none,
    'returnsBytes': returns_bytes,
    'returnsDict': returns_dict,
    'returnsList': returns_list,
    'returnsTuple': returns_tuple,
    'returnsSet': returns_set,
    'returnsFloat': returns_float,
    'returnsInt': returns_int,
    'returnsStr': returns_str,
    'raisesException': raises_exception,
}

if __name__ == '__main__':
    wv = Webview()
    wv.title = 'Hello World'
    wv.size = wv.min_size = (1080, 720)
    wv.icon_path = ICON_PATH.as_posix()
    wv.content = HTML
    wv.api = api
    wv.dev_tools = True
    wv.run()
