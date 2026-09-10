"""Read-only qualification of run 4's frozen controller result; never resumes the trial."""
import json
from pathlib import Path
from support import attributed_idle, complete_rows, ready_session

BASE=Path(__file__).resolve().parent
OUT=BASE/'run'
def read(name):return json.loads((OUT/name).read_text())
events=complete_rows((OUT/'events.jsonl').read_text())
record=read('final-runtime.json');activity=read('final-activity.json')
snapshot=read('final-runtime-sessions.json');ledger=read('cost-ledger.json')
sessions=[r for p in (OUT/'sessions').glob('*.jsonl') for r in complete_rows(p.read_text())]
journal=[r for p in (OUT/'team/state/messaging-v2/segments').glob('*.jsonl') for r in complete_rows(p.read_text())]
mid=record['recovery']['claim']['journal']['message_id']
receipts=[r for r in journal if r.get('payload',{}).get('message_id')==mid]
user_cards=[r for r in sessions if r.get('type')=='response_item' and r.get('payload',{}).get('role')=='user' and '[taurhaus] recovery_card' in json.dumps(r)]
reads=[r for r in sessions if r.get('type')=='event_msg' and r.get('payload',{}).get('type')=='item_completed' and r['payload'].get('item',{}).get('type')=='CommandExecution' and '[taurhaus] recovery_card' in r['payload']['item'].get('stdout','')]
ended=next(r['at'] for r in events if r['kind']=='stopped')
locks=complete_rows((OUT/'terminal-locks.jsonl').read_text())
previous={};increments=[]
for r in sessions:
    p=r.get('payload',{})
    if r.get('type')!='event_msg' or p.get('type')!='token_count' or not p.get('info'):continue
    total=p['info']['total_token_usage'];u={k:total.get(k,0)-previous.get(k,0) for k in ('input_tokens','cached_input_tokens','output_tokens')}
    increments.append({'at':r['timestamp'],'usage':u,'api_equivalent_usd':((u['input_tokens']-u['cached_input_tokens'])*.2+u['cached_input_tokens']*.02+u['output_tokens']*1.2)/1e6,'conservative_usd':(u['input_tokens']+u['output_tokens'])*1.2/1e6})
    previous=total
result={'outcome':'UNAVAILABLE / INCOMPLETE','classification':'harness','raw_controller_result':read('step1-outcome.json'),
        'reason':'Controller requires recovery-card text as user input; actual card arrived through successful explicit mesh read. In addition, a different-session notify-only turn has no retained token accounting; no further input was permitted.',
        'fresh_attributed_idle_at_stop':attributed_idle(record,activity,ended),'attributed_runtime_row':ready_session(record,snapshot),
        'onboarding_message_id':mid,'submission_count':sum(r['payload'].get('stage')=='submitted' for r in receipts),
        'explicit_read_receipt_count':sum(r['payload'].get('kind')=='consumed_by_read' for r in receipts),
        'recovery_card_user_rows':len(user_cards),'successful_card_tool_reads':len([r for r in reads if r['payload']['item'].get('exit_code')==0]),
        'tool_read_commands':[r['payload']['item']['command'] for r in reads],
        'completed_rollout_turns':[r['payload']['turn_id'] for r in sessions if r.get('type')=='event_msg' and r.get('payload',{}).get('type')=='task_complete'],
        'metering_complete':ledger['metering_complete'],'unknown_notify_turns':[r for r in ledger['turns'] if r['usd'] is None],
        'known_model_call_increments':increments,'total_spend_cap_verified':False,
        'daemon_jsonl_rows':len(complete_rows((OUT/'taurhaus.log.jsonl').read_text())),
        'passive_flock_samples':sum(bool(r['holders']) for r in locks),'terminal_lock_inode':read('step1-terminal-lock.json')['inode'],
        'product_failure_established':False,'runtime_retried':False,'steps_2_through_6':'NOT RUN',
        'limitations':['No managed-stop lock contention observed.','Model-issued onboarding read relied on scratch CLAUDE_DIR; omitted explicit --claude-dir and did not follow its returned cursor. Step 6 was not reached.','Notify-only identity and its unobserved spend were not inferred away.','No callable Opus reviewer; no review approval.']}
(BASE/'diagnosis.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k not in ('attributed_runtime_row','raw_controller_result','tool_read_commands')}))
