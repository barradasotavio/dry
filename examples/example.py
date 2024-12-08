from dry import Webview, send_message

def greet():
    print('Hello, World! from Python to console')
    send_message('Hello, World! from Python to Rust')

def main():
    webview = Webview()
    webview.api = {'greet': greet}
    webview.content = '<h1>Hello, World! from Python to Webview</h1><button onclick="window.api.greet()">Greet</button>'
    webview.run()

if __name__ == '__main__':
    main()