"""Offline evidence audit and read-only process census; no harness/auth access."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

BASE=Path(__file__).resolve().parent
records=[json.loads(l) for l in (BASE/'events.jsonl').read_text().splitlines()]
isolation=next(r for r in records if r['kind']=='isolation')
root=Path(isolation['environment']['CODEX_HOME']).parent
marker=b'TAURHAUS_TRIAL_ID='+root.name.encode()+b'\0'
owned=json.loads((BASE/'owned-before-cleanup.json').read_text())
survivors=[]
for proc in Path('/proc').iterdir():
    if not proc.name.isdigit(): continue
    try:
        ticks=(proc/'stat').read_text().rsplit(')',1)[1].split()[19]
        if marker in (proc/'environ').read_bytes() or any(int(proc.name)==p['pid'] and ticks==p['start_ticks'] for p in owned):
            survivors.append({'pid':int(proc.name),'start_ticks':ticks})
    except (FileNotFoundError,PermissionError,ProcessLookupError): pass
violations=[]
for path in BASE.rglob('*'):
    if not path.is_file() or path.suffix=='.py': continue
    text=path.read_text()
    if re.search(r'(?<![\w/-])/home/[^/\s]+/(?!projects/(?:taurhaus-l4-resume-team|mesh-l4)(?:/|\b))[^\s"\']*',text): violations.append(str(path.relative_to(BASE))+': operator path')
    if re.search(r'\bsk-[A-Za-z0-9_-]{20,}|\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+',text): violations.append(str(path.relative_to(BASE))+': credential pattern')
    if path.name.startswith('final-pane') and path.suffix=='.json':
        value=json.loads(text)
        if isinstance(value,dict) and len(value.get('lines',[]))>60: violations.append(path.name+': pane too long')
logs=[json.loads(l) for l in (BASE/'taurhaus.log.jsonl').read_text().splitlines()]
assert all(not r.get('event','').startswith('usage.') for r in logs)
assert not any(r.get('kind')=='rpc_request' and r['request']['method']=='coordination.resume_team' for r in records)
assert not survivors and not root.exists() and not violations
mesh=Path('/home/mstie/projects/mesh-l4')
# Cargo may retain a hardlinked copy of exactly the executable this run built.
digest=json.loads((BASE/'build/binaries.json').read_text())['mesh']['sha256']
previous=BASE/'final-audit.json'
removed=json.loads(previous.read_text()).get('matching_cargo_executable_copies_removed',[]) if previous.exists() else []
for candidate in (mesh/'target/debug/deps').glob('mesh-*'):
    if candidate.is_file() and candidate.suffix=='':
        with candidate.open('rb') as stream: same=hashlib.file_digest(stream,'sha256').hexdigest()==digest
        if same:
            candidate.unlink(); removed.append(str(candidate.relative_to(mesh)))
result={'owned_processes_rechecked':len(owned),'survivors':survivors,'scratch_root_removed':not root.exists(),
        'auth_removed':not (root/'codex/auth.json').exists(),'mesh_trial_binary_removed':not (mesh/'target/debug/mesh').exists(),
        'matching_cargo_executable_copies_removed':removed,
        'mesh_descriptor_diff_exit':subprocess.run(['git','-C',str(mesh),'diff','--exit-code','--','src/delivery/app_server/capabilities.rs'],capture_output=True).returncode,
        'daemon_jsonl_rows':len(logs),'privacy_violations':violations,'runtime_seconds':round(records[-1]['at']-records[0]['at'],3),
        'executed_controller_and_support_commit':'f62bb158','gate_processes_excluded':'No gate is a trial seat; gate runner waits its own children.'}
(BASE/'final-audit.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result))
