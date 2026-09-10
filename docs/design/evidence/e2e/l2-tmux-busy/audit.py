"""Post-run read-only cleanup/evidence audit. No signal, CLI or credential access."""
import json
import re
import socket
from pathlib import Path

BASE=Path(__file__).resolve().parent
OUT=BASE/'run'
rows=[json.loads(l) for l in (OUT/'events.jsonl').read_text().splitlines()]
isolation=next(r for r in rows if r['kind']=='isolation')
root=Path(isolation['root']);trial_id=root.name
port=int(isolation['environment']['TAURHAUS_DAEMON_PORT'])
survivors=[]
for p in Path('/proc').iterdir():
    if not p.name.isdigit():continue
    try:
        if ('TAURHAUS_TRIAL_ID='+trial_id).encode()+b'\0' in (p/'environ').read_bytes():
            survivors.append({'pid':int(p.name),'start_ticks':(p/'stat').read_text().rsplit(')',1)[1].split()[19]})
    except (FileNotFoundError,ProcessLookupError,PermissionError):pass
with socket.socket() as probe:
    probe.settimeout(.2);closed=probe.connect_ex(('127.0.0.1',port))!=0
pane_lines={p.name:len(p.read_text().splitlines()) for p in OUT.glob('*pane-*.txt')}
for p in OUT.rglob('*.jsonl'):
    for line in p.read_text().splitlines():json.loads(line)
# Scan the retained packet, not any operator configuration or account database.
for p in OUT.rglob('*'):
    if not p.is_file():continue
    text=p.read_text()
    assert not re.search(r'(?<![\w/.-])/home/mstie/(?!projects/(?:taurhaus-l2-tmux-busy|mesh-l2)/)',text),p
    assert not re.search(r'"(?:access_token|refresh_token|installation_id|account_usage)"\s*:',text),p
assert not survivors and closed and not root.exists()
assert max(pane_lines.values())<=60
result={'exit':0,'survivors':survivors,'port':port,'port_closed':closed,'root_removed':not root.exists(),'auth_removed':not (root/'codex/auth.json').exists(),'pane_lines':pane_lines,'jsonl_complete':True,'forbidden_export_fields':False,'step1':'FAIL — taurhaus','steps2_to_6':'NOT RUN','opus_review':'unavailable: no callable Opus reviewer in session','runtime_trial_count':1,'paid_retry_count':0}
(BASE/'final-audit.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result))
