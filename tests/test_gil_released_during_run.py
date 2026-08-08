"""
The guard on the GIL: `run()` must not hold it for the life of the window.

It held it once, and the symptom was silent — the window worked, and every
Python thread the application had started simply stopped, including the
portal's pool, so no Api callback could ever run. `src/lib.rs` releases it with
`py.detach` before the event loop takes the main thread and never takes it
back. Nothing but this test stops a later change from taking it back again.

The test opens a real window in a subprocess and counts how often an ordinary
Python thread wakes up while that window is on screen. See tests/gil_window.py
for why it has to be a subprocess.
"""

import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

CHILD = Path(__file__).parent / 'gil_window.py'

# The child's own numbers, kept in one place there.
WINDOW_LIFETIME = 1.5
TICK_INTERVAL = 0.05
BACKSTOP_EXIT = 3

# How long the window has to have been alive with the thread awake in it. Well
# under WINDOW_LIFETIME, because the page's timer starts only once the webview
# has rendered, and a cold CI runner takes its time getting there.
MINIMUM_SPAN = 0.5

# How many ticks that span has to hold. A frozen thread manages none; a running
# one manages MINIMUM_SPAN / TICK_INTERVAL, so this asks for a third of them.
MINIMUM_TICKS = 3

# Generous: it covers the child's own backstop, plus a webview starting cold.
TIMEOUT = 120


class GilReleasedDuringRun(unittest.TestCase):
    def test_a_python_thread_keeps_running_for_the_life_of_the_window(self):
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

        context = (
            f'\nexit code: {child.returncode}'
            f'\nstdout:\n{child.stdout}'
            f'\nstderr:\n{child.stderr}'
            f'\njournal: {len(lines)} lines'
        )

        stamps: dict[str, list[float]] = {}
        for line in lines:
            event, _, stamp = line.partition(' ')
            stamps.setdefault(event, []).append(float(stamp))

        self.assertIn('run', stamps, f'The child never reached run().{context}')
        self.assertNotIn(
            'returned',
            stamps,
            f'run() returned, which it is not supposed to do.{context}',
        )
        self.assertNotEqual(
            child.returncode,
            BACKSTOP_EXIT,
            'The window never closed itself, so the page never ran. This is a '
            f'broken webview, not a verdict on the GIL.{context}',
        )

        run_at = stamps['run'][0]
        ticks = [stamp for stamp in stamps.get('tick', []) if stamp > run_at]

        self.assertGreaterEqual(
            len(ticks),
            MINIMUM_TICKS,
            f'A Python thread ticked {len(ticks)} times while the window was '
            f'open. run() is holding the GIL across the event loop.{context}',
        )

        span = ticks[-1] - run_at
        self.assertGreaterEqual(
            span,
            MINIMUM_SPAN,
            f"A Python thread stopped ticking {span:.2f}s into the window's "
            f'life. run() is holding the GIL across the event loop.{context}',
        )


if __name__ == '__main__':
    unittest.main()
