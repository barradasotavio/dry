from datetime import datetime as dt
from pathlib import Path
from random import randint as random_integer

from dry import DryFunction, Webview

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


def add(*args: int) -> int:
    result = sum(args)
    return result


def get_person_info(
    name: str,
) -> dict[str, str | int | bool | float | list[str]]:
    return {
        'name': name,
        'age': random_integer(0, 122),
        'city': ['Brasília', 'Washington', 'Tokyo', 'Buenos Aires', 'Paris'][
            random_integer(0, 4)
        ],
        'has_children': [True, False][random_integer(0, 1)],
        'has_pets': [True, False][random_integer(0, 1)],
        'pronouns': ['he/him', 'she/her', 'they/them'][random_integer(0, 2)],
        'money': random_integer(0, 1_000_000_000_000)
        / random_integer(1, 1_000),
    }


api: dict[str, DryFunction] = {
    'hello': hello,
    'add': add,
    'getPersonInfo': get_person_info,
}

if __name__ == '__main__':
    wv = Webview()
    wv.title = 'Hello World'
    wv.size = wv.min_size = (1080, 720)
    wv.content = HTML
    wv.api = api
    wv.run()
