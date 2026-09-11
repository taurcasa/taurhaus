"""Offline analysis of retained run3 evidence; never launches a CLI or reads credentials."""
import hashlib
import json
from pathlib import Path

BASE=Path(__file__).resolve().parent
RUN=BASE/'run'
def resolve(name):
    manifest=BASE/'export-manifest.json'
    aliases=json.loads(manifest.read_text()).get('aliases',{}) if manifest.exists() else {}
    return RUN/aliases.get(name,name)
def read(name):return json.loads(resolve(name).read_text())
def rows(name):return [json.loads(line) for line in (RUN/name).read_text().splitlines()]

def main():
    before=read('step2-before-format.json')
    after=read('step2-after-format.json')
    census=rows('owner-census.jsonl')
    daemon=rows('taurhaus.log.jsonl')
    stopped=read('step2-owner-stopped.json')['marker']
    messages={}
    journal=rows('team/state/messaging-v2/segments/000001.jsonl')
    workflow=rows('team/state/workflow_events.jsonl')
    inbox=read('team/inboxes/alpha.json')
    native=[r for p in (RUN/'sessions').glob('*.jsonl') for r in map(json.loads,p.read_text().splitlines())]
    for label in ('A','B'):
        accepted=read(label+'-accepted.json');mid=accepted['message_id']
        projection=next(r for r in inbox if r.get('message_id')==mid)
        messages[label]={'marker':accepted['marker'],'logical_id':mid,'delivery_id':projection['id'],
                         'canonical_history':[r for r in journal if r.get('payload',{}).get('message_id')==mid],
                         'legacy_history':[r for r in workflow if r.get('message_id')==projection['id']],
                         'final_projection':projection,
                         'assistant_replies':[r for r in native if r.get('type')=='response_item' and r.get('payload',{}).get('role')=='assistant' and accepted['marker'] in json.dumps(r['payload'].get('content',[]))],
                         'tool_exposure':[r for r in native if r.get('payload',{}).get('type') in ('function_call_output','custom_tool_call_output') and accepted['marker'] in json.dumps(r)]}
    analysis={'classification':'mesh contract mismatch against binding run3 ruling; no product modification',
              'failed_step':3,'reason':read('step3-outcome.json')['reason'],
              'required_format':1,'actual_format':after['config']['messaging_format'],
              'owner_before_format':before['config']['delivery_owner'],
              'owner_after_format':after['config']['delivery_owner'],
              'epoch_before':before['epoch'],'epoch_after':after['epoch'],
              'canonical_history_retained':before['journal']==after['journal']==journal,
              'authority':read('team/state/messaging-authority.json'),
              'handoff_request_observed':any(r.get('kind')=='handoff_observation' and r.get('name')=='handoff' for r in rows('events.jsonl')),
              'ownership_boundary_events':[r for r in workflow if r.get('eventType')=='delivery_owner_changed'],
              'operator_marker_retained':read('team/state/delivery/owner-stopped.json')==stopped,
              'product_observation':{'claim':'owner restart not observed between stop and step3 failure; step4 handoff not reached',
                'owner_census_rows':len(census),'maximum_sample_gap_seconds':max(b['at']-a['at'] for a,b in zip(census,census[1:])),
                'samples_with_live_owner':sum(bool(r['owners']) for r in census),
                'self_heal_rows':[r for r in daemon if 'self_heal.pass' in r.get('event','') or 'team_daemon' in r.get('event','')],
                'member_executors':[r for r in read('identities.json') if len(r['argv'])>1 and r['argv'][1]=='daemon' and r['argv'][0].endswith('/mesh')],
                'daemon_jsonl_rows':len(daemon)},
              'messages':messages,'C':'NOT SENT: step 5 not reached',
              'observer_errors':[r for r in rows('events.jsonl') if 'observer_error' in r.get('kind','')],
              'cleanup':{k:v for k,v in read('cleanup.json').items() if k!='before'},
              'metering':read('cost-ledger.json')}
    (BASE/'analysis.json').write_text(json.dumps(analysis,indent=2)+'\n')
    print(json.dumps({k:v for k,v in analysis.items() if k in ('actual_format','owner_after_format','canonical_history_retained','handoff_request_observed','ownership_boundary_events','cleanup')}))

if __name__=='__main__':main()
