"""Run the six items in order, commit green checkpoints, always reap the controller."""
import json, os, signal, subprocess, sys, time
from pathlib import Path
from support import complete_rows
B=Path(__file__).resolve().parent
OUT=B/'runtime';OUT.mkdir(exist_ok=True)
controller=None
trailers='Co-Authored-By: Codex (gpt-6-astra) <noreply@openai.com>\nClaude-Session: https://claude.ai/code/session_01XJa6LsgXqhBdob1f1BS7BU'
def interrupted(*_):raise KeyboardInterrupt()
for sig in [signal.SIGTERM,signal.SIGINT]:signal.signal(sig,interrupted)
try:
 with (B/'controller-console.txt').open('w') as log:
  controller=subprocess.Popen([sys.executable,str(B/'finish.py'),'--controller'],stdout=log,stderr=subprocess.STDOUT)
  deadline=time.monotonic()+240
  while time.monotonic()<deadline:
   path=OUT/'events.jsonl'
   if path.exists() and any(r['kind']=='inspection_ready' for r in complete_rows(path.read_text())):break
   if controller.poll() is not None:raise RuntimeError('controller setup exited '+str(controller.returncode))
   time.sleep(.5)
  else:raise TimeoutError('initialize readiness')
  for n in range(1,7):
   with (B/f'step{n}-console.txt').open('w') as step_log:
    result=subprocess.run([sys.executable,str(B/'steps.py'),str(n)],stdout=step_log,stderr=subprocess.STDOUT)
   print('STEP',n,'EXIT',result.returncode,flush=True)
   outcome=OUT/f'step{n}-outcome.json'
   if result.returncode and (not outcome.exists() or json.loads(outcome.read_text())['outcome']!='PASS'):break
   paths=[str(p.relative_to(Path.cwd())) for p in OUT.glob(f'step{n}-*') if p.is_file()]
   subprocess.run(['git','add','--',*paths],check=True)
   subprocess.run(['git','commit','-m',f'test(e2e): run7 lane 5 step {n} runtime checkpoint','-m',trailers],check=True)
   if result.returncode:break
  else:
   subprocess.run([sys.executable,str(B/'finish.py')],check=True)
  if controller.poll() is None:
   # A failed step already requests teardown. Allow it to finish naturally.
   controller.wait(timeout=120)
finally:
 if controller and controller.poll() is None:
  controller.terminate()
  controller.wait(timeout=150)
 print('CONTROLLER EXIT',controller.returncode if controller else None,flush=True)

sys.exit(controller.returncode if controller else 1)
