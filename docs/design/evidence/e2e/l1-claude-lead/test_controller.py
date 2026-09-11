"""Offline generated-data tests: no real CLI, credentials, or live roots."""
import unittest
from controller_support import claude_usage, admit_input, complete_rows, sanitize

class ControllerTests(unittest.TestCase):
    def test_sanitizer_excludes_quota_disclosures_but_keeps_turn_metering(self):
        # // Regression: a94047bc retained native quota banners and rate_limits in evidence.
        value={'info': {'total_token_usage': {'input_tokens': 123}},
               'rate_limits': {'secondary': {'used_percent': 80}},
               'pane': 'READY\n⚠ Heads up, you have less than 25% of your weekly limit left. Run /status for a breakdown.\n› Ask Codex to do anything'}
        got=sanitize(value)
        self.assertNotIn('rate_limits', got)
        self.assertNotIn('25%', got['pane'])
        self.assertEqual(got['info']['total_token_usage']['input_tokens'], 123)
        self.assertIn('› Ask Codex to do anything', got['pane'])

    def test_streamed_assistant_usage_is_deduplicated_by_message_id(self):
        rows=[{'type':'assistant','message':{'id':'m','model':'claude-haiku-4-5-20251001','usage':{'input_tokens':100,'output_tokens':10,'cache_read_input_tokens':20,'cache_creation_input_tokens':40}}}]*2
        got=claude_usage(rows)
        self.assertEqual(len(got),1)
        self.assertAlmostEqual(got[0]['usd'],.000202)
    def test_later_stream_usage_replaces_lower_output(self):
        rows=[{'type':'assistant','message':{'id':'m','usage':{'input_tokens':100,'output_tokens':n}}} for n in [2,20,5]]
        self.assertEqual(claude_usage(rows)[0]['output_tokens'],20)
    def test_zero_usage_is_unverified(self):
        with self.assertRaisesRegex(ValueError,'unverified'): claude_usage([{'type':'assistant','message':{'id':'m','usage':{}}}])
    def test_caps_include_compact_and_prior_inputs(self):
        self.assertTrue(admit_input('claude',7,1,.4,.2))
        with self.assertRaisesRegex(ValueError,'input cap'): admit_input('claude',8,1,.4,.2)
        with self.assertRaisesRegex(ValueError,'input cap'): admit_input('codex',1,12,.4,.2)
        with self.assertRaisesRegex(ValueError,'headroom'): admit_input('claude',1,1,1.9,.2)
    def test_incomplete_jsonl_is_not_evidence(self):
        self.assertEqual(complete_rows('{"a":1}\n{"b":2}'),[{'a':1}])
    def test_sanitizer_hides_non_worktree_paths_and_sensitive_fields(self):
        got=sanitize({'accountUuid':'secret','auth':'secret','path':'/home/example/.claude/a','ok':'/home/mstie/projects/taurhaus-l1-claude-lead/x'})
        self.assertNotIn('secret',str(got)); self.assertNotIn('/home/example',str(got))
        self.assertIn('/home/mstie/projects/taurhaus-l1-claude-lead/x',str(got))
if __name__=='__main__': unittest.main()
