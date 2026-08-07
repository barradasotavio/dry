"""
The failures Dry reports.

Every way this library can fail arrives as a `DryError`, so an application can
catch what it means rather than catching everything:

    from dry import Webview
    from dry.exceptions import WebviewError

    try:
        Webview().run()
    except WebviewError as error:
        ...

The classes themselves are defined by the extension module; this is where they
are named.
"""

from .dry import BridgeError, DryError, PanicError, WebviewError

__all__ = ['BridgeError', 'DryError', 'PanicError', 'WebviewError']
