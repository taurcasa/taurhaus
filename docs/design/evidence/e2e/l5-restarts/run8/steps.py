"""Lane 5 ordered assertions. Stop on first failure; commit each green item."""
import datetime, json, secrets, sys, time
from pathlib import Path
from actions import action
from support import transport_proven, host_card_seen, send_ready, read_by_seat, owner_window_evidence, clean, complete_rows, delivered, pending, reply_seen, identity_preserved, attributed_activity, busy
B=Path(__file__).resolve().parent;OUT=B/'runtime';TEAM='l5-restarts'

def save(name,value): (OUT/name).write_text(json.dumps(clean(value),indent=2)+'\n')
def read(name):
 for _ in range(100):
  try:return json.loads((OUT/name).read_text())
  except json.JSONDecodeError:time.sleep(.01)
 raise AssertionError('incomplete JSON '+name)
def root():return next(r['root'] for r in complete_rows((OUT/'events.jsonl').read_text()) if r['kind']=='isolation')
def mesh(argv,name,seat='lead'):
 argv=argv+['--claude-dir',root()+'/claude','--team',TEAM,'--name',seat]
 action({'op':'mesh','argv':argv,'save':name})
 assert read(name+'.exit.json')['exit']==0, 'Mesh command failed: '+name
 return (OUT/name).read_text()
def parse(text):
 for n,line in enumerate(text.splitlines()):
  if line.startswith('{'):
   try:return json.loads('\n'.join(text.splitlines()[n:]))
   except ValueError:pass
 raise AssertionError('no command JSON')
def rpc(method,params,name):
 action({'op':'rpc','method':method,'params':params,'save':name});return read(name)
def rows():return [r for p in (OUT/'team/state/messaging-v2/segments').glob('*.jsonl') for r in complete_rows(p.read_text())]
def receipts(mid):return [r['payload'] for r in rows() if r.get('payload',{}).get('message_id')==mid and r['event_type']!='message_accepted']
def rec(seat):return read('team/runtime/'+seat+'.json')
def logical(r):return r.get('appServer',{}).get('threadId') or r.get('session_id')
def wait(test,why,timeout=120):
 begin=time.time();end=time.monotonic()+timeout
 while time.monotonic()<end:
  if any(r.get('kind')=='stopped' for r in complete_rows((OUT/'events.jsonl').read_text())):raise RuntimeError('controller stopped; '+why)
  if test():return
  time.sleep(.5)
 raise AssertionError(why+f'; polled {time.time()-begin:.1f}s')
def snap():action({'op':'snapshot'})
def activity(seat):
 snap();r=rec(seat)
 view=rpc('get_runtime_session_snapshot',{},'latest-session-snapshot.json')
 a=read('team/state/activity/'+seat+'.json') if (OUT/'team/state/activity'/f'{seat}.json').exists() else {}
 stamp=a.get('last_observed_at',a.get('observed_at'))
 age=time.time()-datetime.datetime.fromisoformat(stamp).timestamp() if stamp else 999
 matches=[s for s in view['runtime_sessions'] if s.get('session_id')==logical(r) and s.get('tmux_pane')==r.get('paneId')]
 s=matches[0] if len(matches)==1 and matches[0].get('activity_attribution')=='attributed' and not view.get('degraded') else {}
 return attributed_activity(s,a,age)
def idle(seat):
 a=activity(seat);return a['state']=='idle' and a['age']<=120 and a['session_id']==logical(rec(seat))
def agent_rows(seat):
 r=rec(seat);thread=logical(r)
 allrows=read('rollout-items.json') if (OUT/'rollout-items.json').exists() else []
 return [x for x in allrows if x.get('thread_id')==thread]
def replied(seat,marker):
 j=[r for r in rows() if r['event_type']=='message_accepted' and r.get('payload',{}).get('sender')==seat]
 host=[]
 if seat=='beta' and (OUT/'host-events.jsonl').exists():
  host=[e for e in complete_rows((OUT/'host-events.jsonl').read_text()) if e.get('method')=='item/completed' and e.get('params',{}).get('item',{}).get('type')=='agentMessage']
 return reply_seen(j,agent_rows(seat)+host,marker)
def host_events():
 return complete_rows((OUT/'host-events.jsonl').read_text()) if (OUT/'host-events.jsonl').exists() else []
def transport(seat):return 'app_server' if rec(seat).get('appServer') else 'tmux'
def card_seen(seat,body):
 if transport(seat)=='app_server':return host_card_seen(host_events(),logical(rec(seat)),body)
 return any(body in str(r.get('payload',{}).get('output','')) for r in agent_rows(seat)
            if r.get('payload',{}).get('type') in ['function_call_output','custom_tool_call_output'])
def ready(seat,onboarding=False):
 a=activity(seat)
 journal=rows()
 startup_rows=complete_rows((OUT/'taurhaus.log.jsonl').read_text())
 recipient=rec(seat).get('recovery',{}).get('claim',{}).get('card_key',{}).get('recipient')
 ok=send_ready(journal,seat,a,onboarding=onboarding,tool_results=agent_rows(seat),
               transport=transport(seat),startup_rows=startup_rows,host_events=host_events(),recipient=recipient)
 save(('onboarding' if onboarding else 'send-guard')+'-'+seat+'.json',
      {'at':time.time(),'seat':seat,'ready':ok,'activity':a,'journal':journal,'transport':transport(seat),'recipient':recipient,'startup_rows':[r for r in startup_rows if r.get('event')=='onboarding.delivery.observed']})
 return ok

def send(seat,label):
 wait(lambda:ready(seat),'pending or in-flight delivery before '+label+' '+seat,180)
 marker='L5_'+label+'_'+seat+'_'+secrets.token_hex(3)
 text='ACTION REQUIRED: Reply exactly '+marker+'. Do not execute tools or modify files.'
 result=parse(mesh(['send',seat,text,'--summary',label],label+'-'+seat+'-send.txt'))
 save(label+'-'+seat+'-message.json',dict(result,marker=marker,seat=seat))
 return result['message_id']
def explicit_read(seat,label):
 # Keep unread/mark-read filters unchanged through every returned cursor.
 cursor=None;page=0
 while True:
  assert page<100,'read page cap exceeded (100)'
  args=['read','--unread','--mark-read','--json']
  if cursor:args+=['--since',cursor]
  value=parse(mesh(args,f'{label}-{seat}-read-{page}.txt',seat))
  if value.get('done') is True:break
  assert value.get('cursor') and value['cursor']!=cursor,'read cursor failed to advance'
  cursor=value['cursor'];page+=1
 # Independently page the canonical journal reader, even on empty pages.
 cursor=None;page=0
 while True:
  assert page<100,'journal page cap exceeded (100)'
  args=['journal','read']
  if cursor:args+=['--since',cursor]
  value=parse(mesh(args,f'{label}-{seat}-journal-{page}.txt',seat))
  if value.get('done',False):break
  assert value.get('cursor') and value['cursor']!=cursor,'journal cursor failed to advance'
  cursor=value['cursor'];page+=1
 snap()
def settle(label,seat):
 item=read(label+'-'+seat+'-message.json');mid=item['message_id']
 wait(lambda: replied(seat,item['marker']) and idle(seat),'reply and attributed idle missing '+label+' '+seat,180)
 explicit_read(seat,label)
 body=next(r['payload']['body'] for r in rows() if r['event_type']=='message_accepted' and r['payload']['message_id']==mid)
 wait(lambda: delivered(receipts(mid),activity(seat),logical(rec(seat)),seat=seat,transport=transport(seat),card_seen=card_seen(seat,body)), 'delivery receipt/read missing '+label+' '+seat)
 save(label+'-'+seat+'-delivery.json',{'message':item,'receipts':receipts(mid),'activity':activity(seat)})
def transport_settle(label,seat):
 item=read(label+'-'+seat+'-message.json');mid=item['message_id']
 def observed():
  snap()
  accepted=[r['payload'] for r in rows() if r['event_type']=='message_accepted' and r['payload']['message_id']==mid]
  if len(accepted)!=1:return False
  if transport(seat)=='app_server':witness=host_card_seen(host_events(),logical(rec(seat)),accepted[0]['body'])
  else:
   action({'op':'capture','name':label+'-transport'})
   pane=OUT/(label+'-transport-pane-'+rec(seat)['paneId'].lstrip('%')+'.txt')
   text=pane.read_text()
   witness=('[mesh]' in text and 'Inbox update from lead' in text and 'Summary: '+label in text)
  ok=transport_proven(accepted[0],receipts(mid),seat,transport(seat),witness)
  save(label+'-'+seat+'-transport.json',{'at':time.time(),'accepted':accepted[0],
       'receipts':receipts(mid),'native_witness':witness,'proven':ok,'read_observed':read_by_seat(receipts(mid),seat)})
  return ok
 wait(observed,'transport receipt and native witness missing '+label+' '+seat,180)

def checkpoint(n):
 action({'op':'capture','name':f'step{n}'})
 mesh(['team-daemon','status'],f'step{n}-owner.txt')
 snap()
 save(f'step{n}-identity.json',{'alpha':rec('alpha'),'beta':rec('beta'),'config':read('team/config.json'),'epoch':read('team/state/delivery/epoch.json'),'processes':read('identities.json')})
 save(f'step{n}-cost.json',read('cost-ledger.json'))
 save(f'step{n}-journal.json',rows())
 rpc('get_runtime_session_snapshot',{},f'step{n}-session-snapshot.json')
 # Passive /proc lock census; never acquire or seize locks.
 save(f'step{n}-locks.json',{'at':time.time(),'locks':Path('/proc/locks').read_text()})
def start_busy(seat,label):
 wait(lambda:ready(seat) and idle(seat),'prior mail or active delivery before busy input '+seat,180)
 text="For this bounded timing probe, execute one python3 command (python3 is the executable, never python): import time; for each integer from 1 through 400 print that number on its own line with flush=True and sleep 0.1 seconds, then print done. Wait for the command to finish, then reply done. This ordinary task must take at least 40 seconds. Do not modify files or run other commands."
 r=rec(seat)
 if seat=='beta':
  rpc('coordination.hosted_input',{'team_name':TEAM,'member_name':seat,'generation':r['attachmentGeneration'],'text':text},label+'-input-beta.json')
 else:
  action({'op':'tmux','argv':['send-keys','-t',r['paneId'],'-l',text]})
  action({'op':'tmux','argv':['send-keys','-t',r['paneId'],'Enter']})
def backlog(label, boundary):
 for seat in ['alpha','beta']:start_busy(seat,label)
 def both_working():
  observations={seat:activity(seat) for seat in ['alpha','beta']}
  return all(busy(a['runtime']) for a in observations.values())
 wait(both_working,'both bounded turns did not overlap in daemon working state')
 for seat in ['alpha','beta']:send(seat,label)
 # The controller samples the live journal and both seats in one RPC snapshot,
 # then initiates the boundary in the same action, with no capture/commit delay.
 action({'op':'pending_restart','label':label,'boundary':boundary,
         'messages':{seat:read(label+'-'+seat+'-message.json')['message_id'] for seat in ['alpha','beta']},
         'save':'step3-resume-result.json' if boundary=='restart_daemon' else 'step5-restart.txt'})
def unchanged(n):
 before=read('step1-identity.json')
 for seat in ['alpha','beta']:
  assert identity_preserved(before[seat],rec(seat)), 'logical identity changed or generation regressed '+seat
  item=read('baseline-'+seat+'-message.json')
  stages=[r for r in receipts(item['message_id']) if r.get('stage') in ['submitted','native_enqueued']]
  assert len(stages)<=1 and read_by_seat(receipts(item['message_id']),seat), 'baseline missing/duplicate exposure '+seat
 assert before['config']['team_incarnation_id']==read('team/config.json')['team_incarnation_id'],'team incarnation changed'

if __name__=='__main__':
 n=int(sys.argv[1]);action({'op':'step','step':n})
 try:
  if n==1:
   for seat in ['alpha','beta']:wait(lambda:ready(seat,onboarding=True),'transport-specific onboarding completion and fresh idle missing '+seat,180)
   for seat in ['alpha','beta']:send(seat,'baseline');settle('baseline',seat)
   checkpoint(1)
   assert read('team/config.json')['messaging_format']==2
   assert read('team/config.json')['delivery_owner']=='team'
   assert logical(rec('alpha')) and logical(rec('alpha'))!=logical(rec('beta'))
  elif n==2:
   checkpoint(2);backlog('taurhaus-backlog','restart_daemon')
  elif n==3:
   checkpoint(3)
   old=read('step2-identity.json')['processes'];new=read('step3-identity.json')['processes']
   old=[p for p in old if p['argv'][0].endswith('/taurhaus-daemon')];new=[p for p in new if p['argv'][0].endswith('/taurhaus-daemon')]
   assert len(old)==len(new)==1 and (old[0]['pid'],old[0]['start_ticks'])!=(new[0]['pid'],new[0]['start_ticks'])
   rpc('ping',{},'step3-ping.json')
  elif n==4:
   for seat in ['alpha','beta']:settle('taurhaus-backlog',seat)
   unchanged(4);checkpoint(4)
  elif n==5:
   checkpoint('5-before');backlog('mesh-backlog','restart_mesh')
   def changed_owner():
    snap();return read('team/state/delivery/epoch.json')!=read('step5-before-identity.json')['epoch']
   wait(changed_owner,'delivery owner epoch did not change')
   for seat in ['alpha','beta']:transport_settle('mesh-backlog',seat)
   unchanged(5);checkpoint(5)
   observations=complete_rows((OUT/'owner-observations.jsonl').read_text())
   start=next(e['at'] for e in complete_rows((OUT/'events.jsonl').read_text()) if e['kind']=='normal_mesh_restart_initiated')
   before=read('step5-before-identity.json')['processes'];after=read('step5-identity.json')['processes']
   owner=lambda ids: next(i['pid'] for i in ids if 'team-daemon' in i['argv'] and 'start' in i['argv'])
   deliveries=[datetime.datetime.fromisoformat(r['committed_at']).timestamp() for r in rows() if r.get('payload',{}).get('message_id') in [read('mesh-backlog-'+s+'-message.json')['message_id'] for s in ['alpha','beta']] and r['payload'].get('stage') in ['submitted','native_enqueued']]
   end=time.time()
   wait(lambda: complete_rows((OUT/'owner-observations.jsonl').read_text())[-1]['at']>=end,'owner observer did not cover window end',65)
   observations=complete_rows((OUT/'owner-observations.jsonl').read_text())
   census=owner_window_evidence(observations,start,end,owner(before),owner(after),min(deliveries))
   save('step5-owner-census.json',census)
   assert census['outcome']=='PASS', census['classification']+': '+census['reason']
  elif n==6:
   accounting=[]
   for seat in ['alpha','beta']:
    explicit_read(seat,'final')
    for label in ['baseline','taurhaus-backlog','mesh-backlog']:
     item=read(label+'-'+seat+'-message.json');mid=item['message_id']
     accepted=[r for r in rows() if r['event_type']=='message_accepted' and r['payload']['message_id']==mid]
     assert len(accepted)==1,'accepted obligation missing or duplicate'
     rs=receipts(mid)
     targets=[t for t in accepted[0]['payload']['delivery_targets'] if t['recipient']==seat]
     assert len(targets)==1,'accepted target missing or duplicate'
     # Step 5 established the native witness; this rechecks receipt accounting only.
     if label=='mesh-backlog':assert transport_proven(accepted[0]['payload'],rs,seat,transport(seat),True),'receipt accounting changed (native witness established at step 5)'
     accounting.append({'label':label,'seat':seat,'message':item,'accepted':accepted[0],'receipts':rs})
     assert len([r for r in rs if r.get('stage') in ['submitted','native_enqueued']])<=1,'duplicate exposure receipt'
     assert read_by_seat(rs,seat),'unread obligation'
   save('obligation-accounting.json',accounting)
   unchanged(6);checkpoint(6)
  else:raise ValueError(n)
  save(f'step{n}-outcome.json',{'step':n,'outcome':'PASS','classification':'runtime','at':time.time()})
 except BaseException as e:
  if n==2 and (OUT/'step2-outcome.json').exists() and read('step2-outcome.json')['outcome']=='PASS':n=3
  save(f'step{n}-outcome.json',{'step':n,'outcome':'FAIL','classification':'unclassified pending evidence review','reason':str(e),'at':time.time()})
  try:action({'op':'fail','reason':f'step {n}: {e}'})
  except Exception:pass
  raise


# Inline offline tests keep this review confined to the requested packet files.
# Run: PYTHONPATH=docs/design/evidence/e2e/l5-restarts/run8 python3 -m unittest steps.ReviewRegressions
import unittest
from unittest.mock import patch

class ReviewRegressions(unittest.TestCase):
 def render_report(self):
  import tempfile
  with tempfile.TemporaryDirectory() as directory:
   base=Path(directory)/'l5-restarts/run8';base.mkdir(parents=True)
   (base/'final-audit.json').write_text((B/'final-audit.json').read_text())
   document=base.parent.parent/'l5-restarts.md'
   document.write_text('# Old verdict\n\nRun7: historical spend.\n\n## Historical run6 continuation verdict\n\nOlder evidence.\n')
   namespace={'__file__':str(base/'report.py'),'__name__':'__main__'}
   code=compile((B/'report.py').read_text(),str(B/'report.py'),'exec')
   exec(code,namespace)
   first=document.read_text()
   exec(code,namespace)
   self.assertEqual(document.read_text(),first,'regeneration must be idempotent')
   return first
 def test_report_structural_limit(self):
  # // Regression: d90ce598 called an impossible completed hosted turn mere timing.
  report=self.render_report()
  self.assertIn('harness structural limit',report.splitlines()[0])
  self.assertIn('normal_daemon_stop',report)
  self.assertIn('cannot',report)
  self.assertIn('orchestrator',report)
  self.assertIn('3.459',report)
 def test_report_history(self):
  # // Regression: d90ce598 orphaned the displaced run7 lead below run8's headline.
  report=self.render_report()
  self.assertLess(report.index('## Historical run7 verdict'),report.index('Run7:'))
 def test_report_cost_and_duration(self):
  # // Regression: d90ce598 omitted conservative spend and rendered null as None.
  report=self.render_report().split('## Run8 — eighth-attempt evidence',1)[1]
  self.assertIn('$0.361888800',report)
  self.assertIn('all input at the output rate',report)
  self.assertIn('| taurhaus-backlog / beta | — |',report)
 def test_report_controller_disclosure(self):
  # // Regression: d90ce598 labelled a controller needing an external Enter exact.
  report=self.render_report()
  paragraph=next(p for p in report.split('\n\n') if '(l5-restarts/run8/controller.py)' in p)
  self.assertIn('out-of-band Enter',paragraph)
  self.assertIn('warmup-quit-confirmation.json',paragraph)
 def test_audit_structural_verdict(self):
  # // Regression: d90ce598 classified the terminated original hosted turn as timing.
  import ast
  tree=ast.parse((B/'audit.py').read_text())
  fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='runtime_verdict')
  scope={};exec(compile(ast.Module(body=[fn],type_ignores=[]),'audit','exec'),scope)
  audit=json.loads((B/'final-audit.json').read_text())
  self.assertIn('harness structural limit',scope['runtime_verdict'](audit['step_outcomes'],audit['working_windows']))
 def test_read_caps_each_reader(self):
  # // Regression: 1ae4ece0 followed endlessly advancing cursors without a local cap.
  for reader in ['read','journal']:
   with self.subTest(reader=reader):
    calls=[]
    def fake_mesh(args,name,seat):
     calls.append(args)
     if len(calls)>150:raise RuntimeError('test safety bound exceeded')
     return json.dumps({'done':args[0]!=reader,'cursor':str(len(calls))})
    with patch(__name__+'.mesh',side_effect=fake_mesh),patch(__name__+'.snap'):
     with self.assertRaisesRegex(AssertionError,reader+' page cap exceeded'):
      explicit_read('beta','offline')
 def test_read_follows_empty_pages_to_done(self):
  # // Regression: 1ae4ece0 lacked a local bound; bounding must preserve cursor reads.
  pages=[{'done':False,'cursor':'r1'}, {'done':True},
         {'done':False,'cursor':'j1'}, {'done':True}]
  with patch(__name__+'.mesh',side_effect=[json.dumps(p) for p in pages]) as mocked,patch(__name__+'.snap'):
   explicit_read('beta','offline')
  self.assertEqual(mocked.call_args_list[1].args[0],['read','--unread','--mark-read','--json','--since','r1'])
  self.assertEqual(mocked.call_args_list[3].args[0],['journal','read','--since','j1'])
