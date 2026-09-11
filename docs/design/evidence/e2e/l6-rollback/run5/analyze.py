"""Offline run5 identity/spend/boundary reconciliation; no live CLI access."""
from pathlib import Path
import hashlib
import json
from rules import startup_delivery, input_count
from datetime import datetime

BASE=Path(__file__).resolve().parent
RUN=BASE/'run'
def rows(path):return [json.loads(line) for line in path.read_text().splitlines()]
def read(name):
    manifest=BASE/'export-manifest.json'
    aliases=json.loads(manifest.read_text()).get('aliases',{}) if manifest.exists() else {}
    path=RUN/aliases.get(name,name)
    return json.loads(path.read_text()) if path.exists() else None

def main():
    events=rows(RUN/'events.jsonl')
    journal=[r for p in (RUN/'team/state/messaging-v2/segments').glob('*.jsonl') for r in rows(p)]
    workflow=rows(RUN/'team/state/workflow_events.jsonl')
    daemon=rows(RUN/'taurhaus.log.jsonl')
    census=rows(RUN/'owner-census.jsonl')
    native=[r for p in (RUN/'sessions').glob('*.jsonl') for r in rows(p)]
    inbox=read('team/inboxes/alpha.json') or []
    messages={}
    for label in ('A','B','C'):
        accepted=read(label+'-accepted.json')
        if not accepted:messages[label]={'outcome':'NOT SENT'};continue
        mid=accepted['message_id'];marker=accepted['marker']
        projection=next((r for r in inbox if r.get('message_id',r['id'])==mid),None)
        history=[r for r in journal if r.get('payload',{}).get('message_id')==mid]
        legacy=[r for r in workflow if projection and r.get('message_id')==projection['id']]
        submissions=[r for r in history if r.get('payload',{}).get('stage')=='submitted']
        legacy_submissions=[r for r in legacy if r.get('eventType')=='message_delivery_recorded' and r.get('outcome')=='tmux_injected' and r.get('channel')=='tmux']
        replies=[r for r in native if r.get('type')=='response_item' and r.get('payload',{}).get('role')=='assistant' and any(c.get('text','').strip().rstrip('.')==marker for c in r['payload'].get('content',[]))]
        tool=[r for r in native if r.get('payload',{}).get('type') in ('function_call_output','custom_tool_call_output') and marker in json.dumps(r)]
        reads=[r for r in history if r.get('payload',{}).get('kind')=='consumed_by_read' and r['payload'].get('reader_name')=='alpha']
        messages[label]={'logical_id':mid,'marker':marker,'projection':projection,'canonical_history':history,'legacy_history':legacy,'canonical_submissions':len(submissions),'legacy_submissions':len(legacy_submissions),'total_submissions':len(submissions)+len(legacy_submissions),'explicit_canonical_reads':len(reads),'assistant_replies':replies,'tool_exposure':tool}
    complete=next((r for r in census if (r.get('authority') or {}).get('transition')=='complete'),None)
    first_executor=next((r for r in census if r.get('executors')),None)
    begin=next((r for r in events if r['kind']=='same_owner_begin'),None)
    end=next((r for r in events if r['kind']=='same_owner_end'),None)
    stopped=read('step1-owner-stopped.json')
    owner_window=[r for r in census if stopped and r['at']>=stopped['at'] and (not end or r['at']<=end['at'])]
    result={'steps':[read(f'step{i}-outcome.json') for i in range(1,6)],'controller':read('controller-exit.json'),
      'messages':messages,'metering':read('cost-ledger.json')|{'paid_inputs':input_count(read('cost-ledger.json')),'raw_transport_count':read('cost-ledger.json').get('transport_inputs'),'count_correction':'max(observed model turns, transport inputs); consumed-by-read has no submitted receipt'},
      'cleanup':{k:v for k,v in (read('cleanup.json') or {}).items() if k!='before'},
      'canonical_history_retained':(read('step2-before-format.json') or {}).get('journal')==journal,
      'owner_observation':{'census_samples':len(owner_window),'maximum_sample_gap_seconds':max((b['at']-a['at'] for a,b in zip(owner_window,owner_window[1:])),default=None),'live_owner_samples_after_stop':sum(bool(r['owners']) for r in owner_window),'named_skips':[r for r in daemon if r.get('event')=='coordination.team_daemon.skipped'],'self_heal_passes':[r for r in daemon if r.get('event')=='self_heal.pass.completed']},
      'executor_observation':{'first_transition_complete_sample':complete,'first_executor_sample':first_executor,'same_owner_begin':begin,'same_owner_end':end,'attached_after_handoff':read('step4-executor-before.json')},
      'daemon_jsonl_rows':len(daemon),'observer_errors':[r for r in events if 'observer_error' in r['kind']]}
    result['startup_delivery']=startup_delivery(journal,native)
    result['alpha_journal_traffic']=[{'accepted':r,'receipts':[v for v in journal if v.get('payload',{}).get('message_id')==r['payload']['message_id'] and v.get('event_type')!='message_accepted'} for r in journal if r.get('event_type')=='message_accepted' and r.get('payload',{}).get('author',{}).get('name')=='alpha']
    result['raw_outcome_hashes']={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in RUN.glob('*outcome.json')}
    (BASE/'analysis.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'steps':result['steps'],'controller':result['controller'],'cleanup':result['cleanup'],'inputs':result['metering']['paid_inputs'],'usd':result['metering']['api_equivalent_usd'],'messages':{k:{key:v[key] for key in ('total_submissions','canonical_submissions','legacy_submissions') if key in v} for k,v in messages.items()}}))

if __name__=='__main__':main()
