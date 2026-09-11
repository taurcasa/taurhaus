"""Serialized, bounded builds in the two explicitly authorized worktrees."""
import json, os, subprocess, time
from pathlib import Path
OUT=Path(__file__).resolve().parent
ROOT=OUT.parents[4]
rows=[]
for cwd, command, target in [(ROOT,['just','build-daemon'],ROOT/'src-tauri/target'),(ROOT.parent/'mesh-l7',['cargo','build','--bin','mesh'],ROOT.parent/'mesh-l7/target')]:
    deadline=time.monotonic()+1800
    while True:
        p=subprocess.run(['pgrep','-af','(^|/)cargo( |$)'],capture_output=True,text=True)
        rows.append({'command': "pgrep -af '(^|/)cargo( |$)'",'at':time.time(),'exit':p.returncode,'pids':[line.split()[0] for line in p.stdout.splitlines()]})
        (OUT/'builds.json').write_text(json.dumps(rows,indent=2)+'\n')
        if len(p.stdout.splitlines()) < 3:break
        if time.monotonic()>=deadline:raise SystemExit(78)
        time.sleep(30)
    env={**os.environ,'CARGO_TARGET_DIR':str(target),'CARGO_BUILD_JOBS':'1'}
    log=OUT/('daemon-build.txt' if cwd==ROOT else 'mesh-build.txt')
    with log.open('w') as f: result=subprocess.run(command,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT)
    rows.append({'command':' '.join(command),'cwd':str(cwd),'CARGO_TARGET_DIR':str(target),'CARGO_BUILD_JOBS':'1','exit':result.returncode})
    (OUT/'builds.json').write_text(json.dumps(rows,indent=2)+'\n')
    print(command,result.returncode,flush=True)
    if result.returncode:raise SystemExit(result.returncode)
