"""Offline evidence guards; no CLI, credentials or model calls."""
import unittest
from attempt9_support import enforce_budget, retained_log, validate_compaction, pack_events, unpack_events

class Evidence(unittest.TestCase):
    def test_lossless_event_dedup(self):
        rows=[{'at':1,'kind':'command_result','output':'pane'}, {'at':2,'kind':'command_result','output':'pane'}, {'at':3,'kind':'daemon_response','response':{'id':'r','result':[]}}]
        packed,payloads=pack_events(rows)
        self.assertEqual(unpack_events(packed,payloads),rows)
        self.assertEqual(packed[0]['payload_ref'],packed[1]['payload_ref'])
        self.assertEqual(len(payloads),2)

    def test_fresh_budget(self):
        enforce_budget(16, 3)
        with self.assertRaises(AssertionError): enforce_budget(17, 0)
        with self.assertRaises(AssertionError): enforce_budget(1, 3.01)

    def test_complete_log(self):
        # // Regression: ddb7aef1 inherited family filtering and periodic thinning.
        rows=[{'event':e,'at':i} for i,e in enumerate(['other','inotify.telemetry','inotify.telemetry','inotify.telemetry'])]
        self.assertEqual(retained_log(rows+rows)[0],rows)

    def test_compaction_requires_generation_logs_and_card(self):
        # // Regression: ddb7aef1 observed a boundary with no recovery delivery.
        before={'contextGeneration':'1','appServer':{'threadId':'t'}}
        after={'contextGeneration':'2','appServer':{'threadId':'t'}}
        rows=[{'event':'compaction.codex_host.'+s} for s in ['received','delivered']]
        events=[{'params':{'item':{'type':'userMessage','text':'[taurhaus] recovery_card'}}}]
        validate_compaction(before,after,rows,events,'[taurhaus] recovery_card')
        for bad in [(before,after,[],events,'[taurhaus] recovery_card'),(before,before,rows,events,'[taurhaus] recovery_card'),(before,after,rows,[], '')]:
            with self.assertRaises(AssertionError):validate_compaction(*bad)

if __name__=='__main__':unittest.main()
