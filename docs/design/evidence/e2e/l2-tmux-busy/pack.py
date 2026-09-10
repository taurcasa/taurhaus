"""Offline final export: bounded inventory, valid JSONL, byte-identical deduplication."""
import hashlib
import json
from pathlib import Path

BASE=Path(__file__).resolve().parent
OUT=BASE/'run'


def main():
    manifest={'format_changes':[], 'aliases':{}}
    # The controller snapshots a parsed segment as an array; retain its complete
    # rows with actual JSONL framing. Values/checksums are not rewritten.
    for p in (OUT/'team/state/messaging-v2/segments').glob('*.jsonl'):
        text=p.read_text()
        if text.lstrip().startswith('['):
            rows=json.loads(text)
            p.write_text(''.join(json.dumps(r,sort_keys=True)+'\n' for r in rows))
            manifest['format_changes'].append({'file':str(p.relative_to(OUT)),'rows':len(rows),'change':'JSON array to complete JSONL rows; values unchanged'})
    p=OUT/'codex-session-inventory.json'
    entries=json.loads(p.read_text())
    selected=[r for r in entries if r['path'].startswith(('sessions/','shell_snapshots/','thread-writer-locks/')) or r['path']=='config.toml']
    p.write_text(json.dumps({'total_generated_file_count':len(entries),'session_files':selected,'rollout_files':[], 'scope':'Relevant generated filenames/sizes only. No file contents, installation IDs, credential bytes, caches or account usage exported.'},indent=2)+'\n')
    for path in OUT.glob('*pane-*.txt'):
        path.write_text(path.read_text().rstrip()+'\n')
    preferred=['final-runtime.json','final-pane-0.txt','final-pane-1.txt']
    files=[OUT/p for p in preferred]+sorted(p for p in OUT.rglob('*') if p.is_file() and str(p.relative_to(OUT)) not in preferred)
    seen={}
    for path in files:
        digest=hashlib.sha256(path.read_bytes()).hexdigest()
        relative=str(path.relative_to(OUT))
        if digest in seen:
            manifest['aliases'][relative]=seen[digest];path.unlink()
        else:seen[digest]=relative
    (BASE/'export-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')

if __name__=='__main__':main()
