"""Offline artifact integrity, sanitation and post-teardown checks."""
import ast,hashlib,json,re,subprocess
from pathlib import Path
from support import clean
from pack import unpack
b=Path(__file__).resolve().parent
for p in b.glob('*.py'):ast.parse(p.read_text())
a=json.loads((b/'final-audit.json').read_text());packet=json.loads((b/'runtime/snapshots.json').read_text())
for k,v in packet['payloads'].items():assert hashlib.sha256(v.encode()).hexdigest()==k
for name,text in unpack(packet).items():
 if '-pane-' in name:assert len(text.splitlines())<=60,(name,len(text.splitlines()))
for p in b.rglob('*'):
 if not p.is_file() or '__pycache__' in p.parts:continue
 text=p.read_text()
 assert not re.search(r'eyJ[A-Za-z0-9_-]{30,}\.[A-Za-z0-9_-]{20,}',text),p
 if p.suffix=='.json':assert clean(json.loads(text))==json.loads(text),p
 if p.suffix=='.jsonl':
  for line in text.splitlines():
   row=json.loads(line);assert clean(row)==row,p
 for path in re.findall(r'(?<![\w/-])/home/[A-Za-z0-9_.-]+/[^\s"\']+',text):
  assert any(path==root or path.startswith(root+'/') for root in ['/home/mstie/projects/taurhaus-l5-restarts','/home/mstie/projects/mesh-l5']),p
assert hashlib.sha256((b/'runtime/taurhaus.log.jsonl').read_bytes()).hexdigest()==a['daemon_jsonl']['sha256']
assert not Path(a['cleanup']['root']).exists()
survivors=[]
for p in Path('/proc').iterdir():
 if not p.name.isdigit():continue
 try:
  if ('TAURHAUS_TRIAL_ID='+Path(a['cleanup']['root']).name).encode()+b'\0' in (p/'environ').read_bytes():survivors.append(p.name)
 except OSError:pass
assert not survivors,survivors
assert not subprocess.check_output(['git','diff','--name-only','a7e6db7e','--','src-tauri','src'],text=True)
assert not subprocess.check_output(['git','-C','/home/mstie/projects/mesh-l5','status','--porcelain'],text=True)
result={'syntax':'pass','offline_tests':13,'packet_files':len(packet['files']),'unique_payloads':len(packet['payloads']),
 'payload_hashes':'pass','sanitization':'pass','pane_bound':'<=60 lines','daemon_jsonl_sha256':a['daemon_jsonl']['sha256'],
 'scratch_survivors':survivors,'scratch_root_absent':True,'product_diff':False,'mesh_clean':True,
 'gates':{k:v['exit'] for k,v in a['gates'].items()}}
(b/'verification.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
