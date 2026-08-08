"""
The fourth quadrant of the Bridge, on a real window.

An Event travels Python to frontend and frontend to Python, reaching every
listener registered for its name, returning nothing to anybody. None of that
can be seen without a window: the frontend half of the bus is JavaScript
injected into a page, and the Python half is reached from the thread that draws
it. So the test opens a real window in a subprocess, has the two sides throw an
Event back and forth, and reads the account both sides left in a journal.

See tests/events_window.py for the round they run and why it has to be a
subprocess.
"""

import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

CHILD = Path(__file__).parent / 'events_window.py'

# The child's own number, kept in one place there.
BACKSTOP_EXIT = 3

# Generous: it covers the child's own backstop, plus a webview starting cold.
TIMEOUT = 120


class EventsCrossTheBridge(unittest.TestCase):
    # One window for the whole class: opening a webview is the expensive part,
    # and every assertion below reads the same round.
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
            f'\nexit code: {cls.child.returncode}'
            f'\nstdout:\n{cls.child.stdout}'
            f'\nstderr:\n{cls.child.stderr}'
            f'\njournal:\n' + '\n'.join(cls.lines)
        )

    def line(self, prefix: str) -> str:
        """
        The one journal line starting with that word, and its remainder.
        """
        found = [line for line in self.lines if line.split(' ')[0] == prefix]
        self.assertEqual(
            len(found),
            1,
            f"Expected exactly one '{prefix}' line in the journal."
            f'{self.context}',
        )
        return found[0][len(prefix) :].strip()

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
            f'broken webview, not a verdict on Events.{self.context}',
        )

    def test_an_event_from_the_frontend_reaches_every_python_listener(self):
        # Both recording listeners for 'ping' ran, and the value the page sent
        # arrived intact under the Bridge contract.
        self.assertEqual(
            self.line('ping'),
            "{'n': 1, 'deep': [True, None, 'x']}",
            f'The Event the page emitted did not arrive intact.{self.context}',
        )
        self.assertIn(
            'ping-again',
            self.lines,
            'The second listener for the same Event never ran, so a raising '
            f'listener or an ordering assumption stopped it.{self.context}',
        )

    def test_a_python_listener_that_raises_does_not_stop_the_others(self):
        # The first listener registered for 'ping' raises. The two after it
        # recorded anyway, which the two assertions above have just checked,
        # and the traceback went to the dry.bridge logger rather than to the
        # journal.
        self.assertIn(
            'ping-again',
            self.lines,
            f'A raising listener silenced the ones after it.{self.context}',
        )

    def test_an_event_from_python_reaches_every_frontend_listener(self):
        page = self.line('done')

        for entry in (
            "['pong-a', 1]",
            "['pong-b', [True, None, 'x']]",
            "['pong-c', 'string']",
        ):
            self.assertIn(
                entry,
                page,
                f'A frontend listener for the Event Python emitted did not '
                f'get it: {entry} is missing.{self.context}',
            )

    def test_an_event_emitted_from_another_thread_arrives(self):
        self.assertIn(
            "['pong-thread', 'from a thread']",
            self.line('done'),
            'An Event emitted from a thread of the application\'s own never '
            f'reached the page.{self.context}',
        )

    def test_a_frontend_listener_that_throws_does_not_stop_the_others(self):
        # The throwing listener sits between pong-b and pong-c.
        self.assertIn(
            "['pong-c', 'string']",
            self.line('done'),
            f'A throwing listener silenced the ones after it.{self.context}',
        )

    def test_an_unregistered_listener_hears_nothing(self):
        self.assertNotIn(
            "['deaf', true]",
            self.line('done').lower(),
            f'A frontend listener taken off still heard.{self.context}',
        )
        self.assertNotIn(
            'still-listening',
            self.lines,
            f'A Python listener taken off still heard.{self.context}',
        )

    def test_an_event_with_no_listeners_is_not_an_error(self):
        # The page emits 'nobody-is-listening' and carries on to the rest of
        # the round, which the journal's 'done' line proves it reached.
        self.assertTrue(
            self.line('done'),
            f'An Event nobody listens for broke the round.{self.context}',
        )

    def test_a_reserved_name_is_refused_on_both_sides(self):
        self.assertEqual(
            self.line('reserved'),
            'BridgeError',
            f'Python was allowed to emit under a reserved name.{self.context}',
        )
        self.assertIn(
            "['reserved', 'refused']",
            self.line('done'),
            f'The page was allowed to emit under a reserved name.{self.context}',
        )

    def test_an_event_emitted_before_the_window_says_so(self):
        self.assertEqual(
            self.line('early-emit'),
            'BridgeError',
            'Emitting before run() should say there is no frontend to reach '
            f'yet.{self.context}',
        )


if __name__ == '__main__':
    unittest.main()
