"""Execute step 1 assertions, then stop at the first unmet prerequisite."""
import json
from pathlib import Path
import time
from actions import action, OUT
from controller_support import complete_rows
from continuation_support import compact_hook_registered, wait_lead_identity
start=time.monotonic(); samples=[]
while time.monotonic()-start<65:
    p=OUT/'latest/claude-hooks.json'
    try: hooks=json.loads(p.read_text()) if p.exists() else {}
    except ValueError: continue
    samples.append({'elapsed_seconds':round(time.monotonic()-start,3),'hooks':hooks})
    if compact_hook_registered(hooks): break
    time.sleep(1)
action({'op':'capture','step':1,'label':'step1-observation-end'})
(OUT/'step1-hook-poll.json').write_text(json.dumps({'deadline_seconds':65,'elapsed_seconds':time.monotonic()-start,'samples':samples},indent=2)+'\n')
status=json.loads((OUT/'initialize-result.json').read_text())
assert status['outcome']['status']=='completed'
config=json.loads((OUT/'latest/team/config.json').read_text())
assert config['messaging_format']==2 and config['delivery_owner']=='team'
if OUT.name in ('run9', 'run10'):
    from run9_support import registry_facts
    end=time.monotonic()+65
    observations=[]
    while True:
        lead=json.loads((OUT/'latest/team/runtime/lead.json').read_text())
        runtime=json.loads((OUT/'runtime-session-snapshot.json').read_text())
        activity=next((r for r in runtime['runtime_sessions'] if r.get('member_name')=='lead'),{})
        registry=json.loads((OUT/'sessions/index.json').read_text())
        facts=registry_facts(lead,activity,registry['files'])
        observations.append({'at':time.time(),**facts})
        (OUT/'step1-registry-poll.json').write_text(json.dumps({'deadline_seconds':65,'observations':observations},indent=2)+'\n')
        if facts['ready'] or time.monotonic()>=end:break
        time.sleep(1)
    if not facts['ready'] and not facts['continue_without_attribution']:
        action({'op':'capture','step':1,'label':'step1-registry-failure'})
        action({'op':'stop','step':1,'result':{'outcome':'FAIL' if facts['classification']=='taurhaus' else 'UNAVAILABLE','classification':facts['classification'],'step':1,'reason':facts['reason']}})
        raise SystemExit(2)
else:
    lead=wait_lead_identity(lambda: json.loads((OUT/'latest/team/runtime/lead.json').read_text()), time.monotonic, time.sleep)
end=time.monotonic()+100
while time.monotonic()<end:
    transcript=json.loads((OUT/'claude-transcript.json').read_text())
    if any(r.get('type')=='assistant' and r.get('message',{}).get('model')=='claude-haiku-4-5-20251001' for r in transcript): break
    time.sleep(1)
else: raise AssertionError('no authenticated Claude generation within 100 seconds')
if not compact_hook_registered(hooks):
    action({'op':'stop','step':1,'result':{'outcome':'UNAVAILABLE','classification':'harness','step':1,'reason':'claude_compaction_hook_unregistered: production daemon initialize completed and Claude authenticated/consumed its startup card, but scratch Claude settings have no SessionStart(compact) hook after a 65-second poll. Controller used daemon initialize without the desktop post-initialize hook reconciliation. Stop before step 2; no synthetic hook/card, no retry.','subchecks':{'initialize_once':'PASS','canonical_format_2':'PASS','delivery_owner_team':'PASS','claude_authenticated_and_team_bound':'PASS','claude_compaction_hook_registration':'FAIL'}}})
    raise SystemExit(2)
action({'op':'pass','step':1,'evidence':'initialize/result, runtime, native Claude transcript and hook registration'})
