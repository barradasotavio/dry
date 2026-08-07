"""Renders a Root: a local directory served to the Webview, so that the
stylesheet, the module import and the image an index.html names relatively all
resolve against the directory."""

from pathlib import Path

from dry import Webview

ICON_PATH = Path(__file__).parent / 'icon.ico'
ROOT_PATH = Path(__file__).parent / 'root'

if __name__ == '__main__':
    wv = Webview()
    wv.title = 'Root Example'
    wv.size = wv.min_size = (1080, 720)
    wv.icon_path = ICON_PATH
    wv.root = ROOT_PATH
    wv.dev_tools = True
    wv.run()
