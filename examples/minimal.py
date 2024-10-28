from dry import Webview

webview = Webview()
webview.title = 'Hello, World!'
webview.content = 'https://www.example.com' or '<h1>Hello, World!</h1>'
webview.run()
