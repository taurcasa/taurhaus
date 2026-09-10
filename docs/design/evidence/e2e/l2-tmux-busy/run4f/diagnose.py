"""Offline reconciliation of retained run-4f evidence; never mutates runtime."""
import json
from datetime import datetime
from pathlib import Path
from support import complete_rows, reply_evidence

BASE=Path(__file__).resolve().parent
OUT=BASE/'run'
def read(name):return json.loads((OUT/name).read_text())
def stamp(value):return datetime.fromisoformat(value.replace('Z','+00:00')).timestamp()

events=complete_rows((OUT/'events.jsonl').read_text())
journal=[r for p in (OUT/'team/state/messaging-v2/segments').glob('*.jsonl') for r in complete_rows(p.read_text())]
rollout=[r for p in (OUT/'sessions').glob('*.jsonl') for r in complete_rows(p.read_text())]
messages={}
for label in ('Q','Q2'):
    accepted=read(label+'-accepted.json');mid=accepted['message_id']
    rows=[r for r in journal if r.get('payload',{}).get('message_id')==mid]
    submissions=[r for r in rows if r.get('payload',{}).get('stage')=='submitted']
    captured=next(read(str(p.relative_to(OUT))) for p in (OUT/'submissions').glob('*.json') if read(str(p.relative_to(OUT)))['receipt']['payload']['message_id']==mid)
    submitted=submissions[0]['payload'];attachment=submitted['attachment']
    runtime=captured['runtime']
    identity=all(attachment[key]==runtime[field] for key,field in [('pane','paneId'),('pane_pid','panePid'),('pane_start','paneStartTime'),('socket','tmuxSocket'),('tmux_session','tmuxSessionId')])
    messages[label]={'marker':accepted['marker'],'message_id':mid,'submission_count':len(submissions),
        'generations':[r['payload']['attachment']['attachment_generation'] for r in submissions],
        'pane_identity_matches':identity,'idle_at_submission':captured['activity']['activity_confidence'],
        'idle_age_seconds':stamp(submitted['observed_at'])-stamp(captured['activity']['observed_at']),
        'accepted_to_submitted_seconds':stamp(submitted['observed_at'])-stamp(rows[0]['committed_at']),
        'explicit_read_receipts':[r for r in rows if r.get('payload',{}).get('kind')=='consumed_by_read'],
        'reply':reply_evidence(journal,rollout,mid,accepted['marker']),
        'alpha_journal_replies':[r for r in journal if r.get('event_type')=='message_accepted' and r.get('payload',{}).get('author',{}).get('name')=='alpha' and accepted['marker'] in r.get('payload',{}).get('body','')]}
locks=complete_rows((OUT/'terminal-locks.jsonl').read_text())
held=[r for r in locks if r['holders']]
daemon=complete_rows((OUT/'taurhaus.log.jsonl').read_text())
requests=[r['request'] for r in events if r['kind']=='daemon_request']
result={'verdict':'UNAVAILABLE / INCOMPLETE','controller_exit':read('controller-exit.json'),
    'steps':[read(f'step{i}-outcome.json') for i in range(1,7)],
    'startup':read('startup-preflight.json'),
    'classification':'harness: post-reply fresh idle wait timed out; cause unestablished',
    'messages':messages,'resume_member_calls':sum(r['method']=='coordination.resume_member' for r in requests),
    'stop_session_calls':sum(r['method']=='stop_session' for r in requests),
    'generation_before':read('step4-before-stop.json')['attachmentGeneration'],
    'generation_after':read('step5-resumed.json')['attachmentGeneration'],
    'final_activity':read('final-activity.json'),
    'final_runtime_sessions':read('final-runtime-sessions.json'),
    'passive_lock_samples':len(locks),'samples_with_flock':len(held),
    'holder_operations':sorted({r['diagnostic']['op'] for r in held if r.get('diagnostic')}),
    'daemon_jsonl_rows':len(daemon),'daemon_last_event':daemon[-1].get('event'),
    'codex_log_files_retained':[str(p.relative_to(OUT)) for p in (OUT/'codex-log').rglob('*') if p.is_file()],
    'codex_log_note':'Failure exporter visited scratch CODEX_HOME/log after child shutdown; no regular non-symlink log files were copied. No log content is available; daemon stderr and complete JSONL remain.',
    'cleanup':read('cleanup.json'),'cost_ledger':read('cost-ledger.json'),
    'step6_note':'No controller explicit read/mark or journal reconcile command ran. Alpha explicitly read Q2 during step 5; offline reconciliation above is diagnostic, not step 6 PASS.'}
(BASE/'diagnosis.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'exit':0,'messages':{k:{f:v[f] for f in ('submission_count','generations','pane_identity_matches','idle_age_seconds')} for k,v in messages.items()},'daemon_rows':len(daemon)}))
