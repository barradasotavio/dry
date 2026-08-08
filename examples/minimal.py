from pathlib import Path

from dry import Webview

ICON_PATH = Path(__file__).parent / 'icon.ico'

webview = Webview(
    title='My Dry Webview',
    app_id='com.example.dry.minimal',
    size=(1200, 800),
    min_size=(1200, 800),
    icon_path=ICON_PATH,
    url='https://www.example.com',
    dev_tools=True,
)
webview.run()
