"""
The guard on drag-resize: the right platform, and an edge that really moves.

`tao` 0.36 answers `Window::drag_resize_window` on macOS with an unconditional
`Err(NotSupported)`, so Dry runs the drag in the frontend there and hands the
grab to the platform on Windows. Which of the two happens is decided by one
line of JavaScript in src/js/window_functions.js:

    /Mac/i.test(navigator.platform || navigator.userAgent)

Nothing else. **If WebView2 does not report a platform that fails that test,
Windows falls into the macOS path** and resizes in JavaScript where the native
call worked — silently, because both paths do something. So the first thing
this test does is find out what the two webviews actually call themselves and
assert the guard lands on the right side of the split on each.

The second, and the point, is that an undecorated window's east edge really
moves when it is dragged. That is real-window behaviour no compilation reaches,
and it is driven differently on each platform because the two platforms resize
by different machinery — DOM events on macOS, where the drag *is* JavaScript,
and injected mouse input on Windows, where the grab goes to a modal loop in
`DefWindowProc` that reads the real cursor. See tests/resize_window.py.
"""

import json
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any

CHILD = Path(__file__).parent / 'resize_window.py'

# The child's own numbers, kept in one place there.
GROWTH = 120
BACKSTOP_EXIT = 3

# How much of the drag has to show up in the window's width. Not all of it:
# the injected drag on Windows travels in physical pixels and the page measures
# in CSS pixels, so a runner at anything above 100% scaling reports less than
# it was dragged. Two fifths of it is still far more than the handful of pixels
# a window that did not resize at all could drift by.
LEAST_GROWTH = GROWTH * 0.4

# Generous: it covers the child's own backstop, plus a webview starting cold.
TIMEOUT = 240


class ResizeEdgesDragTheWindow(unittest.TestCase):
    # One window for the whole class: opening a webview is the expensive part,
    # and every assertion below reads the same drag.
    child: subprocess.CompletedProcess[str]
    lines: list[str]
    context: str

    @classmethod
    def setUpClass(cls) -> None:
        with TemporaryDirectory() as directory:
            journal = Path(directory) / 'journal.txt'
            cls.child = subprocess.run(
                [sys.executable, str(CHILD), str(journal)],
                capture_output=True,
                text=True,
                timeout=TIMEOUT,
            )
            cls.lines = (
                journal.read_text(encoding='utf-8').splitlines()
                if journal.exists()
                else []
            )

        cls.context = (
            f'\nplatform: {sys.platform}'
            f'\nexit code: {cls.child.returncode}'
            f'\nstdout:\n{cls.child.stdout}'
            f'\nstderr:\n{cls.child.stderr}'
            f'\njournal:\n' + '\n'.join(cls.lines)
        )

    def payload(self, prefix: str) -> Any:
        """
        The JSON the one journal line under that word carries.
        """
        found = [
            line[len(prefix) :].strip()
            for line in self.lines
            if line.split(' ')[0] == prefix
        ]
        self.assertEqual(
            len(found),
            1,
            f"Expected exactly one '{prefix}' line in the journal." f'{self.context}',
        )
        return json.loads(found[0])

    def test_the_window_ran_the_whole_drag(self):
        self.assertTrue(
            any(line.startswith('run ') for line in self.lines),
            f'The child never reached run().{self.context}',
        )
        self.assertNotIn(
            'returned',
            self.lines,
            f'run() returned, which it is not supposed to do.{self.context}',
        )
        self.assertNotEqual(
            self.child.returncode,
            BACKSTOP_EXIT,
            'The window never closed itself. Either the page never ran, or a '
            'native resize loop never ended and the event loop could no longer '
            f'hear anything.{self.context}',
        )

    def test_the_webview_reports_a_platform_the_guard_can_read(self):
        report = self.payload('report')
        self.assertTrue(
            report['platform'] or report['userAgent'],
            'The webview reports neither a platform nor a user agent, so the '
            f'guard has nothing to decide on.{self.context}',
        )

    def test_the_guard_sends_the_drag_down_the_right_path(self):
        report = self.payload('report')
        expected = sys.platform == 'darwin'
        self.assertEqual(
            report['guard'],
            expected,
            'The platform guard in src/js/window_functions.js resolved the '
            f'wrong way. It read navigator.platform as '
            f'{report["platform"]!r} and navigator.userAgent as '
            f'{report["userAgent"]!r}, and decided the drag runs in '
            f'JavaScript: {report["guard"]}. On {sys.platform} it must be '
            f'{expected}.{self.context}',
        )

    def test_an_undecorated_window_draws_a_handle_on_its_east_edge(self):
        report = self.payload('report')
        self.assertIn(
            'resize-right',
            report['handle'] or '',
            'Nothing that resizes sits on the east edge of an undecorated '
            f'window, so there is no drag to test.{self.context}',
        )

    def test_dragging_the_east_edge_widens_the_window(self):
        report = self.payload('report')
        measured = self.payload('measured')
        before, after = report['size'][0], measured[0]

        self.assertGreaterEqual(
            after - before,
            LEAST_GROWTH,
            f'The east edge was dragged {GROWTH} pixels outwards and the '
            f'window went from {before} to {after} CSS pixels wide.'
            f'{self.context}',
        )

    def test_dragging_the_east_edge_leaves_the_other_edges_alone(self):
        report = self.payload('report')
        measured = self.payload('measured')
        self.assertAlmostEqual(
            report['size'][1],
            measured[1],
            delta=4,
            msg='Dragging the east edge changed the window height, so an edge '
            f'the drag does not touch moved.{self.context}',
        )

    @unittest.skipUnless(sys.platform == 'win32', 'Windows drags natively.')
    def test_real_mouse_input_reaches_the_resize_handle(self):
        # The rest of the Windows case means nothing if the injected press
        # never landed on the page: an untouched window would look the same.
        presses = [
            json.loads(line[len('mousedown') :])
            for line in self.lines
            if line.startswith('mousedown ')
        ]
        trusted = [press for press in presses if press['trusted']]
        self.assertTrue(
            trusted,
            'No real mouse press reached the page. Input injection did not '
            'work on this machine, so nothing here is a verdict on resizing.'
            f'{self.context}',
        )
        self.assertIn(
            'resize-right',
            trusted[0]['target'],
            f'The injected press missed the east resize handle.{self.context}',
        )

    @unittest.skipUnless(sys.platform == 'win32', 'Windows drags natively.')
    def test_the_platform_moved_the_window_itself(self):
        # Measured through GetClientRect rather than through the page, so this
        # is the operating system's account of the window and not Dry's, and
        # the client area rather than the frame, because an undecorated tao
        # window keeps a WS_THICKFRAME border some eight pixels wide that the
        # page cannot see and a grab on it would resize without Dry's help.
        before = self.payload('native-before')
        after = self.payload('native-after')
        widened = (after[2] - after[0]) - (before[2] - before[0])
        self.assertAlmostEqual(
            widened,
            GROWTH,
            delta=16,
            msg=f'The native drag-resize loop moved the window by {widened} '
            f'physical pixels, not by the {GROWTH} the cursor travelled.'
            f'{self.context}',
        )


if __name__ == '__main__':
    unittest.main()
