"""Offline privacy contract for the fifth trial's additional native evidence."""
import unittest
from run5_capture import public_row


class CaptureRules(unittest.TestCase):
    def test_native_evidence_keeps_identity_without_private_text(self):
        row = {'timestamp': 'fixture-time', 'type': 'event_msg', 'payload': {
            'type': 'task_complete', 'turn_id': 'fixture-turn',
            'last_agent_message': 'private fixture body'}}
        result = public_row(row)
        self.assertEqual(result['payload'], {'type': 'task_complete', 'turn_id': 'fixture-turn'})
        self.assertEqual(result['timestamp'], 'fixture-time')
        self.assertNotIn('private fixture body', str(result))
        self.assertEqual(len(result['source_sha256']), 64)

    def test_notify_hyphenated_identity_is_retained(self):
        result = public_row({'type': 'agent-turn-complete', 'turn-id': 'turn',
                             'thread-id': 'thread', 'input-messages': ['private'],
                             'last-assistant-message': 'private'})
        self.assertEqual(result['turn-id'], 'turn')
        self.assertEqual(result['thread-id'], 'thread')
        self.assertNotIn('private', str(result))


if __name__ == '__main__':
    unittest.main()
