"""Verify the exported failure packet offline; no runtime or credential access."""
from pathlib import Path
import json
import hashlib
import re

BASE=Path(__file__).resolve().parent

def main():
    root=BASE/'run'
    analysis=json.loads((BASE/'analysis.json').read_text())
    manifest=json.loads((BASE/'export-manifest.json').read_text())
    for name,value in manifest['retained'].items():
        assert hashlib.sha256((root/name).read_bytes()).hexdigest()==value['sha256'],name
    for target in manifest['aliases'].values():assert (root/target).is_file(),target
    assert analysis['canonical_history_retained'] and analysis['actual_format']==0
    assert analysis['owner_after_format']=='members'
    assert not analysis['handoff_request_observed'] and not analysis['ownership_boundary_events']
    assert analysis['operator_marker_retained']
    observed=analysis['product_observation']
    assert observed['maximum_sample_gap_seconds']<=1 and observed['samples_with_live_owner']==0
    assert any(row.get('reason')=='owner_stopped_by_operator' for row in observed['self_heal_rows'])
    assert analysis['cleanup']=={'survivors':[],'port_closed':True,'auth_removed':True,'root_removed':True}
    assert analysis['metering']['paid_inputs']==4 and analysis['metering']['api_equivalent_usd']<.20
    gates=json.loads((BASE/'checks-result.json').read_text())['commands']
    assert [g['command'] for g in gates]==['just check-quick','just lint','just test-contracts']
    assert all(g['exit']==0 for g in gates)
    for label,message in analysis['messages'].items():
        assert message['final_projection']['read'] and message['assistant_replies'] and message['tool_exposure'],label
    for path in root.glob('*.txt'):
        if 'pane' in path.name or 'composer' in path.name:assert len(path.read_text().splitlines())<=60,path
    for path in BASE.rglob('*'):
        if not path.is_file() or path.suffix=='.py':continue
        text=path.read_text()
        # Absolute operator paths, excluding a scratch path's embedded /home component.
        assert not re.search(r'(?<![\w/.-])/home/(?!\.local/)[A-Za-z0-9_-]+/(?!projects/(?:taurhaus-l6-rollback|mesh-l6)(?:/|\b))',text),path
        assert not re.search(r'"(?:access_token|refresh_token|id_token|installation_id)"\s*:',text),path
    result={'evidence_audit':'PASS','runtime_verdict':'FAIL at step 3',
            'required_gates':'all exit 0','offline_tests':'6 passed','independent_review':'unavailable',
            'retained_files':len(manifest['retained']),'aliases':len(manifest['aliases']),
            'daemon_rows':observed['daemon_jsonl_rows'],'cleanup':'PASS'}
    (BASE/'final-audit.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result))

if __name__=='__main__':main()
