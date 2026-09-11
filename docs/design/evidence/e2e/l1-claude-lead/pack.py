"""Deduplicate already-stopped evidence; never changes or aborts a runtime step."""
import hashlib
import json
import sys
from pathlib import Path
BASE=Path(__file__).resolve().parent
run=BASE/(sys.argv[1] if len(sys.argv)>1 else 'run')
objects={}; aliases={}
for directory in sorted(p for p in run.iterdir() if p.is_dir()):
    for path in sorted(directory.rglob('*')):
        if not path.is_file(): continue
        raw=path.read_text(); digest=hashlib.sha256(raw.encode()).hexdigest()
        objects.setdefault(digest,{'format':'json' if path.suffix=='.json' else 'text','content':json.loads(raw) if path.suffix=='.json' else raw})
        aliases[str(path.relative_to(run))]=digest
        path.unlink()
    for child in sorted(directory.rglob('*'),reverse=True):
        if child.is_dir(): child.rmdir()
    directory.rmdir()
(run/'snapshots.json').write_text(json.dumps({'aliases':aliases,'objects':objects},indent=2)+'\n')
print(json.dumps({'snapshot_paths':len(aliases),'unique_objects':len(objects)}))
