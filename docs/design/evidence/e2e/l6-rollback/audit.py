"""Read-only post-teardown evidence/ownership audit; never invokes a harness."""
import hashlib
import json
from pathlib import Path
import re

BASE=Path(__file__).resolve().parent

def audit():
    root=BASE/'run';manifest=json.loads((BASE/'export-manifest.json').read_text())
    def resolve(name):return root/manifest['aliases'].get(name,name)
    cleanup=json.loads(resolve('cleanup.json').read_text())
    assert cleanup['survivors']==[] and cleanup['port_closed'] and cleanup['auth_removed'] and cleanup['root_removed']
    still_owned=[]
    for row in cleanup['before']:
        try:tick=(Path('/proc')/str(row['pid'])/'stat').read_text().rsplit(')',1)[1].split()[19]
        except (FileNotFoundError,ProcessLookupError):continue
        if tick==row['start_ticks']:still_owned.append({'pid':row['pid'],'start_ticks':tick})
    assert not still_owned,'owned process survived'
    files=0;streams={};panes={}
    for relative,expected in manifest['retained'].items():
        p=root/relative;raw=p.read_bytes();text=raw.decode()
        assert hashlib.sha256(raw).hexdigest()==expected['sha256'],relative
        assert not re.search(r'\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b|\bsk-[A-Za-z0-9_-]{12,}',text),'credential-shaped bytes in '+relative
        assert not re.search(r'(?<![\w/.-])/home/[^/\s]+/(?!projects/(?:taurhaus-l6-rollback|mesh-l6)(?:/|\b))',text),'operator path in '+relative
        if p.suffix=='.jsonl':
            assert not raw or raw.endswith(b'\n'),'partial JSONL '+relative
            streams[relative]=len([json.loads(line) for line in text.splitlines()])
        if p.suffix=='.txt' and ('pane' in p.name or 'composer' in p.name):
            panes[relative]=len(text.splitlines());assert panes[relative]<=60,relative
        files+=1
    for alias,target in manifest['aliases'].items():
        assert target in manifest['retained'] and not (root/alias).exists(),alias
    assert 'taurhaus.log.jsonl' in streams
    return {'exit':0,'owned_survivors':still_owned,'cleanup_verified':True,'files_verified':files,'jsonl_rows':streams,'pane_lines':panes,'aliases_verified':len(manifest['aliases'])}

if __name__=='__main__':
    result=audit();(BASE/'final-audit.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
