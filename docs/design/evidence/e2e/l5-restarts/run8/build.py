"""Checkout-local builds/gates; no descriptor mutation or installation."""
import json, os, signal, subprocess, sys, time
from pathlib import Path
ROOT=Path.cwd(); BASE=Path(__file__).resolve().parent
MESH=Path('/home/mstie/projects/mesh-l5')
assert ROOT.name=='taurhaus-l5-restarts'
assert subprocess.check_output(['git','branch','--show-current'],text=True).strip()=='feat/e2e-l5-restarts'
child=None

def run(name,cwd,command):
 global child
 out=BASE/'build';out.mkdir(exist_ok=True)
 probes=[];deadline=time.monotonic()+1800
 while True:
  p=subprocess.run(['pgrep','-af','(^|/)cargo( |$)'],capture_output=True,text=True)
  probes.append({'at':time.time(),'exit':p.returncode,'output':p.stdout})
  if len(p.stdout.splitlines())<3:break
  if time.monotonic()>=deadline:raise TimeoutError('three Cargo processes remained for 30 minutes')
  time.sleep(30)
 env=dict(os.environ,CARGO_TARGET_DIR=str(cwd/('src-tauri/target' if cwd==ROOT else 'target')),CARGO_BUILD_JOBS='1')
 start=time.time()
 with (out/(name+'.txt')).open('w') as f:
  child=subprocess.Popen(command,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT,start_new_session=True)
  code=child.wait()
 result={'command':command,'cwd':str(cwd),'exit':code,'seconds':time.time()-start,'cargo_preflight':probes}
 (out/(name+'.json')).write_text(json.dumps(result,indent=2)+'\n')
 print(name,code,flush=True)
 return code

def stop(*_):raise KeyboardInterrupt()
for sig in [signal.SIGTERM,signal.SIGINT]:signal.signal(sig,stop)
try:
 subprocess.run(['git','merge-base','--is-ancestor','26c06132','HEAD'],check=True)
 assert subprocess.check_output(['git','-C',str(MESH),'rev-parse','--short','HEAD'],text=True).strip()=='310144d'
 for name,cwd,cmd in [('resources',ROOT,['just','ensure-tauri-resources']),('daemon-build',ROOT,['just','build-daemon']),('mesh-build',MESH,['cargo','build','--bin','mesh'])]:
  if run(name,cwd,cmd):sys.exit(1)
finally:
 if child and child.poll() is None:
  os.killpg(child.pid,signal.SIGTERM);child.wait(timeout=30)
