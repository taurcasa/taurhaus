"""Offline controller checks: temporary data only, no CLI or credentials."""
import unittest
from support import delivered, ready, receipt_retry, clean
from controller import output_text, objects


class EvidenceRules(unittest.TestCase):
    # // Regression: 030980a7 required transport recipient on Mesh read receipts, whose actor is reader_name.
    def test_live_read_receipt_schema_uses_reader_name(self):
        rows = [{'payload': {'message_id': 'onboard', 'recipient': 'alpha', 'stage': 'submitted'}},
                {'payload': {'message_id': 'onboard', 'reader_name': 'alpha', 'reader': 'alpha@l7-ledger', 'kind': 'consumed_by_read'}}]
        self.assertTrue(ready(rows, 'onboard', True))
        self.assertTrue(delivered(rows, 'onboard', []))
        rows[-1]['payload']['reader_name'] = 'lead'
        self.assertFalse(ready(rows, 'onboard', True))
        self.assertFalse(delivered(rows, 'onboard', []))

    def test_native_tool_result_blocks_preserve_receipt_json(self):
        blocks = [{'type': 'input_text', 'text': 'Script completed\n'},
                  {'type': 'input_text', 'text': '{\n"receipts": [{"event_id": "e"}]\n}'}]
        self.assertIn({'receipts': [{'event_id': 'e'}]}, objects(output_text(blocks)))

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

    # // Regression: 030980a7 matched member_control_token but missed Mesh's CONTROL_TOKEN argv spelling.
    def test_control_token_is_redacted_inside_shell_argv(self):
        self.assertNotIn('fixture-secret', clean('env MESH_CONTROL_TOKEN=fixture-secret mesh'))


if __name__ == '__main__':
    unittest.main()
