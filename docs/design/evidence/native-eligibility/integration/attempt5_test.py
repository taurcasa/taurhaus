"""Credential-free checks of evidence filtering and paid-generation accounting."""
import unittest
from attempt5_support import clean, ledger


class EvidenceChecks(unittest.TestCase):
    # Regression: d3b95226 over-redacted scratch /home segments and authority
    # metadata, while retaining rollout account rate-limit rows.
    def test_preserve_scratch_authority_and_remove_account_metadata(self):
        result = clean({'path': '/tmp/trial/home/.local/bin/mesh',
                        'rootAuthorityRevision': '0', 'auth_removed': True,
                        'rate_limits': {'account': 'private'}})
        self.assertEqual(result['path'], '/tmp/trial/home/.local/bin/mesh')
        self.assertEqual(result['rootAuthorityRevision'], '0')
        self.assertTrue(result['auth_removed'])
        self.assertNotIn('rate_limits', result)

    def test_account_rows_removed_but_token_usage_retained(self):
        rows = [{'method': 'account/rateLimits/updated', 'params': {'secret': 'private'}},
                {'method': 'thread/tokenUsage/updated', 'params': {'inputTokens': 20}},
                {'installationId': 'private', 'auth': 'private'}]
        result = clean(rows)
        self.assertEqual(len(result), 2)
        self.assertEqual(result[0]['params']['inputTokens'], 20)
        self.assertNotIn('private', str(result))

    def test_repeated_snapshots_do_not_double_charge(self):
        event = {'method': 'thread/tokenUsage/updated', 'params': {
            'threadId': 'thread', 'turnId': 'turn', 'tokenUsage': {
                'last': {'inputTokens': 1000, 'cachedInputTokens': 100,
                         'outputTokens': 10, 'reasoningOutputTokens': 2},
                'total': {'inputTokens': 1000, 'outputTokens': 10}}}}
        result = ledger([event, event], ['turn'])
        self.assertEqual(len(result['generations']), 1)
        self.assertAlmostEqual(result['api_equivalent_usd'], .000194)
        self.assertTrue(result['metering_complete'])

    def test_unmetered_turn_is_explicit_and_never_null(self):
        result = ledger([], ['unmetered'])
        self.assertFalse(result['metering_complete'])
        self.assertEqual(result['unmetered_turn_ids'], ['unmetered'])
        self.assertEqual(result['api_equivalent_usd'], 0)


if __name__ == '__main__':
    unittest.main()
