"""Read the published Mesh activity snapshot while step 7 polls delivery."""
import json
from pathlib import Path
import time
B=Path(__file__).resolve().parent/'attempt10/run'
record=json.loads((B/'step7-runtime-after.json').read_text())
source=Path(record['activitySnapshotPath'])
assert str(source).startswith('/tmp/th-int-')
for _ in range(130):
    try:
        value=json.loads(source.read_text())
        (B/'step7-mesh-activity.json').write_text(json.dumps(value,indent=2)+'\n')
    except (FileNotFoundError, json.JSONDecodeError):
        break
    if (B/'cleanup.json').exists():break
    time.sleep(1)
