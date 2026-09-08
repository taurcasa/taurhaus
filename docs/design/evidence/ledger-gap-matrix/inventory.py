from pathlib import Path
import json,re,hashlib,collections
OUT=Path(__file__).resolve().parent
POP=OUT/'population'

def norm(s):
    return re.sub(r'\s+',' ',re.sub(r'[`*_]','',s)).strip().lower()

def pieces(s):
    # Syntactic candidate units only. Never describe this as semantic atomization.
    splits=[0]
    for m in re.finditer(r';\s+|(?<=[.!?])\s+(?=[A-Z0-9`*—])|,\s+(?=(?:accepted|closed|completed|rejected|deferred)\b)',s):
        splits.extend([m.start(),m.end()])
    splits.append(len(s))
    return [(s[a:b].strip(),a,b) for a,b in zip(splits[::2],splits[1::2]) if s[a:b].strip()]

units=[]; structural=[]
def add(pop,sec,s,lo,hi,field=None,task=None,malformed=False):
    for text,a,b in pieces(s):
        uid=f'{pop}-{len([u for u in units if u["population"]==pop])+1:04}'
        if pop=='W2': stratum='wave-2 live capture'
        elif pop=='R1': stratum='governance and rulings'
        elif task in ['T3','T4','T5','T8']: stratum='short measure results'
        elif task=='T25': stratum='T25 rubric chronology'
        elif task=='T26': stratum='T26 integration'
        elif task in ['T27','T28']: stratum='T27/T28 closure'
        elif task: stratum='hero implementation and review; other task rows'
        else: stratum='governance and rulings'
        units.append(dict(id=uid,population=pop,section=sec,line_start=lo,line_end=hi,field=field,task=task,text=text,stratum=stratum,malformed=malformed))

for pop,path in [('W1','docs/wave-1/ledger.md'),('R1','docs/wave-1/rulings.md'),('W2','docs/wave-2/ledger.md')]:
    lines=(POP/path).read_text().splitlines(); sec='preamble'; buf=[]; start=0; fence=False
    def flush(end):
        global buf
        if buf: add(pop,sec,' '.join(buf),start,end);buf=[]
    for n,line in enumerate(lines,1):
        if line.startswith('#'):
            flush(n-1);sec=line.lstrip('# ');structural.append((pop,n,'heading'));continue
        if not line.strip(): flush(n-1);structural.append((pop,n,'blank'));continue
        if line.lstrip().startswith('```'):
            flush(n-1);fence=not fence;structural.append((pop,n,'fence'));continue
        if re.match(r'^\| T\d',line) or (pop=='W2' and re.match(r'^\| #\d',line)):
            flush(n-1);cells=[c.strip() for c in line.strip('|').split('|')]; width=8 if pop=='W1' else 10; fields=['identity','kind','owner','acceptance','deliverable','state','commit','remaining'] if width==8 else ['identity','scope','kind','owner','acceptance','reviewer','budget','state','commit','remaining']; task=re.match(r'(T\d+a?|#\d+)',cells[0]).group(0)
            for i,cell in enumerate(cells): add(pop,sec,cell,n,n,fields[i] if len(cells)==width else f'fragment-{i+1}',task,len(cells)!=width)
            continue
        if line.startswith('|'):
            flush(n-1)
            if re.match(r'^\|[-: |]+$',line):structural.append((pop,n,'table separator'));continue
            add(pop,sec,line,n,n,'other table row');continue
        if re.match(r'^\s*(?:[-*]|\d+[.)])\s',line): flush(n-1)
        if not buf:start=n
        buf.append(line.strip())
    flush(len(lines))

projection=json.loads((OUT/'render.json').read_text()); tasks={t['id']:t for t in projection['tasks']}; external=[]
for p in sorted((OUT/'artifacts').rglob('*.md')):
    text=p.read_text(); lines=text.splitlines()
    # Paragraph and line windows; only literal normalized containment qualifies as a hit.
    for i in range(len(lines)):
        if not lines[i].strip():continue
        block=[]
        for j in range(i,min(i+12,len(lines))):
            if not lines[j].strip() and block:break
            block.append(lines[j])
        external.append((str(p.relative_to(OUT/'artifacts')),i+1,i+len(block),norm(' '.join(block))))
# Search a smaller unique block index with a token inverted index.
inv=collections.defaultdict(set)
for i,(_,_,_,s) in enumerate(external):
    for token in set(re.findall(r'\w{4,}',s)):inv[token].add(i)
seen={}
for u in units:
    s=norm(u['text']); key=(u['population'],s)
    u['exact_text_mirror_of']=seen.get(s) if len(s)>=35 else None
    if len(s)>=35:seen.setdefault(s,u['id'])
    u['source_class']='UNRESOLVED';u['proposed_class']=None;u['evidence_label']='U';u['source_evidence']=[];u['review_state']='candidate; semantic review required'
    tid=(u['task'] or '')[1:]; t=tasks.get(tid) if u['population']=='W1' else None
    if u['malformed']:
        u['reason']='Malformed-width row: column attribution is undecidable; retain exact fragment and require owner mapping.';continue
    if t and u['field']=='identity':
        u.update(source_class='retained-structured',evidence_label='S',source_evidence=[f'render.json tasks[id={tid}].id; {t["snapshot_source"]["file"]}:1'],reason='Explicit hand-row task id maps to snapshot; display suffix is not a new task id.')
        continue
    if t and u['field']=='owner' and u['text']==t['owner_at_snapshot']:
        u.update(source_class='retained-structured',evidence_label='S',source_evidence=[f'render.json tasks[id={tid}].owner_at_snapshot; tasks/{tid}.json:1'],reason='Exact sole-owner equality at captured snapshot.');continue
    if t and u['field']=='commit' and re.fullmatch('[0-9a-f]{7,40}',u['text']) and u['text'] in json.dumps(t):
        refs=[f'completion {c["event_id"]}' for c in t['completions'] if u['text'] in json.dumps(c)] + [f'ruling seq {r["seq"]} ref={r.get("ref")}' for r in t['rulings'] if u['text'] in json.dumps(r)]
        u.update(source_class='retained-structured',evidence_label='S',source_evidence=[f'render.json tasks[id={tid}]: '+', '.join(refs)],reason='Literal commit token retained; token occurrence alone does not type candidate versus landing.');continue
    # Statement-level literal matches, constrained to the matching task.
    if t and len(s)>=35:
        for kind in ['completions','rulings','assignments']:
            for rec in t[kind]:
                for f,v in rec.items():
                    if isinstance(v,str) and s in norm(v):
                        u['source_evidence'].append(f'render.json tasks[id={tid}].{kind} '+str(rec.get('event_id',rec.get('seq')))+f'.{f}')
        if u['source_evidence']:
            u.update(source_class='retained-structured',evidence_label='S',reason='Literal proposition text occurs in this task record; reported claim only, no runtime re-execution.');continue
    toks=set(re.findall(r'\w{4,}',s)); candidate=set()
    if len(s)>=35 and len(toks)>=4:
        postings=sorted((inv.get(x,set()) for x in toks),key=len)
        if postings:
            candidate=postings[0].copy()
            for ids in postings[1:]:candidate.intersection_update(ids)
        matches=[external[i] for i in sorted(candidate) if s in external[i][3]]
        if matches:
            best=sorted(matches,key=lambda x:(x[2]-x[1],len(x[3]),x[0]))[0]
            u.update(source_class='source-adapter/link gap',evidence_label='S',source_evidence=[f'{best[0]}:{best[1]}-{best[2]} @ '+json.loads((OUT/'population-manifest.json').read_text())['taurjob_revision']],reason='Literal text recoverable in a committed external artifact. This verifies an artifact declaration, not the experiment or authority behind it.');continue
    if u['population']=='W2':
        u['proposed_class']='retention gap' if u['field'] in ['identity','owner','state','acceptance','reviewer','budget'] else 'source-adapter/link gap'
        u['reason']='U pending wave-2 export or a proposition-specific committed-artifact check; absence of an exact phrase is not evidence of absence.'
    elif u['task'] and (not t or u['field'] in ['state','commit','acceptance','owner']):
        u['proposed_class']='retention gap';u['reason']='Later task or final state absent from early cut; check cited artifact separately before treating this as unavailable content.'
    elif u['field']=='remaining':
        u['proposed_class']='new authored declaration (remaining/outcome)';u['reason']='Completion cannot establish this remaining disposition; need explicit existing result text or authorized declaration.'
    elif u['field']=='kind' and t:
        u['proposed_class']='source-adapter/link gap';u['reason']='Display kind differs from source vocabulary; requires reviewed work_kind mapping, not a new ledger status.'
    elif u['field'] in ['deliverable','acceptance'] and t:
        u['proposed_class']='source-adapter/link gap';u['reason']='Contract prose is retained; exact scope/route equivalence needs semantic review.'
    elif re.search(r'\bbudget\b|\bceiling\b|\braise[ds]?\b',s):
        u['proposed_class']='existing-authority schema gap';u['reason']='Proposed budget carrier is existing task/ruling authority; history retention and old/new/counting fields need separate confirmation.'
    else:
        u['proposed_class']='new authored declaration (decision/note)';u['reason']='Governance, explanation or measurement clause; check artifact sufficiency and correction versus observation before any new entry.'

(OUT/'proposition-candidates.jsonl').write_text(''.join(json.dumps(u,ensure_ascii=False)+'\n' for u in units))
stats={'candidate_units':len(units),'note':'Syntactic units, NOT a verified exhaustive semantic proposition denominator. Compound clauses and mirrors need adjudication.','by_population':dict(collections.Counter(u['population'] for u in units)),'literal_source_classes':dict(collections.Counter(u['source_class'] for u in units)),'exact_text_mirror_occurrences':sum(bool(u['exact_text_mirror_of']) for u in units),'by_stratum':dict(collections.Counter(u['stratum'] for u in units)),'structural_lines':len(structural)}
(OUT/'inventory-stats.json').write_text(json.dumps(stats,indent=2)+'\n');print(json.dumps(stats,indent=2))
