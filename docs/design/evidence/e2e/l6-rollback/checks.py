"""Checkout-local builds/gates, with machine-wide Cargo admission."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

BASE=Path(__file__).resolve().parent
ROOT=BASE.parents[4]

def run(label,cmd,cwd):
    polls=[]
    deadline=time.monotonic()+1800
    while True:
        probe=subprocess.run(['pgrep','-af','(^|/)cargo( |$)'],capture_output=True,text=True)
        rows=probe.stdout.splitlines()
        polls.append({'at':time.time(),'count':len(rows),'exit':probe.returncode})
        if len(rows)<3:break
        if time.monotonic()>=deadline:raise TimeoutError('Cargo queue exceeded 30 minutes')
        time.sleep(30)
    env=os.environ.copy();env.pop('CARGO_TARGET_DIR',None);env['CARGO_BUILD_JOBS']='1'
    start=time.time()
    with (BASE/(label+'.txt')).open('w') as log:
        result=subprocess.run(cmd,cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT)
    row={'command':cmd,'cwd':str(cwd),'exit':result.returncode,'seconds':time.time()-start,'cargo_polls':polls,'last_lines':(BASE/(label+'.txt')).read_text().splitlines()[-20:]}
    (BASE/(label+'.json')).write_text(json.dumps(row,indent=2)+'\n')
    print(json.dumps(row),flush=True)
    return result.returncode

if __name__=='__main__':
    if sys.argv[1]=='build':
        code=run('daemon-build',['just','build-daemon'],ROOT)
        if code==0:code=run('mesh-build',['cargo','build','--bin','mesh'],ROOT.parent/'mesh-l6')
    else:
        code=0
        for label in ['check-quick','lint','test-contracts']:
            code=max(code,run(label,['just',label],ROOT))
    raise SystemExit(code)
