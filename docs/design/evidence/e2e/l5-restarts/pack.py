"""Lossless final artifact interning; daemon JSONL and primary audit files stay direct."""
import hashlib,json
from pathlib import Path

def pack(files):
 payloads={};index={}
 for name,text in files.items():
  key=hashlib.sha256(text.encode()).hexdigest()
  assert key not in payloads or payloads[key]==text
  payloads[key]=text;index[name]=key
 return {'files':index,'payloads':payloads}

def unpack(packet):
 return {name:packet['payloads'][key] for name,key in packet['files'].items()}

if __name__=='__main__':
 root=Path(__file__).resolve().parent/'run'
 keep={'taurhaus.log.jsonl','events.jsonl','host-events.jsonl','owner-observations.jsonl',
       'cost-ledger.json','usage-events.json','rollout-items.json','cleanup.json','controller-exit.json',
       'step1-outcome.json','step2-outcome.json','step3-outcome.json','step4-outcome.json',
       'step5-outcome.json','step6-outcome.json','step2-diagnostics.json','snapshots.json'}
 files={str(p.relative_to(root)):p.read_text() for p in sorted(root.rglob('*')) if p.is_file() and str(p.relative_to(root)) not in keep}
 if files:
  packet=pack(files)
  assert unpack(packet)==files
  (root/'snapshots.json').write_text(json.dumps(packet,indent=2)+'\n')
  for name in files:(root/name).unlink()
