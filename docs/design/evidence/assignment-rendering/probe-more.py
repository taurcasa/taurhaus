import pathlib,json,subprocess,shlex
b=pathlib.Path('/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0');root=pathlib.Path((b/'probe-root.txt').read_text());es=json.loads((b/'mesh-probes.json').read_text())
def run(args):
    argv=['/home/mstie/.local/bin/mesh','--claude-dir',str(root),'--team','render-probe','--name','lead',*args]
    p=subprocess.Popen(argv,cwd=root,env={},stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    try: out,err=p.communicate(timeout=15)
    finally:
        if p.poll() is None:p.kill();p.wait()
    e=dict(id=len(es)+1,argv=argv,env={},cwd=str(root),exit=p.returncode,stdout=out,stderr=err);es.append(e)
    (b/'mesh-probes.json').write_text(json.dumps(es,indent=2))
    with (b/'mesh-probes.log').open('a') as f:f.write(f"PROBE {e['id']}\n$ env -i {shlex.join(argv)}\nexit={p.returncode}\nstdout:\n{out}stderr:\n{err}\n")
    assert p.returncode==0,err
    return out
run(['task','get','1','--verbose'])
run(['task','create','--subject','GO delivery observation','--first-step','Read isolated artifact','--deliverable','result.md','--completion-signal','RESULT','--json'])
run(['task','assign','4','--owner','seat2','--awaiting-go'])
def counts():return {f.name:len(json.loads(f.read_text())) for f in (root/'teams/render-probe/inboxes').glob('*.json')}
before=counts();run(['task','update','4','--go']);after=counts();assert before==after
with (b/'probe-observations.txt').open('w') as f:
    f.write(f'GO inbox counts before={before}; after={after}; equal={before==after}\n')
    for inbox in ['seat','seat2']:
        for msg in json.loads((root/f'teams/render-probe/inboxes/{inbox}.json').read_text()):
            if 'Assignment:' in msg.get('text',''):f.write(f'INBOX {inbox} id={msg.get("id")}\n{msg["text"]}\n\n')
print('ALL PROBES',len(es),'GO before/after',before,after)
