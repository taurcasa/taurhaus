"""Offline evidence regression; no CLI, credentials or network."""
import unittest
from attempt6_support import new_events

class EvidenceTests(unittest.TestCase):
    def test_repeated_and_rolling_snapshots(self):
        # Regression: d3b95226 re-appended the entire host buffer every poll.
        a = {'emittedAtMs': 1, 'method': 'delta', 'params': {'text': 'a'}}
        b = {'emittedAtMs': 2, 'method': 'delta', 'params': {'text': 'b'}}
        c = {'emittedAtMs': 3, 'method': 'delta', 'params': {'text': 'c'}}
        self.assertEqual(new_events([a, b], [a, b]), [])
        self.assertEqual(new_events([a, b], [b, c]), [c])
        self.assertEqual(new_events([], [a, a]), [a, a])
        self.assertEqual(new_events([a], [a, a]), [a])

if __name__ == '__main__': unittest.main()
