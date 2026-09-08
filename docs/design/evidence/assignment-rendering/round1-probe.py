import pathlib, runpy, json, re
b=pathlib.Path('/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0')
(b/'mesh-probes.log').write_text('')
runpy.run_path(str(b/'probe-mesh.py'))
n=runpy.run_path(str(b/'probe-more.py'))
run=n['run']
run(['task','block','--help'])
run(['task','update','--help'])
run(['task','create','--subject','Scaffold metadata preservation','--description','Inspect synthetic scaffold metadata','--first-step','Read fixture','--deliverable','fixture-note.md','--completion-signal','RESULT or BLOCKED','--work-kind','scaffolding','--lane-id','rendering','--criticality','scaffolding_only','--parent','1','--anchor','2','--scaffold-class','fixture_note','--sunset-decision','archive','--sunset-owner','lead','--sunset-trigger','fixture accepted','--json'])
run(['task','get','5','--json'])
p=json.loads((b/'mesh-probes.json').read_text())
checks=[]
for assign_id,get_id in [(9,10),(17,18),(23,24)]:
    a=p[assign_id-1]; g=json.loads(p[get_id-1]['stdout'])
    msg=re.search(r'\(id: ([^)]+)\)',a['stdout']).group(1)
    token=g['metadata']['assignment_id']
    inbox=json.loads((pathlib.Path(a['cwd'])/'teams/render-probe/inboxes'/f"{g['owner']}.json").read_text())
    assert msg!=token
    checks.append(f'assign probe {assign_id}: message_id={msg}; get probe {get_id}: assignment_id={token}; distinct=True; message-id classification: main.rs:3561-3565')
(b/'round1-token-check.log').write_text('\n'.join(checks)+'\n')
print('\n'.join(checks))
print('ROUND-1 PROBES COMPLETE:',len(p),'isolated invocations; child env={} and explicit --claude-dir for each')
