from pathlib import Path

from dry import Webview

ICON_PATH = Path(__file__).parent / 'icon.ico'
HTML_PATH = Path(__file__).parent / 'titlebar.html'

with open(HTML_PATH, encoding='utf-8') as f:
    HTML = f.read()

if __name__ == '__main__':
    wv = Webview(
        title='Titlebar Example',
        app_id='com.example.dry.titlebar',
        size=(1080, 720),
        min_size=(1080, 720),
        decorations=False,
        icon_path=ICON_PATH,
        html=HTML,
        dev_tools=True,
    )
    wv.run()
