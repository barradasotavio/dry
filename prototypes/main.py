from typing import Any, Callable
from dry import Webview
from pathlib import Path

HTML_PATH = Path(__file__).parent / "main.html"

with open(HTML_PATH, encoding="utf-8") as f:
    HTML = f.read()

def hello(*args: str) -> str:
    message = f"Hello {', '.join(args)}"
    return message

def add(*args: int) -> int:
    result = sum(args)
    return result

def get_person_info(name: str | None = None) -> dict[str, Any] | None:
    if name is None:
        return None
    return {
        "name": name,
        "age": 31,
        "city": "Brasília",
        "has_children": False,
        "has_pets": True,
        "pronouns": ["he", "him", "his"],
        "money": 200_000.50,
    }

api: dict[str, Callable[..., Any]] = {
    "hello": hello,
    "add": add,
    "getPersonInfo": get_person_info
}

if __name__ == "__main__":
    wv = Webview()
    wv.title = "Hello World"
    wv.content = HTML
    wv.api = api
    wv.run()