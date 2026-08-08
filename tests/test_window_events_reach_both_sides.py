"""
Window state, on a real window.

A window Event is an Event with a reserved name, so the thing worth proving is
that the window's own states come out of the same bus as everything else and
reach both halves of it. None of that can be seen without a window: the states
are the platform's, and half the listeners are JavaScript in a page. So the
test opens a real window in a subprocess, drives it through maximize,
unmaximize, minimize, restore and a refused close, and reads the account both
sides left in a journal.

See tests/window_state_window.py for the round it runs and why it has to be a
subprocess.
"""

import json
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

CHILD = Path(__file__).parent / 'window_state_window.py'

# The child's own number, kept in one place there.
BACKSTOP_EXIT = 3

# Generous: it covers the child's own backstop, plus a webview starting cold.
TIMEOUT = 150


class WindowEventsReachBothSides(unittest.TestCase):
    # One window for the whole class: opening a webview is the expensive part,
    # and every assertion below reads the same round.
    child: subprocess.CompletedProcess[str]
    lines: list[str]
    context: str
    page: list[list[object]]

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
            f'\nexit code: {cls.child.returncode}'
            f'\nstdout:\n{cls.child.stdout}'
            f'\nstderr:\n{cls.child.stderr}'
            f'\njournal:\n' + '\n'.join(cls.lines)
        )

        cls.page = []
        for line in cls.lines:
            if not line.startswith('report '):
                continue
            loaded: object = json.loads(line[len('report ') :])
            if isinstance(loaded, list):
                cls.page = [entry for entry in loaded if isinstance(entry, list)]

    def python_saw(self, word: str) -> bool:
        """
        Whether a Python listener wrote its line down.
        """
        return any(line.split(' ')[0] == word for line in self.lines)

    def page_saw(self, word: str) -> list[object]:
        """
        Every entry a frontend listener pushed under that name.
        """
        return [
            entry
            for entry in self.page
            if isinstance(entry, list) and entry and entry[0] == word
        ]

    def test_the_window_ran_the_whole_round(self):
        self.assertIn(
            'run', self.lines, f'The child never reached run().{self.context}'
        )
        self.assertNotIn(
            'returned',
            self.lines,
            f'run() returned, which it is not supposed to do.{self.context}',
        )
        self.assertNotEqual(
            self.child.returncode,
            BACKSTOP_EXIT,
            'The window never closed itself, so the page never ran. This is a '
            f'broken webview, not a verdict on window Events.{self.context}',
        )
        self.assertNotIn(
            'stalled',
            self.lines,
            'The round stopped advancing, so a window state the platform was '
            f'asked for never arrived.{self.context}',
        )

    def test_the_page_can_listen_for_a_reserved_name_but_not_emit_one(self):
        self.assertEqual(
            self.page_saw('forged'),
            [['forged', False]],
            'The page was allowed to emit under a reserved name, so a window '
            f'Event cannot be trusted to come from Dry.{self.context}',
        )

    def test_a_maximize_reaches_both_sides(self):
        self.assertTrue(
            self.python_saw('maximized'),
            f'No Python listener heard window:maximized.{self.context}',
        )
        self.assertTrue(
            self.page_saw('maximized'),
            f'No frontend listener heard window:maximized.{self.context}',
        )

    def test_a_window_event_reaches_every_python_listener_for_its_name(self):
        self.assertIn(
            'maximized-again',
            self.lines,
            'The second Python listener for window:maximized never ran, so a '
            f'window Event is not on the ordinary bus.{self.context}',
        )

    def test_leaving_the_maximized_state_reaches_both_sides(self):
        self.assertTrue(
            self.python_saw('unmaximized'),
            f'No Python listener heard window:unmaximized.{self.context}',
        )
        self.assertTrue(
            self.page_saw('unmaximized'),
            f'No frontend listener heard window:unmaximized.{self.context}',
        )

    def test_a_minimize_and_a_restore_reach_both_sides(self):
        self.assertTrue(
            self.python_saw('minimized'),
            f'No Python listener heard window:minimized.{self.context}',
        )
        self.assertTrue(
            self.page_saw('minimized'),
            f'No frontend listener heard window:minimized.{self.context}',
        )
        self.assertTrue(
            self.python_saw('restored'),
            f'No Python listener heard window:restored.{self.context}',
        )
        self.assertTrue(
            self.page_saw('restored'),
            f'No frontend listener heard window:restored.{self.context}',
        )

    def test_a_resize_carries_the_new_size_in_logical_pixels(self):
        resizes = self.page_saw('resized')
        self.assertTrue(
            resizes,
            f'No frontend listener heard window:resized.{self.context}',
        )
        self.assertTrue(
            self.python_saw('resized'),
            f'No Python listener heard window:resized.{self.context}',
        )
        for entry in resizes:
            value = entry[1]
            self.assertIsInstance(
                value,
                dict,
                f'window:resized carried {value!r} rather than a size.'
                f'{self.context}',
            )
            assert isinstance(value, dict)
            self.assertEqual(
                sorted(value),
                ['height', 'width'],
                f'window:resized carried the wrong keys.{self.context}',
            )
            # The page compared the size it was handed with its own
            # window.innerWidth and innerHeight, which are CSS pixels. A size
            # in physical pixels would be double on any 2x display.
            self.assertTrue(
                entry[2],
                f'window:resized carried {value!r}, which is not the size the '
                f'page renders into — so it is not in logical pixels.'
                f'{self.context}',
            )

    def test_a_close_request_reaches_both_sides_before_the_hook_decides(self):
        self.assertTrue(
            self.python_saw('close-requested'),
            f'No Python listener heard window:close-requested.{self.context}',
        )
        self.assertTrue(
            self.page_saw('close-requested'),
            'No frontend listener heard window:close-requested, so the page '
            f'was never told about a close it could still have seen.'
            f'{self.context}',
        )
        # The Event is a notification, not a vote: the hook still refused the
        # first close, and the window was still there afterwards to run the
        # rest of the round.
        self.assertIn(
            'hook 1',
            self.lines,
            f'The close hook was never asked.{self.context}',
        )
        self.assertIn(
            'hook 2',
            self.lines,
            'The window never got a second close, so the first refusal did '
            f'not leave it open.{self.context}',
        )

    def test_the_high_frequency_events_are_not_repeated(self):
        # Coalescing: the window is read once per turn of the event loop and
        # only a changed value is emitted, so no two resizes in a row carry
        # the same size.
        sizes = [entry[1] for entry in self.page_saw('resized')]
        for before, after in zip(sizes, sizes[1:]):
            self.assertNotEqual(
                before,
                after,
                'The same size was emitted twice in a row, so nothing is '
                f'coalescing window:resized.{self.context}',
            )


if __name__ == '__main__':
    unittest.main()
