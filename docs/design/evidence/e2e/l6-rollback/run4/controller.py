"""Lane 6 isolated live controller. Explicit authorized auth source is required.

Only this controller copies that single file; children cannot see operator homes.
Run from this checkout: python3 -B <this-file> --auth-source AUTHORIZED_FILE --out NEW_DIRECTORY
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
import sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from preflight import credential_source, AUTHORIZED_AUTH_SOURCE
from rollback import retry_transient
from rules import downgrade_boundary, same_owner_commit, attached_executor
from support import validate_candidate, onboarding_delivered, confirm_submission, reply_evidence, startup_composer
from support import clean, complete_rows, meter, native_runtime, retained_daemon_rows, evidence_jsonl, attributed_idle, ready_session
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'run2'))
from run2_rules import pending as pending_observation, format_boundary, assistant_reply

BASE=Path(__file__).resolve().parent
CHECKOUT=BASE.parents[5]
TEAM='l6-rollback-run4'
MEMBER='alpha'

def mesh_json(output):
    """Decode canonical objects or legacy arrays after Mesh's diagnostic banner."""
    lines=output.splitlines()
    for i,line in enumerate(lines):
        if line.lstrip().startswith(('{','[')):
            try:return json.loads('\n'.join(lines[i:]))
            except ValueError:continue
    raise ValueError('Mesh command did not return a complete JSON object or array')

class Trial:
    def __init__(self,out=BASE/'run'):
        self.out=Path(out);self.out.mkdir()
        self.root=Path(tempfile.mkdtemp(prefix='th-l6-'))
        self.children=[];self.step=1;self.started=time.monotonic();self.port=None
        self.stop=threading.Event();self.observer=None;self.seen={};self.reservations=[]
        self.classification='harness';self.code=1;self.identities_seen={}
        self.env={};self.receipts_seen=set();self.first_submissions={};self.first_pending={}
        self.evidence_lock=threading.RLock()
        self.digest_cache={}
        self.owner_window=False;self.owner_observer=None;self.handoff_observer=None;self.handoff_window=False
        self.events=(self.out/'events.jsonl').open('w',buffering=1)

    def save(self,name,value):
        path=self.out/name;path.parent.mkdir(parents=True,exist_ok=True)
        data=evidence_jsonl(value) if path.suffix=='.jsonl' else json.dumps(clean(value),indent=2)+'\n'
        temporary=path.with_suffix(path.suffix+'.tmp');temporary.write_text(data);temporary.replace(path)

    def log(self,kind,**fields):
        row=clean({'at':time.time(),'kind':kind,**fields})
        with self.evidence_lock:
            self.events.write(json.dumps(row)+'\n')

    def run(self,argv,timeout=25,check=True):
        self.log('command',argv=argv)
        child=subprocess.Popen(argv,env=self.env,cwd=self.root/'project',stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=True)
        self.children.append(child)
        try:output=child.communicate(timeout=timeout)[0].decode(errors='replace')
        except subprocess.TimeoutExpired:
            os.killpg(child.pid,signal.SIGTERM);child.communicate(timeout=10);raise
        self.log('command_result',exit=child.returncode,output=output)
        if check and child.returncode:raise RuntimeError(f'command exit {child.returncode}: {clean(output)}')
        return output

    def rpc(self,method,params):
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
            assert time.monotonic()-self.started<=720,'12 minute runtime cap reached'
            self.budget(observe_only=True);self.snapshot()
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
    def budget(self,next_input=False,observe_only=False):
        value=meter(self.sessions(),self.notify());value['input_reservations']=self.reservations
        direct=[r for r in self.reservations if r['reason'] in ('B-work',)]
        deliveries={r.get('payload',{}).get('attempt_id') for r in self.journals() if r.get('payload',{}).get('recipient')=='alpha' and r.get('payload',{}).get('stage')=='submitted'}
        value['rollout_turns']=value['paid_inputs']
        value['controller_input_attempts']=len(direct)
        value['controller_issued_inputs']=sum(bool(r.get('confirmed')) for r in direct);value['mesh_terminal_deliveries']=len(deliveries)
        legacy={r.get('message_id') for r in self.workflow() if r.get('eventType')=='message_delivery_recorded' and r.get('channel')=='tmux' and r.get('target')==self.record().get('paneId') and r.get('outcome')=='tmux_injected'}
        value['legacy_terminal_deliveries']=len(legacy)
        value['paid_inputs']=value['controller_issued_inputs']+len(deliveries)+len(legacy)
        self.save('cost-ledger.json',value)
        if observe_only:return value
        assert value['paid_inputs']<=10 and len(self.reservations)<=10,'input cap exceeded'
        assert value['api_equivalent_usd']<=.20,'cost cap exceeded'
        assert time.monotonic()-self.started<=720,'12 minute runtime cap reached'
        if next_input:
            assert max(value['paid_inputs'],len(self.reservations))<10,'no input headroom'
            assert value['api_equivalent_usd']+.04<=.20,'no conservative cost headroom'
        return value
    def reserve(self,reason):
        self.budget(next_input=True);self.reservations.append({'reason':reason,'step':self.step,'at':time.time(),'generation':self.record().get('attachmentGeneration')})
        self.budget()

    def identities(self):
        result=[]
        for p in Path('/proc').iterdir():
            if not p.name.isdigit():continue
            try:
                if ('TAURHAUS_TRIAL_ID='+self.root.name).encode()+b'\0' not in (p/'environ').read_bytes():continue
                row={'pid':int(p.name),'start_ticks':(p/'stat').read_text().rsplit(')',1)[1].split()[19], 'argv':(p/'cmdline').read_bytes().decode(errors='replace').split('\0')}
                result.append(row)
                with self.evidence_lock:
                    self.identities_seen[(row['pid'],row['start_ticks'])]=row
            except (FileNotFoundError,ProcessLookupError,PermissionError):pass
        return result

    def snapshot(self):
        # Observation errors never abort a step or a receipt wait. Only complete rows retained.
        try:
            for glob in ['config.json','runtime/*.json','inboxes/*.json','state/*.json','state/workflow_events.jsonl','state/delivery/*.json','state/messaging-v2/segments/*.jsonl','state/terminal/*.holder.json']:
                for path in self.team.glob(glob):
                    try:
                        value=json.loads(path.read_text()) if path.suffix=='.json' else complete_rows(path.read_text())
                        self.save(str(Path('team')/path.relative_to(self.team)),value)
                    except (OSError,ValueError):pass
            for label,value in [('runtime',self.record()),('activity',self.activity())]:
                stable=json.dumps(value,sort_keys=True)
                with self.evidence_lock:
                    if self.seen.get(label)!=stable:
                        self.log(label,value=value);self.seen[label]=stable
            journal=self.journals()
            try:health=json.loads((self.team/'state/delivery/health-alpha.json').read_text())
            except (OSError,ValueError):health={}
            for row in journal:
                payload=row.get('payload',{})
                mid=payload.get('message_id')
                if row.get('event_type')=='message_accepted' and mid not in self.first_pending and any(t.get('recipient')=='alpha' for t in payload.get('delivery_targets',[])):
                    obligations=[r for p in (self.team/'state/delivery').glob('pending-*.json') for r in json.loads(p.read_text())]
                    pending=pending_observation(journal,mid,health,activity=self.activity(),now=time.time(),obligations=obligations)
                    if pending and self.activity().get('activity_confidence') in ('active','likely_working'):
                        observed={'pending':pending,'runtime':self.record(),'activity':self.activity(),'at':time.time(),'native_rows':[r for rows in self.sessions() for r in rows if r.get('type')=='response_item']}
                        self.first_pending[mid]=observed
                        self.save('pending/'+mid+'.json',observed)
            for row in journal:
                payload=row.get('payload',{})
                if payload.get('stage')=='submitted' and row['event_id'] not in self.receipts_seen:
                    self.receipts_seen.add(row['event_id'])
                    observation={'receipt':row,'runtime':self.record(),'activity':self.activity(),'at':time.time()}
                    self.first_submissions[payload['message_id']]=observation
                    self.save('submissions/'+row['event_id']+'.json',observation)
            self.identities()
        except Exception as error:self.log('observer_error',error=str(error))

    def assert_no_pending_delivery(self):
        if not (self.team/'config.json').exists():return
        if self.config().get('messaging_format',1)==2:
            pending=[]
            for row in self.journals():
                p=row.get('payload',{})
                if row.get('event_type')!='message_accepted' or not any(t.get('recipient')=='alpha' for t in p.get('delivery_targets',[])):continue
                history=self.delivery_rows(p['message_id'])
                read=any(r.get('payload',{}).get('kind')=='consumed_by_read' and r['payload'].get('reader_name')=='alpha' for r in history)
                submitted=any(r.get('payload',{}).get('stage')=='submitted' and r['payload'].get('recipient')=='alpha' for r in history)
                if not read and not (submitted and self.tool_exposure(p.get('body',''))):pending.append(p['message_id'])
        else:
            pending=[r['id'] for r in json.loads((self.team/'inboxes/alpha.json').read_text()) if not r.get('read')]
        assert not pending,'delivery to seat still pending: '+str(pending)

    def owner_census(self):
        processes=self.identities()
        mesh=str(self.root/'home/.local/bin/mesh')
        owners=[r for r in processes if r['argv'] and r['argv'][0]==mesh and r['argv'][1:3]==['team-daemon','start']]
        executors=[]
        for row in processes:
            if row['argv'][:2]!=[mesh,'daemon']:continue
            try:
                proc=Path('/proc')/str(row['pid'])
                ns=next(line for line in (proc/'status').read_text().splitlines() if line.startswith('NSpid:'))
                row=dict(row,namespace_pid=int(ns.split()[-1]))
                key=(row['pid'],row['start_ticks'])
                if key not in self.digest_cache:
                    self.digest_cache[key]=hashlib.sha256((proc/'exe').read_bytes()).hexdigest()
                row['sha256']=self.digest_cache[key];executors.append(row)
            except (OSError,StopIteration):pass
        def read(relative):
            try:return json.loads((self.team/relative).read_text())
            except (OSError,ValueError):return None
        return {'at':time.time(),'owners':owners,'operator_marker':read('state/delivery/owner-stopped.json'),
                'executors':executors,'runtime':self.record(),'authority':read('state/messaging-authority.json'),
                'config':read('config.json')}

    def observe_owner(self):
        with (self.out/'owner-census.jsonl').open('w',buffering=1) as stream:
            while not self.stop.is_set():
                if self.owner_window:
                    try:stream.write(json.dumps(clean(self.owner_census()))+'\n')
                    except Exception as error:self.log('owner_census_error',error=str(error))
                self.stop.wait(.5)

    def observe_handoff(self):
        previous=None
        while not self.stop.is_set():
            if self.handoff_window:
                try:
                    path=self.team/'state/delivery/handoff.json'
                    if path.exists():
                        value=json.loads(path.read_text())
                        if value!=previous:
                            self.log('handoff_observation',name='handoff',value=value)
                            previous=value
                except (OSError,ValueError):pass
                self.stop.wait(.002)
            else:self.stop.wait(.02)

    def observe_self_heal_window(self):
        # Observation only: absence/restart is a product observation, never a rollback gate.
        started=time.monotonic();rows=[]
        while time.monotonic()-started<65:
            rows=complete_rows((self.root/'data/taurhaus.log.jsonl').read_text())
            skips=[r for r in rows if any(x in json.dumps(r) for x in ('owner_stopped_by_operator','rollback_pending'))]
            if skips:break
            self.snapshot();time.sleep(.5)
        self.save('step1-self-heal-observation.json',{'classification':'PRODUCT OBSERVATION','elapsed_seconds':time.monotonic()-started,'named_skip_rows':skips,'census':self.owner_census()})

    def observe(self):
        previous=None
        with (self.out/'terminal-locks.jsonl').open('w',buffering=1) as stream:
            while not self.stop.is_set():
                try:
                    for name in ('handoff','epoch','rollback'):
                        path=self.team/f'state/delivery/{name}.json'
                        if path.exists():
                            value=json.loads(path.read_text());key='handoff-'+name
                            with self.evidence_lock:
                                if self.seen.get(key)!=value:
                                    self.log('handoff_observation',name=name,value=value);self.seen[key]=value
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
            (self.out/f'{label}-pane-{pane[1:]}.txt').write_text(clean('\n'.join(value.splitlines()[-60:]))+'\n')

    def mesh_raw(self,args,member="lead",*,retry=True):
        def attempt():
            name=secrets.token_hex(4);output=self.root/(name+'.out');status=self.root/(name+'.exit')
            argv=[str(self.root/'home/.local/bin/mesh'),*args,'--claude-dir',str(self.root/'claude'),'--team',TEAM,'--name',member]
            command=shlex.join(argv)+' >'+shlex.quote(str(output))+' 2>&1; echo $? >'+shlex.quote(str(status))
            self.log('mesh_command',argv=argv)
            self.run(['tmux','new-window','-d','-t','taurhaus','/bin/bash -c '+shlex.quote(command)])
            self.wait(lambda:status.exists(),'mesh command completion',90)
            value=output.read_text();code=int(status.read_text());self.log('mesh_result',exit=code,output=value)
            return code,value
        return retry_transient(attempt,time.monotonic,time.sleep,90) if retry else attempt()

    def mesh(self,args,member="lead"):
        code,output=self.mesh_raw(args,member)
        assert code==0,f'mesh exit {code}: {output}'
        return output

    def candidate_preflight(self):
        def git(cwd, *args):
            return subprocess.check_output(['git', *args], cwd=cwd, text=True).strip()
        mesh_root=CHECKOUT.parent/'mesh-l6'
        assert git(CHECKOUT,'branch','--show-current')=='feat/e2e-l6-rollback'
        subprocess.run(['git','merge-base','--is-ancestor','ac2bc513','HEAD'],cwd=CHECKOUT,check=True)
        candidate={'product_commit':git(CHECKOUT,'rev-parse','ac2bc513'),
                   'checkout_tip':git(CHECKOUT,'rev-parse','HEAD'),
                   'product_diff':git(CHECKOUT,'diff','ac2bc513','--','src-tauri','src'),
                   'mesh_commit':git(mesh_root,'rev-parse','HEAD'),
                   'mesh_diff':git(mesh_root,'diff','HEAD'),
                   'flush_tolerance_count':(CHECKOUT/'src-tauri/src/session_scanner/idle/codex.rs').read_text().count('COMPLETION_FLUSH_TOLERANCE'),
                   'protocol':int(re.search(r'PROTOCOL_VERSION: u32 = (\d+)',(CHECKOUT/'src-tauri/src/daemon/protocol.rs').read_text())[1])}
        assert not git(CHECKOUT,'diff','HEAD','--','src-tauri','src') and not candidate['mesh_diff']
        assert candidate['mesh_commit']==git(mesh_root,'rev-parse','release/overhaul-rc')
        assert candidate['mesh_commit'].startswith('3015cb0') and candidate['protocol']==27
        assert candidate['flush_tolerance_count']>=1
        self.save('preflight-candidate.json',candidate)
        self.log('candidate_preflight', **candidate)



    def startup_preflight(self):
        started=time.monotonic();next_capture=0
        while True:
            elapsed=time.monotonic()-started
            self.budget(observe_only=True);self.snapshot()
            record=self.record()
            pane=self.run(['tmux','-S',record['tmuxSocket'],'capture-pane','-p','-t',record['paneId']])
            if elapsed>=next_capture:
                (self.out/f'startup-{int(next_capture):03d}-pane-{record["paneId"][1:]}.txt').write_text(clean('\n'.join(pane.splitlines()[-60:]))+'\n')
                next_capture+=30
            if startup_composer(pane):
                self.save('startup-preflight.json',{'outcome':'PASS','elapsed_seconds':elapsed,'composer':True,'onboarding_window_seconds':90})
                return
            if elapsed>=120:
                self.classification='harness'
                self.save('startup-preflight.json',{'outcome':'UNAVAILABLE','classification':'harness','reason':'codex_startup_stall','elapsed_seconds':elapsed,'composer':False})
                raise RuntimeError('codex_startup_stall: no Codex composer and model footer within 120 seconds')
            time.sleep(min(1,120-elapsed))


    def tool_exposure(self,marker):
        return [r for rows in self.sessions() for r in rows if r.get('type')=='response_item' and r.get('payload',{}).get('type') in ('function_call_output','custom_tool_call_output') and marker in json.dumps(r.get('payload',{}))]


    def reply(self,marker,message_id):
        return reply_evidence(self.journals(),[r for rows in self.sessions() for r in rows],message_id,marker)


    def settle_delivery(self,marker,message_id):
        for name,predicate,why,owner in [
            ('reply',lambda:self.reply(marker,message_id),'marker reply evidence missing','mesh'),
            ('idle',self.fresh_idle,'post-reply fresh idle missing','taurhaus')]:
            value=self.wait(predicate,why,90,owner=owner)
            self.save(f'step{self.step}-{name}-wait.json',{'at':time.time(),'outcome':'PASS','evidence':value})

        self.wait(lambda:assistant_reply([r for rows in self.sessions() for r in rows],marker,next(r['payload']['observed_at'] for r in self.delivery_rows(message_id) if r.get('payload',{}).get('stage')=='submitted')),'assistant marker reply absent',90,owner='mesh')
        self.save(f'step{self.step}-metering-observation.json',self.budget())


    def boot(self,source):
        self.candidate_preflight()
        for directory in ['home/.local/bin','codex','project','tmp','tmux','claude','grok','gemini','agy','data']:(self.root/directory).mkdir(parents=True,exist_ok=True,mode=0o700)
        binpath=self.root/'home/.local/bin'
        self.env={'PATH':f'{binpath}:/usr/bin:/bin','HOME':str(self.root/'home'),'CODEX_HOME':str(self.root/'codex'),'TMPDIR':str(self.root/'tmp'),'TMUX_TMPDIR':str(self.root/'tmux'),'TAURHAUS_DATA_DIR':str(self.root/'data'),'TAURHAUS_CLAUDE_DIR':str(self.root/'claude'),'CLAUDE_CONFIG_DIR':str(self.root/'claude'),'CLAUDE_DIR':str(self.root/'claude'),'GROK_HOME':str(self.root/'grok'),'TAURHAUS_AGY_DIR':str(self.root/'agy'),'GEMINI_CLI_HOME':str(self.root/'gemini'),'LANG':'C.UTF-8','TERM':'xterm-256color','SHELL':'/bin/bash','RUST_LOG':'info','TAURHAUS_TRIAL_ID':self.root.name}
        source=credential_source(source,authorized_source=AUTHORIZED_AUTH_SOURCE)
        assert not list((self.root/'codex').iterdir())
        shutil.copyfile(source,self.root/'codex/auth.json');(self.root/'codex/auth.json').chmod(0o600)
        self.log('auth_copy',copied_files=['auth.json'],mode='0600',initial_codex_entries=['auth.json'],source_label=source.parent.name+'/'+source.name)
        package=Path(shutil.which('codex')).resolve().parents[1]
        native=next(package.glob('node_modules/@openai/codex-linux-x64/vendor/*/bin/codex'))
        for name,path in [*native_runtime(native),('claude',Path(shutil.which('claude')).resolve()),('mesh',CHECKOUT.parent/'mesh-l6/target/debug/mesh'),('taurhaus-daemon',CHECKOUT/'src-tauri/target/release/taurhaus-daemon')]:
            shutil.copyfile(path,binpath/name);(binpath/name).chmod(0o700)
            self.log('binary',name=name,sha256=hashlib.sha256((binpath/name).read_bytes()).hexdigest())
        for name in ['agy','grok','gemini']:
            (binpath/name).write_text('#!/bin/sh\nexit 77\n');(binpath/name).chmod(0o700)
        for rc in ['.bashrc','.profile','.zshrc']:(self.root/'home'/rc).write_text('export PATH="$HOME/.local/bin:/usr/bin:/bin"\n')
        instructions='For every inbox notification including onboarding, explicitly run mesh read --json --unread --mark-read --claude-dir \"$CLAUDE_DIR\" --team l6-rollback-run4 --name alpha; follow returned --since cursors with the same filters until done. Reply briefly. When a message assigns you a task, run exactly the mesh lifecycle commands the message names, in order, then reply with the task id and the word done. Never modify files. For ACTION REQUIRED marker messages, read your inbox with mesh read --json --mark-read --claude-dir "$CLAUDE_DIR" --team l6-rollback-run4 --name alpha and reply exactly with the requested marker. Follow every returned cursor until done. Do not send other messages.\n'
        (self.root/'project/AGENTS.md').write_text(instructions);(self.out/'scratch-AGENTS.md').write_text(instructions)
        self.run(['git','init','-q']);self.run(['git','add','AGENTS.md']);self.run(['git','-c','user.name=Trial','-c','user.email=trial@example.invalid','commit','-qm','trial instructions'])
        with socket.socket() as probe:probe.bind(('127.0.0.1',0));self.port=probe.getsockname()[1]
        assert self.port!=17233
        self.env['TAURHAUS_DAEMON_PORT']=str(self.port)
        self.log('isolation',root=str(self.root),environment=self.env,protocol=27,model='gpt-5.6-luna',effort='low',descriptor='unchanged disabled; no hosted seat')
        (self.root/'codex/config.toml').write_text('model="gpt-5.6-luna"\nmodel_reasoning_effort="low"\napproval_policy="never"\nsandbox_mode="danger-full-access"\nweb_search="disabled"\n[projects.'+json.dumps(str(self.root/'project'))+']\ntrust_level="trusted"\n')
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
        self.owner_observer=threading.Thread(target=self.observe_owner);self.owner_observer.start()
        self.handoff_observer=threading.Thread(target=self.observe_handoff);self.handoff_observer.start()
        self.save('candidate.json',{'taurhaus_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=CHECKOUT,text=True).strip(),'mesh_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=CHECKOUT.parent/'mesh-l6',text=True).strip(),'protocol':27,'descriptor':'unchanged; no hosted seat'})
        policy=json.loads(re.search(r'DEFAULT_CANONICAL_POLICY = Object.freeze\((\{.*?\})\)',(CHECKOUT/'src/lib/components/meshTabUtils.js').read_text(),re.S)[1])
        self.commands={'codex':{'fresh':'codex --yolo','continue_cmd':'codex --yolo','resume':'codex --yolo resume'}}
        request={'team_name':TEAM,'team_description':'Isolated lane 6 ownership and format rollback','lead_mode':'launch_new','lead':{'name':'lead','cli_tool':'claude','model':'claude-haiku-4-5','delivery':'tmux','project_id':str(self.root/'project')},'agents':[{'name':'alpha','cli_tool':'codex','model':'gpt-5.6-luna','reasoning_effort':'low','delivery':'tmux','project_id':str(self.root/'project'),'instructions':instructions}],'messaging':{'mode':'canonical','retentionPolicy':policy}}
        self.reserve('production initialization/onboarding')
        self.operation('coordination.initialize_team',{'request':request,'cli_commands':self.commands,'tmux_layout':'new_window'})
        record=self.wait(lambda:self.record() if self.record().get('terminalContract')==1 else None,'terminal contract record absent',90,owner='taurhaus')
        self.save('step1-runtime.json',record)
        for field in ['attachmentGeneration','tmuxSocket','tmuxSessionId','paneId','panePid','paneStartTime','contextGeneration','harness','launchRoot','activitySnapshotPath']:assert record.get(field) is not None,field
        assert not record.get('appServer'),'unexpected hosted seat'
        self.save('step1-pane-identity.json',{'record':record,'probe':self.run(['tmux','-S',record['tmuxSocket'],'display-message','-p','-t',record['paneId'],'#{socket_path} #{session_id} #{pane_id} #{pane_pid}'])})
        self.capture('step1-initial')
        self.save('step1-terminal-lock.json',{'path':str(self.team/'state/terminal/alpha.lock'),'inode':(self.team/'state/terminal/alpha.lock').stat().st_ino})
        self.startup_preflight()
        # A launch is not a ready seat. Diagnose this known production failure before any direct input.
        def onboarded():
            snapshot=self.rpc('get_runtime_session_snapshot',{})
            return onboarding_delivered(self.record(),self.activity(),snapshot,self.journals(),time.time())
        self.wait(onboarded,'alpha onboarding not delivered/completed with fresh idle activity',90,owner='taurhaus')
        self.save('step1-metering-observation.json',self.budget())
        def attributed():
            snapshot=self.rpc('get_runtime_session_snapshot',{})
            row=ready_session(self.record(),snapshot)
            return {'snapshot':snapshot,'alpha':row} if row and self.fresh_idle() else None
        attribution=self.wait(attributed,'alpha runtime attribution/idle not confirmed',90,owner='taurhaus')
        self.save('step1-ready.json',{'runtime':self.record(),'activity':self.activity(),'attribution':attribution,'journal':self.journals()})
        self.capture('step1-ready')

    def pass_step(self):
        self.snapshot();self.save(f'step{self.step}-outcome.json',{'step':self.step,'outcome':'PASS','classification':'S-runtime verified','at':time.time()})
        report=BASE.parents[1]/'l6-rollback.md'
        text=report.read_text()
        text=re.sub(r'^# .*',f'# L6 rollback — run4 IN PROGRESS (step {self.step} PASS)',text,count=1)
        text+=f'\nRun4 step {self.step}: PASS (S-runtime); see run4/run/step{self.step}-outcome.json.\n'
        report.write_text(text)
        # Commit only this lane's explicitly named evidence directory and report.
        paths=[str(BASE.relative_to(CHECKOUT)),str(report.relative_to(CHECKOUT))]
        subprocess.run(['git','add',*paths],cwd=CHECKOUT,check=True)
        subprocess.run(['git','commit','-m',f'test(e2e): Lane 6 run4 step {self.step} runtime evidence\n\nCo-Authored-By: Codex (gpt-6-astra) <noreply@openai.com>\nClaude-Session: https://claude.ai/code/session_01XJa6LsgXqhBdob1f1BS7BU'],cwd=CHECKOUT,check=True,stdout=subprocess.DEVNULL)
        print(f'Step {self.step} PASS and committed',flush=True)

    def journals(self):
        return [r for p in (self.team/'state/messaging-v2/segments').glob('*.jsonl') for r in complete_rows(p.read_text())]

    def response(self,label):
        self.classification='harness'
        self.assert_no_pending_delivery()
        assert self.fresh_idle(), 'ordinary input requires fresh idle'
        assert time.monotonic()-self.started+115<=720,'no full confirmation and observation headroom'
        self.reserve(label)
        record=self.record()
        prior={r.get('payload',{}).get('turn_id') for rows in self.sessions() for r in rows if r.get('payload',{}).get('type')=='task_started'}
        prefix=['tmux','-S',record['tmuxSocket']]
        def send(value):
            self.run(prefix+['send-keys','-t',record['paneId']]+(['Enter'] if value=='Enter' else ['-l',value]))
        def read():
            self.budget(observe_only=True);self.snapshot()
            pane=self.run(prefix+['capture-pane','-p','-t',record['paneId']])
            (self.out/(label+'-composer.txt')).write_text(clean('\n'.join(pane.splitlines()[-60:]))+'\n')
            return pane,[r for rows in self.sessions() for r in rows]
        confirmation=confirm_submission('Print exactly 60 numbered lines, each with 15 words about rivers. Then print DONE. Do not use tools.',send,read,time.monotonic,time.sleep,prior)
        confirmation['at']=time.time();confirmation['observation_window_seconds']=90
        self.reservations[-1]['confirmed']=True
        self.save(label+'-confirmation.json',confirmation)
        self.log('codex_submission_confirmed',label=label,**confirmation)
        try:
            working=self.wait(lambda:self.activity() if self.activity().get('activity_confidence') in ('active','likely_working') else None,'no production busy observation',90)
        except AssertionError as error:
            if str(error)!='no production busy observation':raise
            self.classification='unproved corner'
            raise AssertionError('confirmed response but no observed busy window; audit requires busy deferral so steps 4–6 cannot proceed') from error
        self.save(label+'-working.json',{'runtime':self.record(),'activity':working,'at':time.time()})

    def send_marker(self,label):
        self.assert_no_pending_delivery()
        # Current ordinary turn is intentionally active. Reserve the queued turn from
        # the previously metered headroom, never assume this active turn is free.
        value=self.budget()
        assert max(value['paid_inputs'],len(self.reservations))+1<=10
        assert value['api_equivalent_usd']+.08<=.20
        self.reservations.append({'reason':label,'step':self.step,'at':time.time(),'generation':self.record().get('attachmentGeneration')})
        marker=label+'-'+secrets.token_hex(4)
        output=self.mesh(['send','alpha','ACTION REQUIRED: Read this message explicitly, then reply exactly '+marker+'.','--summary',label])
        if self.config().get('messaging_format',1)==2:
            accepted=mesh_json(output);message_id=accepted['message_id']
        else:
            projection=next(r for r in json.loads((self.team/'inboxes/alpha.json').read_text()) if marker in r.get('text',''))
            message_id=projection.get('message_id',projection['id']);accepted={'output':output,'projection':projection,'message_id':message_id}
        self.save(label+'-accepted.json',{'marker':marker,**accepted})
        return marker,message_id

    def pending(self,marker,message_id):
        def observed():
            return self.first_pending.get(message_id)
        value=self.wait(observed,'no pending evidence while production snapshot reports working',90)
        assert marker not in json.dumps(value['native_rows']),'marker exposed before pending observation'
        self.save(f'step{self.step}-pending.json',value)
        self.capture(f'step{self.step}-pending')

    def delivered(self,marker,message_id,generation):
        self.settle_delivery(marker,message_id)
        self.wait(lambda:self.attributed() and any(r.get('payload',{}).get('kind')=='consumed_by_read' for r in self.delivery_rows(message_id)), 'attributed fresh idle and read missing',90,owner='mesh')
        assert any(r.get('payload',{}).get('message_id')==message_id and r.get('payload',{}).get('kind')=='consumed_by_read' for r in self.journals()),'explicit marker read missing'
        assert self.record()['attachmentGeneration']==generation,'unexpected delivery generation'
        rows=[r for r in self.journals() if r.get('payload',{}).get('message_id')==message_id]
        assert any(r.get('payload',{}).get('stage') in ('submitted','consumed') for r in rows),'no terminal submission receipt'
        submitted=[r['payload'] for r in rows if r.get('payload',{}).get('stage')=='submitted']
        assert len(submitted)==1,'terminal submission count differs from one'
        attachment=submitted[0]['attachment'];record=self.record()
        assert attachment['attachment_generation']==generation,'old-generation submission'
        for key,field in [('pane','paneId'),('pane_pid','panePid'),('pane_start','paneStartTime'),('socket','tmuxSocket'),('tmux_session','tmuxSessionId')]:assert attachment[key]==record[field],key
        observation=self.first_submissions[message_id]
        delivery_at=datetime.fromisoformat(submitted[0]['observed_at']).timestamp()
        self.save(f'step{self.step}-receipt-observation.json',observation)
        self.save(f'step{self.step}-delivery.json',{'receipts':rows,'runtime':self.record(),'activity':self.activity(),'tool_exposure':self.tool_exposure(marker),'reply':self.reply(marker,message_id),'accepted_to_submitted_seconds':delivery_at-datetime.fromisoformat(next(r['committed_at'] for r in rows if r.get('event_type')=='message_accepted')).timestamp()})
        self.capture(f'step{self.step}-delivered')

    def workflow(self):
        path=self.team/'state/workflow_events.jsonl'
        return complete_rows(path.read_text()) if path.exists() else []

    def config(self):return json.loads((self.team/'config.json').read_text())

    def projection(self,message_id):
        path=self.team/'inboxes/alpha.json'
        if not path.exists():return None
        return next((r for r in json.loads(path.read_text()) if r.get('message_id')==message_id or r.get('id')==message_id),None)

    def boundary_snapshot(self,label):
        self.snapshot()
        value={'runtime':self.record(),'activity':self.activity(),'config':self.config(),'journal':self.journals(),'workflow':self.workflow()}
        for name in ['handoff','epoch','rollback','owner-stopped']:
            path=self.team/f'state/delivery/{name}.json'
            value[name]=json.loads(path.read_text()) if path.exists() else None
        self.save(label+'.json',value)
        return value

    def delivery_rows(self,message_id):
        return [r for r in self.journals() if r.get('payload',{}).get('message_id')==message_id]

    def attributed(self):
        snapshot=self.rpc('get_runtime_session_snapshot',{})
        return {'snapshot':snapshot,'activity':self.activity(),'runtime':self.record()} if self.fresh_idle() and ready_session(self.record(),snapshot) else None

    def read_all(self,label):
        cursor=None;pages=[]
        while True:
            args=['read','--json','--mark-read','--last','16']
            if cursor:args+=['--since',cursor]
            page=mesh_json(self.mesh(args,member='alpha'));pages.append(page)
            if self.config().get('messaging_format',1)!=2 or page.get('done'):break
            next_cursor=page.get('next_cursor',page.get('cursor'))
            assert next_cursor and next_cursor!=cursor,'read cursor made no progress'
            cursor=next_cursor
        self.save(label+'.json',pages)

    def legacy_delivery(self,marker,message_id):
        def complete():
            projection=self.projection(message_id)
            if not projection or not projection.get('read'):return None
            did=projection['id']
            receipts=[r for r in self.workflow() if r.get('eventType')=='message_delivery_recorded' and r.get('message_id')==did and r.get('outcome')=='tmux_injected' and r.get('channel')=='tmux']
            if not receipts:return None
            started=receipts[0]['timestamp']
            reply=assistant_reply([r for rows in self.sessions() for r in rows],marker,started)
            if not reply:return None
            idle=self.attributed()
            return {'projection':projection,'receipts':receipts,'reply':reply,'idle':idle} if idle else None
        value=self.wait(complete,'legacy delivery/read/reply not confirmed',90,owner='mesh')
        assert len(value['receipts'])==1,'duplicate legacy presentation'
        self.save(f'step{self.step}-{marker.split("-")[0]}-legacy-delivery.json',value)
        return value

    def remaining_steps(self):
        # Five ordered items (a)-(e) in the fourth-run ruling, not the older six.
        self.classification='mesh'
        self.a,self.aid=self.send_marker('A')
        self.generation=self.record()['attachmentGeneration']
        self.delivered(self.a,self.aid,self.generation)
        self.save('A-history.json',self.delivery_rows(self.aid))
        self.response('B-work')
        self.classification='mesh'
        self.b,self.bid=self.send_marker('B')
        self.pending(self.b,self.bid)
        assert not any(r.get('payload',{}).get('stage') in ('attempt_started','outcome_unknown','submitted','native_enqueued') for r in self.delivery_rows(self.bid)),'B already begun before rollback'
        self.boundary_snapshot('step1-pending-boundary')
        self.owner_window=True
        self.save('step1-owner-before.json',self.owner_census())
        code,output=self.mesh_raw(['team-daemon','stop'])
        self.save('step1-owner-stop.json',{'exit':code,'output':output})
        assert code==0,'owner stop refused: '+output
        stopped=self.team/'state/delivery/owner-stopped.json'
        self.wait(lambda:stopped.exists() and not self.owner_census()['owners'],'owner did not exit with durable operator marker',90,owner='mesh')
        self.save('step1-owner-stopped.json',self.owner_census())
        self.observe_self_heal_window()
        self.pass_step()

        self.step=2;self.classification='mesh'
        self.format_history=self.journals()
        self.boundary_snapshot('step2-before-format')
        assert not any(r.get('payload',{}).get('stage') in ('attempt_started','submitted','native_enqueued','outcome_unknown') for r in self.delivery_rows(self.bid)),'B no longer pending at format command'
        code,output=self.mesh_raw(['team','format','--legacy','--quiescent'])
        self.save('step2-command.json',{'exit':code,'output':output})
        if code:
            assert any(x in output.lower() for x in ('quiescent_required; exclude all producers and native consumers','quiescent required; member executor ')),'permanent format refusal: '+output
            self.save('step2-temporary-refusal.json',{'reason':output,'B_history':self.delivery_rows(self.bid),'config':self.config()})
            self.wait(self.attributed,'ordinary work did not settle for retry',90,owner='taurhaus')
            code,output=self.mesh_raw(['team','format','--legacy','--quiescent'])
            self.save('step2-retry.json',{'exit':code,'output':output})
            assert code==0,'permanent format refusal after settling: '+output
        after=self.boundary_snapshot('step2-after-format')
        report_path=self.team/'state/messaging-downgrade.json'
        format_verified=hashlib.sha256(report_path.read_bytes()).hexdigest()==self.config().get('messaging_downgrade_sha256')
        rollback_path=self.team/'state/delivery/rollback.json'
        rollback_verified=hashlib.sha256(rollback_path.read_bytes()).hexdigest()==self.config().get('delivery_rollback_sha256')
        authority=json.loads((self.team/'state/messaging-authority.json').read_text())
        projection=self.projection(self.bid)
        rollback=json.loads(rollback_path.read_text())
        downgrade_boundary(self.config(),authority,self.format_history,self.journals(),format_verified,rollback_verified,projection,rollback,self.bid)
        assert not self.tool_exposure(self.b),'B exposed before pending downgrade boundary snapshot'
        self.save('step2-verified-format.json',{'at':time.time(),'reconciliation':mesh_json(output),'report':json.loads(report_path.read_text()),'authority':authority,'format_verified':format_verified,'rollback_verified':rollback_verified,'B':projection,'rollback':rollback})
        assert stopped.exists(),'operator marker missing before same-owner commit'
        self.pass_step()

        self.step=3;self.classification='mesh'
        before=self.boundary_snapshot('step3-before-ownership')
        self.handoff_window=True
        self.log('same_owner_begin')
        code,output=self.mesh_raw(['team','delivery','--owner','members'])
        self.log('same_owner_end',exit=code)
        self.handoff_window=False
        after=self.boundary_snapshot('step3-after-ownership')
        self.save('step3-command.json',{'exit':code,'output':output})
        verified=hashlib.sha256(rollback_path.read_bytes()).hexdigest()==self.config().get('delivery_rollback_sha256')
        same_owner_commit(before,after,code,verified)
        assert self.projection(self.bid),'B missing across marker handoff commit'
        self.save('step3-verified-ownership.json',{'verified':verified,'before_marker':before['owner-stopped'],'after_marker':after['owner-stopped'],'epoch_before':before['epoch'],'epoch_after':after['epoch'],'handoff_before':before['handoff'],'handoff_after':after['handoff'],'B':self.projection(self.bid)})
        self.pass_step()

        self.step=4;self.classification='mesh'
        binary=str(self.root/'home/.local/bin/mesh')
        digest=hashlib.sha256(Path(binary).read_bytes()).hexdigest()
        census=self.owner_census();record=self.record()
        # A process can start before the daemon persists its pid. Observe for the
        # normal 90-second window, never treat this race as permission to duplicate.
        if census['executors'] and not record.get('daemon_pid'):
            self.wait(lambda:self.record().get('daemon_pid'),'executor pid was not attached to runtime',90,owner='taurhaus')
            census=self.owner_census();record=self.record()
        attached=attached_executor(record,census['executors'],binary,digest,TEAM)
        self.save('step4-executor-before.json',{'census':census,'attached':attached,'manual_start_needed':attached is None})
        if attached is None:
            argv=[binary,'daemon','--pane',record['paneId'],'--claude-dir',str(self.root/'claude'),'--team',TEAM,'--name','alpha','--debounce-ms','200']
            self.log('member_executor_start',argv=argv,runtime=record)
            command=shlex.join(argv)+' >'+shlex.quote(str(self.root/'member-executor.log'))+' 2>&1'
            self.run(['tmux','new-window','-d','-t','taurhaus','/bin/bash -c '+shlex.quote(command)])
            self.wait(lambda:(self.team/'daemons/alpha.pid').exists(),'guarded member executor did not start',90,owner='mesh')
        self.legacy_delivery(self.b,self.bid)
        self.c,self.cid=self.send_marker('C')
        self.legacy_delivery(self.c,self.cid)
        for marker,mid in [(self.a,self.aid),(self.b,self.bid),(self.c,self.cid)]:
            p=self.projection(mid)
            legacy=[r for r in self.workflow() if r.get('eventType')=='message_delivery_recorded' and r.get('message_id')==p['id'] and r.get('channel')=='tmux' and r.get('outcome')=='tmux_injected']
            count=sum(r.get('payload',{}).get('stage')=='submitted' for r in self.delivery_rows(mid))+len(legacy)
            assert count==1,f'{marker}: presentation count {count}'
        assert self.record()['attachmentGeneration']==self.generation,'unexpected attachment generation'
        self.save('step4-executor-after.json',self.owner_census())
        self.owner_window=False
        self.boundary_snapshot('step4-fresh-legacy');self.pass_step()

        self.step=5;self.classification='mesh'
        self.read_all('step5-read-pages')
        dispositions={}
        for marker,mid in [(self.a,self.aid),(self.b,self.bid),(self.c,self.cid)]:
            projection=self.projection(mid);assert projection and projection['read'],'explicit read missing'
            ack=self.mesh(['ack',projection['id']],member='alpha')
            status=self.mesh(['ack-status',projection['id'],'--json'],member='alpha')
            dispositions[marker]={'logical_id':mid,'projection':self.projection(mid),'ack':ack,'ack_status':status,'canonical_history':self.delivery_rows(mid),'legacy_history':[r for r in self.workflow() if r.get('message_id')==projection['id']]}
        self.save('step5-dispositions.json',dispositions)
        self.boundary_snapshot('step5-final')
        # The final numbered commit follows export, teardown and survival audit.

    def teardown(self):
        self.stop.set()
        if self.observer:self.observer.join(timeout=10)
        if self.owner_observer:self.owner_observer.join(timeout=10)
        if self.handoff_observer:self.handoff_observer.join(timeout=10)
        self.snapshot()
        with self.evidence_lock:
            identities=list(self.identities_seen.values())
        self.save('identities.json',identities)
        self.budget(observe_only=True)
        for path in (self.root/'codex/sessions').rglob('rollout-*.jsonl'):
            # Scratch-only native records; never export config/database/account rows.
            self.save('sessions/'+path.name,[r for r in complete_rows(path.read_text()) if r.get('type') in ('session_meta','turn_context','event_msg','response_item')])
        self.save('codex-session-inventory.json',[{'path':str(p.relative_to(self.root/'codex')),'bytes':p.stat().st_size} for p in (self.root/'codex').rglob('*') if p.is_file() and (p.suffix=='.jsonl' or p.parent.name in ('thread-writer-locks','shell_snapshots'))])
        self.save('codex-notify.json',self.notify())
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
        for path in (self.root/'data').glob('taurhaus.log*.jsonl'):
            self.save(path.name,retained_daemon_rows(complete_rows(path.read_text())))
        raw=self.root/'member-executor.log'
        if raw.exists():(self.out/'member-executor.txt').write_text(clean(raw.read_text()))
        raw=self.root/'daemon.log'
        if raw.exists():(self.out/'daemon-stderr.txt').write_text(clean(raw.read_text()))
        with socket.socket() as probe:
            probe.settimeout(.2);closed=self.port is None or probe.connect_ex(('127.0.0.1',self.port))!=0
        auth=self.root/'codex/auth.json'
        if auth.exists():auth.unlink()
        auth_removed=not auth.exists();shutil.rmtree(self.root)
        self.save('cleanup.json',{'before':before,'survivors':survivors,'port_closed':closed,'auth_removed':auth_removed,'root_removed':not self.root.exists()})
        self.events.close()
        if survivors or not closed:self.code=2


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--auth-source',required=True)
    parser.add_argument('--out',type=Path,default=BASE/'run');args=parser.parse_args()
    assert Path.cwd()==CHECKOUT, 'wrong checkout'
    os.umask(0o077)
    trial=Trial(args.out)
    def interrupted(sig,frame):raise RuntimeError(f'controller interrupted {sig}')
    signal.signal(signal.SIGINT,interrupted);signal.signal(signal.SIGTERM,interrupted)
    try:
        trial.boot(args.auth_source)
        trial.remaining_steps()
        trial.code=0
    except BaseException as error:
        trial.save(f'step{trial.step}-outcome.json',{'step':trial.step,'outcome':'FAIL','classification':trial.classification,'reason':str(error)})
        trial.log('stopped',step=trial.step,classification=trial.classification,error=str(error))
        print(json.dumps({'step':trial.step,'classification':trial.classification,'error':clean(str(error))}),flush=True)
    finally:
        trial.teardown()
        for step in range(trial.step+1,6):trial.save(f'step{step}-outcome.json',{'step':step,'outcome':'NOT RUN','reason':f'blocked by step {trial.step}'})
        if trial.code==0:
            trial.save('step5-outcome.json',{'step':5,'outcome':'PASS','classification':'S-runtime verified; teardown complete'})
        trial.save('controller-exit.json',{'exit':trial.code,'last_step':trial.step,'runtime_seconds':time.monotonic()-trial.started})
    return trial.code

if __name__=='__main__':raise SystemExit(main())
