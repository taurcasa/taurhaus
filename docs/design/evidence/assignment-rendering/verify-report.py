import pathlib,json,re,shlex,hashlib,tarfile
b=pathlib.Path('/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0')
s=(b/'assignment-rendering-design.md').read_text()
pairs=json.loads((b/'nine-pairs.json').read_text());cards=json.loads((b/'nine-rendered-cards.json').read_text())
linked=json.loads((b/'nine-linked-cards.json').read_text());syn=json.loads((b/'synthetic-rendered-card.json').read_text())
probes=json.loads((b/'mesh-probes.json').read_text())
assert all('## '+x in s for x in ['Result','Evidence','Recommendation'])
assert len(re.findall(r'^#### Pair ',s,re.M))==9
archive=pathlib.Path('/home/mstie/projects/taurjob/docs/wave-1/mesh-archive/taurjob-team-archive.tar.gz')
assert hashlib.sha256(archive.read_bytes()).hexdigest()=='60bd9ba90b941211821c14b36b10dc583ddf21e961ff80547ff223141d2643e1'
with tarfile.open(archive) as t:
    for x in pairs:
        rows=json.load(t.extractfile('taurjob-team/inboxes/'+x['seat']+'.json'))
        for kind in ('prose','card'):assert rows[x[kind+'_index']]['text']==x[kind]['text']
    row=json.load(t.extractfile('taurjob-team/inboxes/judge-astra-1.json'))[5]
    assert row['from']=='lead-taurjob' and row['timestamp']=='2026-09-06T19:58:00.249Z'
    assert row['text'] in (b/'t9-go-evidence.txt').read_text()
assert sum(len(x['prose']['text']) for x in pairs)==15616
assert sum(len(x['card']['text']) for x in pairs)==7485
# Compare independently to source-authored values, not to renderer-produced fields.
fields_checked=0;source_commands=0
for x,c in zip(pairs,cards):
    body=c['body'];assert body in s
    fields=dict(re.findall(r'^[1-5]\. (Objective|Deliverable|First action|Completion signal|Review route): (.*)$',x['prose']['text'],re.M))
    assert len(fields)==5
    for k,v in fields.items():
        if k=='Review route':
            cut=min([i for marker in [' Commit only your file;',' Commit only your files;',' Do not read product-reviewer',' Isolation:'] if (i:=v.find(marker))>=0]+[len(v)])
            assert f'{k}: {v[:cut]}' in body
            if cut<len(v):assert v[cut:].strip() in c['Context']
        else:assert f'{k}: {v}' in body,(x['seat'],k)
        fields_checked+=1
    intro=x['prose']['text'].split('1. Objective:')[0].strip().replace('\n\n',' | ').replace('\n',' ')
    assert intro in c['Context']
    for cmd in re.findall(r'`([^`]+)`',x['prose']['text']):
        assert cmd in body,(c['pair'],cmd)
        source_commands+=cmd.startswith('mesh ')
    token=re.search(r'^Assignment: (.+)$',x['card']['text'],re.M).group(1)
    assert c['assignment_id']==token and f'Assignment: {token} ·' in body
    assert x['card']['text'].splitlines()[0][:-1]+' · team: taurjob-team]'==body.splitlines()[0]
    ev=next(e for e in json.loads((b/'archive-journals.json').read_text())['workflow_events'] if e['eventType']=='task_assigned' and e['assignment_id']==token)
    assert f'RECORDED status={ev["task_status"]}' in body
    assert 'commit HOLD' not in body and 'Perform only the action permitted' not in body
    assert not any(line.startswith('Execution: WAIT') for line in body.splitlines())
    if c['pair'] in (1,2,3,5,6,7,9):assert 'Reconciliation: CONFLICT:' in body and 'BLOCKED route:' in body
# Unique generated-card details: literals and semantic obligations identified in source inventory.
extras={1:['main','master'],2:['main','master'],3:['WebKitGTK/system','≤80 added lines','≤80 lines'],4:['cargo clippy -D warnings'],5:['main','master'],6:['--ref <T2 commit>','Pre-read only'],7:['main','master'],8:['each listed public portal type'],9:['pathspec','--ref <T2 commit>','Pre-read only','Change: supersedes a387dac0']}
for n,needles in extras.items():
    for v in needles:assert v in cards[n-1]['body'],(n,v)
assert source_commands==5,source_commands
# Literal template conformance: one recognized heading per slot, shared fixed order.
order=['Assignment','Effort','ACTION REQUIRED','Execution','Objective','Deliverable','First action','Completion signal','Review route','References','Deadline','Budget','Context','Change','Reconciliation','Record metadata']
for c in cards+linked+[syn]:
    body=c['body'];lines=body.splitlines()
    assert re.fullmatch(r'\[#.+ · owner: .+ · team: .+\]',lines[0])
    slots=[]
    for line in lines[1:-1]:
        key=line.split(': ',1)[0];assert key in order,(key,line);slots.append(key)
    assert slots==sorted(set(slots),key=order.index)
    for k in ['Assignment','ACTION REQUIRED','Execution','Objective','Deliverable','First action','Completion signal','Review route']:assert k in slots
    assert lines[-1]=='No pure acknowledgment. Follow the completion signal; report an execution blocker as specified.'
assert syn['body'] in s
for k in ['References','Budget','Change','Effort','Deadline']:assert k+': ' in syn['body']
# The five commands lost in round 1 must each remain verbatim at their source positions.
for n,cmd in [(3,'mesh task complete 4'),(4,'mesh task complete 8'),(6,'mesh task complete 5'),(6,'mesh task ruling 2 --kind verdict --value accepted|rejected --ref <T2 commit>'),(9,'mesh task ruling 2 --kind verdict --value accepted|rejected --ref <T2 commit>')]:
    assert '`'+cmd+'`' in cards[n-1]['body']
# Executed-command assertion is deliberately ONLY over this named evidence section.
executed_section=s.split('### Executed command evidence\n',1)[1].split('\n## Recommendation',1)[0]
executed={shlex.join(['mesh',*e['argv'][7:]]) for e in probes}
quoted=re.findall(r'`(mesh [^`\n]+)`',executed_section)
for cmd in quoted:assert cmd in executed,cmd
output_records=re.findall(r'\*\*\[S\] Probe (\d+), actor `[^`]+`; `(mesh [^`\n]+)` — exit \d+\.\*\*[^\n]*\n\n```text\n(.*?)```',executed_section,re.S)
assert len(output_records)==len(quoted)==22
for i,cmd,out in output_records:
    e=probes[int(i)-1]
    assert cmd==shlex.join(['mesh',*e['argv'][7:]])
    assert out==e['stdout']+(('stderr:\n'+e['stderr']) if e['stderr'] else '')
assert len(probes)==41
assert all(e['env']=={} for e in probes)
assert all(e['argv'][0]=='/home/mstie/.local/bin/mesh' and e['argv'][1]=='--claude-dir' and pathlib.Path(e['argv'][2]).is_relative_to(b) and e['cwd']==e['argv'][2] for e in probes)
assert len({e['cwd'] for e in probes})==1
assert [(e['id'],e['exit']) for e in probes if e['exit']!=0]==[(16,1),(19,1)]
for aid,gid in [(9,10),(17,18),(23,24)]:
    message_id=re.search(r'\(id: ([^)]+)\)',probes[aid-1]['stdout']).group(1)
    assignment_id=json.loads(probes[gid-1]['stdout'])['metadata']['assignment_id']
    assert message_id!=assignment_id
meta=json.loads(probes[40]['stdout'])['metadata']
assert {k:meta[k] for k in ['parent_task_ids','anchor_task_ids','scaffold_class','sunset_decision','sunset_owner','sunset_trigger']}==dict(parent_task_ids=['1'],anchor_task_ids=['2'],scaffold_class='fixture_note',sunset_decision='archive',sunset_owner='lead',sunset_trigger='fixture accepted')
# Lossless source spans after explicit doctrine substitution; linkage integrity is checked, not model uptake.
mapping=json.loads((b/'linked-doctrine-map.json').read_text())
for c,l in zip(cards,linked):
    for k in ['Objective','Deliverable','First action','Completion signal','Review route','Context']:
        value=c[k] if k=='Context' else c['fields'][k]
        for d in mapping:
            if d['pair']==c['pair'] and d['slot']==k:value=value.replace(d['removed'],d['retained']).strip()
        assert value==(l[k] if k=='Context' else l['fields'][k])
    for cmd in re.findall(r'`([^`]+)`',c['body']):assert cmd in l['body']
    assert hashlib.sha256((b/'round1-standard.md').read_bytes()).hexdigest() in l['References']
    assert l['body'] in (b/'linked-doctrine-cards.md').read_text()
assert 'each citing the mockup file or design.md line as evidence with impact and action' in linked[8]['fields']['Deliverable']
for variant,cs in [('verbatim',cards),('linked',linked)]:
    m=json.loads((b/'rendered-metrics.json').read_text())[variant]
    body=''.join(c['body'] for c in cs)
    assert m['new_characters']==len(body) and m['new_utf8_bytes']==len(body.encode())
    assert m['new_TE']==len(body)/4 and m['net_characters']==23101-len(body)
    assert m['share_original_task_characters']==m['net_characters']/84524
# Historical GO is a distinct source row. Never execute its contents.
events=json.loads((b/'archive-journals.json').read_text())['workflow_events']
prior=[e for e in events if e['eventType']=='task_assigned' and e['task_id']=='9' and e['timestamp']<=row['timestamp']]
last=max(prior,key=lambda e:e['timestamp'])
assert last['assignee']=='judge-astra-1' and last['assignment_id']==cards[8]['assignment_id']
go=(b/'historical-go-rendered.txt').read_text();assert go in s
for v in ['8f97e28','6a44056','60 checks','57 → 60','R1 to R10','47(d)','deferred','--ref 8f97e28','49f0e0e3-6833-40f8-906a-78419909a5b1']:
    assert v in go
for cmd in re.findall(r'`([^`]+)`',row['text']):assert cmd in go,cmd
print('PASS: pinned archive matches all 18 source bodies; exact assertion 15616/7485; 45 authored fields and 9 contexts covered against source, with line-5 relocation checked.')
print('PASS: all 5 archived mesh command occurrences restored verbatim; 9 generation tokens, recorded-state labels and listed generated-card additions checked.')
print('PASS: 9 historical cards, 9 linked-doctrine cards and 1 synthetic card conform to the literal heading order; optional References/Budget/Change/Effort/Deadline exercised.')
print(f'PASS: {len(quoted)} command quotes checked only inside Executed command evidence; quoted outputs equal captured outputs; 41 empty-environment scratch-root invocations, expected exit-1 probes 16 and 19 only.')
print('PASS: assign message IDs differ from task-get generation tokens in all 3 cases; parent/anchor/scaffold/sunset keys persist in fresh probe 41.')
print('PASS: linked-doctrine substitutions preserve remaining source spans and commands; retained standard digest and both body-only estimators checked; historical GO source and template checked.')
for v,m in json.loads((b/'rendered-metrics.json').read_text()).items():print(f"METRIC {v}: {m['new_characters']} chars, {m['new_utf8_bytes']} bytes, {m['new_TE']:.2f} TE; net removed {m['net_characters']} chars ({100*m['share_original_task_characters']:.2f}% of 84524).")
print('Scope: source-to-artifact checks only; semantic completeness beyond checked spans, product implementation, model uptake and orchestrator approval are not established by this script.')
