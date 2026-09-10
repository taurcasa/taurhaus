"""Real L4 continuation. Run once; write {"step":N} to run2/action.json per checkpoint.

Uses production initialize/stop/resume RPCs. No observer app-server socket, fault
injection or automatic paid retry. The only operator auth read is the authorized
copy of CODEX_HOME/auth.json. Historical preflight-only controller: commit 0267819e.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shlex
import shutil
import signal
import socket
import subprocess
import tempfile
import time
import traceback
from preflight import auth_source, meter, require_headroom

BASE=Path(__file__).resolve().parent
CHECKOUT=Path('/home/mstie/projects/taurhaus-l4-resume-team')
MESH=Path('/home/mstie/projects/mesh-l4')
TEAM='l4-resume'
OUT=BASE/'run2'


def clean(value):
    if isinstance(value,dict):
        if value.get('method','').startswith('account/') or value.get('event','').startswith('usage.'): return None
        return {k:clean(v) for k,v in value.items() if 'installation' not in k.lower() and k.lower() not in {'auth','accountid','account_id','controlauthtokenhash','accesstoken','refreshtoken','idtoken','access_token','refresh_token','id_token','rate_limits','ratelimits'}}
    if isinstance(value,list): return [v for x in value if (v:=clean(x)) is not None]
    if isinstance(value,str):
        return re.sub(r'/home/[^/\s]+/(?!projects/(?:taurhaus-l4-resume-team|mesh-l4)(?:/|\b))[^\s\"\']*','<operator-path-redacted>',value)
    return value


def rows(path):
    try: text=path.read_text()
    except FileNotFoundError: return []
    result=[]
    for line in text.splitlines(keepends=True):
        if not line.endswith('\n'): continue
        try: result.append(json.loads(line))
        except ValueError: continue
    return result


def save(name,value):
    target=OUT/name
    target.parent.mkdir(parents=True,exist_ok=True)
    text=json.dumps(clean(value),indent=2)+'\n'
    temp=target.with_suffix(target.suffix+'.tmp'); temp.write_text(text); temp.replace(target)


class Lane:
    def __init__(self):
        self.root=Path(tempfile.mkdtemp(prefix='th-l4-run2-'))
        for directory in ['home/.local/bin','codex','claude','gemini','grok','data','tmp','tmux','project']:
            (self.root/directory).mkdir(parents=True,exist_ok=True)
        self.bin=self.root/'home/.local/bin'
        self.team=self.root/'claude/teams'/TEAM
        self.env={'PATH':f'{self.bin}:/usr/bin:/bin','HOME':str(self.root/'home'),
                  'CODEX_HOME':str(self.root/'codex'),'CLAUDE_CONFIG_DIR':str(self.root/'claude'),
                  'CLAUDE_DIR':str(self.root/'claude'),'TAURHAUS_CLAUDE_DIR':str(self.root/'claude'),
                  'TAURHAUS_DATA_DIR':str(self.root/'data'),'TMPDIR':str(self.root/'tmp'),
                  'TMUX_TMPDIR':str(self.root/'tmux'),'GROK_HOME':str(self.root/'grok'),
                  'GEMINI_CLI_HOME':str(self.root/'gemini'),'TAURHAUS_AGY_DIR':str(self.root/'gemini'),
                  'LANG':'C.UTF-8','TERM':'xterm-256color','SHELL':'/bin/bash',
                  'RUST_LOG':'info','TAURHAUS_TRIAL_ID':self.root.name}
        self.children=[]; self.host_events={}; self.host_ready=False; self.step=1
        self.views={}; self.completed=set(); self.started=set(); self.port=None
        self.start_time=time.monotonic(); self.seq=0; self.old={}; self.markers={}; self.message_ids={}
        self.ledger=meter([],[]); self.rollouts={}; self.previous={}; self.event_count=0

    def event(self,kind,**values):
        row=clean({'at':time.time(),'step':self.step,'kind':kind,**values})
        with (OUT/'events.jsonl').open('a') as f: f.write(json.dumps(row)+'\n')
        if kind in {'checkpoint','failure','ready','cleanup','operation_finished'}:
            print(json.dumps(row),flush=True)

    def command(self,argv,label=None,check=True,timeout=30):
        self.seq+=1; label=label or f'command-{self.seq}'
        p=subprocess.Popen(argv,cwd=self.root/'project',env=self.env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=True)
        self.children.append(p)
        try: output=p.communicate(timeout=timeout)[0].decode(errors='replace')
        except subprocess.TimeoutExpired:
            os.killpg(p.pid,signal.SIGTERM); p.wait(timeout=10); raise
        save(label+'.json',{'argv':argv,'exit':p.returncode,'output':output})
        self.event('command',argv=argv,exit=p.returncode,evidence=label+'.json')
        if check: assert p.returncode==0, f'harness command {label} exit {p.returncode}: {output[:1000]}'
        return output

    def rpc(self,method,params,record=True):
        request={'id':secrets.token_hex(8),'method':method,'params':params}
        if record: self.event('rpc_request',request=request)
        with socket.create_connection(('127.0.0.1',self.port),timeout=20) as conn:
            wire={**request,'auth':(self.root/'data/daemon.token').read_text().strip()}
            conn.sendall(json.dumps(wire).encode()+b'\n')
            stream=conn.makefile('rb')
            while True:
                line=stream.readline()
                if not line: raise RuntimeError('daemon RPC closed: '+method)
                response=json.loads(line)
                if response.get('id')==request['id']: break
        if record: self.event('rpc_response',response=response)
        if 'error' in response: raise RuntimeError(method+': '+str(response['error']))
        return response['result']

    def operation(self,method,params):
        result=self.rpc(method,params)
        endpoint='coordination.initialize_status' if method=='coordination.initialize_team' else method+'_status'
        deadline=time.monotonic()+150
        samples=[]
        while time.monotonic()<deadline:
            status=self.rpc(endpoint,{'run_id':result['run_id']},record=False)
            if not samples or status!=samples[-1]:
                samples.append(status); save(f'step{self.step}-operation.json',{'method':method,'accepted':result,'status_samples':samples})
            if status['outcome']['status']!='running':
                self.event('operation_finished',method=method,status=status)
                assert status['outcome']['status']=='completed', f'{method} refused: {status}'
                report=status['outcome']['report']
                assert not report.get('failed_step'), f'{method} failed: {report}'
                return status
            self.observe(); time.sleep(1)
        raise AssertionError(method+' status deadline exceeded')

    def record(self,member):
        try: return json.loads((self.team/'runtime'/f'{member}.json').read_text())
        except FileNotFoundError: return {}

    def identities(self):
        result=[]; marker=b'TAURHAUS_TRIAL_ID='+self.root.name.encode()+b'\0'
        for proc in Path('/proc').iterdir():
            if not proc.name.isdigit(): continue
            try:
                if marker not in (proc/'environ').read_bytes(): continue
                result.append({'pid':int(proc.name),'start_ticks':(proc/'stat').read_text().rsplit(')',1)[1].split()[19],
                               'namespace_pids':next((l.split(':',1)[1].split() for l in (proc/'status').read_text().splitlines() if l.startswith('NSpid:')),[]),
                               'argv':(proc/'cmdline').read_bytes().decode(errors='replace').split('\0')})
            except (FileNotFoundError,ProcessLookupError,PermissionError): pass
        return result

    def observe(self):
        usage=[]; self.started=set(); self.completed=set(); self.rollouts={}
        for path in self.root.rglob('rollout-*.jsonl'):
            current=None; session=None; retained=[]
            for row in rows(path):
                p=row.get('payload',{}); typ=p.get('type')
                if row.get('type')=='session_meta': session=p.get('id')
                if row.get('type')=='event_msg':
                    if typ=='task_started': current=p.get('turn_id'); self.started.add(current)
                    if typ=='task_complete': self.completed.add(p.get('turn_id',current))
                    if typ=='token_count' and p.get('info') and current:
                        u=p['info'].get('last_token_usage',{})
                        if u:
                            usage.append({'turn_id':current,'session_id':session,'input':u.get('input_tokens',0),'cached_input':u.get('cached_input_tokens',0),'output':u.get('output_tokens',0),'observer':'rollout'})
                    if typ in {'task_started','task_complete','token_count'}:
                        retained.append({k:v for k,v in row.items() if k!='payload'}|{'payload':{k:v for k,v in p.items() if k not in {'rate_limits','rateLimits'}}})
                if row.get('type')=='response_item' and p.get('type')=='message': retained.append(row)
            if session: self.rollouts[session]=retained
        if self.host_ready:
            view=self.rpc('coordination.hosted_transcript',{'team_name':TEAM,'member_name':'beta'},record=False)
            self.views['beta']=view
            for e in view.get('events',[]):
                if e.get('method','').endswith('/delta') or e.get('method','').startswith('account/'): continue
                key=hashlib.sha256(json.dumps(e,sort_keys=True).encode()).hexdigest()
                if len(self.host_events)<2000: self.host_events[key]=e
            save('hosted-view.json',{k:v for k,v in view.items() if k!='events'})
        for e in self.host_events.values():
            p=e.get('params',{}); method=e.get('method')
            if method=='turn/started': self.started.add(p['turn']['id'])
            if method=='turn/completed': self.completed.add(p['turn']['id'])
            if method=='thread/tokenUsage/updated':
                u=p['tokenUsage']['last']
                usage.append({'turn_id':p['turnId'],'thread_id':p['threadId'],'input':u['inputTokens'],'cached_input':u['cachedInputTokens'],'output':u['outputTokens'],'observer':'host'})
        self.started.discard(None); self.completed.discard(None)
        self.ledger=meter(self.started,usage)
        self.ledger.update({'model':'gpt-5.6-luna','effort':'low','max_inputs':16,'max_usd':.25,'completed_turn_ids':sorted(self.completed),'attachment_generations':{m:self.record(m).get('attachmentGeneration') for m in ['alpha','beta']}})
        save('cost-ledger.json',self.ledger)
        save('rollouts.json',self.rollouts)
        save('host-events.json',list(self.host_events.values()))
        assert self.ledger['paid_inputs']<=16, 'hard input cap exceeded'
        assert self.ledger['conservative_usd']<=.25, 'hard cost cap exceeded'
        assert time.monotonic()-self.start_time<=900, '15-minute runtime cap'

    def wait(self,test,reason,timeout=100):
        assert timeout>=60
        deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            self.observe()
            if test(): return
            time.sleep(1)
        raise AssertionError(reason)

    def settled(self):
        return bool(self.started) and not self.ledger['unmetered'] and self.started<=self.completed

    def journal_rows(self):
        return [r for p in sorted((self.team/'state/messaging-v2/segments').glob('*.jsonl')) for r in rows(p)]

    def snapshot(self,label):
        files={}
        for pattern in ['config.json','runtime/*.json','state/delivery/*.json','state/activity/*.json','state/messaging-v2/manifest.json','state/workflow_events.jsonl']:
            for path in self.team.glob(pattern):
                try: files[str(path.relative_to(self.team))]=json.loads(path.read_text()) if path.suffix=='.json' else rows(path)
                except (FileNotFoundError,ValueError): continue
        save(label+'-state.json',{'files':files,'journal':self.journal_rows(),'processes':self.identities(),'tasks':[p.name for p in (self.root/'claude/tasks'/TEAM).glob('*')]})
        # Passive kernel lock samples only; never acquire a product lock.
        locks=[]
        for path in self.team.rglob('*.lock'):
            try:
                inode=path.stat().st_ino
                held=[line for line in Path('/proc/locks').read_text().splitlines() if f':{inode} ' in line]
                locks.append({'path':str(path.relative_to(self.team)),'inode':inode,'holders':held})
            except FileNotFoundError: continue
        save(label+'-locks.json',locks)
        output=self.command(['tmux','list-panes','-a','-F','#{pane_id}'],label+'-panes',check=False)
        for pane in output.splitlines():
            if not re.fullmatch(r'%\d+',pane): continue
            p=subprocess.run(['tmux','capture-pane','-p','-S','-12','-t',pane],env=self.env,capture_output=True,text=True)
            save(label+'-pane'+pane[1:]+'.json',{'argv':['tmux','capture-pane','-p','-S','-12','-t',pane],'exit':p.returncode,'lines':p.stdout.splitlines()[-60:]})
        logs=[]
        for path in (self.root/'data').glob('*.jsonl'):
            for row in rows(path):
                if row.get('event','').startswith(('hosted.','onboarding.','coordination.')): logs.append(row)
        # First/last by event retains diagnostic transitions without periodic growth.
        first={}; last={}
        for row in logs:
            key=(row.get('event'),row.get('message'))
            first.setdefault(key,row); last[key]=row
        save('daemon-events.json',list(first.values())+[v for k,v in last.items() if v!=first[k]])

    def mesh(self,argv,label,member='lead'):
        argv=[str(self.bin/'mesh'),*argv,'--claude-dir',str(self.root/'claude'),'--team',TEAM,'--name',member]
        seq=secrets.token_hex(4); result=self.root/(seq+'.out'); status=self.root/(seq+'.status')
        text=shlex.join(argv)+' >'+shlex.quote(str(result))+' 2>&1; echo $? >'+shlex.quote(str(status))
        self.command(['tmux','new-window','-d','-t','taurhaus','/bin/bash -c '+shlex.quote(text)],label+'-invoke')
        deadline=time.monotonic()+60
        while not status.exists() and time.monotonic()<deadline: time.sleep(.1)
        assert status.exists(), 'mesh command deadline: '+label
        code=int(status.read_text()); output=result.read_text()
        save(label+'.json',{'argv':argv,'exit':code,'output':output})
        assert code==0, 'Mesh refusal: '+output
        try: return json.loads(output)
        except ValueError: return output

    def read_pages(self,member,label,mark=False):
        cursor=None; pages=[]
        for index in range(64):
            argv=['read','--json','--last','16']
            if mark: argv+=['--mark-read']
            if cursor: argv+=['--since',cursor]
            page=self.mesh(argv,f'{label}-{member}-{index}',member)
            assert isinstance(page,dict), 'canonical read did not return object'
            pages.append(page)
            if page.get('done'): return pages
            cursor=page.get('cursor'); assert cursor, 'missing canonical read cursor'
        raise AssertionError('read paging deadline')

    def send(self,member,phase):
        require_headroom(self.ledger,1)
        marker=f'L4_{phase}_{member}_'+secrets.token_hex(3)
        self.markers[phase,member]=marker
        result=self.mesh(['send',member,'ACTION REQUIRED: Reply exactly '+marker+'. No tools are needed for this marker.','--summary',f'L4 {phase} marker'],f'{phase}-{member}-send')
        if isinstance(result,str): result=next(json.loads(l) for l in result.splitlines() if l.startswith('{'))
        self.message_ids[phase,member]=result['message_id']
        save('markers.json',{'markers':{f'{p}/{m}':v for (p,m),v in self.markers.items()},'message_ids':{f'{p}/{m}':v for (p,m),v in self.message_ids.items()}})

    def replies(self,member,marker):
        record=self.record(member); session=record.get('session_id') or self.old.get(member,{}).get('session_id')
        replies=[]
        for row in self.rollouts.get(session,[]):
            p=row.get('payload',{})
            if row.get('type')=='response_item' and p.get('role')=='assistant' and marker in json.dumps(p): replies.append(p)
        return replies

    def receipts(self,phase,member):
        mid=self.message_ids[phase,member]
        return [r for r in self.journal_rows() if r.get('payload',{}).get('message_id')==mid]

    def step1(self):
        require_headroom(self.ledger,2)
        result=self.operation('coordination.initialize_team',{'request':self.request,'cli_commands':self.commands,'tmux_layout':'new_window'})
        save('initialize-result.json',result)
        self.host_ready=bool(self.record('beta').get('appServer'))
        assert self.host_ready, 'hosted attachment missing after initialize'
        self.wait(lambda:self.settled(),'startup usage/completion did not settle')
        self.old={m:self.record(m) for m in ['lead','alpha','beta']}
        save('original-identities.json',self.old)
        self.original_config=json.loads((self.team/'config.json').read_text())
        for member in ['alpha','beta']:
            assert self.old[member].get('session_id'), member+' missing session identity'
            self.send(member,'old')
            marker=self.markers['old',member]
            self.wait(lambda:bool(self.replies(member,marker)) and self.settled(),member+' initial marker missing')
            self.wait(lambda:any(r.get('payload',{}).get('stage') in {'submitted','native_enqueued'} for r in self.receipts('old',member)),member+' completed transport missing')
            self.read_pages(member,'step1-read',mark=True)
            self.wait(lambda:'consumed_by_read' in json.dumps(self.receipts('old',member)),member+' read receipt missing')
        self.old_counts={m:len(self.replies(m,self.markers['old',m])) for m in ['alpha','beta']}
        assert all(v==1 for v in self.old_counts.values()), 'duplicate initial reply'

    def step2(self):
        self.before_stop=self.identities(); save('before-stop-processes.json',self.before_stop)
        self.host_ready=False
        for member in ['alpha','beta','lead']:
            record=self.old[member]
            self.rpc('stop_session',{'tmux_pane':record['paneId'],'cli_tool':'claude' if member=='lead' else 'codex'})
        def stopped():
            panes=subprocess.run(['tmux','list-panes','-a','-F','#{pane_id}'],env=self.env,capture_output=True,text=True).stdout.splitlines()
            original_panes=[r['paneId'] for r in self.old.values()]
            # Include the daemon-owned hosted child, not merely its attached TUI.
            alive=self.identities()
            codex=[p for p in alive if Path(p['argv'][0]).name in {'codex','claude'}]
            save('step2-stop-poll.json',{'panes':panes,'codex_processes':codex,'retained_runtime':{m:self.record(m) for m in self.old}})
            return not set(panes)&set(original_panes) and not codex
        self.wait(stopped,'supported stop_session left a recorded pane or Codex/app-server process alive',timeout=100)
        assert (self.team/'config.json').exists() and self.journal_rows(), 'stopped team state lost'

    def step3(self):
        # Sends accepted while stopped reserve a future input, one per seat.
        for member in ['alpha','beta']:
            self.send(member,'pending')
            self.wait(lambda:any(r.get('payload',{}).get('stage')=='pending' for r in self.receipts('pending',member)),member+' pending receipt missing while stopped')
            assert not self.replies(member,self.markers['pending',member]), 'presentation while stopped'
            assert not any(r.get('payload',{}).get('stage') in {'submitted','native_enqueued'} for r in self.receipts('pending',member)), 'transport while stopped'

    def step4(self):
        # Two recovery inputs plus the two already accepted pending obligations.
        require_headroom(self.ledger,4)
        self.operation('coordination.resume_team',{'request':{'team_name':TEAM},'cli_commands':self.commands,'tmux_layout':'new_window'})
        self.host_ready=bool(self.record('beta').get('appServer'))

    def step5(self):
        self.wait(lambda:self.settled() and all(self.replies(m,self.markers['pending',m]) for m in ['alpha','beta']),'resumed pending markers missing',timeout=120)
        new_config=json.loads((self.team/'config.json').read_text())
        for key in ['teamId','team_id','incarnation','teamIncarnation','messaging']:
            if key in self.original_config: assert new_config.get(key)==self.original_config[key], 'team identity/policy changed: '+key
        for member in ['alpha','beta']:
            old=self.old[member]; new=self.record(member)
            assert new['attachmentGeneration']>old['attachmentGeneration'], member+' attachment not advanced'
            assert new['session_id']==old['session_id'],member+' session/thread recovery identity changed'
            if member=='beta': assert new['appServer']['threadId']==old['appServer']['threadId']
            else: assert not new.get('appServer')
            assert len(self.replies(member,self.markers['pending',member]))==1, member+' pending marker replayed'
            recovery=new.get('recovery',{})
            save('step5-'+member+'-recovery.json',{'before':old.get('recovery'),'after':recovery})
            # Native context must carry the recovery card; record only new-turn messages.
            text=json.dumps(self.rollouts.get(new['session_id'],[]))
            assert '[taurhaus] recovery_card' in text, member+' recovery card absent from native transcript'

    def step6(self):
        for member in ['alpha','beta']:
            self.read_pages(member,'step6-read',mark=True)
            self.wait(lambda:'consumed_by_read' in json.dumps(self.receipts('pending',member)),member+' pending read receipt missing')
            assert len(self.replies(member,self.markers['old',member]))==self.old_counts[member], 'completed old message replayed'
        self.mesh(['who'],'step6-who')
        self.mesh(['team-daemon','status'],'step6-team-daemon')
        processes=self.identities()
        assert not any('daemon' in p['argv'][1:] and 'team-daemon' not in p['argv'] for p in processes if Path(p['argv'][0]).name=='mesh'), 'member delivery executor on team-owned team'
        for member in ['lead','alpha','beta']:
            cursor=None
            for n in range(64):
                args=['journal','read']+(['--since',cursor] if cursor else [])
                page=self.mesh(args,f'step6-journal-{member}-{n}',member)
                if page.get('done'): break
                cursor=page['cursor']
            else: raise AssertionError('journal paging exceeded bound')

    def setup(self):
        source=auth_source(os.environ,Path.home())
        assert source.is_file() and not source.is_symlink(), 'authorized auth source is not a regular file'
        shutil.copyfile(source,self.root/'codex/auth.json'); (self.root/'codex/auth.json').chmod(0o600)
        native=os.environ.get('CODEX_TRIAL_BINARY')
        if not native:
            package=Path(shutil.which('codex')).resolve().parents[1]
            native=next(package.glob('node_modules/@openai/codex-linux-x64/vendor/*/bin/codex'))
        for name,source in [('codex',native),('claude',Path(shutil.which('claude')).resolve()),('mesh',MESH/'target/debug/mesh'),('taurhaus-daemon',CHECKOUT/'src-tauri/target/release/taurhaus-daemon')]:
            shutil.copyfile(source,self.bin/name); (self.bin/name).chmod(0o700)
            with (self.bin/name).open('rb') as f: digest=hashlib.file_digest(f,'sha256').hexdigest()
            self.event('binary',name=name,sha256=digest)
        for tool in ['agy','grok','gemini']:
            (self.bin/tool).write_text('#!/bin/sh\nexit 77\n'); (self.bin/tool).chmod(0o700)
        for rc in ['.bashrc','.profile','.zshrc']: (self.root/'home'/rc).write_text('export PATH="$HOME/.local/bin:/usr/bin:/bin"\n')
        instructions='This is an isolated messaging trial. Reply briefly (at most 40 words). When a message assigns you a task, run exactly the mesh lifecycle commands it names. Never touch files. Do not send unsolicited messages.\n'
        (self.root/'project/AGENTS.md').write_text(instructions)
        self.command(['git','init','-q'],'scratch-git-init')
        self.command(['git','add','AGENTS.md'],'scratch-git-add')
        self.command(['git','-c','user.name=L4','-c','user.email=l4@example.invalid','commit','-qm','L4 instructions'],'scratch-git-commit')
        (self.root/'codex/config.toml').write_text('model="gpt-5.6-luna"\nmodel_reasoning_effort="low"\napproval_policy="never"\nsandbox_mode="danger-full-access"\nweb_search="disabled"\nmodel_context_window=32768\n[projects.'+json.dumps(str(self.root/'project'))+']\ntrust_level="trusted"\n')
        with socket.socket() as probe: probe.bind(('127.0.0.1',0)); self.port=probe.getsockname()[1]
        assert self.port!=17233
        self.env['TAURHAUS_DAEMON_PORT']=str(self.port)
        self.event('isolation',environment=self.env,initial_codex_entries=['auth.json'],auth_copy_mode='0600')
        bw=['bwrap','--die-with-parent','--unshare-pid','--ro-bind','/','/','--tmpfs','/home','--tmpfs','/tmp','--tmpfs','/run','--proc','/proc','--dev','/dev','--bind',str(self.root),str(self.root),'--chdir',str(self.root/'project')]
        version=self.command(bw+[str(self.bin/'codex'),'--version'],'codex-version')
        assert '0.153.4' in version, 'unapproved Codex build'
        argv=[str(self.bin/'taurhaus-daemon'),'--port',str(self.port),'--data-dir',str(self.root/'data')]
        boot='#!/bin/bash\nset -eu\ntmux -D -f /dev/null &\nfor i in {1..600}; do test -S "$TMUX_TMPDIR/tmux-$(id -u)/default" && break; sleep .1; done\ntmux set-option -g default-shell /bin/bash\ntmux new-session -d -s taurhaus -x 140 -y 48 /bin/bash\n'+shlex.join(argv)+' &\nwait $!\n'
        (self.root/'boot.sh').write_text(boot); save('bootstrap.json',{'argv':bw+['/bin/bash',str(self.root/'boot.sh')],'script':boot})
        with (self.root/'daemon.log').open('w') as log:
            p=subprocess.Popen(bw+['/bin/bash',str(self.root/'boot.sh')],env=self.env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        self.children.append(p)
        deadline=time.monotonic()+90
        while not (self.root/'data/daemon.token').exists() and time.monotonic()<deadline:
            assert p.poll() is None, 'private daemon launch exited'
            time.sleep(.2)
        deadline=time.monotonic()+90
        while True:
            try:
                self.rpc('ping',{}); break
            except (ConnectionRefusedError,FileNotFoundError):
                if time.monotonic()>=deadline: raise
                time.sleep(.2)
        source=(CHECKOUT/'src/lib/components/meshTabUtils.js').read_text()
        policy=json.loads(re.search(r'DEFAULT_CANONICAL_POLICY = Object.freeze\((\{.*?\})\)',source,re.S)[1])
        self.commands={'codex':{'fresh':'codex --sandbox danger-full-access --ask-for-approval never','continue_cmd':'codex --sandbox danger-full-access --ask-for-approval never','resume':'codex --sandbox danger-full-access --ask-for-approval never resume'}}
        self.request={'team_name':TEAM,'team_description':'L4 bounded whole-team stop/resume','lead_mode':'launch_new',
                      'lead':{'name':'lead','cli_tool':'claude','model':'claude-haiku-4-5','project_id':str(self.root/'project')},
                      'agents':[{'name':name,'cli_tool':'codex','model':'gpt-5.6-luna','reasoning_effort':'low','delivery':delivery,'project_id':str(self.root/'project'),'instructions':instructions} for name,delivery in [('alpha','tmux'),('beta','app_server')]],
                      'messaging':{'mode':'canonical','retentionPolicy':policy}}
        save('policy.json',policy)

    def cleanup(self):
        # Observation never submits a new model input. Drain already started usage.
        try:
            if self.record('beta').get('appServer'): self.host_ready=True
            end=time.monotonic()+60
            while time.monotonic()<end:
                self.observe()
                if self.settled() or not self.started: break
                time.sleep(1)
        except Exception as e: self.event('cleanup_observation',error=str(e))
        try: self.snapshot('final')
        except Exception as e: self.event('snapshot_error',error=str(e))
        save('owned-before-cleanup.json',self.identities())
        for child in reversed(self.children):
            if child.poll() is None:
                os.killpg(child.pid,signal.SIGTERM)
                try: child.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid,signal.SIGKILL); child.wait(timeout=8)
        # Reaping the private namespace terminates its descendants; no operator PID targeted.
        deadline=time.monotonic()+10
        while self.identities() and time.monotonic()<deadline: time.sleep(.2)
        survivors=self.identities()
        with socket.socket() as probe:
            probe.settimeout(.2); closed=self.port is None or probe.connect_ex(('127.0.0.1',self.port))!=0
        try: save('daemon-stderr.json',{'lines':(self.root/'daemon.log').read_text().splitlines()[-60:]})
        except FileNotFoundError: pass
        shutil.rmtree(self.root)
        restore=subprocess.run(['git','-C',str(MESH),'checkout','--','src/delivery/app_server/capabilities.rs'],capture_output=True,text=True)
        (CHECKOUT/'src-tauri/resources/mesh').unlink(missing_ok=True)
        result={'survivors':survivors,'port_closed':closed,'root_removed':not self.root.exists(),'auth_removed':not (self.root/'codex/auth.json').exists(),'mesh_descriptor_restore_exit':restore.returncode,'scratch_resource_removed':not (CHECKOUT/'src-tauri/resources/mesh').exists()}
        save('cleanup.json',result); self.event('cleanup',**result)
        assert not survivors and closed and restore.returncode==0, 'cleanup verification failed'


def main():
    assert Path.cwd()==CHECKOUT
    assert not OUT.exists(), 'run2 already exists; never silently reset the cumulative lane ledger'
    OUT.mkdir(); os.umask(0o077)
    for sig in [signal.SIGTERM,signal.SIGINT]: signal.signal(sig,lambda s,f: (_ for _ in ()).throw(RuntimeError('controller interrupted')))
    lane=Lane(); code=1
    try:
        lane.setup()
        for step in range(1,7):
            lane.step=step
            if step>1:
                deadline=time.monotonic()+180
                while not (OUT/'action.json').exists() and time.monotonic()<deadline:
                    lane.observe(); time.sleep(1)
                action=json.loads((OUT/'action.json').read_text()); (OUT/'action.json').unlink()
                assert action=={'step':step}, 'out-of-order action'
            getattr(lane,'step'+str(step))()
            lane.snapshot('step'+str(step)); lane.observe()
            save('step'+str(step)+'-outcome.json',{'step':step,'outcome':'PASS','classification':'runtime','at':time.time()})
            lane.event('checkpoint',step_completed=step,ledger=lane.ledger)
        code=0
    except BaseException as e:
        reason=clean(str(e))
        classification='mesh' if 'Mesh refusal' in reason or 'team-daemon' in reason else 'taurhaus'
        if any(x in reason for x in ['harness command','auth source','unapproved Codex','cap','headroom','metered']): classification='harness'
        save('step'+str(lane.step)+'-outcome.json',{'step':lane.step,'outcome':'FAIL','classification':classification,'reason':reason})
        lane.event('failure',reason=reason,classification=classification)
        save('controller-error.json',{'traceback':traceback.format_exc()})
    finally: lane.cleanup()
    return code

if __name__=='__main__': raise SystemExit(main())
