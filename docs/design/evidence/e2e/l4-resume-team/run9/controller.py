"""Real L4 ninth run. Run once; write {"step":N} to run9/action.json per checkpoint.

Uses production initialize/stop/resume RPCs. No observer app-server socket, fault
injection or automatic paid retry. The only operator auth read is the authorized
copy of the explicitly authorized auth.json file. Historical preflight-only controller: commit 0267819e.
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
import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from preflight import meter, require_resume_success, require_headroom
from support import copy_native_runtime, sanitize_log_rows, classify_failure, observe_host, reconciled_spend, rollout_usage, retry_busy, seat_process, stopped_backlog, require_alpha_resume

BASE=Path(__file__).resolve().parent
CHECKOUT=Path('/home/mstie/projects/taurhaus-l4-resume-team')
MESH=Path('/home/mstie/projects/mesh-l4')
TEAM='l4-resume'
OUT=BASE


def clean(value):
    if isinstance(value,dict):
        if any(isinstance(value.get(key),str) and value[key].startswith(prefix)
               for key,prefix in [('method','account/'),('event','usage.')]): return None
        return {k:('<signed-read-cursor-redacted>' if k=='cursor' and v else clean(v)) for k,v in value.items() if 'installation' not in k.lower() and k.lower() not in {'auth','accountid','account_id','controlauthtokenhash','accesstoken','refreshtoken','idtoken','access_token','refresh_token','id_token','rate_limits','ratelimits','account_observations'}}
    if isinstance(value,list): return [v for x in value if (v:=clean(x)) is not None]
    if isinstance(value,str):
        if value.lstrip().startswith(('{','[')):
            try: return json.dumps(clean(json.loads(value)))
            except ValueError: pass
        return re.sub(r'(?<![\w/-])'+ '/' + r'home/[^/\s]+/(?!projects/(?:taurhaus-l4-resume-team|mesh-l4)(?:/|\b))[^\s\"\']*','<operator-path-redacted>',value)
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
        self.root=Path(tempfile.mkdtemp(prefix='th-l4-'+OUT.name+'-'))
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
        self.seat_starts=0; self.acceptances={}
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
                if method=='coordination.resume_team': require_resume_success(report)
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
                    if typ in {'task_started','task_complete','token_count'}:
                        retained.append({k:v for k,v in row.items() if k!='payload'}|{'payload':{k:v for k,v in p.items() if k not in {'rate_limits','rateLimits'}}})
                if row.get('type')=='response_item' and p.get('type')=='message': retained.append(row)
            if session:
                self.rollouts[session]=retained
                usage.extend(rollout_usage(rows(path),session,self.meter_diagnostic))
        if self.host_ready:
            view=observe_host(lambda:self.rpc('coordination.hosted_transcript',{'team_name':TEAM,'member_name':'beta'},record=False), lambda error:self.event('host_observation_pending',error=error))
            view=view or self.views.get('beta',{})
            self.views['beta']=view
            for e in view.get('events',[]):
                if e.get('method','').endswith('/delta') or e.get('method','').startswith('account/'): continue
                key=hashlib.sha256(json.dumps(e,sort_keys=True).encode()).hexdigest()
                self.host_events[key]=e
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
        self.ledger.update({'seat_start_reservations':self.seat_starts,'inputs_plus_conservative_starts':self.ledger['paid_inputs']+self.seat_starts,'model':'gpt-5.6-luna','effort':'low','max_inputs':16,'max_usd':.25,'completed_turn_ids':sorted(self.completed),'attachment_generations':{m:self.record(m).get('attachmentGeneration') for m in ['alpha','beta']}})
        save('cost-ledger.json',self.ledger)
        save('spend-reconciliation.json',reconciled_spend(self.ledger))
        save('rollouts.json',self.rollouts)
        save('host-events.json',list(self.host_events.values()))
        facts={'input_cap_verified':self.ledger['paid_inputs']+self.seat_starts<=16,
               'cost_cap_verified':not self.ledger['unmetered'] and self.ledger['api_equivalent_usd']<=.25,
               'runtime_cap_verified':time.monotonic()-self.start_time<=900}
        save('cap-observation.json',facts)
        if not all(facts.values()): self.meter_diagnostic({'kind':'cap_observation',**facts})

    def meter_diagnostic(self, row):
        # Full rollout replays and repeated polls retain one diagnostic per fact.
        key=json.dumps(row,sort_keys=True)
        if key not in self.previous:
            self.previous[key]=True
            self.event('meter_diagnostic',diagnostic=row)

    def allow_input(self, inputs):
        if require_headroom(self.ledger,inputs) and time.monotonic()-self.start_time<=900:
            return True
        self.event('input_refused',inputs=inputs,ledger=self.ledger,
                   reason='next paid input lacks verified cap/headroom; passive observation continues')
        return False

    def wait(self,test,reason,timeout=100):
        assert timeout>=60
        deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            self.observe()
            if test(): return
            time.sleep(1)
        raise AssertionError(reason)

    def settled(self):
        return bool(self.started) and self.started<=self.completed

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
        self.export_daemon_log()
        activity=self.rpc('get_runtime_session_snapshot',{})
        save(label+'-activity.json',activity)

    def alpha_idle(self):
        value=self.rpc('get_runtime_session_snapshot',{},record=False)
        save('alpha-activity.json',value)
        session=self.record('alpha').get('session_id')
        # Snapshot schema is serialized SessionInfo; match the exact native session.
        sessions=value.get('runtime_sessions',[])
        return not value.get('degraded') and any(r.get('session_id')==session and r.get('tmux_pane')==self.record('alpha').get('paneId') and r.get('state')=='idle' and r.get('activity_attribution')=='attributed' for r in sessions)

    def export_daemon_log(self):
        retained=[]
        for path in sorted((self.root/'data').glob('taurhaus.log*.jsonl')):
            retained.extend(sanitize_log_rows(rows(path)))
        with (OUT/'taurhaus.log.jsonl').open('w') as stream:
            for row in retained:
                value=clean(row)
                if value is not None: stream.write(json.dumps(value)+'\n')

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
        if not self.allow_input(1): return False
        marker=f'L4_{phase}_{member}_'+secrets.token_hex(3)
        self.markers[phase,member]=marker
        result=self.mesh(['send',member,'ACTION REQUIRED: Reply exactly '+marker+'. No tools are needed for this marker.','--summary',f'L4 {phase} marker'],f'{phase}-{member}-send')
        if isinstance(result,str): result=next(json.loads(l) for l in result.splitlines() if l.startswith('{'))
        self.message_ids[phase,member]=result['message_id']
        self.acceptances[phase,member]=result
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

    def marker_reply_rows(self, marker):
        return sorted({json.dumps(row,sort_keys=True) for records in self.rollouts.values() for row in records
                       if row.get('type')=='response_item' and row.get('payload',{}).get('role')=='assistant'
                       and marker in json.dumps(row['payload'])})

    def step1(self):
        if not self.allow_input(4): return False
        self.seat_starts+=2
        result=self.operation('coordination.initialize_team',{'request':self.request,'cli_commands':self.commands,'tmux_layout':'new_window'})
        save('initialize-result.json',result)
        self.host_ready=bool(self.record('beta').get('appServer'))
        assert self.host_ready, 'hosted attachment missing after initialize'
        self.wait(lambda:self.settled(),'startup usage/completion did not settle',timeout=120)
        self.wait(self.alpha_idle,'alpha attribution/idle missing',timeout=100)
        self.old={m:self.record(m) for m in ['lead','alpha','beta']}
        save('original-identities.json',self.old)
        self.original_config=json.loads((self.team/'config.json').read_text())
        for member in ['alpha','beta']:
            assert self.old[member].get('session_id'), member+' missing session identity'
            if self.send(member,'old') is False: return False
            marker=self.markers['old',member]
            self.wait(lambda:bool(self.replies(member,marker)) and self.settled(),member+' initial marker missing')
            self.wait(lambda:any(r.get('payload',{}).get('stage') in {'submitted','native_enqueued'} for r in self.receipts('old',member)),member+' completed transport missing')
            self.read_pages(member,'step1-read',mark=True)
            self.wait(lambda:'consumed_by_read' in json.dumps(self.receipts('old',member)),member+' read receipt missing')
        self.old_counts={m:len(self.replies(m,self.markers['old',m])) for m in ['alpha','beta']}
        assert all(v==1 for v in self.old_counts.values()), 'duplicate initial reply'
        self.old_reply_rows={m:self.marker_reply_rows(self.markers['old',m]) for m in ['alpha','beta']}
        self.old_transport_rows={m:[r for r in self.receipts('old',m) if r.get('payload',{}).get('stage') in {'submitted','native_enqueued'}] for m in ['alpha','beta']}

    def step2(self):
        self.before_stop=self.identities(); save('before-stop-processes.json',self.before_stop)
        self.host_ready=False
        for member in ['alpha','beta','lead']:
            record=self.old[member]
            retry_busy(lambda:self.rpc('stop_session',{'tmux_pane':record['paneId'],'cli_tool':'claude' if member=='lead' else 'codex'}), lambda error:self.event('stop_busy_retry',member=member,error=error))
        def stopped():
            panes=subprocess.run(['tmux','list-panes','-a','-F','#{pane_id}'],env=self.env,capture_output=True,text=True).stdout.splitlines()
            original_panes=[r['paneId'] for r in self.old.values()]
            # Include the daemon-owned hosted child, not merely its attached TUI.
            alive=self.identities()
            codex=[p for p in alive if seat_process(p)]
            save('step2-stop-poll.json',{'panes':panes,'codex_processes':codex,'retained_runtime':{m:self.record(m) for m in self.old}})
            return not set(panes)&set(original_panes) and not codex
        self.wait(stopped,'supported stop_session left a recorded pane or Codex/app-server process alive',timeout=100)
        assert (self.team/'config.json').exists() and self.journal_rows(), 'stopped team state lost'

    def step3(self):
        # Scheduler refuses a stopped runtime before constructing a receipt.
        for member in ['alpha','beta']:
            if self.send(member,'pending') is False: return False
            def backlog():
                status=self.mesh(['team-daemon','status'],f'step3-{member}-team-daemon')
                line=next((s for s in status.splitlines() if s.startswith(f'[mesh] delivery {member}: ')), '')
                match=re.search(r' deferred=(.*?) stale=',line)
                health={'member':member,'last_defer_reason':match[1] if match else None}
                records=self.receipts('pending',member)
                result=stopped_backlog(records,self.message_ids['pending',member],member,
                                       self.acceptances['pending',member].get('projection'),health)
                save(f'step3-{member}-backlog.json',{'accepted':self.acceptances['pending',member],
                     'journal':records,'health':health,'status_line':line,'backlogged':result})
                return result
            self.wait(backlog,member+' stopped backlog predicate missing',timeout=100)
            assert not self.replies(member,self.markers['pending',member]), 'presentation while stopped'

    def step4(self):
        # Two recovery inputs plus the two already accepted pending obligations.
        if not self.allow_input(4): return False
        if self.ledger['paid_inputs']+self.seat_starts+6>16:
            self.event('input_refused',reason='input cap including resumed starts'); return False
        self.seat_starts+=2
        self.operation('coordination.resume_team',{'request':{'team_name':TEAM},'cli_commands':self.commands,'tmux_layout':'new_window'})
        self.host_ready=bool(self.record('beta').get('appServer'))

    def step5(self):
        self.wait(lambda:self.settled() and all(self.replies(m,self.markers['pending',m]) for m in ['alpha','beta']),'resumed pending markers missing',timeout=120)
        self.wait(self.alpha_idle,'resumed alpha attribution/idle missing',timeout=100)
        for member,stage in [('alpha','submitted'),('beta','native_enqueued')]:
            self.wait(lambda:any(r.get('payload',{}).get('stage')==stage for r in self.receipts('pending',member)),member+' resumed transport receipt missing',timeout=100)
        new_config=json.loads((self.team/'config.json').read_text())
        assert self.original_config.get('team_incarnation_id'), 'missing original team incarnation'
        for key in ['team_incarnation_id','messaging_format','delivery','messaging']:
            if key in self.original_config: assert new_config.get(key)==self.original_config[key], 'team identity/policy changed: '+key
        choices=lambda c:{m['name']:m.get('adapter_mode','tmux') for m in c['members']}
        assert choices(new_config)==choices(self.original_config), 'adapter choices changed'
        for member in ['lead','alpha','beta']:
            assert int(self.record(member)['attachmentGeneration'])>int(self.old[member]['attachmentGeneration']), member+' attachment not advanced'
        for member in ['alpha','beta']:
            old=self.old[member]; new=self.record(member)
            assert int(new['attachmentGeneration'])>int(old['attachmentGeneration']), member+' attachment not advanced'
            if member=='beta':
                assert new['session_id']==old['session_id'], 'beta thread recovery identity changed'
            else:
                self.export_daemon_log()
                logs=rows(OUT/'taurhaus.log.jsonl')
                launches=[r for r in logs if r.get('event')=='launch.command.rendered' and r.get('member')=='alpha']
                activity=self.rpc('get_runtime_session_snapshot',{})
                save('step5-alpha-identity.json',{'old':old,'new':new,'launch':launches[-1],'activity':activity})
                require_alpha_resume(old,new,launches[-1],activity,logs)
            if member=='beta': assert new['appServer']['threadId']==old['appServer']['threadId']
            else: assert not new.get('appServer')
            assert len(self.replies(member,self.markers['pending',member]))==1, member+' pending marker replayed'
            transport='submitted' if member=='alpha' else 'native_enqueued'
            assert sum(r.get('payload',{}).get('stage')==transport for r in self.receipts('pending',member))==1, member+' duplicate transport receipt'
            assert new.get('tmuxSessionId')==old.get('tmuxSessionId'), member+' tmux session infrastructure changed'
            recovery=new.get('recovery',{})
            save('step5-'+member+'-recovery.json',{'before':old.get('recovery'),'after':recovery})
            # Native context must carry the recovery card; record only new-turn messages.
            delivered=recovery.get('last_delivered') or {}
            key=delivered.get('card_key')
            assert key and key['context']==[int(new['attachmentGeneration']),int(new['contextGeneration'])], member+' recovery receipt generation mismatch'
            cards=[]
            for row in self.rollouts.get(new['session_id'],[]):
                payload=row.get('payload',{})
                if payload.get('role')!='user': continue
                text=''.join(c.get('text','') for c in payload.get('content',[]))
                if '[taurhaus] recovery_card' not in text: continue
                for line in text.splitlines():
                    if line.startswith('Identity: ') and '; key=' in line:
                        try: found=json.loads(line.split('; key=',1)[1])
                        except ValueError: continue
                        if found==key: cards.append(row)
            assert len(cards)==1, member+' needs exactly one generation-bound recovery card in native context'

    def step6(self):
        for member in ['alpha','beta']:
            self.read_pages(member,'step6-read',mark=True)
            self.wait(lambda:'consumed_by_read' in json.dumps(self.receipts('pending',member)),member+' pending read receipt missing')
            assert self.marker_reply_rows(self.markers['old',member])==self.old_reply_rows[member], 'completed old message replayed'
            assert [r for r in self.receipts('old',member) if r.get('payload',{}).get('stage') in {'submitted','native_enqueued'}]==self.old_transport_rows[member], 'old transport replayed'
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
        source=Path('/home')/'mstie'/'.codex-account-b'/'auth.json'
        assert source.is_file() and not source.is_symlink(), 'authorized auth source is not a regular file'
        shutil.copyfile(source,self.root/'codex/auth.json'); (self.root/'codex/auth.json').chmod(0o600)
        copy_native_runtime(shutil.which('codex'),self.bin)
        for name in ['codex','codex-code-mode-host']:
            with (self.bin/name).open('rb') as f: digest=hashlib.file_digest(f,'sha256').hexdigest()
            self.event('binary',name=name,sha256=digest)
        for name,source in [('claude',Path(shutil.which('claude')).resolve()),('mesh',MESH/'target/debug/mesh'),('taurhaus-daemon',CHECKOUT/'src-tauri/target/release/taurhaus-daemon')]:
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
        capabilities=self.mesh(['delivery','capabilities'],'delivery-capabilities')
        descriptors=capabilities if isinstance(capabilities,list) else capabilities.get('native_descriptors',[])
        assert any(d.get('adapter')=='app_server' and d.get('build')=='0.153.4' and d.get('enabled') is True for d in descriptors), 'shipped 0.153.4 app_server descriptor not enabled'
        source=(CHECKOUT/'src/lib/components/meshTabUtils.js').read_text()
        policy=json.loads(re.search(r'DEFAULT_CANONICAL_POLICY = Object.freeze\((\{.*?\})\)',source,re.S)[1])
        self.commands={'codex':{'fresh':'codex --sandbox danger-full-access --ask-for-approval never','continue_cmd':'codex --sandbox danger-full-access --ask-for-approval never','resume':'codex --sandbox danger-full-access --ask-for-approval never resume'}}
        self.request={'team_name':TEAM,'team_description':'L4 bounded whole-team stop/resume','lead_mode':'launch_new',
                      'lead':{'name':'lead','cli_tool':'claude','model':'claude-haiku-4-5','project_id':str(self.root/'project')},
                      'agents':[{'name':name,'cli_tool':'codex','model':'gpt-5.6-luna','reasoning_effort':'low','delivery':delivery,'project_id':str(self.root/'project'),'instructions':instructions} for name,delivery in [('alpha','tmux'),('beta','app_server')]],
                      'messaging':{'mode':'canonical','retentionPolicy':policy}}
        save('policy.json',policy)

    def warm_codex_home(self):
        # // Regression: 4a4c65d8 initialized two Codex seats against a cold SQLite home.
        # One startup reservation, no prompt or model mutation; same private namespace.
        self.seat_starts+=1
        status=self.root/'warmup-exit'
        launch='codex --sandbox danger-full-access --ask-for-approval never; code=$?; printf "%s" "$code" >'+shlex.quote(str(status))
        pane=self.command(['tmux','new-window','-d','-P','-F','#{pane_id}','-t','taurhaus',
                           '/bin/bash -c '+shlex.quote(launch)],'warmup-launch').strip()
        assert re.fullmatch(r'%\d+',pane), 'harness warm-up pane identity missing'
        def composer():
            output=self.command(['tmux','capture-pane','-p','-t',pane],'warmup-composer',check=False)
            return 'OpenAI Codex' in output and any(line.startswith('› ') for line in output.splitlines())
        self.wait(composer,'harness warm-up composer missing',timeout=100)
        self.command(['tmux','send-keys','-t',pane,'/quit','Enter'],'warmup-quit')
        self.wait(status.exists,'harness warm-up exit missing',timeout=100)
        code=int(status.read_text())
        assert code==0, 'harness warm-up exit was '+str(code)
        sqlite=[]
        for path in sorted((self.root/'codex').glob('*.sqlite')):
            with path.open('rb') as stream:
                if stream.read(16)==b'SQLite format 3\0': sqlite.append(path.name)
        assert sqlite, 'harness warm-up SQLite database missing'
        self.wait(lambda:not any(seat_process(p) for p in self.identities()),
                  'harness warm-up process survived clean exit',timeout=100)
        self.observe()
        save('warmup.json',{'pane':pane,'composer_seen':True,'exit_code':code,
             'sqlite_files':sqlite,'input_reservation':1,'startup_turn_ids':sorted(self.started),
             'startup_cost_usd':None,'cost_note':'Startup reserved as one input; any unreported startup cost is unknown, not zero.',
             'remaining_seat_processes':[]})

    def cleanup(self):
        # Observation never submits a new model input. Drain already started usage.
        if self.record('beta').get('appServer'): self.host_ready=True
        end=time.monotonic()+60
        while time.monotonic()<end:
            try:
                self.observe()
                if (self.settled() and not self.ledger['unmetered']) or not self.started: break
            except Exception as e: self.meter_diagnostic({'kind':'cleanup_observation','error':str(e)})
            time.sleep(1)
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
        self.export_daemon_log()
        shutil.rmtree(self.root)
        mesh_diff=subprocess.run(['git','-C',str(MESH),'diff','--exit-code'],capture_output=True)
        result={'survivors':survivors,'port_closed':closed,'root_removed':not self.root.exists(),'mesh_source_unchanged':mesh_diff.returncode==0}
        save('cleanup.json',result); self.event('cleanup',**result)
        assert not survivors and closed and mesh_diff.returncode==0, 'cleanup verification failed'



def main():
    assert Path.cwd()==CHECKOUT
    assert not (OUT/'events.jsonl').exists(), OUT.name+' already executed; no paid retry'
    os.umask(0o077)
    for sig in [signal.SIGTERM,signal.SIGINT]: signal.signal(sig,lambda s,f: (_ for _ in ()).throw(RuntimeError('controller interrupted')))
    lane=Lane(); code=1
    try:
        lane.setup()
        lane.warm_codex_home()
        for step in range(1,7):
            lane.step=step
            if step>1:
                deadline=time.monotonic()+180
                while not (OUT/'action.json').exists() and time.monotonic()<deadline:
                    lane.observe(); time.sleep(1)
                action=json.loads((OUT/'action.json').read_text()); (OUT/'action.json').unlink()
                assert action=={'step':step}, 'out-of-order action'
            if getattr(lane,'step'+str(step))() is False:
                save(f'step{step}-outcome.json',{'step':step,'outcome':'NOT RUN','classification':'harness',
                     'reason':'next paid input refused by recorded cap/headroom facts'})
                break
            lane.snapshot('step'+str(step)); lane.observe()
            save('step'+str(step)+'-outcome.json',{'step':step,'outcome':'PASS','classification':'runtime','at':time.time()})
            lane.event('checkpoint',step_completed=step,ledger=lane.ledger)
        else: code=0
    except BaseException as e:
        reason=clean(str(e))
        classification=classify_failure(reason)
        save('step'+str(lane.step)+'-outcome.json',{'step':lane.step,'outcome':'FAIL','classification':classification,'reason':reason})
        lane.event('failure',reason=reason,classification=classification)
        save('controller-error.json',{'traceback':traceback.format_exc()})
    finally:
        lane.cleanup()
        for step in range(lane.step+1,7):
            save(f'step{step}-outcome.json',{'step':step,'outcome':'NOT RUN','classification':'not evaluated','reason':f'stop after step {lane.step}'})
    return code

if __name__=='__main__': raise SystemExit(main())
