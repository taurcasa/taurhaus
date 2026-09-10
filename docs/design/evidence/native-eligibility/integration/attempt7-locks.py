"""One bounded live lock interleave; only pauses verified trial-owned processes.

Kernel fdinfo is the holder evidence: host locks have no production holder JSON.
No app-server connection; only the daemon's read-only transcript RPC is used.
"""
import concurrent.futures
import json
import os
from pathlib import Path
import shlex
import signal
import socket
import subprocess
import time

OUT = Path(__file__).resolve().parent / 'attempt7/run'
rows = [json.loads(l) for l in (OUT/'events.jsonl').read_text().splitlines()]
isolation = next(r for r in rows if r['kind']=='isolation')
ROOT = Path(isolation['root']); ENV = isolation['environment']
PORT = int(ENV['TAURHAUS_DAEMON_PORT'])
lock = ROOT/'claude/teams/integration/state/app-server/seat.lock'
inode = lock.stat().st_ino
paused = set(); evidence = []

def record(kind, **fields):
    evidence.append({'at':time.time(),'kind':kind, **fields})
    (OUT/'step4-locks.json').write_text(json.dumps(evidence,indent=2))

def owned(pid):
    return ('TAURHAUS_TRIAL_ID='+ROOT.name).encode()+b'\0' in Path(f'/proc/{pid}/environ').read_bytes()

def pause(pid):
    assert owned(pid)
    os.kill(pid, signal.SIGSTOP); paused.add(pid)
    record('pause_owned',pid=pid,start_ticks=Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()[19])

def resume(pid):
    os.kill(pid,signal.SIGCONT);paused.discard(pid);record('resume_owned',pid=pid)

def holder(duration, executable):
    end=time.monotonic()+duration
    while time.monotonic()<end:
        for line in Path('/proc/locks').read_text().splitlines():
            fields=line.split()
            if '->' in fields or len(fields)<6:continue
            if fields[1]=='FLOCK' and fields[3]=='WRITE' and fields[5].endswith(':'+str(inode)):
                pid=int(fields[4])
                try:
                    argv=Path(f'/proc/{pid}/cmdline').read_bytes().split(b'\0')
                    if not argv[0].endswith(executable.encode()) or not owned(pid):continue
                    pause(pid)
                    info=[]
                    for fd in Path(f'/proc/{pid}/fdinfo').iterdir():
                        text=fd.read_text()
                        if f'ino:\t{inode}\n' in text and 'lock:' in text:info.append(text)
                    if not info:resume(pid);continue
                    record('kernel_holder',pid=pid,argv=[a.decode() for a in argv if a],lock=str(lock),inode=inode,proc_lock=line,fdinfo=info)
                    return pid
                except FileNotFoundError:continue
        time.sleep(.002)
    raise TimeoutError('did not capture '+executable+' host lock')

def rpc():
    req={'id':'trial-lock-'+str(time.time_ns()),'method':'coordination.hosted_transcript','params':{'team_name':'integration','member_name':'seat'}}
    record('daemon_request',request=req.copy())
    req['auth']=(ROOT/'data/daemon.token').read_text().strip()
    with socket.create_connection(('127.0.0.1',PORT),timeout=8) as s:
        s.sendall(json.dumps(req).encode()+b'\n')
        response=json.loads(s.makefile('rb').readline())
    record('daemon_read_result',id=response.get('id'),error=response.get('error'),success='result' in response)
    return response

def tmux(argv):
    record('tmux_command',argv=argv)
    r=subprocess.run(['tmux']+argv,env=ENV,capture_output=True,text=True,timeout=10)
    record('tmux_result',exit=r.returncode,output=r.stdout,stderr=r.stderr)
    assert r.returncode==0

controller=None
for p in Path('/proc').iterdir():
    if not p.name.isdigit():continue
    try:
        args=(p/'cmdline').read_bytes().split(b'\0')
        if len(args)>2 and args[1].endswith(b'/attempt7-controller.py') and args[2]==b'attempt7/run':controller=int(p.name)
    except (PermissionError,FileNotFoundError):pass
assert controller
# This is the exact controller started for this run; its inherited environment
# predates the scratch environment, so verify its random-root evidence instead.
def interrupted(sig, frame):raise RuntimeError('signal '+str(sig))
for sig in [signal.SIGTERM,signal.SIGINT]:signal.signal(sig,interrupted)
try:
    os.kill(controller,signal.SIGSTOP);paused.add(controller)
    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
        server=None
        for proc in Path('/proc').iterdir():
            if not proc.name.isdigit():continue
            try:
                argv=(proc/'cmdline').read_bytes().split(b'\0')
                if b'app-server' in argv and owned(int(proc.name)):server=int(proc.name)
            except (FileNotFoundError,PermissionError,ProcessLookupError):pass
        assert server
        pause(server)
        future=pool.submit(rpc)
        daemon=holder(3,'taurhaus-daemon')
        resume(server)
        output=ROOT/'lock-send.out'
        command=shlex.join([str(ROOT/'home/.local/bin/mesh'),'send','seat','ACTION REQUIRED: Reply exactly willow4f83ad. Do not execute tools.','--team','integration','--name','lead','--summary','lock exclusion marker'])+' >'+shlex.quote(str(output))+' 2>&1'
        tmux(['new-window','-d','-t','taurhaus','/bin/bash -c '+shlex.quote(command)])
        # One scheduler retry window while the daemon's real lock remains held.
        time.sleep(1.1)
        records=list((ROOT/'claude/teams/integration/state/delivery').glob('*.json'))
        pending=[json.loads(p.read_text()) for p in records if 'host_operation_busy' in p.read_text()]
        record('mesh_deferral_while_daemon_holds',pending=pending,send_output=output.read_text() if output.exists() else '')
        assert pending,'Mesh did not record host_operation_busy'
        resume(daemon);future.result(timeout=8)
        mesh=holder(5,'mesh')
        response=rpc()
        assert 'lock busy' in str(response.get('error')),response.get('error')
        record('daemon_deferral_while_mesh_holds',error=response['error'])
        resume(mesh)
    record('outcome',status='PASS')
except BaseException as e:
    record('outcome',status='INCONCLUSIVE',error=str(e));raise
finally:
    for pid in list(paused):
        try:os.kill(pid,signal.SIGCONT)
        except ProcessLookupError:pass
    record('cleanup',resumed_all=True)
