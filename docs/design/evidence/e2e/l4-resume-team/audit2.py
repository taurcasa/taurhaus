"""Read-only final census of the exact owned runtime namespace and scratch roots."""
import json
import re
import socket
import subprocess
from pathlib import Path

BASE=Path(__file__).resolve().parent
OUT=BASE/'run2'
events=[json.loads(line) for line in (OUT/'events.jsonl').read_text().splitlines()]
env=next(r['environment'] for r in events if r['kind']=='isolation')
root=Path(env['TAURHAUS_DATA_DIR']).parent
marker=b'TAURHAUS_TRIAL_ID='+env['TAURHAUS_TRIAL_ID'].encode()+b'\0'
survivors=[]
for proc in Path('/proc').iterdir():
    if not proc.name.isdigit(): continue
    try:
        if marker not in (proc/'environ').read_bytes(): continue
        survivors.append({'pid':int(proc.name),'start_ticks':(proc/'stat').read_text().rsplit(')',1)[1].split()[19]})
    except (FileNotFoundError,ProcessLookupError,PermissionError): pass
with socket.socket() as probe:
    probe.settimeout(.2)
    closed=probe.connect_ex(('127.0.0.1',int(env['TAURHAUS_DAEMON_PORT'])))!=0
mesh=subprocess.run(['git','-C','/home/mstie/projects/mesh-l4','status','--porcelain'],capture_output=True,text=True)
violations=[]
for path in [BASE.with_suffix('.md'),*BASE.rglob('*')]:
    if not path.is_file() or '__pycache__' in path.parts: continue
    for match in re.finditer(r'(?<![\w/-])/home/[A-Za-z0-9_.-]+/[^\s\"\'<>`]+',path.read_text()):
        if not match.group().startswith(('/home/mstie/projects/taurhaus-l4-resume-team','/home/mstie/projects/mesh-l4')):
            violations.append(str(path.relative_to(BASE.parent)))
result={'owned_runtime_survivors':survivors,'private_port_closed':closed,'scratch_root_removed':not root.exists(),
        'auth_copy_removed':not (root/'codex/auth.json').exists(),'mesh_status_exit':mesh.returncode,'mesh_status':mesh.stdout,
        'resource_removed':not Path('/home/mstie/projects/taurhaus-l4-resume-team/src-tauri/resources/mesh').exists(),
        'disallowed_operator_home_paths':violations,'runtime_observation_seconds':round(events[-1]['at']-events[0]['at'],3)}
(OUT/'final-audit.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
assert not survivors and closed and not root.exists() and not mesh.stdout and result['resource_removed'] and not violations
