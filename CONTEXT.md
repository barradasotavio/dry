# Dry

A Python library that opens one native window rendering a web frontend, plus a bridge between that frontend and Python. Anything a frontend legitimately needs from its host window is in scope; general application concerns such as filesystem access are not — the developer wires those through the Bridge.

## Language

**Webview**:
The single window and the web content it renders, taken together. The library's one public object.
_Avoid_: App, window (the window alone is not the Webview)

**Bridge**:
The two-way channel between Python and the frontend. Carries exactly two message shapes — Call and Event — and both travel in both directions.
_Avoid_: IPC, RPC

**Call**:
A Bridge message that returns a value to its sender. Reaches a Python callable registered in the Api, and resolves a JavaScript Promise.
_Avoid_: Request, invoke, command

**Event**:
A Bridge message that returns nothing, delivered to every listener registered for its name. Window events are Events with reserved names, on the same bus as any other.
_Avoid_: Message, signal, notification, emit

**Api**:
The mapping of names to Python callables the frontend may Call.
_Avoid_: Handlers, bindings, exports

**Content**:
What the Webview renders. Exactly one of three explicit modes — an HTML string, a URL, or a Root.
_Avoid_: Source, page

**Root**:
A local directory served to the Webview over an internal protocol, so relative assets resolve. The Content mode for a compiled frontend.
_Avoid_: Static files, assets folder, file path

**Drag region**:
An element marked `data-drag-region` that moves the window when dragged, standing in for a native titlebar. Its whole subtree drags with it, except where a descendant marked `data-no-drag-region` opts its own subtree back out.
_Avoid_: Titlebar, header

**Bridge contract**:
The closed set of values that may cross the Bridge: the JSON data model, following `json.dumps` / `json.loads` semantics. A value outside it raises rather than converting silently. See [ADR-0002](./docs/adr/0002-the-bridge-contract-is-the-json-data-model.md).
_Avoid_: Type mapping, serialization rules

**App id**:
A stable reverse-domain identifier for the application, such as `com.example.myapp`. Determines where the Webview stores cookies, local storage and cache, so that data survives a change of title.
_Avoid_: App name, bundle id
