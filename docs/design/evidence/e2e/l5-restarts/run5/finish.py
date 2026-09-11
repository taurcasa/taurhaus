"""Finish step 6 through the owned controller, without signaling any foreign PID."""
import json
from pathlib import Path
import runpy
import signal
import subprocess
import sys
import time
B=Path(__file__).resolve().parent
OUT=B/'runtime'
def run_controller(argv, out):
    """Parent records the OS exit status only after the controller is reaped."""
    out.mkdir(parents=True, exist_ok=True)
    status = out / 'controller-exit.json'
    status.unlink(missing_ok=True)
    child = subprocess.Popen(argv)
    try:
        return child.wait()
    finally:
        if child.poll() is None:
            child.terminate()  # Controller's handler runs its owned-process cleanup.
            try:
                child.wait(timeout=120)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
        status.write_text(json.dumps({'exit': child.returncode}) + '\n')


if __name__ == '__main__' and sys.argv[1:] == ['--controller']:
    def interrupted(signum, frame):
        raise KeyboardInterrupt(f'supervisor interrupted {signum}')
    signal.signal(signal.SIGTERM, interrupted)
    sys.exit(run_controller([sys.executable, str(B / 'controller.py'), 'runtime'], OUT))

# Some CLI diagnostics were named .json by the inherited instrument. Preserve
# the exact stdout in a string so final recursive JSON sanitation stays valid.
wrapped=[]
for p in OUT.rglob('*.json'):
    try: json.loads(p.read_text())
    except ValueError:
        p.write_text(json.dumps({'raw_stdout':p.read_text()},indent=2)+'\n')
        wrapped.append(str(p.relative_to(B)))
(OUT/'teardown-stdout-wrappers.json').write_text(json.dumps(wrapped,indent=2)+'\n')
action=runpy.run_path(str(B/'actions.py'))['action']
action({'op':'step','step':6})
path=OUT/'action.json';assert not path.exists()
tmp=path.with_suffix('.tmp');tmp.write_text('{"op":"finish"}');tmp.replace(path)
end=time.monotonic()+120
while time.monotonic()<end:
    if (OUT/'cleanup.json').exists():
        cleanup=json.loads((OUT/'cleanup.json').read_text())
        assert not cleanup['survivors'] and cleanup['port_closed'] and cleanup['root_removed'] and cleanup['auth_removed'],cleanup
        print(json.dumps(cleanup));break
    time.sleep(.25)
else: raise AssertionError('controller cleanup deadline exceeded')
