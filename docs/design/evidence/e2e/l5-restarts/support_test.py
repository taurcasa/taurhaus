"""Offline acceptance checks; generated data only, no CLI or credentials."""
import unittest
from support import delivered, pending, reply_seen, identity_preserved
class Acceptance(unittest.TestCase):
 def test_delivery_requires_receipt_fresh_attributed_idle_and_read(self):
  r=[{'stage':'submitted'},{'kind':'consumed_by_read'}]
  a={'state':'idle','session_id':'session','age':2}
  self.assertTrue(delivered(r,a,'session'))
  for receipts,activity in [(r[:1],a),(r[1:],a),(r,dict(a,age=121)),(r,dict(a,session_id='foreign')),(r,dict(a,state='working'))]:
   self.assertFalse(delivered(receipts,activity,'session'))
 def test_pending_excludes_transport_exposure(self):
  self.assertTrue(pending([{'stage':'pending','reason':'working'}]))
  self.assertFalse(pending([{'stage':'pending'},{'stage':'native_enqueued'}]))
  self.assertFalse(pending([]))
 def test_reply_any_rollout_row_or_journal(self):
  self.assertTrue(reply_seen([], [{'payload':{'type':'agent_message','message':'R_ALPHA'}}], 'R_ALPHA'))
  self.assertTrue(reply_seen([{'payload':{'body':'R_ALPHA'}}], [], 'R_ALPHA'))
  self.assertFalse(reply_seen([], [], 'R_ALPHA'))
 def test_generation_and_logical_identity(self):
  before={'session_id':'a','attachmentGeneration':1}
  self.assertTrue(identity_preserved(before,dict(before,attachmentGeneration=2)))
  self.assertFalse(identity_preserved(before,dict(before,session_id='b')))
  self.assertFalse(identity_preserved(dict(before,attachmentGeneration=2),before))
if __name__=='__main__':unittest.main()

class HostedActivity(unittest.TestCase):
 def test_host_state_comes_from_fresh_attributed_daemon_snapshot(self):
  # // Regression: 9fa886ee required a hosted sidecar state that the publisher omits.
  from support import attributed_activity
  s={'session_id':'beta','state':'idle','source':'host','activity_attribution':'attributed'}
  a=attributed_activity(s,{'observed_at':'synthetic'},2)
  self.assertEqual(a['state'],'idle')
  self.assertEqual(a['session_id'],'beta')
  self.assertEqual(a['age'],2)
  self.assertIsNone(attributed_activity(dict(s,activity_attribution='unattributed'),{},2)['session_id'])

class EvidencePacking(unittest.TestCase):
 def test_duplicate_artifacts_round_trip_with_exact_newlines(self):
  from pack import pack, unpack
  files={'a.json':'{"x": 1}\n','b.json':'{"x": 1}\n','pane.txt':'line 1\n\n'}
  packet=pack(files)
  self.assertEqual(len(packet['payloads']),2)
  self.assertEqual(unpack(packet),files)
