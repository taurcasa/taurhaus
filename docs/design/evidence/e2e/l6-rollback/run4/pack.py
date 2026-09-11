"""Deduplicate identical exported snapshots; retain every complete stream."""
from pathlib import Path
import hashlib
import json
BASE=Path(__file__).resolve().parent

def main():
    root=BASE/'run';seen={};manifest={'retained':{},'aliases':{}}
    old=BASE/'export-manifest.json'
    if old.exists():manifest['aliases']=json.loads(old.read_text())['aliases']
    for p in sorted(root.rglob('*')):
        if not p.is_file():continue
        digest=hashlib.sha256(p.read_bytes()).hexdigest();name=str(p.relative_to(root))
        if p.suffix in ('.json','.txt') and digest in seen:
            manifest['aliases'][name]=seen[digest];p.unlink()
        else:
            seen[digest]=name;manifest['retained'][name]={'sha256':digest,'bytes':p.stat().st_size}
    old.write_text(json.dumps(manifest,indent=2)+'\n')
    print(json.dumps({'retained':len(manifest['retained']),'aliases':len(manifest['aliases'])}))
if __name__=='__main__':main()
