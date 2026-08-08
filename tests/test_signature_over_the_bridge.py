"""
The signature check as a developer meets it: through a real window, from real
JavaScript.

tests/test_signature.py already covers what the check decides. This asks the
other half of the question — that the refusal leaves Python, crosses the
Bridge, and arrives in the page as a rejected Promise whose Error still says
`TypeError`, which is the mechanism #13 put there. It also runs the three
callable shapes #11 opened the Api to over that same Bridge, rather than only
against the check.

The window is real on both platforms CI runs on, so this is an end-to-end run
and not a headless stand-in. See tests/signature_window.py for the child.
"""

import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

CHILD = Path(__file__).parent / 'signature_window.py'

# The child's own backstop exit code, kept in one place there.
BACKSTOP_EXIT = 3

# Generous: it covers the child's backstop, plus a webview starting cold on a
# Windows runner.
TIMEOUT = 120


class SignatureChecksOverTheBridge(unittest.TestCase):
    # One window for the whole class: opening eleven of them to ask eleven
    # questions would be eleven cold webview starts on a CI runner.
    context: str
    returncode: int
    outcomes: dict[str, str]

    @classmethod
    def setUpClass(cls) -> None:
        with TemporaryDirectory() as directory:
            journal = Path(directory) / 'journal.txt'
            child = subprocess.run(
                [sys.executable, str(CHILD), str(journal)],
                capture_output=True,
                text=True,
                timeout=TIMEOUT,
            )
            lines = (
                journal.read_text(encoding='utf-8').splitlines()
                if journal.exists()
                else []
            )

        cls.context = (
            f'\nexit code: {child.returncode}'
            f'\nstdout:\n{child.stdout}'
            f'\nstderr:\n{child.stderr}'
            f'\njournal:\n' + '\n'.join(lines)
        )
        cls.returncode = child.returncode
        cls.outcomes = dict(
            (label, outcome)
            for label, _, outcome in (line.partition(' ') for line in lines)
        )

    def outcome(self, label: str) -> str:
        self.assertNotEqual(
            self.returncode,
            BACKSTOP_EXIT,
            'The window never closed itself, so the page never ran. This is a '
            f'broken webview, not a verdict on the check.{self.context}',
        )
        self.assertIn(
            label, self.outcomes, f'The page reported no {label}.{self.context}'
        )
        return self.outcomes[label]

    def test_a_wrong_type_rejects_the_promise_as_a_type_error(self):
        self.assertEqual(
            self.outcome('wrong-type'),
            'rejected TypeError | save_file expects str for path, received '
            'number instead.',
            self.context,
        )

    def test_too_few_arguments_reject_the_promise(self):
        self.assertEqual(
            self.outcome('too-few'),
            'rejected TypeError | save_file takes 1 argument, received 0. path '
            'was not passed.',
            self.context,
        )

    def test_too_many_arguments_reject_the_promise(self):
        self.assertEqual(
            self.outcome('too-many'),
            'rejected TypeError | save_file takes 1 argument, received 2.',
            self.context,
        )

    def test_a_call_that_matches_its_declaration_still_runs(self):
        self.assertEqual(
            self.outcome('right-type'), 'resolved "saved notes.txt"', self.context
        )

    def test_a_partial_is_a_working_api_entry(self):
        self.assertEqual(self.outcome('partial'), 'resolved "ab"', self.context)

    def test_a_partial_is_checked_on_the_parameters_it_has_left(self):
        self.assertEqual(
            self.outcome('partial-wrong-type'),
            'rejected TypeError | concatenate expects str for second, received '
            'number instead.',
            self.context,
        )

    def test_an_object_with_call_is_a_working_api_entry(self):
        self.assertEqual(
            self.outcome('call-object'), 'resolved "Hello, World"', self.context
        )

    def test_an_object_with_call_is_checked_without_counting_self(self):
        self.assertEqual(
            self.outcome('call-object-wrong-type'),
            'rejected TypeError | greet expects str for name, received number '
            'instead.',
            self.context,
        )

    def test_a_builtin_is_a_working_api_entry(self):
        self.assertEqual(self.outcome('builtin'), 'resolved 3', self.context)

    def test_a_builtin_with_a_readable_signature_is_checked_for_arity(self):
        self.assertEqual(
            self.outcome('builtin-too-many'),
            'rejected TypeError | length takes 1 argument, received 2.',
            self.context,
        )

    def test_a_builtin_with_no_signature_is_let_through_rather_than_refused(self):
        self.assertEqual(self.outcome('unreadable-builtin'), 'resolved 3', self.context)


if __name__ == '__main__':
    unittest.main()
