"""
The child half of the end-to-end signature test: one real Webview, one Api
holding every callable shape it is allowed to hold.

Run as a subprocess by tests/test_signature_over_the_bridge.py, never imported.
It has to be a subprocess for the same reason tests/gil_window.py does:
`run()` never returns, so everything the page learns has to be on disk before
the window closes.

The page makes a Call for each case, writes down what came back — the resolved
value, or the rejected Error's `name` and `message` — and hands the lot to
`report`, which is itself an ordinary Api entry. The last thing it does is
close the window, which ends the process.

Usage: python signature_window.py <journal-path>
"""

import os
import sys
import threading
import time
from functools import partial

# When the page has failed to run at all and the ticker ends the process, so a
# broken webview cannot hang the parent until its timeout.
BACKSTOP = 30.0
BACKSTOP_EXIT = 3

JOURNAL = ''


def concatenate(first: str, second: str) -> str:
    return first + second


class Greeter:
    """
    An object with `__call__`. Legal in the Api since #11, and its `self` must
    not be counted among the parameters a Call has to fill.
    """

    def __call__(self, name: str) -> str:
        return f'Hello, {name}'


def save_file(path: str) -> str:
    """
    The declaration ADR-0003 uses as its example. The page Calls it with a
    number, and the developer's sentence has to come back through the Bridge.
    """
    return f'saved {path}'


def report(lines: list) -> None:  # pyright: ignore[reportMissingTypeArgument, reportUnknownParameterType]
    """
    Writes the page's findings out and flushes them. `tao` exits the process
    from under the interpreter, so anything left in a buffer is lost.
    """
    with open(JOURNAL, 'a', encoding='utf-8') as file:
        for line in lines:  # pyright: ignore[reportUnknownVariableType]
            file.write(f'{line}\n')
        file.flush()
        os.fsync(file.fileno())


PAGE = """
<!doctype html>
<meta charset="utf-8">
<title>Signature checks</title>
<body>Dry is checking callback declarations.</body>
<script>
  const lines = [];

  const attempt = async (label, call) => {
    try {
      lines.push(label + ' resolved ' + JSON.stringify(await call()));
    } catch (error) {
      lines.push(label + ' rejected ' + error.name + ' | ' + error.message);
    }
  };

  (async () => {
    await attempt('wrong-type', () => window.dry.api.save_file(3));
    await attempt('too-few', () => window.dry.api.save_file());
    await attempt('too-many', () => window.dry.api.save_file('a', 'b'));
    await attempt('right-type', () => window.dry.api.save_file('notes.txt'));
    await attempt('partial', () => window.dry.api.concatenate('b'));
    await attempt('partial-wrong-type', () => window.dry.api.concatenate(3));
    await attempt('call-object', () => window.dry.api.greet('World'));
    await attempt('call-object-wrong-type', () => window.dry.api.greet(3));
    await attempt('builtin', () => window.dry.api.length([1, 2, 3]));
    await attempt('builtin-too-many', () => window.dry.api.length([], []));
    await attempt('unreadable-builtin', () => window.dry.api.largest([3, 1, 2]));
    await window.dry.api.report(lines);
    window.dry.close();
  })();
</script>
"""


def backstop() -> None:
    """
    Ends the process if the page never got as far as closing the window.
    """
    time.sleep(BACKSTOP)
    os._exit(BACKSTOP_EXIT)


def main() -> int:
    global JOURNAL
    JOURNAL = sys.argv[1]

    from dry import Webview

    webview = Webview(
        title='Dry signature checks',
        size=(320, 240),
        min_size=(320, 240),
        html=PAGE,
        api={
            'save_file': save_file,
            # The three shapes #11 opened the Api to, over a real Bridge.
            'concatenate': partial(concatenate, 'a'),
            'greet': Greeter(),
            'length': len,
            # A builtin with no introspectable signature at all.
            'largest': max,
            'report': report,
        },
    )

    threading.Thread(target=backstop, daemon=True).start()
    webview.run()

    return 0


if __name__ == '__main__':
    raise SystemExit(main())
