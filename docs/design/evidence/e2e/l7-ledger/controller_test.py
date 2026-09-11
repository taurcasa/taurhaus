"""Offline controller checks: temporary data only, no CLI or credentials."""
import unittest
from support import delivered, ready, receipt_retry, clean


class EvidenceRules(unittest.TestCase):
    def test_first_send_requires_onboarding_transport_read_and_idle(self):
        rows = [{'payload': {'message_id': 'onboard', 'recipient': 'alpha', 'stage': 'submitted'}}]
        self.assertFalse(ready(rows, 'onboard', True))
        rows.append({'payload': {'message_id': 'onboard', 'recipient': 'alpha', 'kind': 'consumed_by_read'}})
        self.assertTrue(ready(rows, 'onboard', True))
        self.assertFalse(ready(rows, 'onboard', False))

    def test_delivery_requires_read_or_transport_and_tool_result(self):
        rows = [{'payload': {'message_id': 'm', 'recipient': 'alpha', 'stage': 'submitted'}}]
        self.assertFalse(delivered(rows, 'm', []))
        self.assertFalse(delivered(rows, 'm', [{'payload': {'type': 'message', 'content': 'm'}}]))
        self.assertTrue(delivered(rows, 'm', [{'payload': {'type': 'function_call_output', 'output': 'm'}}]))
        rows.append({'payload': {'message_id': 'm', 'recipient': 'alpha', 'kind': 'consumed_by_read'}})
        self.assertTrue(delivered(rows, 'm', []))

    def test_retry_requires_identical_receipt_and_no_source_replay(self):
        receipt = {'event_id': 'event', 'root_id': 'root', 'sequence': 1, 'request_digest': 'digest'}
        self.assertTrue(receipt_retry(receipt, dict(receipt), 1, 1))
        self.assertFalse(receipt_retry(receipt, {**receipt, 'sequence': 2}, 1, 1))
        self.assertFalse(receipt_retry(receipt, receipt, 1, 2))

    def test_public_evidence_drops_private_fields_and_message_bodies(self):
        value = {'memberControlToken': 'private', 'body': 'mail', 'message': 'mail', 'access_token': 'private', 'event_id': 'e'}
        self.assertEqual(clean(value), {'body': '<message-body-redacted>', 'message': '<message-body-redacted>', 'event_id': 'e'})


if __name__ == '__main__':
    unittest.main()
