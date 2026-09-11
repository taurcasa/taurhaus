"""Link the harness adapter to the already-built production library; no product edits."""
from pathlib import Path
import subprocess
import time
import json
import os
import hashlib
from controller_support import sanitize
base=Path(__file__).resolve().parent
root=Path.cwd(); target=root/'src-tauri/target/release'
end=time.monotonic()+1800
while True:
    probe = subprocess.run(['pgrep','-af','(^|/)cargo( |$)'],capture_output=True,text=True)
    if probe.returncode in (0, 1) and len(probe.stdout.splitlines()) < 3: break
    if time.monotonic()>end: raise TimeoutError('Cargo admission')
    time.sleep(30)
argv=['rustc','--edition=2021',str(base/'install_hook.rs'),'--extern',f'taurhaus_lib={target}/deps/libtaurhaus_lib.rlib','-L',f'dependency={target}/deps','-o',str(root/'.check-logs/l1-install-hook')]
for path in target.glob('build/*/out'):
    argv+=['-L','native='+str(path)]
    for child in path.glob('*/lib'): argv+=['-L','native='+str(child)]
r=subprocess.run(argv,capture_output=True,text=True,timeout=300)
value={'command':argv,'exit':r.returncode,'stderr':r.stderr[-5000:]}
if not r.returncode: value['sha256']=hashlib.sha256((root/'.check-logs/l1-install-hook').read_bytes()).hexdigest()
(base/os.environ.get('TRIAL_EVIDENCE_LABEL','.')/'hook-build.json').write_text(json.dumps(sanitize(value),indent=2)+'\n');print(sanitize(r.stderr[-3000:]));raise SystemExit(r.returncode)
