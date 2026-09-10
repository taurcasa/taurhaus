"""Offline regression checks; no subprocess, socket, real CLI or credential access."""
import unittest
from attempt8_continuation_support import wait_receipt, enforce_budget, compaction_metering_gaps

class ContinuationTests(unittest.TestCase):
    def test_compaction_counter_reset_is_not_zero_billed_usage(self):
        # // Regression: cd07ae6f inherited accounting that called any tokenUsage row complete.
        e={'method':'thread/tokenUsage/updated','params':{'turnId':'compact','tokenUsage':{'last':{'inputTokens':0,'cachedInputTokens':0,'outputTokens':0,'totalTokens':6344}}}}
        self.assertEqual(compaction_metering_gaps([e]), ['compact'])

    def test_mirror_busy_then_journal_pending_is_not_an_early_failure(self):
        # // Regression: 4ff2ac17 asserted on the first snapshot before a scheduler retry.
        snapshots=iter([[], [{'payload':{'message_id':'m','stage':'pending','evidence':'thread_active'}}]])
        result=wait_receipt(lambda:next(snapshots), 'm', sleep=lambda _:None)
        self.assertEqual(result['stage'],'pending')

    def test_receipt_must_belong_to_current_message(self):
        snapshots=iter([[{'payload':{'message_id':'old','stage':'pending','evidence':'thread_active'}}],
            [{'payload':{'message_id':'m','stage':'pending','evidence':'thread_active'}}]])
        self.assertEqual(wait_receipt(lambda:next(snapshots),'m',sleep=lambda _:None)['message_id'],'m')

    def test_enqueue_without_pending_is_not_promoted_to_deferral(self):
        with self.assertRaisesRegex(AssertionError,'without thread_active'):
            wait_receipt(lambda:[{'payload':{'message_id':'m','stage':'native_enqueued'}}],'m')

    def test_previous_four_turns_and_cost_are_counted(self):
        enforce_budget(12, .01)
        with self.assertRaisesRegex(AssertionError,'turn budget'): enforce_budget(13, .01)
        with self.assertRaisesRegex(AssertionError,'cost budget'): enforce_budget(1, 2.96)

if __name__=='__main__': unittest.main()
