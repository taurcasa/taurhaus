from pathlib import Path
import json,collections,re
out=Path(__file__).resolve().parent
units=[json.loads(l) for l in (out/'proposition-candidates.jsonl').read_text().splitlines()]
tasks={t['id']:t for t in json.loads((out/'render.json').read_text())['tasks']}
for u in units:
 if u['source_class']=='source-adapter/link gap':
  u['proposed_class']=u['source_class'];u['source_class']='UNRESOLVED';u['evidence_label']='U'
  u['reason']='Literal match is a discovery pointer only; applicability, direction of copying, source role and proposition equivalence have NOT been checked. No coverage credit.'
 if u['stratum']=='short measure results':
  tid=u['task'][1:];t=tasks[tid];field=u['field'];u['review_state']='manually checked short-result proposition';u['classification_label']='I — source-class judgment; UNVERIFIED until acceptance-owner review'
  if field in ['identity','owner','commit']:pass
  elif field=='acceptance':
   u.update(source_class='retained-structured',evidence_label='S',source_evidence=[f'render.json tasks[id={tid}].contract.completion_signal: Acceptance architect; ruling seq 2 by architect'],reason='The archived contract names architect; the positive ruling is separately present, not inferred from task completion.')
  elif field=='deliverable':
   u.update(source_class='retained-structured',evidence_label='S',source_evidence=[f'render.json tasks[id={tid}].contract.deliverable'],reason='Retained contract explicitly names this relative deliverable path and, for T3, facts update. Display shortens the path; this does not independently prove file delivery.')
  elif field=='state' and u['text']=='complete':
   u.update(source_class='retained-structured',evidence_label='S',source_evidence=[f'render.json tasks[id={tid}].snapshot_status=completed; completion '+t['completions'][0]['event_id']],reason='Terminal lifecycle is supported at this cut; it supplies no remaining-work claim.')
  elif field=='state' and u['text']=='accepted':
   u.update(source_class='retained-structured',evidence_label='S',source_evidence=[f'render.json tasks[id={tid}].rulings[seq=2]: kind=verdict, value=accepted, by=architect, ref='+t['rulings'][1]['ref']],reason='Positive own-task ruling is explicit. Candidate/completion association remains the reader’s literal, untyped join.')
  elif field=='kind':
   u.update(source_class='source-adapter/link gap',evidence_label='I',source_evidence=[f'render.json tasks[id={tid}].contract.work_kind='+t['contract']['work_kind']],reason='UNVERIFIED mapping: measure←verification, diagnose←docs in these examples. Retain source kind unless owner approves the display vocabulary adapter; no new authored declaration.')
  elif field=='remaining':
   u.update(source_class='new authored declaration (outcome)',evidence_label='I',source_evidence=[f'render.json tasks[id={tid}].completions[0] and contract; no explicit remaining_status carrier'],reason='UNVERIFIED truth of none. It is an explicit hand-ledger declaration, not established by completion. In future ingest it from an authorized existing result if present; otherwise outcome.remaining_status, never implicit none.')
  u['proposed_class']=None
stats={'candidate_units':len(units),'by_population':dict(collections.Counter(u['population'] for u in units)),'coding_counts':dict(collections.Counter(u['source_class'] for u in units)),'manually_checked_short_result_units':sum(u['review_state'].startswith('manually checked') for u in units),'exact_text_mirror_occurrences':sum(bool(u['exact_text_mirror_of']) for u in units),'unresolved_units':sum(u['source_class']=='UNRESOLVED' for u in units),'semantic_denominator':'UNVERIFIED: syntactic candidates are not an exhaustive unique-proposition count'}
(out/'reviewed-index.jsonl').write_text(''.join(json.dumps(u,ensure_ascii=False)+'\n' for u in units)); (out/'review-stats.json').write_text(json.dumps(stats,indent=2)+'\n');print(json.dumps(stats,indent=2))
