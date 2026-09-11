"""Serialize one controller action; every runtime wait allows >=60 seconds."""
import json
import os
from pathlib import Path
import sys
import time
from controller_support import complete_rows
OUT=Path(__file__).resolve().parent/os.environ.get('TRIAL_EVIDENCE_LABEL','continued-run')

def action(value):
    path=OUT/'action.json'
    assert not path.exists(), 'an action is pending'
    assert not (OUT/'result.json').exists(), 'runtime controller already stopped'
    size=(OUT/'events.jsonl').stat().st_size
    temporary=path.with_suffix('.tmp'); temporary.write_text(json.dumps(value)); temporary.replace(path)
    end=time.monotonic()+(300 if value['op'] in ('wait_alpha_delivery', 'input', 'ui') else 120)
    while time.monotonic()<end:
        with (OUT/'events.jsonl').open() as f: f.seek(size); rows=complete_rows(f.read())
        if any(r['kind']=='action_done' for r in rows): return
        if (OUT/'result.json').exists():
            if value['op']=='stop': return
            raise RuntimeError('controller stopped before completing action')
        if any(r['kind']=='stopped' for r in rows): raise RuntimeError(rows[-1])
        time.sleep(.25)
    raise TimeoutError('controller action deadline')
if __name__=='__main__': action(json.loads(sys.argv[1]))
