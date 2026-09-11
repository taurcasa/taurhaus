"""Offline audit of the failed run4 packet; never treats gates as runtime PASS."""
from pathlib import Path
import hashlib
import json
import re
from analyze import read
from rules import input_count
BASE=Path(__file__).resolve().parent

def main():
    manifest=json.loads((BASE/'export-manifest.json').read_text())
    for name,fact in manifest['retained'].items():
        assert hashlib.sha256((BASE/'run'/name).read_bytes()).hexdigest()==fact['sha256'],name
    for target in manifest['aliases'].values():assert (BASE/'run'/target).exists(),target
    analysis=json.loads((BASE/'analysis.json').read_text())
    diagnosis=json.loads((BASE/'startup-diagnosis.json').read_text())
    assert analysis['diagnosis']['classification']=='harness'
    assert analysis['diagnosis']['onboarding_consumed_by_alpha']
    assert diagnosis['inherited_predicate_result'] is False and diagnosis['attributed_idle'] and diagnosis['ready_session']
    assert len(diagnosis['read_receipts'])==1
    assert read('controller-exit.json')['exit']==1
    assert read('step1-outcome.json')['outcome']=='FAIL'
    for step in range(2,6):assert read(f'step{step}-outcome.json')['outcome']=='NOT RUN'
    assert all(value=={'outcome':'NOT SENT'} for value in analysis['messages'].values())
    assert analysis['cleanup']=={'survivors':[],'port_closed':True,'auth_removed':True,'root_removed':True}
    assert input_count(read('cost-ledger.json'))==1
    assert analysis['metering']['paid_inputs']==1 and analysis['metering']['api_equivalent_usd']<.20
    assert analysis['metering']['metering_complete']
    gates=json.loads((BASE/'checks-result.json').read_text())['commands']
    assert [g['command'] for g in gates]==['just check-quick','just lint','just test-contracts']
    assert all(g['exit']==0 for g in gates)
    for path in (BASE/'run').glob('*.txt'):
        if 'pane' in path.name or 'composer' in path.name:assert len(path.read_text().splitlines())<=60,path
    provenance=json.loads((BASE/'provenance.json').read_text())
    assert hashlib.sha256((BASE/'controller.py').read_bytes()).hexdigest()==provenance['executed_controller_sha256']
    for path in BASE.rglob('*'):
        if not path.is_file() or path.suffix=='.py':continue
        text=path.read_text()
        assert not re.search(r'(?<![\w/.-])/home/(?!\.local/)[A-Za-z0-9_-]+/(?!projects/(?:taurhaus-l6-rollback|mesh-l6)(?:/|\b))',text),path
        assert not re.search(r'"(?:access_token|refresh_token|id_token|installation_id|account_usage)"\s*:',text),path
    result={'evidence_audit':'PASS','runtime_verdict':'FAIL step (a), harness readiness predicate; (b)-(e) NOT RUN','controller_exit':1,'gate_exits':{g['command']:g['exit'] for g in gates},'offline_tests':6,'cleanup':'PASS','independent_Opus_review':'unavailable','retained_files':len(manifest['retained']),'aliases':len(manifest['aliases']),'daemon_rows':analysis['daemon_jsonl_rows']}
    (BASE/'final-audit.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result))
if __name__=='__main__':main()
