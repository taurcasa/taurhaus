"""Offline final packet audit. Does not turn operational success into full PASS."""
from pathlib import Path
import hashlib
import json
import re

BASE=Path(__file__).resolve().parent

def main():
    manifest=json.loads((BASE/'export-manifest.json').read_text())
    def read(name):
        return json.loads((BASE/'run'/manifest['aliases'].get(name,name)).read_text())
    for name,fact in manifest['retained'].items():
        p=BASE/'run'/name
        assert hashlib.sha256(p.read_bytes()).hexdigest()==fact['sha256'],name
    for target in manifest['aliases'].values():assert (BASE/'run'/target).exists(),target
    a=json.loads((BASE/'analysis.json').read_text());j=json.loads((BASE/'adjudication.json').read_text())
    for name,digest in j['raw_outcome_hashes'].items():
        assert hashlib.sha256((BASE/'run'/name).read_bytes()).hexdigest()==digest,name
    assert read('controller-exit.json')['exit']==0
    assert [read(f'step{i}-outcome.json')['outcome'] for i in range(1,6)]==['PASS']*5
    assert j['verdict'].startswith('INCOMPLETE') and j['Opus_review']['status']=='unavailable'
    assert all(m['total_submissions']==1 and len(m['assistant_replies'])==1 for m in a['messages'].values())
    assert j['messages']['B']['reads']==4 and not j['messages']['B']['single_read']
    assert a['canonical_history_retained'] and not a['observer_errors']
    assert not a['owner_observation']['live_owner_samples_after_stop']
    assert a['owner_observation']['maximum_sample_gap_seconds']<=1
    assert any(r.get('reason')=='owner_stopped_by_operator' for r in a['owner_observation']['named_skips'])
    assert any(r.get('owner_ensure_refused')==1 for r in a['owner_observation']['self_heal_passes'])
    assert j['executor']['max_live_alpha_executors']==1
    assert j['executor']['seconds_after_handoff_end']>0 and j['executor']['seconds_after_verified_downgrade']>0
    assert a['cleanup']=={'survivors':[],'port_closed':True,'auth_removed':True,'root_removed':True}
    assert a['metering']['paid_inputs']==5 and a['metering']['metering_complete']
    assert abs(a['metering']['api_equivalent_usd']-.01063136)<1e-10
    assert a['metering']['api_equivalent_usd']<.20
    for v in read('step5-dispositions.json').values():
        assert v['projection']['read'] and json.loads(v['ack_status'])['acked']
    assert not a['alpha_journal_traffic']
    gates=json.loads((BASE/'checks-result.json').read_text())['commands']
    assert [g['command'] for g in gates]==['just check-quick','just lint','just test-contracts']
    assert all(g['exit']==0 for g in gates)
    provenance=json.loads((BASE/'provenance.json').read_text())
    assert hashlib.sha256((BASE/'controller.py').read_bytes()).hexdigest()==provenance['executed_controller_sha256']
    for path in (BASE/'run').glob('*.txt'):
        if 'pane' in path.name or 'composer' in path.name:assert len(path.read_text().splitlines())<=60,path
    for path in BASE.rglob('*'):
        if not path.is_file() or path.suffix=='.py':continue
        text=path.read_text()
        assert not re.search(r'(?<![\w/.-])/home/(?!\.local/)[A-Za-z0-9_-]+/(?!projects/(?:taurhaus-l6-rollback|mesh-l6)(?:/|\b))',text),path
        assert not re.search(r'"(?:access_token|refresh_token|id_token|installation_id|account_usage)"\s*:',text),path
    result={'evidence_audit':'PASS','lane_verdict':j['verdict'],'controller_exit':0,
            'gate_exits':{g['command']:g['exit'] for g in gates},'offline_tests':10,
            'cleanup':'PASS','raw_outcomes_byte_exact':True,'retained_files':len(manifest['retained']),
            'aliases':len(manifest['aliases']),'daemon_jsonl_rows':a['daemon_jsonl_rows']}
    (BASE/'final-audit.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result))

if __name__=='__main__':main()
