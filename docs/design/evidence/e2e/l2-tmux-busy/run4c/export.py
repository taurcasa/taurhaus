"""Offline run-4 export: remove account quota fields, retain all daemon rows, dedup files."""
import hashlib
import json
from pathlib import Path
from support import clean, complete_rows, evidence_jsonl

BASE=Path(__file__).resolve().parent
OUT=BASE/'run'
manifest={'aliases':{}, 'sanitization':'Drop account rate_limits; retain per-turn token counts and every daemon JSONL row.'}
for p in OUT.rglob('*'):
    if not p.is_file():continue
    if p.suffix=='.jsonl':
        p.write_text(evidence_jsonl(complete_rows(p.read_text())))
    elif p.suffix=='.json':
        p.write_text(json.dumps(clean(json.loads(p.read_text())),indent=2)+'\n')
    else:p.write_text(clean(p.read_text()))
preferred=['final-runtime.json','final-pane-0.txt','final-pane-1.txt']
paths=[OUT/p for p in preferred]+sorted(p for p in OUT.rglob('*') if p.is_file() and str(p.relative_to(OUT)) not in preferred)
seen={}
for p in paths:
    digest=hashlib.sha256(p.read_bytes()).hexdigest(); name=str(p.relative_to(OUT))
    if digest in seen:
        manifest['aliases'][name]=seen[digest];p.unlink()
    else:seen[digest]=name
(BASE/'export-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'exit':0,'aliases':len(manifest['aliases'])}))
