"""Send serialized actions to the isolated controller, retaining the exact script."""
import json
from pathlib import Path
import sys
import time
OUT = Path(__file__).resolve().parent / 'attempt11/run'

def action(value):
    path = OUT / 'action.json'
    assert not path.exists()
    before = (OUT / 'events.jsonl').stat().st_size
    temporary=path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value)); temporary.replace(path)
    end = time.monotonic() + 110
    while time.monotonic() < end:
        with (OUT / 'events.jsonl').open() as f:
            f.seek(before)
            rows = [json.loads(line) for line in f if line.endswith('\n') and line.strip()]
        if any(r['kind'] == 'stopped' for r in rows):
            raise RuntimeError(rows[-1])
        if any(r['kind'] == 'action_done' for r in rows): return
        time.sleep(.1)
    raise TimeoutError(value)

if __name__ == '__main__':
    for value in json.loads(sys.argv[1]): action(value)
