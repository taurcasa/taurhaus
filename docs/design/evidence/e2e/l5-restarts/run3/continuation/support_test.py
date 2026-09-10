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
 def test_pending_is_accepted_unexposed_and_working_at_same_sample(self):
  # // Regression: 92108455 inherited a pending-stage requirement; deferral writes no receipt.
  a={'state':'active','session_id':'alpha-session','activity_attribution':'attributed'}
  row={'event_type':'message_accepted','payload':{'message_id':'q','delivery_targets':[{'recipient':'alpha'}]}}
  self.assertTrue(pending([row],'q','alpha',a))
  self.assertFalse(pending([],'q','alpha',a))
  self.assertFalse(pending([row],'other','alpha',a))
  self.assertFalse(pending([row],'q','beta',a))
  self.assertFalse(pending([row],'q','alpha',dict(a,state='idle')))
  self.assertFalse(pending([row],'q','alpha',dict(a,activity_attribution='unattributed')))
  for stage in ['submitted','consumed','native_enqueued','outcome_unknown']:
   receipt={'event_type':'delivery_receipt','payload':{'message_id':'q','stage':stage}}
   self.assertFalse(pending([row,receipt],'q','alpha',a))
  foreign={'event_type':'delivery_receipt','payload':{'message_id':'other','stage':'submitted'}}
  self.assertTrue(pending([row,foreign],'q','alpha',a))
 def test_reply_any_rollout_row_or_journal(self):
  self.assertTrue(reply_seen([], [{'payload':{'type':'agent_message','message':'R_ALPHA'}}], 'R_ALPHA'))
  self.assertTrue(reply_seen([{'payload':{'body':'R_ALPHA'}}], [], 'R_ALPHA'))
  self.assertFalse(reply_seen([], [], 'R_ALPHA'))
 def test_generation_and_logical_identity(self):
  before={'session_id':'a','attachmentGeneration':1}
  self.assertTrue(identity_preserved(before,dict(before,attachmentGeneration=2)))
  self.assertFalse(identity_preserved(before,dict(before,session_id='b')))
  self.assertFalse(identity_preserved(dict(before,attachmentGeneration=2),before))

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

class TmuxActivityRegression(unittest.TestCase):
 def test_daemon_active_overrides_missing_or_old_sidecar_state(self):
  # // Regression: f95ec193 used the optional sidecar state as the tmux activity authority.
  from support import attributed_activity
  session={'session_id':'alpha','state':'active','source':None,'activity_attribution':'attributed'}
  for sidecar in [{}, {'state':'idle'}, {'activity_confidence':'active'}]:
   with self.subTest(sidecar=sidecar):
    actual=attributed_activity(session,sidecar,2)
    self.assertEqual(actual['state'],'active')
    self.assertEqual(actual['session_id'],'alpha')
    self.assertFalse(delivered([{'stage':'submitted'},{'kind':'consumed_by_read'}],actual,'alpha'))
 def test_idle_requires_current_daemon_state_and_attribution(self):
  from support import attributed_activity
  for state in [None, 'active']:
   session={'session_id':'alpha','state':state,'activity_attribution':'attributed'}
   actual=attributed_activity(session,{'state':'idle'},1)
   self.assertFalse(delivered([{'stage':'submitted'},{'kind':'consumed_by_read'}],actual,'alpha'))
  actual=attributed_activity({'session_id':'alpha','state':'idle','activity_attribution':'unattributed'},{},1)
  self.assertIsNone(actual['session_id'])

class Run2Ruling(unittest.TestCase):
 def test_fresh_twenty_input_thirty_cent_cap(self):
  from support import enforce_budget
  enforce_budget(20,.30)
  for turns,cost in [(21,.01),(1,.301)]:
   with self.assertRaises(AssertionError):enforce_budget(turns,cost)
 def test_busy_uses_daemon_attribution_without_sidecar(self):
  # // Regression: f95ec193 let optional activity sidecars veto daemon busy evidence.
  from support import busy
  for state in ['active','likely_working']:
   self.assertTrue(busy({'state':state,'session_id':'a','activity_attribution':'attributed'}))
  self.assertFalse(busy({'state':'active','activity_attribution':'unattributed'}))

class TransientRefusals(unittest.TestCase):
 def test_only_explicit_busy_refusals_retry_inside_window(self):
  from support import retry_busy
  for message in ['host member busy','LOCK BUSY']:
   self.assertTrue(retry_busy(1,message,1,65))
   self.assertFalse(retry_busy(1,message,66,65))
  self.assertFalse(retry_busy(0,'host member busy',1,65))
  self.assertFalse(retry_busy(1,'authorization refused',1,65))

class RestartRegression(unittest.TestCase):
 def test_checkpoint_refreshes_owned_process_identities(self):
  # // Regression: 2600f95a compared cached identities.json across the restart.
  from unittest.mock import patch
  import steps
  current={'pid':1};saved={}
  def action(value):
   if value['op']=='snapshot':current['pid']=2
  def read(name):return [dict(current)] if name=='identities.json' else {}
  with patch.object(steps,'action',side_effect=action), patch.object(steps,'mesh'), patch.object(steps,'rec',return_value={}), patch.object(steps,'read',side_effect=read), patch.object(steps,'rows',return_value=[]), patch.object(steps,'rpc'), patch.object(steps,'save',side_effect=lambda n,v:saved.update({n:v})), patch.object(steps.Path,'read_text',return_value=''):
   steps.checkpoint(3)
  self.assertEqual(saved['step3-identity.json']['processes'],[{'pid':2}])
 def test_stopped_host_requires_supported_resume(self):
  # // Regression: 2600f95a mistook stopped:true RPC success for a live readable host.
  from support import host_needs_resume
  self.assertTrue(host_needs_resume({'result':{'stopped':True,'attachmentGeneration':2}}))
  self.assertTrue(host_needs_resume({'error':{'message':'not running'}}))
  self.assertFalse(host_needs_resume({'result':{'thread':{'status':{'type':'idle'}},'events':[]}}))

if __name__=='__main__':unittest.main()
