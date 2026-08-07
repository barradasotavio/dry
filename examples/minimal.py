from pathlib import Path

from dry import Webview

ICON_PATH = Path(__file__).parent / 'icon.ico'

webview = Webview()
webview.title = 'My Dry Webview'
webview.size = webview.min_size = (1200, 800)
webview.icon_path = ICON_PATH
webview.url = 'https://www.example.com'
webview.dev_tools = True
webview.run()
