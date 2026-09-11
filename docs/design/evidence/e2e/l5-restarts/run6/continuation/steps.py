"""Lane 5 ordered assertions. Stop on first failure; commit each green item."""
import datetime, json, secrets, sys, time
from pathlib import Path
from actions import action
from support import host_card_seen, send_ready, read_by_seat, owner_window_evidence, clean, complete_rows, delivered, pending, reply_seen, identity_preserved, attributed_activity, busy, owner_evidence
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
  args=['read','--unread','--mark-read','--json']
  if cursor:args+=['--since',cursor]
  value=parse(mesh(args,f'{label}-{seat}-read-{page}.txt',seat))
  if value.get('done',True):break
  cursor=value['cursor'];page+=1
 # Independently page the canonical journal reader, even on empty pages.
 cursor=None;page=0
 while True:
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
   for seat in ['alpha','beta']:settle('mesh-backlog',seat)
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
   for seat in ['alpha','beta']:
    explicit_read(seat,'final')
    for label in ['baseline','taurhaus-backlog','mesh-backlog']:
     rs=receipts(read(label+'-'+seat+'-message.json')['message_id'])
     assert len([r for r in rs if r.get('stage') in ['submitted','native_enqueued']])<=1,'duplicate exposure receipt'
     assert read_by_seat(rs,seat),'unread obligation'
   checkpoint(6)
  else:raise ValueError(n)
  save(f'step{n}-outcome.json',{'step':n,'outcome':'PASS','classification':'runtime','at':time.time()})
 except BaseException as e:
  if n==2 and (OUT/'step2-outcome.json').exists() and read('step2-outcome.json')['outcome']=='PASS':n=3
  save(f'step{n}-outcome.json',{'step':n,'outcome':'FAIL','classification':'unclassified pending evidence review','reason':str(e),'at':time.time()})
  try:action({'op':'fail','reason':f'step {n}: {e}'})
  except Exception:pass
  raise
