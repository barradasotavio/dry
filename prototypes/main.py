from dry import Webview
from pathlib import Path

HTML_PATH = Path(__file__).parent / "main.html"

with open(HTML_PATH, encoding="utf-8") as f:
    HTML = f.read()

def hello(*args):
    message = f"Hello {', '.join(args)}"
    print(message)
    return message

api = {
    "hello": hello,
}

if __name__ == "__main__":
    wv = Webview()
    wv.title = "Hello World"
    wv.content = HTML
    wv.api = api
    wv.run()