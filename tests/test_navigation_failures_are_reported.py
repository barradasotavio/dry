"""
The guard on the blank window: a navigation that fails has to say so.

A URL that never loads leaves an empty window and, before this was fixed, no
word anywhere about why. wry has no failed-navigation hook — WKWebView reports
the failure to a delegate method wry does not implement — so Dry watches the
navigation itself and, when nothing has arrived, asks the address what is wrong
with Python's own `urllib`. The answer reaches the `dry.webview` logger.

The detection turns on one measured fact: a failed navigation leaves the page
at `about:blank`, so a `Finished` at `about:blank` is not an arrival. That was
measured on WKWebView only. **If WebView2 shows an error page of its own
instead, `Finished` fires at a URL that is not blank, the watchdog calls the
navigation an arrival and says nothing** — and the blank window is silent
again, which is the entire bug.

So this runs on both platforms, against the three failures that read
differently to a developer: a refused connection, an unresolvable host and a
certificate nothing trusts. Each opens a real window in a subprocess pointed at
a real bad address. See tests/navigation_window.py for the journal it leaves,
and why the level of the record it leaves is what tells a suppressed report
apart from a disagreeing one.
"""

import multiprocessing
import socket
import subprocess
import sys
import unittest
from dataclasses import dataclass
from pathlib import Path
from tempfile import TemporaryDirectory

import tls_server

CHILD = Path(__file__).parent / 'navigation_window.py'

# The child's own number, kept in one place there.
CAP_EXIT = 3

# Generous: it covers the child's own hard cap, plus a webview starting cold.
TIMEOUT = 180

# How long to wait for the HTTPS server to report the port it landed on.
SERVER_START = 30


@dataclass
class Run:
    """
    One window, one bad address, and everything it left behind.
    """

    url: str
    returncode: int
    stdout: str
    stderr: str
    lines: list[str]

    @property
    def records(self) -> list[str]:
        """
        The lines Dry itself wrote, in the order it wrote them.
        """
        return [line for line in self.lines if ' dry.' in line.partition(':')[0]]

    @property
    def context(self) -> str:
        return (
            f'\nurl: {self.url}'
            f'\nexit code: {self.returncode}'
            f'\nstdout:\n{self.stdout}'
            f'\nstderr:\n{self.stderr}'
            f'\njournal:\n' + '\n'.join(self.lines)
        )


def open_window_at(url: str) -> Run:
    """
    Point a real Webview at an address and read the log it wrote.
    """
    with TemporaryDirectory() as directory:
        journal = Path(directory) / 'journal.log'
        child = subprocess.run(
            [sys.executable, str(CHILD), str(journal), url],
            capture_output=True,
            text=True,
            timeout=TIMEOUT,
        )
        lines = (
            journal.read_text(encoding='utf-8').splitlines() if journal.exists() else []
        )

    return Run(url, child.returncode, child.stdout, child.stderr, lines)


def a_port_nothing_listens_on() -> int:
    """
    A loopback port the kernel has just handed out and taken back again.
    """
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.bind(('127.0.0.1', 0))
        return probe.getsockname()[1]


class NavigationFailuresAreReported(unittest.TestCase):
    # Three windows for the whole class: opening a webview is the expensive
    # part, and each address is asked about exactly once.
    refused: Run
    unresolvable: Run
    untrusted: Run
    server: 'multiprocessing.Process'

    @classmethod
    def setUpClass(cls) -> None:
        port: 'multiprocessing.Queue[int]' = multiprocessing.Queue()
        cls.server = multiprocessing.Process(
            target=tls_server.serve, args=(port,), daemon=True
        )
        cls.server.start()
        try:
            https_port = port.get(timeout=SERVER_START)
        except Exception:  # pragma: no cover - only on a broken runner
            cls.server.terminate()
            raise

        cls.refused = open_window_at(f'http://127.0.0.1:{a_port_nothing_listens_on()}/')
        cls.unresolvable = open_window_at('http://dry-no-such-host.invalid/')
        cls.untrusted = open_window_at(f'https://127.0.0.1:{https_port}/')

    @classmethod
    def tearDownClass(cls) -> None:
        cls.server.terminate()
        cls.server.join(timeout=10)

    def assert_reported(self, run: Run, reason: str) -> None:
        """
        Dry has to have said, at error, why that address did not load.

        The three ways this can go wrong read differently, and the message says
        which one happened, because they need different fixes: a suppressed
        report is the webview committing an error page, a debug record is
        Python and the webview disagreeing about the address, and a wrong
        reason is the classification.
        """
        errors = [line for line in run.records if line.startswith('ERROR dry.webview:')]

        if not errors:
            debug = [
                line for line in run.records if line.startswith('DEBUG dry.webview:')
            ]
            if debug:
                self.fail(
                    'The navigation was diagnosed, but Dry reached the address '
                    'and declined to accuse it. The webview and Python disagree '
                    f'about this address.{run.context}'
                )
            self.assertNotEqual(
                run.returncode,
                CAP_EXIT,
                'Dry said nothing at all about a navigation that could not '
                'have arrived, so the page-load handler called it an arrival. '
                'The webview committed a page of its own — an error page — and '
                f'the failure report is suppressed.{run.context}',
            )
            self.fail(
                f'The child left no report and did not reach its cap.{run.context}'
            )

        self.assertTrue(
            any(reason in line for line in errors),
            f"Dry reported the failure but not as '{reason}'.{run.context}",
        )
        self.assertTrue(
            any(run.url in line for line in errors),
            f'The report does not name the address it is about.{run.context}',
        )

    def test_a_refused_connection_is_reported(self):
        self.assert_reported(self.refused, 'the connection was refused')

    def test_an_unresolvable_host_is_reported(self):
        self.assert_reported(self.unresolvable, 'the host could not be resolved')

    def test_an_untrusted_certificate_is_reported(self):
        self.assert_reported(
            self.untrusted, "the server's TLS certificate is not trusted"
        )


if __name__ == '__main__':
    unittest.main()
