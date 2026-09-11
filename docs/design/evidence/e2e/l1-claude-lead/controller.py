"""Lane-1 runtime controller. Explicit credentials arguments; no fallback.

Controller input is serialized through input(); initialize reserves onboarding.
Transcript rows meter usage only; generated/native rows are not controller inputs.
Steps are operator-controller actions, never automatically retried. No product edits.
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
import time
from controller_support import complete_rows, sanitize, claude_usage, admit_input
from continuation_support import cumulative, compact_hook_registered, codex_bundle, submit_input, installed_codex_bundle, codex_submitted, opportunity_deadline

CHECKOUT=Path('/home/mstie/projects/taurhaus-l1-claude-lead')
MESH=Path('/home/mstie/projects/mesh-l1')
BASE=Path(__file__).resolve().parent
TEAM='l1-claude-lead'
TAURHAUS_BASE='a7e6db7e'
MESH_COMMIT='310144d'

def valid_evidence_label(label):
    return bool(re.fullmatch(r'[a-z0-9-]+', label)) or label == 'run6/continuation'

class Trial:
    def __init__(self,args):
        self.args=args; self.children=[]; self.started=time.monotonic()-args.prior_runtime_seconds; self.step=1
        self.inputs={'claude':0,'codex':0}; self.root=None; self.port=None; self.seen={}; self.count=0
        self.prior=json.loads(args.prior_ledger.read_text())
        if 'cap_inputs' in self.prior:
            self.prior['controller_inputs']=dict(self.prior['cap_inputs'])
        self.out=BASE/args.evidence_label; self.out.mkdir(exist_ok=True)
        assert not (self.out/'events.jsonl').exists(), 'never overwrite a runtime'
        self.outer_deadline=self.started+900
        self.result={'outcome':'UNAVAILABLE','classification':'harness','step':1}
    def save(self,name,value):
        text=value if isinstance(value,str) else json.dumps(sanitize(value),indent=2)+'\n'
        text=sanitize(text); digest=hashlib.sha256(text.encode()).hexdigest()
        if self.seen.get(name)==digest: return
        self.seen[name]=digest
        path=self.out/name; path.parent.mkdir(parents=True,exist_ok=True); temporary=path.with_suffix(path.suffix+'.tmp'); temporary.write_text(text); temporary.replace(path)
    def event(self,kind,**value):
        row=sanitize({'at':time.time(),'step':self.step,'kind':kind,**value})
        with (self.out/'events.jsonl').open('a') as f: f.write(json.dumps(row)+'\n')
    def run(self,argv,check=True):
        child=subprocess.Popen(argv,env=self.env,cwd=self.root/'project',stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=True)
        self.children.append(child)
        data=child.communicate(timeout=60)[0].decode(errors='replace')
        self.event('command',argv=argv,exit=child.returncode,output='\n'.join(data.splitlines()[-60:]))
        if check and child.returncode: raise RuntimeError(f'command exit {child.returncode}: {data[-1500:]}')
        return data
    def rpc(self,method,params,record=True):
        request={'id':secrets.token_hex(8),'method':method,'params':params}
        # Log before adding the private RPC token.
        if record: self.event('rpc_request',request=request)
        request['auth']=(self.root/'data/daemon.token').read_text().strip()
        with socket.create_connection(('127.0.0.1',self.port),timeout=15) as sock:
            sock.sendall(json.dumps(request).encode()+b'\n'); stream=sock.makefile('rb')
            while True:
                response=json.loads(stream.readline())
                if response.get('id')==request['id']: break
        if record: self.event('rpc_response',response=response)
        if 'error' in response: raise RuntimeError(str(response['error']))
        return response['result']
    def rows(self,glob):
        return [r for p in self.root.glob(glob) for r in complete_rows(p.read_text())]
    def ledger(self):
        c=claude_usage(self.rows('claude/projects/**/*.jsonl'))
        turns={}; totals={}
        for path in (self.root/'codex').rglob('rollout-*.jsonl'):
            for row in complete_rows(path.read_text()):
                p=row.get('payload',{})
                if row.get('type')!='event_msg': continue
                if p.get('type')=='task_started': turns[p['turn_id']]={'turn_id':p['turn_id'],'rollout':path.name}
                if p.get('type')=='token_count' and p.get('info'):
                    usage=p['info'].get('total_token_usage',{})
                    prev=totals.get(path.name,{})
                    totals[path.name]={k:max(v,prev.get(k,0)) for k,v in usage.items() if isinstance(v,(int,float))}
        usd=sum(x['usd'] for x in c); upper=sum(x['upper_usd'] for x in c)
        for u in totals.values():
            usd+=(max(0,u.get('input_tokens',0)-u.get('cached_input_tokens',0))*.2+u.get('cached_input_tokens',0)*.02+u.get('output_tokens',0)*1.2)/1e6
            upper+=(u.get('input_tokens',0)+u.get('output_tokens',0))*1.2/1e6
        value={'controller_inputs':self.inputs,'claude_generations':c,'codex_turns':list(turns.values()),'codex_usage_by_rollout':totals,'seat_usd_estimate':usd,'seat_usd_upper_rate':upper,'rates_per_million':{'claude':[1,.1,1.25,5],'codex':[.2,.02,1.2]},'metering_complete':len(totals)>=len({t['rollout'] for t in turns.values()}),'prior_preflight_spend_usd':0}
        value.update(cumulative(self.prior,self.inputs,usd,upper))
        value['current_inputs']=dict(self.inputs)
        value['prior_trial']=self.prior
        # Group Bash continuations under their native/controller input.
        groups={}; group='startup'; message_groups={}
        for row in self.rows('claude/projects/**/*.jsonl'):
            if row.get('type')=='user' and isinstance(row.get('message',{}).get('content'),str):
                group=row.get('uuid',group)
            if row.get('type')=='assistant': message_groups[row.get('message',{}).get('id')]=group
        for item in c:
            key=message_groups[item['message_id']]
            groups[key]=groups.get(key,0)+item['usd']
        value['claude_usd_by_input']=groups
        value['claude_cap_times_max_observed_input_usd']=8*max(groups.values(),default=0)
        self.save('cost-ledger.json',value)
        if value['seat_usd_upper_rate']>2 or value['controller_inputs']['claude']>8 or value['controller_inputs']['codex']>12 or len(turns)+self.prior['controller_inputs']['codex']>12: raise RuntimeError('seat budget exceeded; stop')
        return value
    def identities(self):
        found=[]
        if not self.root: return found
        for proc in Path('/proc').iterdir():
            if not proc.name.isdigit(): continue
            try:
                if (b'TAURHAUS_TRIAL_ID='+self.root.name.encode()+b'\0') not in (proc/'environ').read_bytes(): continue
                stat=(proc/'stat').read_text().rsplit(')',1)[1].split()
                found.append({'pid':int(proc.name),'start_ticks':stat[19],'argv':(proc/'cmdline').read_bytes().decode(errors='replace').split('\0')})
            except (FileNotFoundError,ProcessLookupError,PermissionError): pass
        return found
    def retain_logs(self):
        logs=[]
        for p in (self.root/'data').glob('*.jsonl'):
            rows=complete_rows(p.read_text())
            retained=[r for r in rows if not str(r.get('event','')).startswith(('usage.','account.'))]
            logs += retained
            self.save(p.name, ''.join(json.dumps(sanitize(r))+'\n' for r in retained))
            self.save('log-retention.json', {'complete_rows':len(rows),'retained_rows':len(retained),'excluded_usage_rows':len(rows)-len(retained)})
        return logs
    def snapshot(self,label='latest'):
        if not self.root: return
        from run9_support import retain_registry
        retain_registry(self.root,self.save)
        team=self.root/'claude/teams'/TEAM
        for pattern in ['config.json','runtime/*.json','state/**/*.json','state/**/*.jsonl','inboxes/*.json']:
            for path in team.glob(pattern):
                if 'control_auth' in path.name: continue
                try: value=json.loads(path.read_text()) if path.suffix=='.json' else complete_rows(path.read_text())
                except (OSError,ValueError): continue
                self.save(label+'/team/'+str(path.relative_to(team)),value)
        self.save(label+'/lead-projection.json',{'exists':(team/'inboxes/lead.json').exists(),'rows':json.loads((team/'inboxes/lead.json').read_text()) if (team/'inboxes/lead.json').exists() else None})
        for p in (self.root/'claude').glob('settings.json'):
            self.save(label+'/claude-hooks.json',json.loads(p.read_text()).get('hooks',{}))
        transcripts=[]
        for r in self.rows('claude/projects/**/*.jsonl'):
            transcripts.append(r)  # Native hook context can occur in any transcript row kind.
        self.save('claude-transcript.json',transcripts)
        self.save('codex-transcript.json',[r for r in self.rows('codex/**/rollout-*.jsonl') if r.get('type') in ('session_meta','response_item','event_msg','turn_context')])
        logs=self.retain_logs()
        self.save('daemon-events.json',logs)
        if (self.root/'daemon.log').exists():
            self.save('daemon-tail.txt','\n'.join((self.root/'daemon.log').read_text().splitlines()[-60:]))
        self.save('identities.json',self.identities()); self.ledger()
        daemon_alive=False
        if (self.root/'data/daemon.token').exists():
            try:
                self.save('runtime-session-snapshot.json',self.rpc('get_runtime_session_snapshot',{},record=False))
                daemon_alive=True
            except (OSError,ValueError,RuntimeError) as error: self.event('snapshot_observation_error',error=str(error))
        def current_json(path):
            try: return json.loads(path.read_text())
            except (OSError, ValueError): return None
        from controller_support import onboarding_facts
        record=current_json(team/'runtime/alpha.json')
        activity=current_json(team/'state/activity/alpha.json')
        journal=[r for path in (team/'state/messaging-v2/segments').glob('*.jsonl')
                 for r in complete_rows(path.read_text())]
        facts=onboarding_facts(record, activity, self.rows('codex/**/rollout-*.jsonl'),
                               journal, daemon_alive, time.time())
        self.save(label+'/onboarding-health.json', facts)
        # Passive locks only; never acquire any product lock.
        self.save('locks.txt','\n'.join(l for l in Path('/proc/locks').read_text().splitlines() if any(str(i['pid']) in l.split() for i in self.identities())))
    def capture(self,label):
        listing=self.run(['tmux','list-panes','-a','-F','#{pane_id} #{pane_current_command}'],False)
        self.save(label+'/panes.txt',listing)
        for line in listing.splitlines():
            pane=line.split()[0]
            if not pane.startswith('%'): continue
            text=self.run(['tmux','capture-pane','-p','-S','-12','-t',pane],False)
            self.save(label+'/pane-'+pane[1:]+'.txt','\n'.join(text.splitlines()[-60:])+'\n')
        self.snapshot(label)
    def wait_for_alpha_delivery(self):
        from run8_guard import delivery_input_facts, passive_terminal_lock
        end = opportunity_deadline(time.monotonic(), self.outer_deadline, 125)
        team = self.root/'claude/teams'/TEAM
        while True:
            self.snapshot()
            def read(path):
                try: return json.loads(path.read_text())
                except (OSError, ValueError): return {}
            journal = [r for path in (team/'state/messaging-v2/segments').glob('*.jsonl')
                       for r in complete_rows(path.read_text())]
            record = read(team/'runtime/alpha.json')
            rollout = Path(record.get('jsonl_path') or '/nonexistent')
            rows = complete_rows(rollout.read_text()) if rollout.is_relative_to(self.root/'codex') and rollout.is_file() else []
            sample = passive_terminal_lock(team/'state/terminal/alpha.lock', self.identities())
            facts = delivery_input_facts(record, read(team/'state/activity/alpha.json'), journal,
                                         rows, sample, time.time())
            self.save(f'step{self.step}-input-guard.json', facts)
            self.event('input_guard_observation', facts=facts)
            if facts['ready']: return
            if time.monotonic() >= end:
                raise TimeoutError('alpha pending-delivery guard: onboarding submitted/read, no pending delivery, fresh idle and free terminal lock not established in 125s; no input sent')
            time.sleep(1)

    def input(self,tool,pane,text):
        if tool == 'codex': self.wait_for_alpha_delivery()
        ledger=self.ledger()
        # Before the first metered turn reserve $0.50; thereafter use the larger
        # of that reserve and the most expensive observed generation at upper rates.
        reserve=max([.5]+[x['upper_usd'] for x in ledger['claude_generations']])
        observed_bound=ledger['claude_cap_times_max_observed_input_usd']+self.prior['seat_usd_upper_rate']
        if observed_bound+reserve>2: raise ValueError('input cap times observed turn cost leaves no dollar headroom')
        if not ledger['metering_complete']: raise ValueError('unverified metering before next input')
        admit_input(tool,ledger['controller_inputs']['claude'],ledger['controller_inputs']['codex'],ledger['seat_usd_upper_rate'],reserve)
        self.inputs[tool]+=1; self.event('controller_input',tool=tool,pane=pane,text=text,count=self.inputs[tool],reserve_usd=reserve)
        self.ledger()
        prior_turns={r.get('payload',{}).get('turn_id') for r in self.rows('codex/**/rollout-*.jsonl') if r.get('payload',{}).get('type')=='task_started'}
        # Allow confirmation AND its following full delivery opportunity.
        opportunity_deadline(time.monotonic(),self.outer_deadline,130)
        submit_input(self.run,time.sleep,pane,text)
        if tool=='codex':
            end=opportunity_deadline(time.monotonic(),self.outer_deadline,65)
            submission=f'submission-step{self.step}-{self.inputs[tool]}'
            while time.monotonic()<end:
                pane_text=self.run(['tmux','capture-pane','-p','-t',pane],False)
                rows=self.rows('codex/**/rollout-*.jsonl')
                self.save(submission+'-pane.txt','\n'.join(pane_text.splitlines()[-60:]))
                if codex_submitted(pane_text,rows,prior_turns):
                    deadline=time.monotonic()+65
                    available=deadline<=self.outer_deadline
                    # Preserve the paid confirmation even if observation used the headroom.
                    self.save(submission+'-confirmation.json',{'confirmed_at':time.time(),'opportunity_deadline':deadline,'opportunity_available':available})
                    self.event('codex_submission_confirmed',pane=pane,composer_empty=True,new_turn_started=True,opportunity_seconds=65,opportunity_available=available,evidence=submission)
                    if not available: raise ValueError('outer deadline cannot provide full opportunity')
                    break
                self.snapshot(); time.sleep(1)
            else: raise RuntimeError('harness: Codex submission not confirmed; no paid retry')
    def mesh(self,argv,label):
        # Run in the existing owned namespace so Mesh sees the published PIDs.
        token=secrets.token_hex(5); output=self.root/(token+'.out'); code=self.root/(token+'.exit')
        command=shlex.join([str(self.bin/'mesh')]+argv+['--claude-dir',str(self.root/'claude'),'--team',TEAM,'--name','lead'])
        command+=' >'+shlex.quote(str(output))+' 2>&1; echo $? >'+shlex.quote(str(code))
        self.run(['tmux','new-window','-d','-t','taurhaus','/bin/bash -c '+shlex.quote(command)])
        end=time.monotonic()+60
        while not code.exists():
            if time.monotonic()>end: raise TimeoutError('Mesh observation command')
            time.sleep(.1)
        self.save(label+'.json',{'argv':argv,'exit':int(code.read_text()),'output':output.read_text()})
    def setup(self):
        # Validate the complete command-capable runtime before credentials or seats.
        codex_files=installed_codex_bundle(shutil.which)
        assert codex_files[0]==self.args.codex_binary.resolve(), 'explicit Codex binary differs from installed bundle'
        assert subprocess.check_output(['git','-C',str(MESH),'rev-parse','--short','HEAD'],text=True).strip()==MESH_COMMIT
        subprocess.run(['git','merge-base','--is-ancestor',TAURHAUS_BASE,'HEAD'],cwd=CHECKOUT,check=True)
        if self.prior['controller_inputs']['claude']+6>8:
            raise ValueError('remaining Claude input cap cannot cover restart plus steps 2–6 (minimum six inputs)')
        os.umask(0o077)
        self.root=Path(tempfile.mkdtemp(prefix='th-l1-runtime-'))
        for d in ['home/.local/bin','claude','codex','grok','gemini','data','tmp','tmux','project','cache','config']: (self.root/d).mkdir(parents=True,exist_ok=True)
        self.bin=self.root/'home/.local/bin'
        self.env={'HOME':str(self.root/'home'),'PATH':f'{self.bin}:/usr/bin:/bin','SHELL':'/bin/bash','LANG':'C.UTF-8','TERM':'xterm-256color','RUST_LOG':'info','TAURHAUS_TRIAL_ID':self.root.name,'DISABLE_AUTOUPDATER':'1','CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC':'1','CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS':'1','CLAUDE_CODE_SKIP_DANGEROUS_MODE_PERMISSION_PROMPT':'1'}
        for key,d in {'CLAUDE_CONFIG_DIR':'claude','CLAUDE_DIR':'claude','TAURHAUS_CLAUDE_DIR':'claude','CODEX_HOME':'codex','GROK_HOME':'grok','GEMINI_CLI_HOME':'gemini','TAURHAUS_AGY_DIR':'gemini','TAURHAUS_DATA_DIR':'data','TMPDIR':'tmp','TMUX_TMPDIR':'tmux','XDG_CONFIG_HOME':'config','XDG_CACHE_HOME':'cache','XDG_DATA_HOME':'data'}.items(): self.env[key]=str(self.root/d)
        for p in [self.args.claude_credential,self.args.codex_auth]:
            if not p.is_file() or p.is_symlink(): raise RuntimeError('explicit credential source is not regular')
        # Only this Codex file is copied; the Claude file is only a mount source.
        shutil.copyfile(self.args.codex_auth,self.root/'codex/auth.json'); (self.root/'codex/auth.json').chmod(0o600)
        (self.root/'claude/.credentials.json').touch()
        for name,source in [('claude',self.args.claude_binary),*[(p.name,p) for p in codex_files],('mesh',MESH/'target/debug/mesh'),('taurhaus-daemon',CHECKOUT/'src-tauri/target/release/taurhaus-daemon')]:
            shutil.copyfile(source,self.bin/name); (self.bin/name).chmod(0o700)
            self.event('binary',name=name,sha256=hashlib.sha256((self.bin/name).read_bytes()).hexdigest())
        for name in ['agy','grok','gemini']:
            (self.bin/name).write_text('#!/bin/sh\nexit 77\n'); (self.bin/name).chmod(0o700)
        for rc in ['.bashrc','.profile','.zshrc']: (self.root/'home'/rc).write_text('export PATH="$HOME/.local/bin:/usr/bin:/bin"\n')
        instructions='Isolated bounded messaging trial. Reply in at most two short sentences. Never modify files. Only execute mesh who, mesh send, mesh read, or the exact mesh lifecycle commands explicitly named in a task. Every mesh command must include --claude-dir "$CLAUDE_CONFIG_DIR" --team l1-claude-lead --name YOUR_MEMBER_NAME. Never use other tools or commands. Lead must never execute mesh read until the controller explicitly requests step 4. When asked for a reply, read/mark your own inbox then send the short reply through mesh. Do not send unsolicited messages. Startup: say READY only, do not send any wait-state or status message to yourself or anyone. On native teammate replies, distinguish each marker in a brief text response without tools. These trial limits override generated startup suggestions to send wait-state messages.'
        (self.root/'project/AGENTS.md').write_text(instructions+'\n')
        (self.root/'project/CLAUDE.md').write_text(instructions.replace('YOUR_MEMBER_NAME','lead')+'\n')
        self.run(['git','init','-q']); self.run(['git','add','AGENTS.md','CLAUDE.md']); self.run(['git','-c','user.name=Trial','-c','user.email=trial@example.invalid','commit','-qm','scratch instructions'])
        (self.root/'claude/.claude.json').write_text(json.dumps({'hasCompletedOnboarding':True,'theme':'dark','bypassPermissionsModeAccepted':True,'projects':{str(self.root/'project'):{'hasTrustDialogAccepted':True,'hasCompletedProjectOnboarding':True}}}))
        (self.root/'codex/config.toml').write_text('model="gpt-5.6-luna"\nmodel_reasoning_effort="low"\napproval_policy="never"\nsandbox_mode="danger-full-access"\nweb_search="disabled"\n[projects.'+json.dumps(str(self.root/'project'))+']\ntrust_level="trusted"\n')
        with socket.socket() as probe:
            probe.bind(('127.0.0.1',0)); self.port=probe.getsockname()[1]
        assert self.port!=17233
        self.env['TAURHAUS_DAEMON_PORT']=str(self.port)
        from run9_support import launch_wrapper
        self.bw=launch_wrapper(self.root,self.args.claude_credential)
        self.save('isolation.json',{'env':self.env,'wrapper':self.bw,'claude_credential_rule':'single file read-only mount, never copied','codex_copy_mode':oct((self.root/'codex/auth.json').stat().st_mode & 0o777),'descriptor':'unchanged production','mesh_commit':MESH_COMMIT,'taurhaus_base':TAURHAUS_BASE,'controller_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=CHECKOUT,text=True).strip()})
        for name in ['claude','codex','mesh']: self.run(self.bw+[str(self.bin/name),'--version'])
        shutil.copyfile(CHECKOUT/'.check-logs/l1-install-hook',self.bin/'install-hook'); (self.bin/'install-hook').chmod(0o700)
        self.run(self.bw+[str(self.bin/'install-hook'),str(self.root/'claude/teams'),str(self.bin/'taurhaus-daemon')])
        hooks=json.loads((self.root/'claude/settings.json').read_text()).get('hooks',{})
        self.save('production-hooks-before-launch.json',hooks)
        assert compact_hook_registered(hooks), 'production installer did not register compact hook'
        boot=self.root/'boot.sh'; boot.write_text('#!/bin/bash\nset -eu\ntmux -D -f /dev/null &\nfor i in {1..100}; do test -S "$TMUX_TMPDIR/tmux-$(id -u)/default" && break; sleep .1; done\ntmux set-option -g default-shell /bin/bash\ntmux new-session -d -s taurhaus -x 140 -y 48 /bin/bash\n'+shlex.join([str(self.bin/'taurhaus-daemon'),'--port',str(self.port),'--data-dir',str(self.root/'data')])+' &\nwait $!\n')
        self.save('boot.sh',boot.read_text()); self.event('spawn',argv=self.bw+['/bin/bash',str(boot)])
        with (self.root/'daemon.log').open('w') as f:
            child=subprocess.Popen(self.bw+['/bin/bash',str(boot)],env=self.env,stdout=f,stderr=subprocess.STDOUT,start_new_session=True)
        self.children.append(child)
        end=time.monotonic()+60
        while not (self.root/'data/daemon.token').exists():
            if child.poll() is not None or time.monotonic()>end: raise RuntimeError('private daemon unavailable')
            time.sleep(.25)
        ping=self.rpc('ping',{}); self.save('ping.json',ping)
        policy=json.loads(re.search(r'DEFAULT_CANONICAL_POLICY = Object.freeze\((\{.*?\})\)',(CHECKOUT/'src/lib/components/meshTabUtils.js').read_text(),re.S)[1])
        lead={'name':'lead','cli_tool':'claude','model':'claude-haiku-4-5-20251001','delivery':'tmux','project_id':str(self.root/'project'),'instructions':instructions.replace('YOUR_MEMBER_NAME','lead')}
        alpha={'name':'alpha','cli_tool':'codex','model':'gpt-5.6-luna','reasoning_effort':'low','delivery':'tmux','project_id':str(self.root/'project'),'instructions':instructions.replace('YOUR_MEMBER_NAME','alpha')}
        claude='claude --dangerously-skip-permissions --tools Bash --allowedTools "Bash(mesh *)"'
        commands={name:{'fresh':base,'continue_cmd':base,'resume':base+' resume'} for name,base in [('claude',claude),('codex','codex --yolo')]}
        request={'request':{'team_name':TEAM,'team_description':'Lane 1 native lead trial','lead_mode':'launch_new','lead':lead,'agents':[alpha],'messaging':{'mode':'canonical','retentionPolicy':policy}},'cli_commands':commands,'tmux_layout':'new_window'}
        # Reserve both production onboarding starts before the sole initialize call.
        self.inputs={'claude':1,'codex':1}; self.event('onboarding_reservation',inputs=self.inputs)
        self.save('initialize-request.json',request)
        accepted=self.rpc('coordination.initialize_team',request); self.save('initialize-accepted.json',accepted)
        end=time.monotonic()+120
        while time.monotonic()<end:
            status=self.rpc('coordination.initialize_status',{'run_id':accepted['run_id']})
            if status['outcome']['status']!='running': break
            self.ledger(); time.sleep(1)
        self.save('initialize-result.json',status); self.capture('initialize')
        if status['outcome']['status']!='completed' or status['outcome']['report'].get('failed_step'): raise RuntimeError('production initialize failed')
        self.event('ready_for_controller_actions',root=str(self.root))
    def loop(self):
        while time.monotonic()<self.outer_deadline:
            self.snapshot()
            path=self.out/'action.json'
            if not path.exists(): time.sleep(1); continue
            action=json.loads(path.read_text()); path.unlink(); self.step=action.get('step',self.step)
            self.event('action',action=action)
            op=action['op']
            if op=='mesh':
                self.mesh(action['argv'],action['label'])
            elif op=='publish_assignment': self.publish_assignment(action)
            elif op=='wait_alpha_delivery': self.wait_for_alpha_delivery()
            elif op=='input':
                if action.get('recipient_inputs'):
                    self.inputs['codex']+=action['recipient_inputs']
                    assert self.ledger()['controller_inputs']['codex']<=12, 'recipient input cap'
                self.input(action['tool'],action['pane'],action['text'])
            elif op=='capture': self.capture(action['label'])
            elif op=='window':
                seconds=action.get('seconds',65)
                deadline=opportunity_deadline(time.monotonic(),self.outer_deadline,seconds)
                self.save(action['label']+'.json',{'started_at':time.time(),'seconds':seconds,'remaining_outer_seconds':self.outer_deadline-time.monotonic(),'monotonic_deadline':deadline})
            elif op=='pass':
                self.capture('step'+str(self.step)); self.save('step'+str(self.step)+'-outcome.json',{'outcome':'PASS','step':self.step,'evidence':action['evidence']})
            elif op=='stop':
                self.result=action['result']; self.save('step'+str(self.step)+'-outcome.json',self.result); return
            elif op=='ui':
                # Non-model setup keys are still counted in the hard input ceiling.
                if action['tool'] == 'codex': self.wait_for_alpha_delivery()
                self.inputs[action['tool']]+=1
                if self.inputs[action['tool']] > (8 if action['tool']=='claude' else 12): raise ValueError('UI input cap')
                self.run(['tmux','send-keys','-t',action['pane']]+action['keys'])
            else: raise ValueError('unknown action')
            self.event('action_done',op=op)
        raise RuntimeError('15 minute runtime deadline')
    def publish_assignment(self, action):
        """Play only the app's production publication call, from persisted Mesh facts."""
        from datetime import datetime, timezone
        task = json.loads((self.root/'claude/tasks'/TEAM/(action['task_id']+'.json')).read_text())
        assert task['metadata']['assignment_id'] == action['assignment_id']
        assert task['owner'] == 'lead' and task['status'] == 'in_progress'
        labels = {'Execution mode': 'execution_mode',
                  'File-ownership boundary': 'file_ownership_boundary',
                  'Adjacent-fix policy': 'adjacent_fix_policy',
                  'Validation expectation': 'validation_expectation',
                  'Response expectation': 'response_expectation'}
        footer = {}
        for line in task['description'].splitlines():
            label, separator, value = line.partition(': ')
            if separator and label in labels:
                footer[labels[label]] = json.loads(value) if label == 'File-ownership boundary' else value
        assert set(footer) == set(labels.values()), 'real assignment lacks explicit operational footer'
        assigned_at = task['metadata']['assigned_at']
        snapshot = {'version': 1, 'team_name': TEAM, 'member_name': 'lead',
                    'updated_at': datetime.now(timezone.utc).isoformat(),
                    'task': {**{key: task[key] for key in ('id', 'subject', 'status', 'owner')},
                             'assigned_at': assigned_at},
                    'assignment_footer': footer,
                    'ownership': {'override_allowed': False, 'active_override_reason': None},
                    'working_set': {'project_path': str(self.root/'project'), 'focal_files': []}}
        params = {'publications': [{'snapshot': snapshot, 'task_state_changed_at': assigned_at}]}
        method = 'coordination.publish_operational_snapshots'
        self.save('step5-task-record.json', task)
        self.save('step5-publication-request.json', {'method': method, 'params': params})
        response = self.rpc(method, params)
        self.save('step5-publication-response.json', response)
        assert response['published'] == 1, response

    def cleanup(self):
        if self.root:
            try: self.capture('final')
            except Exception as error: self.event('final_capture_error',error=str(error))
        before=self.identities(); self.save('cleanup-before.json',before)
        for child in reversed(self.children):
            if child.poll() is None:
                os.killpg(child.pid,signal.SIGTERM)
                try: child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid,signal.SIGKILL); child.wait(timeout=5)
        from run9_support import stop_owned
        stop_owned(before,self.identities,os.kill,time.sleep)
        time.sleep(.3)
        if self.root and (self.root/'data').exists():
            self.retain_logs()
        survivors=self.identities()
        closed=True
        if self.port:
            with socket.socket() as s: s.settimeout(.2); closed=s.connect_ex(('127.0.0.1',self.port))!=0
        placeholder=self.root/'claude/.credentials.json' if self.root else None
        zero=placeholder is not None and placeholder.exists() and placeholder.stat().st_size==0
        if self.root: shutil.rmtree(self.root)
        self.save('cleanup.json',{'survivors':survivors,'private_port_closed':closed,'children_exit_codes':[p.returncode for p in self.children],'root_removed':not self.root or not self.root.exists(),'codex_copy_removed':not self.root or not (self.root/'codex/auth.json').exists(),'claude_mount_placeholder_zero_bytes':zero,'claude_credential_copied':False,'runtime_seconds':time.monotonic()-self.started})
        self.save('result.json',self.result)
        if survivors or not closed: raise RuntimeError('cleanup failure')

# Offline regression tests live here to keep this fix round within the named files.
# Run: PYTHONPATH=docs/design/evidence/e2e/l1-claude-lead python3 -m unittest controller.ReviewRegressionTests
import unittest
from unittest.mock import Mock, patch


class ReviewRegressionTests(unittest.TestCase):
    def test_report_routes_the_unresolved_writer_with_retained_evidence(self):
        # // Regression: a363e328 blamed Mesh's exclusion without the writer/recovery evidence.
        report = (BASE / 'run3/report.md').read_text()
        disposition = json.loads((BASE / 'run3/final-disposition.json').read_text())
        self.assertIn('unresolved', disposition['classification'])
        self.assertIn('claude-code', disposition['classification'])
        for citation in ('taurhaus.log.jsonl:132', 'daemon-events.json:803',
                         '2412', '4033', '5199', '5965', '1212168',
                         'stores/config.rs:944', '12:43:22.869',
                         'step1-observation-end/team/config.json',
                         'step1/team/config.json'):
            with self.subTest(citation=citation):
                self.assertIn(citation, report)
                self.assertIn(citation, json.dumps(disposition))

    def step_module(self):
        import importlib.util
        spec = importlib.util.spec_from_file_location('lane_steps', BASE / 'steps.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_step2_reaches_marker_sends_with_uncertain_activity(self):
        # // Regression: 17a07099 aborted before marker sends despite native readiness.
        import runpy
        import sys
        from unittest.mock import sentinel
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            for directory in ('latest/team/runtime', 'latest/team/state/activity', 'step2-native-ready'):
                (out / directory).mkdir(parents=True)
            (out / 'latest/team/runtime/alpha.json').write_text(
                json.dumps({'session_id': 't', 'jsonl_path': 'scratch/rollout.jsonl'}))
            (out / 'latest/team/state/activity/alpha.json').write_text(
                json.dumps({'activity_confidence': 'uncertain'}))
            rows = [{'type': 'event_msg', 'payload': {'type': kind, 'turn_id': 't'}}
                    for kind in ('task_started', 'task_complete')]
            rows.append({'type': 'response_item', 'payload': {'text': '[taurhaus] recovery_card'}})
            (out / 'codex-transcript.json').write_text(json.dumps(rows))
            (out / 'step2-native-ready/pane-2.txt').write_text('› Ask Codex to do anything')
            sent = []
            def action(value):
                if value['op'] == 'input':
                    sent.append(value)
                    raise StopIteration(sentinel.marker_sent)
            with patch('actions.OUT', out), patch('actions.action', side_effect=action), \
                    patch.object(sys, 'argv', ['steps.py', '2']), \
                    patch('time.monotonic', side_effect=range(0, 10000, 10)), patch('time.sleep'):
                try:
                    runpy.run_path(str(BASE / 'steps.py'), run_name='__main__')
                except (AssertionError, StopIteration):
                    pass
            self.assertEqual(len(sent), 1, 'native READY must reach the marker-send input')
            self.assertEqual(sent[0]['tool'], 'claude')
            self.assertIn('L1_AMBER_42', sent[0]['text'])
            self.assertIn('L1_BIRCH_73', sent[0]['text'])

    def test_native_ready_alpha_continues_despite_uncertain_export(self):
        # // Regression: 17a07099 made derived idle a blocking step-2 prerequisite.
        steps = self.step_module()
        rows = [{'type': 'event_msg', 'payload': {'type': kind, 'turn_id': 't'}}
                for kind in ('task_started', 'task_complete')]
        rows.append({'type': 'response_item', 'payload': {'text': '[taurhaus] recovery_card'}})
        record = {'session_id': 't', 'jsonl_path': 'scratch/rollout.jsonl'}
        activity = {'activity_confidence': 'uncertain'}
        def read(name):
            return record if name.endswith('runtime/alpha.json') else activity
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            (out / 'step2-native-ready').mkdir()
            (out / 'step2-native-ready/pane-2.txt').write_text('› Ask Codex to do anything')
            with patch.object(steps, 'OUT', out), patch.object(steps, 'codex_rows', return_value=rows), \
                    patch.object(steps, 'read', side_effect=read), patch.object(steps, 'snapshot'), \
                    patch.object(steps, 'send_input') as send, \
                    patch.object(steps, 'wait', side_effect=lambda test, *a, **kw: self.assertTrue(test())):
                steps.prepare_alpha()
            send.assert_not_called()
            observation = json.loads((out / 'step2-alpha-observation.json').read_text())
            self.assertFalse(observation['attributed_idle'])
            self.assertIn('deviation', observation)

    def test_native_ready_requires_empty_composer_and_completed_turn(self):
        # // Regression: 17a07099 used derived activity instead of native readiness.
        steps = self.step_module()
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            (out / 'step2-native-ready').mkdir()
            pane = out / 'step2-native-ready/pane-2.txt'
            rows = [{'type': 'event_msg', 'payload': {'type': kind, 'turn_id': 't'}}
                    for kind in ('task_started', 'task_complete')]
            with patch.object(steps, 'OUT', out), patch.object(steps, 'snapshot'), \
                    patch.object(steps, 'codex_rows', return_value=rows):
                pane.write_text('› unsent prompt')
                self.assertFalse(steps.alpha_native_ready())
                pane.write_text('› Ask Codex to do anything')
                self.assertTrue(steps.alpha_native_ready())
                rows.pop()
                self.assertFalse(steps.alpha_native_ready())

    def test_product_classification_requires_current_pending_card_evidence(self):
        # // Regression: 17a07099 classified arbitrary timeouts by a taurhaus: prefix.
        steps = self.step_module()
        facts = {'daemon_alive': True, 'runtime_readable': True, 'activity_fresh': True,
                 'session_attributed': True, 'card_pending': True, 'card_exposed': False}
        error = steps.OnboardingPending('alpha onboarding card never delivered')
        self.assertEqual(steps.classify_failure(error, facts), 'taurhaus')
        self.assertEqual(steps.classify_failure(AssertionError('taurhaus: idle'), facts),
                         'product-owner-pending-evidence-review')
        for key in ('daemon_alive', 'runtime_readable', 'activity_fresh'):
            with self.subTest(key=key):
                self.assertEqual(steps.classify_failure(error, {**facts, key: False}), 'harness')
        for change in ({'card_exposed': True}, {'card_pending': False}, {'session_attributed': False}):
            self.assertNotEqual(steps.classify_failure(error, {**facts, **change}), 'taurhaus')
        self.assertEqual(steps.classify_failure(error, {}), 'harness')

    def test_onboarding_facts_reject_stale_or_consumed_pending_projection(self):
        # // Regression: 17a07099 inferred product failure without retained health facts.
        from controller_support import onboarding_facts
        record = {'session_id': 's', 'jsonl_path': 'scratch/r', 'recovery': {
            'last_delivered': {'journal': {'message_id': 'm', 'projection': 'pending'}}}}
        activity = {'observed_at': '1970-01-01T00:01:40+00:00'}
        facts = onboarding_facts(record, activity, [], [], True, 101)
        self.assertTrue(facts['card_pending'])
        self.assertTrue(facts['activity_fresh'])
        receipts = [{'event_type': 'receipt', 'payload': {'message_id': 'm', 'kind': 'consumed_by_read', 'reader_name': 'alpha'}}]
        facts = onboarding_facts(record, activity, [], receipts, True, 101)
        self.assertFalse(facts['card_pending'])
        self.assertTrue(facts['card_exposed'])
        for value in (None, {}, {'observed_at': 'invalid'}):
            self.assertFalse(onboarding_facts(record, value, [], [], True, 101)['activity_fresh'])
        self.assertFalse(onboarding_facts(record, activity, [], [], True, 221)['activity_fresh'])
        self.assertFalse(onboarding_facts(None, activity, [], [], False, 101)['runtime_readable'])

    def test_log_retention_keeps_exclusion_count(self):
        # // Regression: 17a07099 wrote a counter removed by account-key redaction.
        trial = Trial.__new__(Trial)
        with tempfile.TemporaryDirectory() as tmp:
            trial.root = Path(tmp)
            (trial.root / 'data').mkdir()
            (trial.root / 'data/taurhaus.log.jsonl').write_text(
                '{"event":"startup.ready"}\n{"event":"usage.fetched"}\n')
            trial.saved = {}
            trial.save = lambda name, value: trial.saved.update({name: sanitize(value)})
            trial.retain_logs()
            self.assertEqual(trial.saved['log-retention.json']['excluded_usage_rows'], 1)
            self.assertEqual(trial.saved['log-retention.json']['retained_rows'], 1)

    def test_run3_regression_is_independent_of_latest_headline(self):
        # // Regression: 6a57a879 pinned mutable latest-run wording to run 3.
        original = Path.read_text
        def read(path, *args, **kwargs):
            if path == BASE.with_suffix('.md'):
                return '# L1 latest run — PASS\n'
            return original(path, *args, **kwargs)
        with patch.object(Path, 'read_text', read):
            self.test_report_routes_the_unresolved_writer_with_retained_evidence()

    def test_prior_ledger_charges_historical_input_counts(self):
        # // Regression: bdf98333 hid prior counts under an unread historical_cap_inputs key.
        prior = json.loads((BASE / 'run3/prior-spend.json').read_text())
        totals = cumulative(prior, {'claude': 3, 'codex': 5}, 0, 0)
        self.assertEqual(totals['controller_inputs'], {'claude': 8, 'codex': 11})
        with self.assertRaises(ValueError):
            admit_input('claude', 8, 11, 0, .5)

    def submission_trial(self):
        # Generated in-memory observations; never start a CLI or access a harness home.
        trial = Trial.__new__(Trial)
        trial.inputs = {'claude': 0, 'codex': 0}
        trial.wait_for_alpha_delivery = Mock()  # Input exclusion has separate generated-data coverage.
        trial.prior = {'seat_usd_upper_rate': 0}
        trial.step = 2
        trial.outer_deadline = 150
        trial.ledger = Mock(return_value={
            'claude_generations': [], 'claude_cap_times_max_observed_input_usd': 0,
            'metering_complete': True, 'controller_inputs': trial.inputs,
            'seat_usd_upper_rate': 0,
        })
        trial.run = Mock(return_value='• Working\n› Ask Codex to do anything')
        trial.event = Mock()
        trial.snapshot = Mock()
        trial.saved = {}
        trial.save = lambda name, value: trial.saved.update({name: value})
        started = [{'type': 'event_msg', 'payload': {'type': 'task_started', 'turn_id': 'new'}}]
        trial.rows = Mock(side_effect=[[], started, [], started])
        return trial

    def test_each_submission_retains_its_own_pane_and_confirmation(self):
        # // Regression: bdf98333 overwrote the previous paid input's fixed-name evidence.
        trial = self.submission_trial()
        with patch('controller.time.sleep'), patch('controller.time.monotonic', return_value=0):
            trial.input('codex', '%2', 'first')
            trial.step = 3
            trial.input('codex', '%2', 'second')
        for step, count in ((2, 1), (3, 2)):
            prefix = f'submission-step{step}-{count}'
            self.assertIn(prefix + '-pane.txt', trial.saved)
            confirmation = trial.saved[prefix + '-confirmation.json']
            self.assertEqual(confirmation['opportunity_deadline'], 65)

    def test_late_confirmation_survives_insufficient_opportunity(self):
        # // Regression: bdf98333 raised inside save() arguments after a paid confirmation.
        trial = self.submission_trial()
        with patch('controller.time.sleep'), patch('controller.time.monotonic', side_effect=[0, 1, 2, 100]):
            with self.assertRaisesRegex(ValueError, 'full opportunity'):
                trial.input('codex', '%2', 'bounded input')
        confirmations = [value for name, value in trial.saved.items() if name.endswith('-confirmation.json')]
        self.assertEqual(len(confirmations), 1)
        self.assertEqual(confirmations[0]['opportunity_deadline'], 165)
        self.assertFalse(confirmations[0]['opportunity_available'])


def main():
    parser=argparse.ArgumentParser()
    for option in ['claude-credential','codex-auth','claude-binary','codex-binary']: parser.add_argument('--'+option,type=Path,required=True)
    parser.add_argument('--evidence-label',required=True)
    parser.add_argument('--prior-ledger',type=Path,required=True)
    parser.add_argument('--prior-runtime-seconds',type=float,required=True)
    args=parser.parse_args(); assert Path.cwd()==CHECKOUT
    assert valid_evidence_label(args.evidence_label), 'invalid evidence label'
    assert args.prior_ledger.resolve().is_relative_to(BASE), 'prior ledger must be lane evidence'
    assert 0<=args.prior_runtime_seconds<900, 'runtime budget unavailable'
    trial=Trial(args)
    def interrupted(signum,_frame): raise RuntimeError(f'controller interrupted {signum}')
    for sig in [signal.SIGTERM,signal.SIGINT]: signal.signal(sig,interrupted)
    code=1
    try: trial.setup(); trial.loop(); code=0 if trial.result.get('outcome')=='PASS' else 2
    except BaseException as error:
        trial.result={'outcome':'UNAVAILABLE','classification':'harness','step':trial.step,'reason':str(error)}; trial.event('stopped',error=str(error))
    finally: trial.cleanup()
    trial.save('controller-exit.json',{'exit':code}); return code
if __name__=='__main__': raise SystemExit(main())
