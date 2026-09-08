from pathlib import Path, PurePosixPath
import tarfile, tempfile, json, hashlib, collections, re
base=Path('/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0')
archive=Path('/home/mstie/projects/taurjob/docs/wave-1/mesh-archive/taurjob-team-archive.tar.gz')
root=Path(tempfile.mkdtemp(prefix='wave1-',dir=base))
manifest=[]
with tarfile.open(archive) as t:
 for m in t.getmembers():
  name=PurePosixPath(m.name)
  if not m.isfile() or name.is_absolute() or '..' in name.parts: continue
  if '/inboxes/' not in m.name and not m.name.endswith('state/workflow_events.jsonl'): continue
  data=t.extractfile(m).read(); target=root/name; target.parent.mkdir(parents=True,exist_ok=True); target.write_bytes(data)
  manifest.append({'path':str(target),'archive_member':m.name,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
messages=[]
for p in sorted(root.glob('*/inboxes/*.json')):
 for i,m in enumerate(json.loads(p.read_text())):
  messages.append({'path':str(p),'index':i,'recipient':p.stem,'message':m})
infos=[m for m in messages if re.match(r'\s*INFO ONLY:',m['message'].get('text',''),re.I)]
workflow=root/'taurjob-team/state/workflow_events.jsonl'
events=[(i,json.loads(s)) for i,s in enumerate(workflow.read_text().splitlines(),1)]
sends=[(i,e) for i,e in events if e.get('eventType')=='message_sent']
ids={m['message']['id'] for m in messages if m['message'].get('id')}
lead=[m for m in messages if m['recipient'] in ('team-lead','lead-taurjob')]
leadids={m['message']['id'] for m in lead if m['message'].get('id')}
joins=[{'workflow_line':i,'event':e} for i,e in sends if e.get('message_id') in leadids]
infoids={m['message']['id'] for m in infos if m['message'].get('id')}
replyrefs=[]
for source in infos:
 s=source['message']; sid=s.get('id')
 for target in messages:
  tm=target['message']; text=tm.get('text','')
  if tm.get('from')!=source['recipient'] or tm.get('timestamp','')<s.get('timestamp',''): continue
  if sid and (sid in text or tm.get('replyTo')==sid or tm.get('reply_to_message_id')==sid):
   replyrefs.append({'source_id':sid,'reply_path':target['path'],'reply_index':target['index']})
result={'archive':str(archive),'archive_sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'extraction_root':str(root),'manifest':manifest,'envelopes':len(messages),'info_prefix_envelopes':len(infos),'info_prefix_percent':100*len(infos)/len(messages),'info_chars':sum(len(m['message']['text']) for m in infos),'all_body_chars':sum(len(m['message'].get('text','')) for m in messages),'by_recipient':dict(collections.Counter(m['recipient'] for m in messages)),'info_by_recipient':dict(collections.Counter(m['recipient'] for m in infos)),'workflow_message_sent_rows':len(sends),'distinct_send_ids':len(set(e.get('message_id') for _,e in sends)),'retained_send_id_joins':len(set(e.get('message_id') for _,e in sends)&ids),'lead_envelopes':len(lead),'lead_id_bearing':len(leadids),'lead_send_echo_joins':joins,'explicit_info_reply_id_matches':replyrefs,'info_records':infos}
(base/'archive-census.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k not in ('manifest','info_records','lead_send_echo_joins')},indent=2))
for i,m in enumerate(infos): print('INFO',i,m['recipient'],m['index'],m['message'].get('id'),m['message'].get('from'),m['message']['text'][:700])
print('LEAD JOINS',json.dumps(joins,indent=2))
