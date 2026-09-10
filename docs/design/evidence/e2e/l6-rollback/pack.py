"""Deterministic evidence export: trim pane blanks and alias identical snapshots."""
import hashlib
import json
from pathlib import Path

BASE=Path(__file__).resolve().parent

def pack():
    root=BASE/'run';manifest={'pane_normalization':[],'aliases':{},'retained':{}}
    for p in sorted(root.rglob('*.txt')):
        if 'pane' not in p.name and 'composer' not in p.name:continue
        before=p.read_text();after=before.rstrip('\n')+'\n'
        if before!=after:
            manifest['pane_normalization'].append({'path':str(p.relative_to(root)),'before_sha256':hashlib.sha256(before.encode()).hexdigest(),'after_sha256':hashlib.sha256(after.encode()).hexdigest()})
            p.write_text(after)
    seen={}
    for p in sorted(root.rglob('*')):
        if not p.is_file():continue
        rel=str(p.relative_to(root));digest=hashlib.sha256(p.read_bytes()).hexdigest()
        # Complete streams are retained even when identical to another stream.
        if p.suffix in ('.json','.txt') and digest in seen:
            manifest['aliases'][rel]=seen[digest];p.unlink()
        else:
            manifest['retained'][rel]={'sha256':digest,'bytes':p.stat().st_size}
            seen[digest]=rel
    (BASE/'export-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    return manifest

if __name__=='__main__':
    print(json.dumps({'retained_files':len(pack()['retained'])}))
