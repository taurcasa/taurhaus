"""Offline bounded evidence export after teardown; no runtime or credential reads."""
import collections
import hashlib
import json
from pathlib import Path
import re
from attempt5_support import clean

BASE = Path(__file__).resolve().parent
OUT = BASE / 'attempt6'
RUN = OUT / 'run'
assert json.loads((RUN / 'cleanup.json').read_text())['root_removed']
manifest = {'filters': {}, 'duplicates': {}}
# Noisy health samples caused the executed controller's conservative size stop.
# Preserve every non-periodic structured event, plus first/last periodic samples.
p = RUN / 'taurhaus.log.jsonl'
rows = [json.loads(line) for line in p.read_text().splitlines()]
periodic = {'inotify.telemetry', 'session_scanner.scan.completed'}
keep = []
for event in periodic:
    group = [r for r in rows if r['event'] == event]
    manifest['filters'][event] = {'original_count': len(group), 'retained': min(2, len(group))}
    if group: keep.extend([group[0], group[-1]])
rows = [r for r in rows if r['event'] not in periodic or r in keep]
p.write_text(''.join(json.dumps(clean(row)) + '\n' for row in rows))
# Keep stderr first/last per diagnostic kind; no repeated connection/scan lines.
p = RUN / 'daemon.log'
if p.exists():
    groups = collections.defaultdict(list)
    for line in p.read_text().splitlines():
        line = re.sub(r'\x1b\[[0-9;]*m', '', line)
        key = re.sub(r' addr=127\.0\.0\.1:\d+', ' addr=<ephemeral-port>', line.split(' ', 1)[-1])
        groups[key].append(line)
    retained = []
    for key, group in groups.items():
        retained.extend([group[0]] if len(group) == 1 else [group[0], group[-1]])
        if len(group) > 2: manifest['filters'][key] = {'original_count':len(group), 'retained':2}
    (RUN / 'daemon-excerpts.txt').write_text('\n'.join(sorted(retained))+'\n')
    p.unlink()
# The final state remains; event history lives once in the append-only export.
p = RUN / 'hosted-transcript.json'
v = json.loads(p.read_text());v.pop('events', None)
v['evidenceEvents'] = 'host-events.jsonl'
p.write_text(json.dumps(clean(v), indent=2)+'\n')
for p in OUT.rglob('*.log'):
    p.with_suffix('.txt').write_text(clean(p.read_text()));p.unlink()
for p in OUT.rglob('*'):
    if p.is_file() and p.suffix in ['.json','.jsonl','.txt','.toml']:
        text = p.read_text()
        if p.suffix == '.json': text = json.dumps(clean(json.loads(text)),indent=2)
        elif p.suffix == '.jsonl':
            text = '\n'.join(json.dumps(row) for row in clean([json.loads(l) for l in text.splitlines() if l]))
        p.write_text(text.rstrip()+'\n')
# Copies frozen in the per-step commit win over identical final-run copies.
seen = {}
for p in sorted((p for p in OUT.rglob('*') if p.is_file()), key=lambda p: ('run' in p.parts, str(p))):
    digest = hashlib.sha256(p.read_bytes()).hexdigest()
    if digest in seen:
        manifest['duplicates'][str(p.relative_to(OUT))] = str(seen[digest].relative_to(OUT));p.unlink()
    else: seen[digest] = p
(OUT / 'export-manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
print(json.dumps({'retained_bytes':sum(p.stat().st_size for p in OUT.rglob('*') if p.is_file())}))
