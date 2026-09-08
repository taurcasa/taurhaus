import json, subprocess, tempfile, pathlib, shlex
BASE=pathlib.Path('/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0')
ROOT=pathlib.Path(tempfile.mkdtemp(prefix='mesh-probe-',dir=BASE))
BASE.joinpath('probe-root.txt').write_text(str(ROOT))
entries=[]
def run(args,name='lead',expected=0):
    argv=['/home/mstie/.local/bin/mesh','--claude-dir',str(ROOT),'--team','render-probe','--name',name,*args]
    p=subprocess.Popen(argv,cwd=ROOT,env={},stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    try:
        out,err=p.communicate(timeout=15)
    finally:
        if p.poll() is None:
            p.kill(); p.wait()
    e=dict(id=len(entries)+1,argv=argv,env={},cwd=str(ROOT),exit=p.returncode,stdout=out,stderr=err)
    entries.append(e)
    BASE.joinpath('mesh-probes.json').write_text(json.dumps(entries,indent=2))
    with BASE.joinpath('mesh-probes.log').open('a') as f:
        f.write(f"PROBE {e['id']}\n$ env -i {shlex.join(argv)}\nexit={p.returncode}\nstdout:\n{out}stderr:\n{err}\n")
    if p.returncode!=expected: raise RuntimeError(f"probe {e['id']} exit={p.returncode}: {err}")
    return out
run(['version','--json'])
run(['task','create','--help'])
run(['task','assign','--help'])
run(['task','get','--help'])
run(['join','--type','team-lead'])
run(['join'],name='seat')
run(['join'],name='seat2')
t=json.loads(run(['task','create','--subject','Measure assignment rendering','--description','Objective: preserve all five lines. Review route: measure; acceptance lead.','--first-step','Read scratch candidate.txt','--deliverable','scratch result.md with field coverage','--completion-signal','RESULT with findings or BLOCKED with reason','--effort','high','--why','reference identity requires careful checking','--deadline','30','--lane-id','rendering','--work-kind','verification','--criticality','supporting','--json']))
tid=t['id']
run(['task','assign',tid,'--owner','seat','--awaiting-go'])
t=json.loads(run(['task','get',tid,'--json']))
old=t['metadata']['assignment_id']
run(['task','get',tid])
run(['task','accept',tid,'--assignment',old],name='seat')
run(['task','update',tid,'--go'])
run(['task','get',tid,'--json'])
run(['task','start',tid,'--assignment',old,'--active-form','Checking the isolated assignment'],name='seat')
run(['task','assign',tid,'--owner','seat2'],expected=1)
run(['task','assign',tid,'--owner','seat2','--admin-reason','move isolated review to second seat','--first-step','Read scratch replacement.txt','--deliverable','scratch replacement-result.md','--completion-signal','RESULT replacement or BLOCKED','--effort','medium','--why','bounded replacement','--deadline','20','--awaiting-go'])
t=json.loads(run(['task','get',tid,'--json']))
new=t['metadata']['assignment_id']
assert new!=old
run(['task','accept',tid,'--assignment',old],name='seat2',expected=1)
run(['task','update',tid,'--go'])
run(['task','start',tid,'--assignment',new,'--active-form','Checking replacement'],name='seat2')
run(['task','complete',tid,'--summary','isolated fixture complete'],name='seat2')
run(['task','assign',tid,'--owner','seat','--reopen','--admin-reason','bounded resumed stage','--awaiting-go'])
run(['task','get',tid,'--json'])
run(['task','ruling',tid,'--kind','verdict','--value','accepted','--ref','fixture-candidate'],name='seat')
run(['task','ruling',tid,'--kind','verdict','--value','rejected','--ref','fixture-candidate'],name='seat')
run(['send','seat','ACTION REQUIRED: Task #1 release; first step: inspect fixture; completion signal: RESULT'],expected=0)
run(['send','seat','ACTION REQUIRED: Task #1 release; first step: inspect fixture; deliverable: result.md; completion signal: RESULT'])
run(['task','create','--subject','Optional override omission','--first-step','Read fixture','--deliverable','result.md','--completion-signal','RESULT','--json'])
run(['task','assign','2','--owner','seat2'])
run(['task','get','2','--json'])
run(['task','create','--subject','Effort without reason','--first-step','Read fixture','--deliverable','result.md','--completion-signal','RESULT','--effort','low','--json'])
run(['task','assign','3','--owner','seat2','--why','optional supplied reason'])
print('ALL PROBES COMPLETED',len(entries),'root',ROOT)
