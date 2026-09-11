"""One authorized run, ordered steps and per-green-step commits; never a retry."""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time
from controller_support import sanitize, complete_rows
from continuation_support import installed_codex_bundle

BASE=Path(__file__).resolve().parent
LABEL=os.environ.get('TRIAL_EVIDENCE_LABEL', 'run3')
assert LABEL in ('run3', 'run4', 'run4c', 'run4d', 'run5', 'run6', 'run6/continuation', 'run7', 'run8', 'run9', 'run10')
OUT=BASE/LABEL
LEDGER_NAME='prior-spend.json' if LABEL=='run3' else 'admission-ledger.json'

def driver_failure(step):
    return {'outcome': 'UNAVAILABLE', 'classification': 'harness', 'step': step,
            'reason': 'step driver failed; see driver log (authentication cause not established)'}

def main():
    # Credential paths are explicit caller arguments, never a fallback search.
    claude_credential, codex_auth = sys.argv[1:]
    env={**os.environ, 'TRIAL_EVIDENCE_LABEL':LABEL}
    env.pop('TMUX',None)
    command=[sys.executable,str(BASE/'controller.py'),'--evidence-label',LABEL,
             '--prior-ledger',str(OUT/LEDGER_NAME),'--prior-runtime-seconds','0',
             '--claude-credential',claude_credential,'--codex-auth',codex_auth,
             '--claude-binary',str(Path(shutil.which('claude')).resolve()),
             '--codex-binary',str(installed_codex_bundle(shutil.which)[0])]
    (OUT/'driver-command.json').write_text(json.dumps(sanitize(command),indent=2)+'\n')
    controller=None; step_child=None; step=1; outcomes=[]
    def interrupted(sig,_frame): raise RuntimeError(f'driver interrupted {sig}')
    for sig in [signal.SIGTERM,signal.SIGINT]: signal.signal(sig,interrupted)
    try:
        controller=subprocess.Popen(command,env=env,start_new_session=True)
        deadline=time.monotonic()+160
        while time.monotonic()<deadline:
            if controller.poll() is not None: raise RuntimeError('controller stopped during setup')
            path=OUT/'events.jsonl'
            if path.exists() and any(r['kind']=='ready_for_controller_actions' for r in complete_rows(path.read_text())): break
            time.sleep(.5)
        else: raise TimeoutError('initialize did not become ready')
        for step in range(1,7):
            command=[sys.executable,str(BASE/('step1.py' if step==1 else 'steps.py'))]
            if step>1: command.append(str(step))
            with (OUT/f'step{step}-driver.log').open('w') as stream:
                step_child=subprocess.Popen(command,env=env,stdout=stream,stderr=subprocess.STDOUT,start_new_session=True)
                code=step_child.wait(timeout=420)
            outcomes.append({'step':step,'command':command,'exit':code})
            (OUT/'driver-steps.json').write_text(json.dumps(outcomes,indent=2)+'\n')
            print(f'Step {step}: exit {code}',flush=True)
            if code: break
            # Only paths authored by this run; never stage unrelated checkout work.
            subprocess.run(['git','add',str(OUT)],check=True)
            subprocess.run(['git','commit','-m',f'docs(e2e): record L1 {LABEL} step {step} PASS',
                            '-m','Co-Authored-By: Codex (gpt-6-astra) <noreply@openai.com>\nClaude-Session: https://claude.ai/code/session_01XJa6LsgXqhBdob1f1BS7BU'],check=True)
        if controller.poll() is None and not (OUT/'result.json').exists():
            if outcomes and outcomes[-1]['exit']:
                from actions import action
                action({'op':'stop','step':step,'result':driver_failure(step)})
        controller.wait(timeout=30)
    finally:
        for child in [step_child,controller]:
            if child and child.poll() is None:
                os.killpg(child.pid,signal.SIGTERM)
                try: child.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid,signal.SIGKILL); child.wait(timeout=5)
        (OUT/'driver-exit.json').write_text(json.dumps({'controller_exit':controller.returncode if controller else None,
                    'steps':outcomes,'children_waited':True},indent=2)+'\n')
    return controller.returncode

if __name__=='__main__': raise SystemExit(main())
