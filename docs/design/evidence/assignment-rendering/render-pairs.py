import pathlib,json,re,hashlib
b=pathlib.Path('/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0')
pairs=json.loads((b/'nine-pairs.json').read_text())
events=json.loads((b/'archive-journals.json').read_text())['workflow_events']
line_starts=[1,23,45,65,85,107,127,149,169]
# Literal renderer shared by nine historical records and one synthetic fixture.
keys=['Objective','Deliverable','First action','Completion signal','Review route']
optional=['References','Deadline','Budget','Context','Change','Reconciliation','Record metadata']
def render(r):
    lines=[f"[#{r['task_id']} {r['subject']} · owner: {r['owner']} · team: {r['team']}]",f"Assignment: {r['assignment_id']} · issued by {r['assigned_by']} at {r['assigned_at']}"]
    if r.get('Effort'):lines.append('Effort: '+r['Effort'])
    lines.extend(['ACTION REQUIRED: '+r['action'],'Execution: '+r['execution']])
    lines.extend(k+': '+r['fields'][k] for k in keys)
    lines.extend(k+': '+r[k] for k in optional if r.get(k))
    lines.append('No pure acknowledgment. Follow the completion signal; report an execution blocker as specified.')
    return '\n'.join(lines)
cards=[]
for n,x in enumerate(pairs,1):
    prose=x['prose']['text'];card=x['card']['text']
    token=re.search(r'Assignment: (.+)',card).group(1)
    ev=next(e for e in events if e['eventType']=='task_assigned' and e['assignment_id']==token)
    fields=dict(re.findall(r'^[1-5]\. (Objective|Deliverable|First action|Completion signal|Review route): (.*)$',prose,re.M))
    assert len(fields)==5
    intro=prose.split('1. Objective:')[0].strip().replace('\n\n',' | ').replace('\n',' ')
    # Keep task-specific isolation verbatim, but move it out of the fifth-line schema.
    relocated=[]
    for marker in [' Commit only your file;', ' Commit only your files;', ' Do not read product-reviewer', ' Isolation:']:
        route=fields['Review route']; i=route.find(marker)
        if i>=0:
            relocated.append(route[i:].strip());fields['Review route']=route[:i]
    r=dict(pair=n,task_id=ev['task_id'],assignment_id=token,subject=re.search(r'^\[#\d+ (.*?) · owner:',card).group(1),owner=x['seat'],team='taurjob-team',assigned_by=ev['assigned_by'],assigned_at=ev['timestamp'],fields=fields)
    r['Context']=intro+(' | '+ ' '.join(relocated) if relocated else '')
    r['execution']=f"RECORDED status={ev['task_status']}; assignment-time awaiting_go value unavailable in this event; no lifecycle transition inferred."
    first=fields['First action']
    r['action']=('Pre-read only; execution waits for the authored T9 GO.' if n in (6,9) else {1:'Start permitted action: read BRIEF.md, then job-hunt/SPEC.md.',2:'Start permitted action: search taurhaus/src-tauri/src for process spawning and JSONL parsing.',3:'Start permitted action: run the tool-version checks in First action.',4:'Start permitted action: read docs/reference/toolchain.md and architecture sections 6–7.',5:'Start permitted action: read job-hunt/SPEC.md sections 2–9, then pipeline/jh/runners.py.',7:'Start permitted action: capture the Claude stream with the exact command in First action.',8:'Start permitted action: read pipeline-stages.md and architecture section 4, then probe public portals.'}[n])
    conflict=''
    if n in (1,2,5,7):
        conflict='CONFLICT: authored deliverable says main; recorded deliverable says master.'
    if n==3:
        conflict='CONFLICT: authored budget says ≤80 added lines; recorded deliverable says ≤80 lines. No counting rule selected by renderer.'
        r['Budget']='baseline not recorded; paths docs/reference/toolchain.md; ceiling authored ≤80 added lines / card ≤80 lines; counting rule unresolved; tool and exclusions not recorded; scratch scaffold never committed.'
        r['Context']+=' | Recorded first-action addition: check WebKitGTK/system dependencies reported by tauri info.'
    if n==4:
        r['Budget']='baseline not recorded; paths src/, src-tauri/, src/lib/ipc.ts, package.json; ceiling ≤400 hand-written lines; generated and lock files excluded; list generated files in RESULT; counting tool not recorded.'
        r['Context']+=' | Record explicitly specifies cargo clippy -D warnings.'
    if n in (6,8,9):
        r['Budget']='evidence deliverable ≤120 lines; baseline/counting tool not recorded; measure/review evidence contract, not an inferred implementation diff leash.'
    if n in (6,9):
        conflict='CONFLICT: recorded start-now heading contradicts recorded first_step and authored pre-read-only instruction; no assignment-time UUID wait marker is evidenced. Preserve the pre-read instruction; missing lifecycle declaration needs repair.'
        r['References']='candidate pending T2 delivery; rubric named in First action; immutable candidate/rubric revisions and packet digest not recorded at pre-GO.'
    if n==8:r['Context']+=' | Recorded first-action scope: each listed public portal type, including browser-only types in the inventory.'
    if n==9:
        e=next(e for e in events if e['eventType']=='assignment_superseded' and e['superseded_by_assignment_id']==token)
        r['Change']=f"supersedes {e['assignment_id']} because recorded workflow reason={e['reason']}; admin reason not recorded; changed owner heavy-implementer-1 → judge-astra-1 and authored first action/review scope."
    if conflict:
        issue='T2 candidate and recorded GO' if n in (6,9) else 'branch ruling' if n in (1,2,5,7) else 'budget counting ruling'
        r['Reconciliation']=conflict+f" BLOCKED route: assignee records task #{ev['task_id']} blocked through the existing task block lifecycle and sends BLOCKED T{ev['task_id']} <reason> to lead-taurjob, naming assignment {token}, artifact={issue}, owner=lead-taurjob, release=authoritative correction for this generation. This annotation is an obligation, not a recorded block or a new hold state."
    r['body']=render(r);cards.append(r)
(b/'nine-rendered-cards.json').write_text(json.dumps(cards,indent=2,ensure_ascii=False)+'\n')
# Pinned link illustration: exact doctrine substitutions only, never strip a command or lane constraint.
standard=pathlib.Path('/home/mstie/projects/taurhaus/docs/team-delivery-standard.md').read_bytes()
(b/'round1-standard.md').write_bytes(standard)
digest=hashlib.sha256(standard).hexdigest()
replacements={
 'Commit only your file;':'', 'Commit only your files;':'',
 'Numbered findings in severity order with evidence, impact, and action; open questions separate from defects; score table Finding / Severity / Confidence / Action;':'Findings with evidence, impact, and action; score table Finding / Severity / Confidence / Action;',
 'Numbered findings in severity order, each citing the mockup file or design.md line as evidence with impact and action; open questions separate from defects; score table Finding / Severity / Confidence / Action;':'Findings each citing the mockup file or design.md line as evidence with impact and action; score table Finding / Severity / Confidence / Action;'
}
linked=[];changes=[]
for r in cards:
    c=json.loads(json.dumps(r)); del c['body']
    for k in keys:
        for old,new in replacements.items():
            if old in c['fields'][k]:
                c['fields'][k]=c['fields'][k].replace(old,new).strip()
                changes.append(dict(pair=r['pair'],slot=k,removed=old,authority='round1-standard.md:37' if old.startswith('Commit') else 'round1-standard.md:109',retained=new))
    for old,new in replacements.items():
        if old in c['Context']:
            c['Context']=c['Context'].replace(old,new).strip()
            changes.append(dict(pair=r['pair'],slot='Context',removed=old,authority='round1-standard.md:37',retained=''))
    c['References']=(c.get('References','')+'; ' if c.get('References') else '')+f'standard ./round1-standard.md sha256:{digest} (checkout §37 and results §109); required task-specific rules above remain binding.'
    c['body']=render(c);linked.append(c)
(b/'nine-linked-cards.json').write_text(json.dumps(linked,indent=2,ensure_ascii=False)+'\n')
(b/'linked-doctrine-map.json').write_text(json.dumps(changes,indent=2,ensure_ascii=False)+'\n')
(b/'linked-doctrine-cards.md').write_text('# Linked-doctrine illustration [I — UNVERIFIED design]\n\nSource: nine-pairs-source.txt:1; exact replacement ledger: linked-doctrine-map.json:1; pinned standard: round1-standard.md:1. These are offline bodies, not sent messages.\n\n'+'\n\n'.join(f"## Pair {r['pair']}\n\n```text\n{r['body']}\n```" for r in linked)+'\n')
old=''.join(x[k]['text'] for x in pairs for k in ['prose','card'])
def metrics(cs):
    new=''.join(c['body'] for c in cs)
    return dict(pairs=9,old_characters=len(old),new_characters=len(new),old_utf8_bytes=len(old.encode()),new_utf8_bytes=len(new.encode()),old_TE=len(old)/4,new_TE=len(new)/4,net_characters=len(old)-len(new),net_utf8_bytes=len(old.encode())-len(new.encode()),net_TE=(len(old)-len(new))/4,share_original_task_characters=(len(old)-len(new))/84524)
m={'verbatim':metrics(cards),'linked':metrics(linked)}
(b/'rendered-metrics.json').write_text(json.dumps(m,indent=2)+'\n')
r=dict(task_id='synthetic-1',subject='Illustrative frozen review',owner='fixture-reviewer',team='fixture-only',assignment_id='22222222-2222-4222-8222-222222222222',assigned_by='fixture-lead',assigned_at='2026-09-08T12:00:00Z',Effort='high — synthetic isolation-sensitive review',action='Pre-read only; execution waits for fixture-lead release.',execution='WAIT awaiting_go=22222222-2222-4222-8222-222222222222 matches assignment; owner fixture-lead; artifact fixture-candidate; since 2026-09-08T12:00:00Z, age 0 at issuance; release fixture manifest verified.',fields=dict(zip(keys,['Review the synthetic fixture against its frozen rubric.','fixture-result.md with findings and score table.','Read fixture-rubric.md now; inspect fixture-candidate only after GO.','RESULT with evidence or BLOCKED with task/generation/artifact/owner/release condition.','review; acceptance fixture-lead; reviewer fixture-reviewer; handoff fixture-result.md.'])),References='accepted base fixture-base object '+ 'a'*40+'; candidate pending; rubric fixture-rubric.md sha256:'+'b'*64+'; packet fixture-packet sha256:'+'c'*64,Deadline='30 minutes',Budget='baseline fixture-base; paths fixture-result.md; counting final file lines; tool fixture-count.py; exclusions none; ceiling 120.',Context='checkout fixture-only; manifest stopping=verdict committed; allowed evidence=own candidate/rubric only; no peer opinions.',Change='supersedes 11111111-1111-4111-8111-111111111111 because synthetic admin reason=replace reviewer; changed owner, candidate availability and wait.')
r['body']=render(r)
(b/'synthetic-rendered-card.json').write_text(json.dumps(r,indent=2)+'\n')
intro='''### Nine combined cards, reconstructed from both sources

**[I — UNVERIFIED design]** Literal-template retrospective renderings below are not delivered messages or authority to execute archived commands. Five-line values and introductory constraints are copied from the authored sources; fifth-line checkout/isolation tails move verbatim into Context (standard:25, :90). No command is redacted or paraphrased. Reconciliation is an explicit optional template slot. State reports the assignment event, not the final snapshot; an unavailable marker is not an inferred hold or release. A conflicting field creates the visible BLOCKED route, not a lifecycle mutation by the renderer. The historical pre-read instruction remains binding even where the archive lacks its matching machine wait. Source-to-render assertions and their limits are in `verify-report.py:1`.

**[S]** The nine pairs supply no effort/reason/deadline overrides. Real line ceilings appear in Budget; missing counting tools and frozen references remain marked unavailable. Their later T9 GO is rendered separately below (`nine-pairs-source.txt:107`, `:169`; `t9-go-evidence.txt:1`).

'''
for n,(x,r) in enumerate(zip(pairs,cards),1):
    intro+=f"#### Pair {n}: task #{r['task_id']}, {x['seat']}, authored {x['prose_index']} / card {x['card_index']}\n\n**[S] Source:** `nine-pairs-source.txt:{line_starts[n-1]}`; prose {len(x['prose']['text']):,} characters, card {len(x['card']['text']):,}. **[I — UNVERIFIED design]** Reconciliation routes are proposed obligations; execution fields report the source event.\n\n```text\n{r['body']}\n```\n\n"
intro+='### Synthetic card: optional sections exercised\n\n**[I — UNVERIFIED design]** Synthetic identifiers, references, clock and wait are fixture input, not real commits, tests, or approvals (`synthetic-rendered-card.json:1`). This uses the same renderer as all nine historical cards.\n\n```text\n'+json.loads((b/'synthetic-rendered-card.json').read_text())['body']+'\n```\n\n'
p=b/'assignment-rendering-design.md';s=p.read_text();s=s[:s.index('### Nine combined cards, reconstructed from both sources')]+intro+s[s.index('### Compatibility and inspection surfaces'):];p.write_text(s)
print(json.dumps(m,indent=2))
