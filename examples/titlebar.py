from pathlib import Path

from dry import Webview

ICON_PATH = Path(__file__).parent / 'icon.ico'
HTML_PATH = Path(__file__).parent / 'titlebar.html'

with open(HTML_PATH, encoding='utf-8') as f:
    HTML = f.read()

if __name__ == '__main__':
    wv = Webview()
    wv.title = 'Hello World'
    wv.size = wv.min_size = (1080, 720)
    wv.decorations = False
    wv.icon_path = ICON_PATH.as_posix()
    wv.content = HTML
    wv.dev_tools = True
    wv.run()
