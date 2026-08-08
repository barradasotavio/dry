"""Renders a Root: a local directory served to the Webview, so that the
stylesheet, the module import and the image an index.html names relatively all
resolve against the directory."""

from pathlib import Path

from dry import Webview

ICON_PATH = Path(__file__).parent / 'icon.ico'
ROOT_PATH = Path(__file__).parent / 'root'

if __name__ == '__main__':
    wv = Webview(
        title='Root Example',
        app_id='com.example.dry.root',
        size=(1080, 720),
        min_size=(1080, 720),
        icon_path=ICON_PATH,
        root=ROOT_PATH,
        dev_tools=True,
    )
    wv.run()
