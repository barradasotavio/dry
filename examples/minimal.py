from pathlib import Path
from dry import Webview

ICON_PATH = Path(__file__).parent / 'icon.ico'

webview = Webview()
webview.title = 'My Dry Webview'
webview.size = webview.min_size = (1200, 800)
webview.icon_path = ICON_PATH.as_posix()
webview.content = 'https://www.example.com' or '<h1>Hello, World!</h1>'
webview.dev_tools = True
webview.run()
