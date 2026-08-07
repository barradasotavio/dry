from logging import NullHandler, getLogger

from .exceptions import BridgeError, DryError, PanicError, WebviewError
from .interface import Webview

__all__ = ['BridgeError', 'DryError', 'PanicError', 'Webview', 'WebviewError']

# Dry writes to no stream of its own. Records go to `dry` and its children,
# `dry.webview` and `dry.bridge`, and this handler keeps them silent until an
# application configures logging.
getLogger('dry').addHandler(NullHandler())
