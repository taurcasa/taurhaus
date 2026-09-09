# Codex 0.153.4 — Unix app-server framing

2026-09-09. **FAIL / blocking finding, S-runtime. No descriptor enabled.**
The real Unix listener requires a WebSocket/HTTP upgrade. The stage-5
`unix_ndjson` contract does not match this build. Per the assignment, the
app-server trial stopped at this finding, before authentication or thread creation.
This does not contradict the earlier probe's successful **stdio** experiment.

## Candidate and isolation

- Taurhaus: `cadd533ebd83d75d848e289e32974a2db64875c9` (#154), protocol 26.
- Mesh: `504b2b6bf8f9017c7a5ce7957cd59b2a0ffafc08`, `feat/native-push`.
- Installed `codex --version`: `codex-cli 0.153.4` (exit 0).
- Host: Linux/WSL, private Unix socket, new app-server child; network namespace
  disabled, credential-free scratch `CODEX_HOME`, no existing sessions.
- Scratch root: `/tmp/native-elig-20260909-kj3o83ui` (mode 0700).
- No taurhaus daemon, Mesh owner, tmux server, or model participated in this test.
  This directly tests the transport prerequisite, not the daemon bridge.

## Wire evidence

The controller launched `codex app-server --listen
unix:///tmp/native-elig-20260909-kj3o83ui/app.sock` inside the sandbox below.
It opened two fresh `AF_UNIX/SOCK_STREAM` connections, each with a three-second
read deadline. The same owned server handled both connections.

First connection, exact contract bytes (the final newline is significant):

```json
{"id":"elig-framing-1","method":"initialize","params":{"clientInfo":{"name":"mesh_native_push","version":"1"},"capabilities":{"experimentalApi":true}}}
```

Observed reply: **zero bytes / EOF**, not a timeout. No `initialize` result.

Second connection, exact HTTP request (each line terminates in CRLF, followed
by an empty CRLF line):

```http
GET / HTTP/1.1
Host: localhost
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
Sec-WebSocket-Version: 13

```

Observed response (CRLF framing):

```http
HTTP/1.1 101 Switching Protocols
connection: Upgrade
upgrade: websocket
sec-websocket-accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=

```

The WebSocket key is the public RFC handshake example, not a credential.
No WebSocket JSON messages or model requests followed this handshake.
Server stdout/stderr were empty. Controller exit 0; owned child PID
`1392891` terminated by SIGTERM (wait status `-15`) and reaped in `finally`.
The private PID namespace kills any descendants when its init exits.

## Acceptance and spend

| Signal | Result |
|---|---|
| Raw Unix NDJSON initialize | Failed: immediate EOF |
| HTTP/WebSocket handshake positive control | `101 Switching Protocols` |
| Idle `turn/start` uptake | NOT RUN: framing blocker |
| Active `turn/steer` uptake | NOT RUN: framing blocker |
| Correct thread / turn / model marker | None: no thread or turn created |
| `native_enqueued` through real bridge | None; no submission claimed |
| Steer after completion rejection | NOT RUN: no model turn |
| Turns / generations / tokens / USD | 0 / 0 / 0 / **$0.00** |
| Descriptor flips | None; exact 0.153.4 and wildcard remain disabled |

No inference about comprehension, latency, savings, or other transports follows
from these handshake results. Fixing the transport is outside this eligibility
packet; it needs a revised paired contract and implementation before a new trial.

## Reproduction controller

These are the exact controller sources used, with only the scratch-root pointer
file shared between them. They are audit material, not an automated paid test.
The sandbox helper exposes installed binaries read-only, hides `/home`, `/tmp`
and `/run`, creates private HOME/data/account/tmux directories, clears inherited
environment (including TMUX), and disables network access for these free calls.
The first sandbox setup attempt failed before any CLI ran because `/opt` was
read-only; adding its private tmpfs fixed setup. No model work occurred.

`/tmp/native-elig-probe.py`:

```python
import os,pathlib,subprocess,json,socket,time,signal
ROOT=pathlib.Path('/tmp/native-elig-current-root').read_text()
R=pathlib.Path(ROOT)
NODE='/home/mstie/.nvm/versions/node/v24.14.1'
CLAUDE=os.path.realpath('/home/mstie/.local/bin/claude')
def command(args,network=False):
    env={'HOME':ROOT+'/home','PATH':'/opt/node/bin:/usr/bin:/bin','TMPDIR':ROOT+'/tmp','CODEX_HOME':ROOT+'/codex','CLAUDE_CONFIG_DIR':ROOT+'/claude','TAURHAUS_CLAUDE_DIR':ROOT+'/claude','CLAUDE_DIR':ROOT+'/claude','TAURHAUS_DATA_DIR':ROOT+'/data','TMUX_TMPDIR':ROOT+'/tmux','GROK_HOME':ROOT+'/grok','TAURHAUS_AGY_DIR':ROOT+'/agy','LANG':'C.UTF-8','TERM':'xterm-256color'}
    cmd=['/usr/bin/bwrap','--die-with-parent','--new-session','--unshare-pid','--ro-bind','/','/','--tmpfs','/home','--tmpfs','/tmp','--tmpfs','/run','--tmpfs','/opt','--proc','/proc','--dev','/dev','--bind',ROOT,ROOT,'--ro-bind',NODE,'/opt/node','--ro-bind',CLAUDE,'/opt/claude','--chdir',ROOT+'/project','--clearenv']
    if not network:cmd+=['--unshare-net']
    for k,v in env.items():cmd+=['--setenv',k,v]
    return cmd+args
if __name__=='__main__':
    for name,args in [('codex-version',['codex','--version']),('claude-version',['/opt/claude','--version']),('codex-app-server-help',['codex','app-server','--help'])]:
        cmd=command(args)
        p=subprocess.run(cmd,capture_output=True,text=True,timeout=15)
        result={'command':cmd,'exit_code':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
        (R/'evidence'/f'{name}.json').write_text(json.dumps(result,indent=2))
        print(name,p.returncode,p.stdout,p.stderr)
```

`/tmp/native-elig-framing.py`:

```python
import sys
sys.path.insert(0,'/tmp')
from importlib.machinery import SourceFileLoader
p=SourceFileLoader('probe','/tmp/native-elig-probe.py').load_module()
import subprocess,socket,time,json,signal,os
r=p.R
sock=str(r/'app.sock')
cmd=p.command(['codex','app-server','--listen','unix://'+sock])
log=open(r/'evidence/framing-server.log','w')
child=subprocess.Popen(cmd,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
result={'command':cmd,'controller_pid':os.getpid(),'child_pid':child.pid,'turns':0,'cost_usd':0,'exchanges':[]}
def exchange(name,wire):
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as s:
        s.settimeout(3);s.connect(sock);s.sendall(wire)
        try:reply=s.recv(8192)
        except socket.timeout:reply=b'<timeout>'
    result['exchanges'].append({'name':name,'sent':wire.decode(),'received':reply.decode(errors='backslashreplace'),'received_hex':reply.hex()})
try:
    for _ in range(100):
        if os.path.exists(sock):break
        if child.poll() is not None:raise RuntimeError('app-server exited')
        time.sleep(.05)
    exchange('contract-ndjson',b'{"id":"elig-framing-1","method":"initialize","params":{"clientInfo":{"name":"mesh_native_push","version":"1"},"capabilities":{"experimentalApi":true}}}\n')
    exchange('websocket-upgrade',b'GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n')
finally:
    if child.poll() is None:
        os.killpg(child.pid,signal.SIGTERM)
        try:child.wait(timeout=5)
        except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
    log.close()
    result['child_exit_code']=child.returncode
    result['child_reaped']=child.poll() is not None
    (r/'evidence/framing.json').write_text(json.dumps(result,indent=2))
    print(json.dumps(result,indent=2))
    print((r/'evidence/framing-server.log').read_text())
```

Controller command: `python3 /tmp/native-elig-framing.py` (exit 0).
Full machine-local audit: `evidence/framing.json` beneath the scratch root;
all wire bytes needed to assess the finding are retained above.
