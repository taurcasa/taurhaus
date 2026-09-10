import json,time,secrets
from pathlib import Path
r=Path('/home/mstie/projects/taurhaus-trial/docs/design/evidence/native-eligibility/integration/attempt6/run')
p=r/'action.json'
def action(a):
 old=len((r/'events.jsonl').read_text().splitlines())
 assert not p.exists();p.write_text(json.dumps(a))
 end=time.monotonic()+25
 while time.monotonic()<end:
  ev=[json.loads(l) for l in (r/'events.jsonl').read_text().splitlines()[old:]]
  if any(e['kind']=='stopped' for e in ev):raise RuntimeError('controller stopped')
  if any(e['kind']=='action_done' for e in ev):return
  time.sleep(.05)
 raise TimeoutError(a)
action({'op':'step','step':3})
marker='deferred'+secrets.token_hex(4)
# A bounded 80-line generation provides an active turn, without tools/sleeps.
action({'op':'rpc','method':'coordination.hosted_input','params':{'team_name':'integration','member_name':'seat','generation':1,'text':'For this bounded transport timing probe, write exactly 80 numbered lines, each saying blue river stone. Do not execute tools.'},'save':'step3-active-start.json'})
action({'op':'mesh','argv':['send','seat',f'ACTION REQUIRED: Reply exactly {marker}. Do not execute tools.','--team','integration','--name','lead','--summary','active deferred marker'],'save':'step3-send.txt'})
action({'op':'mesh','argv':['team-daemon','status','--team','integration'],'save':'step3-active-status.txt'})
action({'op':'capture','name':'step3-active'})
print(marker)
