from dry import Webview

webview = Webview()
webview.title = 'My Dry Webview'
webview.size = webview.min_size = (1200, 800)
webview.content = 'https://www.example.com' or '<h1>Hello, World!</h1>'
webview.dev_tools = True
webview.run()
