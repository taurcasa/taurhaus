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
 def test_live_pre_submission_metered_cap(self):
  # // Regression: 1d01e588 tested an unused conservative cap instead of the live metered gate.
  from support import enforce_budget
  enforce_budget(19,.299)
  for turns,cost in [(20,.01),(1,.30),(21,.01),(1,.301)]:
   with self.subTest(turns=turns,cost=cost), self.assertRaises(AssertionError):
    enforce_budget(turns,cost)
 def test_controller_calls_tested_gate_with_metered_cost(self):
  # // Regression: 1d01e588 imported enforce_budget but used separate inline assertions.
  import ast
  from pathlib import Path
  tree=ast.parse(Path(__file__).with_name('controller.py').read_text())
  calls=[n for n in ast.walk(tree) if isinstance(n,ast.Call) and isinstance(n.func,ast.Name) and n.func.id=='enforce_budget']
  self.assertEqual(len(calls),1)
  self.assertEqual([ast.unparse(arg) for arg in calls[0].args],
                   ["accounting['paid_inputs']", "accounting['api_equivalent_usd']"])
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

class OwnerEvidenceRegression(unittest.TestCase):
 def test_empty_census_is_unproved(self):
  # // Regression: 1d01e588 selected team-daemon run; dc8990f2/d0eacf4d certified zero owners.
  from support import owner_evidence
  for samples in [[], [{'owners':[],'epoch':{'epoch':2}}, {'owners':[],'epoch':{'epoch':3}}]]:
   actual=owner_evidence(samples)
   self.assertEqual(actual['outcome'],'UNPROVED')
   self.assertEqual(actual['classification'],'harness')
   self.assertEqual(actual['max_simultaneous_observed_owners'],0)
 def test_census_requires_both_epochs_and_rejects_overlap(self):
  from support import owner_evidence
  before={'owners':[{'pid':1}],'epoch':{'epoch':2}}
  after={'owners':[{'pid':2}],'epoch':{'epoch':3}}
  self.assertEqual(owner_evidence([before])['outcome'],'UNPROVED')
  self.assertEqual(owner_evidence([before,after])['outcome'],'PASS')
  self.assertEqual(owner_evidence([before,dict(after,owners=[{'pid':1},{'pid':2}])])['outcome'],'FAIL')
  self.assertEqual(owner_evidence([before,after,{'error':'OSError'}])['outcome'],'UNPROVED')

class OperatorPathRegression(unittest.TestCase):
 def test_credential_source_is_redacted(self):
  # // Regression: 692c28af retained the authorized credential filename literally.
  from support import clean, allowed_operator_path
  source='/'.join(['','home','fixture','.codex-account-b','auth.json'])
  self.assertFalse(allowed_operator_path(source))
  self.assertEqual(clean(source),'<authorized-source>')
  self.assertEqual(clean(source+'.bak'),'<operator-path-redacted>')
 def test_no_default_credential_source(self):
  from pathlib import Path
  source=Path(__file__).with_name('controller.py').read_text()
  self.assertIn('os.environ["L5_CREDENTIAL_SOURCE"]',source)

if __name__=='__main__':unittest.main()

class Run4Regression(unittest.TestCase):
 def test_retained_run3_identities_match_exactly_one_live_owner(self):
  # // Regression: 1d01e588 filtered team-daemon run, but owners execute start.
  import ast,json
  from pathlib import Path
  base=Path(__file__).parent
  packet=json.loads((base.parent/'run3/continuation/runtime/snapshots.json').read_text())
  ids=json.loads(packet['payloads'][packet['files']['step1-identities.json']])
  tree=ast.parse((base/'controller.py').read_text())
  observer=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='observe_owners')
  predicate=next(n for n in ast.walk(observer) if isinstance(n,ast.ListComp))
  matches=eval(compile(ast.Expression(predicate),'retained-owner-predicate','eval'),{'identities':lambda:ids})
  self.assertEqual(len(matches),1)
  self.assertEqual(matches[0]['pid'],2805580)
 def test_both_boundaries_request_explicit_python3_pacing(self):
  # // Regression: 31c7a8e7 inherited an ambiguous Python command; python exited 127.
  from unittest.mock import patch
  import steps
  for seat in ['alpha','beta']:
   for label in ['taurhaus-backlog','mesh-backlog']:
    with patch.object(steps,'rec',return_value={'paneId':'%2','attachmentGeneration':1}), patch.object(steps,'rpc') as rpc, patch.object(steps,'action') as action:
     steps.start_busy(seat,label)
     text=rpc.call_args.args[1]['text'] if seat=='beta' else action.call_args_list[0].args[0]['argv'][-1]
    self.assertIn('python3',text)
    self.assertIn('400',text)
    self.assertIn('0.1',text)
 def test_owner_observer_retains_every_sample(self):
  # // Regression: 1d01e588 deduplicated owner rows, losing proof of polling gaps.
  import ast
  from pathlib import Path
  tree=ast.parse(Path(__file__).with_name('controller.py').read_text())
  observer=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='observe_owners')
  self.assertNotIn('previous',ast.unparse(observer))

class OwnerWindowRegression(unittest.TestCase):
 def test_full_window_cadence_and_old_owner_gone_before_delivery(self):
  # // Regression: 692c28af could assess epochs without full restart-window sampling.
  from support import owner_window_evidence
  samples=[{'at':n/2,'owners':[{'pid':1}] if n<2 else [] if n==2 else [{'pid':2}],'epoch':{'epoch':2 if n<3 else 3}} for n in range(7)]
  self.assertEqual(owner_window_evidence(samples,.5,2.5,1,2,2)['outcome'],'PASS')
  self.assertNotEqual(owner_window_evidence(samples[2:],.5,2.5,1,2,2)['outcome'],'PASS')
  self.assertNotEqual(owner_window_evidence(samples[:2]+samples[5:],.5,2.5,1,2,2)['outcome'],'PASS')
  self.assertNotEqual(owner_window_evidence(samples,.5,2.5,1,2,.75)['outcome'],'PASS')
  samples[3]['owners'].append({'pid':1})
  self.assertEqual(owner_window_evidence(samples,.5,2.5,1,2,2)['outcome'],'FAIL')
