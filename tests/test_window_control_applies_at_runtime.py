"""
Runtime window control, on a real window.

A setting assigned after the window opened used to change an attribute and
nothing else. What is worth proving is that it now reaches the window, and
none of that can be seen without a window: the size is the platform's, and the
second opinion on it is JavaScript in a page. So the test opens a real window
in a subprocess, resizes, moves, retitles, maximizes, minimizes and
fullscreens it from Python, and reads the account both sides left in a
journal.

Every geometry is checked twice — once through Dry's own state query, once
against the `window.innerWidth` the page renders into, which is CSS pixels. A
size that arrived in physical pixels would be double on a 2x display and the
two would not match.

Visibility is the one control not exercised here: `window:hidden` and
`window:shown` are the proof that it applies, and they are checked in
test_window_events_reach_both_sides.py.

See tests/window_control_window.py for the round it runs and why it has to be
a subprocess.
"""

import json
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any

CHILD = Path(__file__).parent / 'window_control_window.py'

# The child's own number, kept in one place there.
BACKSTOP_EXIT = 3

# Generous: it covers the child's own backstop, plus a webview starting cold.
TIMEOUT = 200

# What the child asks the window for, kept in step with it.
OPENING_SIZE = [520, 380]

# A window is where it was asked to be if it is within a pixel or two of it:
# a logical position lands on a whole physical pixel, and a fractional scale
# factor makes the two disagree by less than one of either.
TOLERANCE = 2


class WindowControlAppliesAtRuntime(unittest.TestCase):
    # One window for the whole class: opening a webview is the expensive part,
    # and every assertion below reads the same round.
    child: subprocess.CompletedProcess[str]
    lines: list[str]
    context: str
    steps: dict[str, dict[str, Any]]

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

        cls.steps = {}
        for line in cls.lines:
            name, _, rest = line.partition(' ')
            if not rest:
                continue
            try:
                loaded: object = json.loads(rest)
            except ValueError:
                continue
            if isinstance(loaded, dict):
                cls.steps[name] = loaded  # pyright: ignore[reportUnknownArgumentType]

    def step(self, name: str) -> dict[str, Any]:
        """
        One step of the round, or a failure naming the step that never ran.
        """
        self.assertIn(
            name,
            self.steps,
            f'The round never reached the {name} step.{self.context}',
        )
        return self.steps[name]

    def assertNear(self, measured: Any, asked: Any, message: str) -> None:  # noqa: N802
        """
        Two pairs of logical pixels that name the same window.
        """
        self.assertIsInstance(measured, list, f'{message}{self.context}')
        pair: list[Any] = measured
        self.assertEqual(len(pair), 2, f'{message}{self.context}')
        for got, wanted in zip(pair, asked):
            self.assertLessEqual(
                abs(int(got) - int(wanted)),
                TOLERANCE,
                f'{message} Got {measured}, asked for {list(asked)}.{self.context}',
            )

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
            'The window never closed itself, so the round never ran. This is a '
            f'broken webview, not a verdict on window control.{self.context}',
        )
        self.assertIn(
            'done',
            self.lines,
            f'The round did not finish.{self.context}',
        )

    def test_the_window_opens_at_the_size_it_was_given(self):
        # The reading the rest of the round is a change from, and the proof
        # that the state query measures the same thing `size=` sets.
        self.assertNear(
            self.step('opened')['size'],
            OPENING_SIZE,
            'The state query does not report the size the Webview was built with.',
        )

    def test_a_size_assigned_while_running_resizes_the_window(self):
        resized = self.step('resized')
        asked: Any = resized['asked']
        self.assertNear(
            resized['state'],
            asked,
            'A size assigned on a running Webview did not reach the window.',
        )
        # And the property reads the window back rather than the last thing it
        # was told, so a developer cannot be shown a size the window is not.
        self.assertNear(
            resized['prop'],
            asked,
            'webview.size does not report what the window measures.',
        )
        page: Any = resized['page']
        self.assertIsNotNone(page, f'The page never answered.{self.context}')
        self.assertNear(
            page['inner'],
            asked,
            'The new size is not the area the page renders into, so it is not '
            'in logical pixels.',
        )

    def test_a_position_assigned_while_running_moves_the_window(self):
        moved = self.step('moved')
        self.assertNear(
            moved['state'],
            moved['asked'],
            'A position assigned on a running Webview did not reach the window.',
        )

    def test_the_settings_that_do_not_move_the_window_are_accepted(self):
        # Neither platform will say what its window's title or minimum size
        # is, so there is nothing to read back: what is checked here is that
        # assigning them on a running Webview neither raises nor stops the
        # round, and that the Webview remembers what it was told.
        assigned = self.step('assigned')
        self.assertEqual(assigned['title'], 'Renamed while running')
        self.assertEqual(assigned['min_size'], [200, 150])

    def test_maximizing_from_python_reaches_the_window(self):
        maximized = self.step('maximized')
        self.assertTrue(
            maximized['state'],
            f'The window did not maximize.{self.context}',
        )
        page: Any = maximized['page']
        self.assertIsNotNone(page, f'The page never answered.{self.context}')
        # The frontend's own state query, answering while the window is
        # maximized: this is the reading a titlebar draws its button from, and
        # it has to agree with the size the page actually renders into.
        self.assertNear(
            page['inner'],
            [page['state']['size']['width'], page['state']['size']['height']],
            'The state query the frontend gets disagrees with the size the '
            'page renders into.',
        )
        self.assertTrue(
            page['state']['maximized'],
            'The frontend state query does not report the window as '
            f'maximized.{self.context}',
        )
        self.assertFalse(
            self.step('unmaximized')['state'],
            f'The window did not leave the maximized state.{self.context}',
        )

    def test_minimizing_and_restoring_from_python_reach_the_window(self):
        self.assertTrue(
            self.step('minimized')['state'],
            f'The window did not minimize.{self.context}',
        )
        self.assertFalse(
            self.step('restored')['state'],
            f'The window did not come back from being minimized.' f'{self.context}',
        )

    def test_fullscreen_is_set_and_read_back(self):
        # The one window state with no Event of its own, so the state query is
        # the only way to observe it at all.
        self.assertTrue(
            self.step('fullscreen')['state'],
            f'The window did not go fullscreen.{self.context}',
        )
        self.assertFalse(
            self.step('windowed')['state'],
            f'The window did not leave fullscreen.{self.context}',
        )

    def test_the_frontend_can_ask_what_the_window_is_doing(self):
        # The page asked before it had heard a single window Event, which is
        # the case the query exists for: a frontend that has just loaded has
        # observed nothing and still has to draw its titlebar one way round.
        reading: Any = self.step('page-state')['reading']
        self.assertEqual(
            sorted(reading),
            [
                'focused',
                'fullscreen',
                'maximized',
                'minimized',
                'position',
                'size',
                'visible',
            ],
            f'The frontend state query answered with the wrong shape.'
            f'{self.context}',
        )
        self.assertTrue(
            reading['visible'],
            f'A window on screen was reported as not visible.{self.context}',
        )
        self.assertNear(
            [reading['size']['width'], reading['size']['height']],
            OPENING_SIZE,
            'The frontend state query does not report the size the window '
            'opened at.',
        )

    def test_taking_the_decorations_off_a_running_window_applies(self):
        # And the reported size follows the frame: a window that has lost its
        # titlebar renders into all of itself, so the state query and the page
        # have to agree about the new number as well.
        undecorated = self.step('undecorated')
        page: Any = undecorated['page']
        self.assertIsNotNone(page, f'The page never answered.{self.context}')
        self.assertNear(
            undecorated['state'],
            page['inner'],
            'After the decorations came off, the size Dry reports is not the '
            'size the page renders into.',
        )


class WindowControlBeforeTheWindowExists(unittest.TestCase):
    """
    The other half of the promise, and the one that needs no window: a control
    that only means something once the window is on screen says so, rather
    than quietly doing nothing the way every one of these did before.
    """

    def test_a_state_only_a_window_has_refuses_to_be_read(self):
        from dry import Webview

        webview = Webview(html='<p>Not running.</p>')
        for name in ('position', 'visible', 'maximized', 'minimized', 'fullscreen'):
            with self.subTest(name=name):
                with self.assertRaises(RuntimeError) as refusal:
                    getattr(webview, name)
                self.assertIn(name, str(refusal.exception))

    def test_a_state_only_a_window_has_refuses_to_be_assigned(self):
        from dry import Webview

        webview = Webview(html='<p>Not running.</p>')
        for name, value in (
            ('position', (10, 10)),
            ('visible', False),
            ('maximized', True),
            ('minimized', True),
            ('fullscreen', True),
        ):
            with self.subTest(name=name):
                with self.assertRaises(RuntimeError):
                    setattr(webview, name, value)

    def test_the_state_query_refuses_rather_than_guessing(self):
        # Answering from the settings the window will be built from would be a
        # guess dressed as a measurement, and a developer would only find out
        # it was one on the platform where the guess was wrong.
        from dry import Webview

        webview = Webview(html='<p>Not running.</p>')
        with self.assertRaises(RuntimeError):
            _ = webview.state()

    def test_the_settings_that_are_also_settings_are_readable_either_way(self):
        # `size` has a window's measurement to report while there is a window
        # and the setting it was given before that, so it never raises.
        from dry import Webview

        webview = Webview(html='<p>Not running.</p>', size=(320, 240))
        self.assertEqual(webview.size, (320, 240))
        webview.size = (400, 300)
        self.assertEqual(webview.size, (400, 300))


if __name__ == '__main__':
    unittest.main()
