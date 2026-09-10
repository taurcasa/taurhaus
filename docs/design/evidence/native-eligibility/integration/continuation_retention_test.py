"""Offline collector regression tests. No credentials, network, or real CLI."""
import json
import unittest
from continuation_retention import retained_log, retained_view, retain_host_event

class RetentionTests(unittest.TestCase):
    def test_periodic_samples_cannot_fill_evidence_reserve(self):
        # Regression: c4735af5 counted repetitive raw telemetry as retained evidence.
        rows = [{'event':'inotify.telemetry','ts':n,'detail':'x'*1000} for n in range(1000)]
        rejection = {'event':'hosted.rpc.rejected','method':'thread/read','code':-32601,'message':'list_turns is not supported yet'}
        pending = {'event':'hosted.rpc.pending','method':'thread/read','code':-32603}
        rows[500:500] = [rejection, pending]
        kept, counts = retained_log(rows)
        self.assertIn(rejection, kept)
        self.assertIn(pending, kept)
        self.assertEqual(counts['inotify.telemetry'], 1000)
        self.assertEqual([r['ts'] for r in kept if r['event']=='inotify.telemetry'], [0,999])
        self.assertLess(len(json.dumps(kept)), 5000)

    def test_snapshot_references_archive_without_duplicating_it(self):
        view={'thread':{'id':'thread','turns':[{'id':'turn','items':[]}]},'events':[{'method':'item/completed'}],'eventsTruncated':True}
        kept=retained_view(view)
        self.assertEqual(kept['thread'], view['thread'])
        self.assertNotIn('events', kept)
        self.assertEqual(kept['evidenceEvents'], 'host-events.jsonl')
        self.assertIn('events', view)

    def test_drop_only_stream_deltas_keep_usage_and_complete_content(self):
        self.assertFalse(retain_host_event({'method':'item/agentMessage/delta'}))
        for method in ['thread/tokenUsage/updated','item/completed','turn/started','turn/completed','thread/status/changed']:
            self.assertTrue(retain_host_event({'method':method}))

if __name__=='__main__': unittest.main()
