import dry
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
    dry.run(
        title="Webview Example",
        min_width=1152,
        min_height=720,
        width=1280,
        height=800,
        html=HTML,
        api=api,
    )