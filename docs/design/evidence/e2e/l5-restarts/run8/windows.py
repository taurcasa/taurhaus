"""Measure the actual original turn straddling each pending boundary, never a resumed substitute."""
import datetime,json
from pathlib import Path
from support import complete_rows
B=Path(__file__).resolve().parent;P=B/'runtime'
def seconds(s):return datetime.datetime.fromisoformat(s).timestamp()
events=complete_rows((P/'events.jsonl').read_text());host=complete_rows((P/'host-events.jsonl').read_text())
usage=json.loads((P/'usage-events.json').read_text());items=json.loads((P/'rollout-items.json').read_text())
result=[]
for boundary in [e for e in events if e['kind']=='pending_boundary_sample']:
 for seat,sample in boundary['samples'].items():
  thread=sample['activity']['session_id'];at=boundary['at']
  starts=[r for r in usage if r['thread_id']==thread and r['payload'].get('type')=='task_started' and seconds(r['timestamp'])<=at]
  turn=max(starts,key=lambda r:r['timestamp']) if starts else None
  tid=turn['payload']['turn_id'] if turn else None;start=seconds(turn['timestamp']) if turn else None
  ends=[r for r in usage if r['thread_id']==thread and r['payload'].get('type')=='task_complete' and r['payload'].get('turn_id')==tid]
  finish=seconds(ends[0]['timestamp']) if ends else None
  completed=next((e['params']['turn'] for e in host if e.get('method')=='turn/completed' and e['params']['turn']['id']==tid),None)
  if finish is None and completed:finish=completed['completedAt']
  duration=finish-start if finish is not None and start is not None else None
  end_limit=finish or next((e['at'] for e in events if e['kind']=='post_daemon_stop' and e['at']>at),at)
  commands=[r for r in items if r['thread_id']==thread and r.get('payload',{}).get('type') in ['function_call','custom_tool_call']
            and 'python3' in json.dumps(r['payload']) and start is not None and start<=seconds(r['timestamp'])<=end_limit]
  proven=bool(duration is not None and duration>=30 and start<=at<=finish and commands)
  result.append({'boundary':boundary['label'],'seat':seat,'thread_id':thread,'turn_id':tid,'started_at':start,'completed_at':finish,
   'duration_seconds':round(duration,3) if duration is not None else None,'pending_at':at,'pending':sample['pending'],
   'python3_commands':commands,'outcome':'PASS' if proven else 'UNPROVED','classification':'runtime' if proven else 'harness timing',
   'reason':'Original paced turn spans boundary and lasts >=30 seconds' if proven else 'Original turn has no >=30-second completion across boundary; resumed execution is not substituted'})
(P/'working-windows.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps([{k:r[k] for k in ['boundary','seat','duration_seconds','outcome']} for r in result]))
