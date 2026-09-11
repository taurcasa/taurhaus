"""Lane 7 isolated live controller. Explicit authorized auth source is required.

Only this controller copies that single file; children cannot see operator homes.
Run from this checkout: python3 -B <this-file> --auth-source AUTHORIZED_FILE
The output directory must be new; a restart never retries a paid mutation.
"""
import argparse
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
import threading
import time
from datetime import datetime
from preflight import credential_source
from support import clean, complete_rows, meter, native_runtime, retained_daemon_rows, evidence_jsonl, attributed_idle, ready, delivered, receipt_retry

BASE=Path(__file__).resolve().parent
CHECKOUT=BASE.parents[4]
TEAM='l7-ledger'
MEMBER='alpha'

def mesh_json(output):
    """Accept compact or pretty JSON after optional stdout banner lines."""
    lines=output.splitlines()
    start=next(i for i,line in enumerate(lines) if line.lstrip().startswith('{'))
    return json.loads('\n'.join(lines[start:]))

class Trial:
    def __init__(self):
        self.out=BASE/os.environ.get('L7_RUN_NAME','run');self.out.mkdir()
        self.root=Path(tempfile.mkdtemp(prefix='th-l7-'))
        self.children=[];self.step=1;self.started=time.monotonic();self.port=None
        self.stop=threading.Event();self.observer=None;self.seen={};self.reservations=[]
        self.classification='harness';self.code=1;self.state={};self.identities_seen={}
        self.env={};self.receipts_seen=set();self.first_submissions={};self.first_pending={}
        self.events=(self.out/'events.jsonl').open('w',buffering=1)

    def save(self,name,value):
        path=self.out/name;path.parent.mkdir(parents=True,exist_ok=True)
        data=evidence_jsonl(value) if path.suffix=='.jsonl' else json.dumps(clean(value),indent=2)+'\n'
        if path.exists() and path.read_text() == data:return
        temporary=path.with_suffix(path.suffix+'.tmp');temporary.write_text(data);temporary.replace(path)

    def log(self,kind,**fields):
        row=clean({'at':time.time(),'kind':kind,**fields})
        if kind == 'daemon_request' and row.get('request', {}).get('method') == 'coordination.initialize_team':
            row['request']['params']['request']['team_description'] = '<message-body-redacted>'
        self.events.write(json.dumps(row)+'\n')

    def run(self,argv,timeout=25,check=True):
        logged=list(argv)
        if len(argv)>1 and argv[0]=='tmux' and argv[1]=='new-window':logged[-1]='<generated mesh command; see mesh_command>'
        self.log('command',argv=logged)
        child=subprocess.Popen(argv,env=self.env,cwd=self.root/'project',stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=True)
        self.children.append(child)
        try:output=child.communicate(timeout=timeout)[0].decode(errors='replace')
        except subprocess.TimeoutExpired:
            os.killpg(child.pid,signal.SIGTERM);child.communicate(timeout=10);raise
        self.log('command_result',exit=child.returncode,output_sha256=hashlib.sha256(output.encode()).hexdigest())
        if check and child.returncode:raise RuntimeError(f'command exit {child.returncode}; see command result digest')
        return output

    def rpc(self,method,params):
        deadline=time.monotonic()+90
        while True:
            try:return self._rpc(method,params)
            except RuntimeError as error:
                if not any(t in str(error).lower() for t in ('host member busy','lock busy')) or time.monotonic()>=deadline:raise
                time.sleep(.5)

    def _rpc(self,method,params):
        request={'id':secrets.token_hex(8),'method':method,'params':params}
        self.log('daemon_request',request=request)
        with socket.create_connection(('127.0.0.1',self.port),timeout=15) as conn:
            request['auth']=(self.root/'data/daemon.token').read_text().strip()
            conn.sendall(json.dumps(request).encode()+b'\n');stream=conn.makefile('rb')
            while True:
                response=json.loads(stream.readline())
                if response.get('id')==request['id']:break
        self.log('daemon_response',response=response)
        if 'error' in response:raise RuntimeError(str(response['error']))
        return response['result']

    def operation(self,method,params):
        accepted=self.rpc(method,params)
        def done():
            status=self.rpc('coordination.initialize_status' if method=='coordination.initialize_team' else method+'_status',{'run_id':accepted['run_id']})
            return status if status['outcome']['status']!='running' else None
        value=self.wait(done,method,120)
        self.save(f'step{self.step}-operation.json',value)
        assert value['outcome']['status']=='completed' and not value['outcome']['report'].get('failed_step'), value
        return value

    def wait(self,predicate,why,timeout=90,*,owner='harness'):
        assert timeout>=60
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            self.budget();self.snapshot()
            value=predicate()
            if value:return value
            time.sleep(.2)
        self.classification=owner
        raise AssertionError(why)

    @property
    def team(self):return self.root/'claude/teams'/TEAM
    def record(self):
        try:return json.loads((self.team/'runtime/alpha.json').read_text())
        except (ValueError,FileNotFoundError):return {}
    def activity(self):
        try:return json.loads(Path(self.record()['activitySnapshotPath']).read_text())
        except (KeyError,ValueError,FileNotFoundError):return {}
    def fresh_idle(self):
        value=self.activity()
        return value if attributed_idle(self.record(),value,time.time()) else False
    def sessions(self):return [complete_rows(p.read_text()) for p in (self.root/'codex/sessions').rglob('rollout-*.jsonl')]
    def notify(self):
        path=self.root/'data/codex-notify.jsonl'
        return complete_rows(path.read_text()) if path.exists() else []
    def budget(self,next_input=False):
        value=meter(self.sessions(),self.notify());value['input_reservations']=self.reservations
        self.save('cost-ledger.json',value)
        assert time.monotonic()-self.started <= 720, '12 minute runtime cap reached'
        prior=json.loads((BASE/'run/cost-ledger.json').read_text()) if self.out.name!='run' else {'paid_inputs':0,'conservative_usd':0}
        value['prior_run_inputs']=prior['paid_inputs'];value['prior_known_conservative_usd']=prior['conservative_usd']
        value['prior_unmetered_turns']=[r['turn_id'] for r in prior.get('turns',[]) if r.get('usd') is None]
        self.save('cost-ledger.json',value)
        if next_input:
            assert prior['paid_inputs'] + max(value['paid_inputs'],len(self.reservations)) < 12, 'input cap reached'
            assert prior['conservative_usd'] + value['conservative_usd'] + .025 <= .20, 'cost headroom exhausted'
        return value
    def reserve(self,reason):
        self.budget(next_input=True)
        self.reservations.append({'reason':reason,'step':self.step,'at':time.time(),'generation':self.record().get('attachmentGeneration')})

    def identities(self):
        result=[]
        for p in Path('/proc').iterdir():
            if not p.name.isdigit():continue
            try:
                if ('TAURHAUS_TRIAL_ID='+self.root.name).encode()+b'\0' not in (p/'environ').read_bytes():continue
                row={'pid':int(p.name),'start_ticks':(p/'stat').read_text().rsplit(')',1)[1].split()[19], 'argv':(p/'cmdline').read_bytes().decode(errors='replace').split('\0')}
                result.append(row);self.identities_seen[(row['pid'],row['start_ticks'])]=row
            except (FileNotFoundError,ProcessLookupError,PermissionError):pass
        return result

    def snapshot(self):
        # Observation errors never abort a step or a receipt wait. Only complete rows retained.
        try:
            for glob in ['config.json','runtime/*.json','state/delivery/*.json','state/messaging-v2/segments/*.jsonl','state/terminal/*.holder.json']:
                for path in self.team.glob(glob):
                    try:
                        value=json.loads(path.read_text()) if path.suffix=='.json' else complete_rows(path.read_text())
                        self.save(str(Path('team')/path.relative_to(self.team)),value)
                    except (OSError,ValueError):pass
            for label,value in [('runtime',self.record()),('activity',self.activity())]:
                stable=json.dumps(value,sort_keys=True)
                if self.seen.get(label)!=stable:
                    self.log(label,value=value);self.seen[label]=stable
            self.identities()
        except Exception as error:self.log('observer_error',error=str(error))

    def observe(self):
        previous=None
        with (self.out/'terminal-locks.jsonl').open('w',buffering=1) as stream:
            while not self.stop.is_set():
                try:
                    lock=self.team/'state/terminal/alpha.lock'
                    if not lock.exists():self.stop.wait(.02);continue
                    inode=lock.stat().st_ino;holders=[]
                    for row in self.identities():
                        proc=Path('/proc')/str(row['pid'])
                        try:
                            for fd in (proc/'fdinfo').iterdir():
                                info=fd.read_text()
                                if f'ino:\t{inode}\n' in info and 'lock:' in info:
                                    holders.append({**row,'fd':fd.name,'fdinfo':info})
                        except (OSError,ProcessLookupError):pass
                    holder=self.team/'state/terminal/alpha.holder.json'
                    diagnostic=json.loads(holder.read_text()) if holder.exists() else None
                    value={'inode':inode,'holders':holders,'diagnostic':diagnostic}
                    if value!=previous:
                        stream.write(json.dumps(clean({'at':time.time(),**value}))+'\n');previous=value
                except Exception as error:self.log('lock_observer_error',error=str(error))
                self.stop.wait(.02)

    def capture(self,label):
        panes=self.run(['tmux','list-panes','-a','-F','#{pane_id}']).splitlines()
        for pane in panes:
            value=self.run(['tmux','capture-pane','-p','-S','-12','-t',pane])
            lines=value.splitlines()[-60:]
            safe=[line if re.fullmatch(r'[ ─│╭╮╰╯]*',line) or ('gpt-5.6-luna' in line and len(line)<150) else '<pane text redacted>' for line in lines]
            (self.out/f'{label}-pane-{pane[1:]}.txt').write_text('\n'.join(safe)+'\n')


    def mesh(self,args,member="lead"):
        # Same PID namespace as daemon; explicit root/team/member on every call.
        name=secrets.token_hex(4);output=self.root/(name+'.out');status=self.root/(name+'.exit')
        argv=[str(self.root/'home/.local/bin/mesh'),*args,'--claude-dir',str(self.root/'claude'),'--team',TEAM,'--name',member]
        command=shlex.join(argv)+' >'+shlex.quote(str(output))+' 2>&1; echo $? >'+shlex.quote(str(status))
        self.log('mesh_command',argv=[('<message-body-redacted>' if i == 3 and args[0] == 'send' else v) for i,v in enumerate(argv)])
        self.run(['tmux','new-window','-d','-t','taurhaus','/bin/bash -c '+shlex.quote(command)])
        self.wait(lambda:status.exists(),'mesh command completion',60)
        value=output.read_text();code=int(status.read_text());self.log('mesh_result',exit=code,output_sha256=hashlib.sha256(value.encode()).hexdigest())
        assert code==0, f'mesh exit {code}; output sha256 {hashlib.sha256(value.encode()).hexdigest()}'
        return value

    def boot(self,source):
        for directory in ['home/.local/bin','codex','project','tmp','tmux','claude','grok','gemini','agy','data']:(self.root/directory).mkdir(parents=True,exist_ok=True,mode=0o700)
        binpath=self.root/'home/.local/bin'
        self.env={'PATH':f'{binpath}:/usr/bin:/bin','HOME':str(self.root/'home'),'CODEX_HOME':str(self.root/'codex'),'TMPDIR':str(self.root/'tmp'),'TMUX_TMPDIR':str(self.root/'tmux'),'TAURHAUS_DATA_DIR':str(self.root/'data'),'TAURHAUS_CLAUDE_DIR':str(self.root/'claude'),'CLAUDE_CONFIG_DIR':str(self.root/'claude'),'CLAUDE_DIR':str(self.root/'claude'),'GROK_HOME':str(self.root/'grok'),'TAURHAUS_AGY_DIR':str(self.root/'agy'),'GEMINI_CLI_HOME':str(self.root/'gemini'),'LANG':'C.UTF-8','TERM':'xterm-256color','SHELL':'/bin/bash','RUST_LOG':'info','TAURHAUS_TRIAL_ID':self.root.name}
        source=credential_source(source,authorized_source=source)
        assert not list((self.root/'codex').iterdir())
        shutil.copyfile(source,self.root/'codex/auth.json');(self.root/'codex/auth.json').chmod(0o600)
        self.log('auth_copy',copied_files=['auth.json'],mode='0600',initial_codex_entries=['auth.json'])
        package=Path(shutil.which('codex')).resolve().parents[1]
        native=next(package.glob('node_modules/@openai/codex-linux-x64/vendor/*/bin/codex'))
        for name,path in [*native_runtime(native),('claude',Path(shutil.which('claude')).resolve()),('mesh',CHECKOUT.parent/'mesh-l7/target/debug/mesh'),('taurhaus-daemon',CHECKOUT/'src-tauri/target/release/taurhaus-daemon')]:
            shutil.copyfile(path,binpath/name);(binpath/name).chmod(0o700)
            self.log('binary',name=name,sha256=hashlib.sha256((binpath/name).read_bytes()).hexdigest())
        for name in ['agy','grok','gemini']:
            (binpath/name).write_text('#!/bin/sh\nexit 77\n');(binpath/name).chmod(0o700)
        for rc in ['.bashrc','.profile','.zshrc']:(self.root/'home'/rc).write_text('export PATH="$HOME/.local/bin:/usr/bin:/bin"\n')
        instructions = ('Use only the scratch project. On onboarding, explicitly read and mark your inbox with '
            'mesh read --json --mark-read --claude-dir "$CLAUDE_DIR" --team l7-ledger --name alpha; '
            'follow every next_cursor using the same filters and --since until done. Reply READY. '
            'For assignments, run mesh task accept and task start with the frozen full assignment, then reply TASK_READY; do not complete yet. '
            'Every Mesh command must name --claude-dir "$CLAUDE_DIR" --team l7-ledger --name alpha. '
            'Read each new instruction explicitly before acting. Only write OBSERVATION.md and RESULT.md, each at most 2048 bytes, when specifically requested. '
            'Ledger entry, task complete --summary-file, and ledger-only retry are permitted only when specifically requested. '
            'Use frozen IDs from the instruction; never invent IDs, resend completion, edit any other file, or send unsolicited messages. '
            'Do not inspect credentials or any operator directory. Keep tool outputs and replies short.\n')
        (self.root/'project/AGENTS.md').write_text(instructions);(self.out/'scratch-AGENTS.md').write_text(instructions)
        self.run(['git','init','-q']);self.run(['git','add','AGENTS.md']);self.run(['git','-c','user.name=Trial','-c','user.email=trial@example.invalid','commit','-qm','trial instructions'])
        with socket.socket() as probe:probe.bind(('127.0.0.1',0));self.port=probe.getsockname()[1]
        assert self.port!=17233
        self.env['TAURHAUS_DAEMON_PORT']=str(self.port)
        self.log('isolation',root=str(self.root),environment=self.env,protocol=27,model='gpt-5.6-luna',effort='low',descriptor='unchanged disabled; no hosted seat')
        (self.root/'codex/config.toml').write_text('model="gpt-5.6-luna"\nmodel_reasoning_effort="low"\napproval_policy="never"\nsandbox_mode="danger-full-access"\nweb_search="disabled"\nmodel_context_window=16384\n[projects.'+json.dumps(str(self.root/'project'))+']\ntrust_level="trusted"\n')
        bw=['bwrap','--die-with-parent','--unshare-pid','--ro-bind','/','/','--tmpfs','/home','--tmpfs','/tmp','--tmpfs','/run','--proc','/proc','--dev','/dev','--bind',str(self.root),str(self.root),'--chdir',str(self.root/'project')]
        assert '0.153.4' in self.run(bw+[str(binpath/'codex'),'--version'])
        daemon_argv=[str(binpath/'taurhaus-daemon'),'--port',str(self.port),'--data-dir',str(self.root/'data')]
        boot=self.root/'boot.sh'
        boot.write_text('#!/bin/bash\nset -eu\ntmux -D -f /dev/null &\nfor i in {1..100}; do test -S "$TMUX_TMPDIR/tmux-$(id -u)/default" && break; sleep .1; done\ntmux set-option -g default-shell /bin/bash\ntmux new-session -d -s taurhaus -x 140 -y 48 /bin/bash\n'+shlex.join(daemon_argv)+' &\nwait $!\n')
        self.log('bootstrap_script',text=boot.read_text());self.log('daemon_spawn',argv=bw+['/bin/bash',str(boot)])
        log=(self.root/'daemon.log').open('w')
        child=subprocess.Popen(bw+['/bin/bash',str(boot)],env=self.env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);self.children.append(child)
        self.wait(lambda:(self.root/'data/daemon.token').exists(),'daemon ready',60)
        ping=self.rpc('ping',{});self.save('ping.json',ping)
        self.observer=threading.Thread(target=self.observe);self.observer.start()
        self.save('candidate.json',{'taurhaus_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=CHECKOUT,text=True).strip(),'mesh_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=CHECKOUT.parent/'mesh-l7',text=True).strip(),'protocol':27,'descriptor':'unchanged; no hosted seat'})
        policy=json.loads(re.search(r'DEFAULT_CANONICAL_POLICY = Object.freeze\((\{.*?\})\)',(CHECKOUT/'src/lib/components/meshTabUtils.js').read_text(),re.S)[1])
        self.commands={'codex':{'fresh':'codex --yolo','continue_cmd':'codex --yolo','resume':'codex --yolo resume'}}
        request={'team_name':TEAM,'team_description':'Isolated lane 7 ledger','lead_mode':'launch_new','lead':{'name':'lead','cli_tool':'claude','model':'claude-haiku-4-5','delivery':'tmux','project_id':str(self.root/'project')},'agents':[{'name':'alpha','cli_tool':'codex','model':'gpt-5.6-luna','reasoning_effort':'low','delivery':'tmux','project_id':str(self.root/'project'),'instructions':instructions}],'messaging':{'mode':'canonical','retentionPolicy':policy}}
        self.reserve('production initialization/onboarding')
        self.operation('coordination.initialize_team',{'request':request,'cli_commands':self.commands,'tmux_layout':'new_window'})
        record=self.wait(lambda:self.record() if self.record().get('terminalContract')==1 else None,'terminal contract record absent',90,owner='taurhaus')
        self.save('step1-runtime.json',record)
        for field in ['attachmentGeneration','tmuxSocket','tmuxSessionId','paneId','panePid','paneStartTime','contextGeneration','harness','launchRoot','activitySnapshotPath']:assert record.get(field) is not None,field
        assert not record.get('appServer'),'unexpected hosted seat'
        self.save('step1-pane-identity.json',{'record':record,'probe':self.run(['tmux','-S',record['tmuxSocket'],'display-message','-p','-t',record['paneId'],'#{socket_path} #{session_id} #{pane_id} #{pane_pid}'])})
        self.capture('step1-initial')
        self.save('step1-terminal-lock.json',{'path':str(self.team/'state/terminal/alpha.lock'),'inode':(self.team/'state/terminal/alpha.lock').stat().st_ino})
        # A launch is not a ready seat. Diagnose this known production failure before any direct input.
        def onboarded():
            rows=self.journals()
            accepted=[r['payload']['message_id'] for r in rows if r.get('event_type')=='message_accepted' and any(t.get('recipient')=='alpha' for t in r.get('payload',{}).get('delivery_targets',[]))]
            return next((mid for mid in accepted if ready(rows,mid,self.fresh_idle())),None)
        onboarding=self.wait(onboarded,'onboarding requires submitted + consumed_by_read and fresh idle',120,owner='taurhaus')
        self.last_delivery=onboarding
        self.save('startup-ready.json',{'onboarding_id':onboarding,'runtime':self.record(),'activity':self.activity(),'journal':self.journals(),'snapshot':self.rpc('get_runtime_session_snapshot',{})})
        assert self.rpc('ping',{})['protocol_version']==27

    def pass_step(self):
        if not self.events.closed:self.snapshot()
        self.save(f'step{self.step}-outcome.json',{'step':self.step,'outcome':'PASS','classification':'S-runtime','at':time.time()})
        paths=[str(BASE.relative_to(CHECKOUT)),str(BASE.with_suffix('.md').relative_to(CHECKOUT))]
        subprocess.run(['git','add',*paths],cwd=CHECKOUT,check=True)
        subprocess.run(['git','commit','-m',f'test(e2e): lane 7 step {self.step} evidence\n\nCo-Authored-By: Codex (gpt-6-astra) <noreply@openai.com>\nClaude-Session: https://claude.ai/code/session_01XJa6LsgXqhBdob1f1BS7BU'],cwd=CHECKOUT,check=True,stdout=subprocess.DEVNULL)
        print(f'Step {self.step} PASS and committed',flush=True)

    def journals(self):
        return [r for p in (self.team/'state/messaging-v2/segments').glob('*.jsonl') for r in complete_rows(p.read_text())]

    def exposure(self,marker,role):
        return [r for rows in self.sessions() for r in rows if r.get('type')=='response_item' and r.get('payload',{}).get('role')==role and marker in json.dumps(r.get('payload',{}).get('content',[]))]

    def teardown(self):
        self.stop.set()
        if self.observer:self.observer.join(timeout=10)
        self.snapshot()
        self.save('identities.json',list(self.identities_seen.values()))
        self.save('cost-ledger.json',{**meter(self.sessions(),self.notify()),'input_reservations':self.reservations})
        self.save('native-turn-meter.json', [{'timestamp':r.get('timestamp'),'type':r.get('type'),'payload':{k:v for k,v in r.get('payload',{}).items() if k in ('type','turn_id','model','effort','info')}} for rows in self.sessions() for r in rows if r.get('type')=='turn_context' or (r.get('type')=='event_msg' and r.get('payload',{}).get('type') in ('task_started','task_complete','token_count'))])
        self.save('notify-identities.json',[{k:v for k,v in r.items() if k in ('ts','event','turn_id','session_id')} for r in self.notify()])
        self.save('rollout-inventory.json',[{'file':p.name,'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in (self.root/'codex/sessions').rglob('rollout-*.jsonl')])
        if self.record():
            try:self.capture('final')
            except Exception as e:self.log('capture_error',error=str(e))
        if self.record():
            try:self.save('final-runtime-sessions.json',self.rpc('get_runtime_session_snapshot',{}))
            except Exception as e:self.log('runtime_snapshot_error',error=str(e))
        self.save('final-runtime.json',self.record());self.save('final-activity.json',self.activity())
        # Namespace PID 1 exit reaps every descendant. No foreign process is signalled.
        before=self.identities()
        for row in before:
            if row['argv'][0]==str(self.root/'home/.local/bin/taurhaus-daemon'):
                try:
                    if (Path('/proc')/str(row['pid'])/'stat').read_text().rsplit(')',1)[1].split()[19]==row['start_ticks']:os.kill(row['pid'],signal.SIGINT)
                except (FileNotFoundError,ProcessLookupError):pass
        for child in reversed(self.children):
            if child.poll() is None:
                try:child.wait(timeout=10)
                except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGTERM);child.wait(timeout=10)
        survivors=self.identities()
        log_manifest=[]
        for path in (self.root/'data').glob('taurhaus.log*.jsonl'):
            data=path.read_bytes();rows=complete_rows(data.decode())
            self.save(path.name,retained_daemon_rows(rows))
            log_manifest.append({'file':path.name,'source_sha256':hashlib.sha256(data).hexdigest(),'physical_lines':len(data.splitlines()),'retained_rows':len(rows),'all_rows_retained':len(data.splitlines())==len(rows)})
        self.save('daemon-log-manifest.json',log_manifest)
        raw=self.root/'daemon.log'
        if raw.exists():self.save('daemon-stderr.json',{'sha256':hashlib.sha256(raw.read_bytes()).hexdigest(),'bytes':raw.stat().st_size})
        with socket.socket() as probe:
            probe.settimeout(.2);closed=self.port is None or probe.connect_ex(('127.0.0.1',self.port))!=0
        auth=self.root/'codex/auth.json'
        if auth.exists():auth.unlink()
        auth_removed=not auth.exists();shutil.rmtree(self.root)
        self.save('cleanup.json',{'before':before,'survivors':survivors,'port_closed':closed,'auth_removed':auth_removed,'root_removed':not self.root.exists()})
        self.events.close()
        if survivors or not closed:self.code=2

