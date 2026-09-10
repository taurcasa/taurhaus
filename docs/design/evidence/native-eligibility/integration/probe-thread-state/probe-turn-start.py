#!/usr/bin/env python3
"""Diagnostic: replicate the daemon's exact host sequence against a real codex app-server and
capture the raw JSON-RPC error the daemon hides. Isolated scratch home (only auth.json copied),
bwrap like the attach trial, <= 2 tiny model turns (gpt-5.6-luna, low)."""
import os, sys, json, tempfile, subprocess, shutil, signal, socket, struct, time
from pathlib import Path
OUT = Path('/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/integration-trial-3')
NODE = Path('/home/mstie/.nvm/versions/node/v24.14.1')
ROOT = Path(tempfile.mkdtemp(prefix='th-probe-'))
os.umask(0o077)
for d in ['home', 'codex', 'project', 'tmp']:
    (ROOT / d).mkdir(mode=0o700)
env = {'PATH': str(NODE / 'bin') + ':/usr/bin:/bin', 'HOME': str(ROOT / 'home'), 'CODEX_HOME': str(ROOT / 'codex'),
       'TMPDIR': str(ROOT / 'tmp'), 'LANG': 'C.UTF-8', 'TERM': 'xterm-256color'}
bw = ['/usr/bin/bwrap', '--die-with-parent', '--unshare-pid', '--ro-bind', '/', '/', '--tmpfs', '/home', '--tmpfs', '/tmp',
      '--tmpfs', '/run', '--proc', '/proc', '--dev', '/dev', '--ro-bind', str(NODE), str(NODE), '--bind', str(ROOT), str(ROOT),
      '--chdir', str(ROOT / 'project'), '--clearenv']
for k, v in env.items():
    bw += ['--setenv', k, v]
logf = (OUT / 'probe-events.jsonl').open('w', buffering=1)
def log(kind, **kw):
    row = {'time': time.time(), 'kind': kind, **kw}
    logf.write(json.dumps(row) + '\n'); print(json.dumps(row)[:600], flush=True)
children = []
class WS:
    def __init__(self):
        self.s = socket.socket(socket.AF_UNIX); self.s.settimeout(10); self.s.connect(str(ROOT / 'app.sock'))
        req = b'GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: ' + __import__('base64').b64encode(os.urandom(16)) + b'\r\nSec-WebSocket-Version: 13\r\n\r\n'
        self.s.sendall(req); data = b''
        while b'\r\n\r\n' not in data:
            data += self.s.recv(1)
        assert data.startswith(b'HTTP/1.1 101'), repr(data)
    def send(self, obj, opcode=1):
        data = json.dumps(obj, separators=(',', ':')).encode() if opcode == 1 else obj
        mask = os.urandom(4); n = len(data); head = bytes([0x80 | opcode, 0x80 | min(n, 126)])
        if n >= 126: head += struct.pack('!H', n)
        self.s.sendall(head + mask + bytes(x ^ mask[i % 4] for i, x in enumerate(data)))
        if opcode == 1: log('rpc_send', message=obj)
    def readn(self, n):
        b = b''
        while len(b) < n:
            c = self.s.recv(n - len(b))
            if not c: raise EOFError('closed')
            b += c
        return b
    def recv(self, timeout=10):
        self.s.settimeout(timeout)
        while True:
            a, b = self.readn(2); n = b & 127
            if n == 126: n = struct.unpack('!H', self.readn(2))[0]
            elif n == 127: n = struct.unpack('!Q', self.readn(8))[0]
            mask = self.readn(4) if b & 128 else None; data = self.readn(n)
            if mask: data = bytes(x ^ mask[i % 4] for i, x in enumerate(data))
            op = a & 15
            if op == 9: self.send(data, 10); continue
            if op == 8: raise EOFError('close')
            obj = json.loads(data); log('rpc_recv', message=obj); return obj
    def rpc(self, method, params, timeout=60):
        rid = 'probe-' + str(time.time_ns()); self.send({'id': rid, 'method': method, 'params': params})
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            obj = self.recv(max(.1, end - time.monotonic()))
            if obj.get('id') == rid:
                if 'error' in obj:
                    log('RPC_ERROR', method=method, params=params, error=obj['error']); return {'__error__': obj['error']}
                return obj['result']
        raise TimeoutError(method)
    def wait_turn_done(self, timeout=90):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            ev = self.recv(max(.1, end - time.monotonic()))
            if ev.get('method') == 'turn/completed': return ev
        raise TimeoutError('turn/completed')
try:
    src = Path('/home/mstie/.codex/auth.json'); assert src.is_file() and not src.is_symlink()
    shutil.copyfile(src, ROOT / 'codex/auth.json'); os.chmod(ROOT / 'codex/auth.json', 0o600)
    # config as attempt 3's seat home (no baseInstructions override; project trusted)
    (ROOT / 'codex/config.toml').write_text('model = "gpt-5.6-luna"\nmodel_reasoning_effort = "low"\napproval_policy = "never"\nsandbox_mode = "read-only"\nweb_search = "disabled"\nmodel_context_window = 32768\n[projects.' + json.dumps(str(ROOT / 'project')) + ']\ntrust_level = "trusted"\n')
    (ROOT / 'project/AGENTS.md').write_text('This is an isolated transport probe. Reply briefly. Never execute tools or commands.\n')
    subprocess.run(['git', 'init', '-q', str(ROOT / 'project')], env={**env, 'GIT_CONFIG_GLOBAL': '/dev/null'}, check=False)
    # the daemon's child argv: -c overrides (as HostedLaunch renders them) + app-server --listen
    argv = sys.argv[1:] or ['codex', '-c', 'model="gpt-5.6-luna"', '-c', 'model_reasoning_effort="low"', '-c', 'sandbox_mode="read-only"', '-c', 'approval_policy="never"', 'app-server']
    argv = argv + ['--listen', 'unix://' + str(ROOT / 'app.sock')]
    log('spawn', argv=argv)
    p = subprocess.Popen(bw + argv, env=env, stdin=subprocess.DEVNULL, stdout=(OUT / 'probe-app-server.log').open('w'), stderr=subprocess.STDOUT, start_new_session=True); children.append(p)
    for _ in range(200):
        if (ROOT / 'app.sock').exists(): break
        if p.poll() is not None: raise RuntimeError('app-server exited ' + str(p.returncode))
        time.sleep(.1)
    ws = WS()
    hs = ws.rpc('initialize', {'clientInfo': {'name': 'taurhaus_host', 'version': '1'}, 'capabilities': {'experimentalApi': True}})
    log('handshake', userAgent=hs.get('userAgent'), codexHome=hs.get('codexHome'))
    ws.send({'method': 'initialized'})
    # 1) the daemon's thread/start shape
    t = ws.rpc('thread/start', {'cwd': str(ROOT / 'project'), 'ephemeral': False})
    tid = t.get('thread', {}).get('id') if isinstance(t, dict) else None
    log('thread', id=tid, instructionSources=t.get('instructionSources') if isinstance(t, dict) else None)
    if not tid: raise RuntimeError('no thread id: ' + json.dumps(t)[:500])
    # 2) the daemon's thread/read then turn/start shape (exactly as hosted_process.rs input())
    r = ws.rpc('thread/read', {'threadId': tid, 'includeTurns': True})
    th = r.get('thread', {}) if isinstance(r, dict) else {}
    log('thread_read', status=th.get('status'), canAcceptDirectInput=th.get('canAcceptDirectInput'), turns=len(th.get('turns') or []))
    a = ws.rpc('turn/start', {'threadId': tid, 'input': [{'type': 'text', 'text': 'Reply exactly PROBE_ONE.'}]})
    log('daemon_shape_result', result=a)
    if '__error__' not in a:
        ws.wait_turn_done()
    # 3) the attach trial's known-good shape (model/effort/serviceTierForTurn)
    b = ws.rpc('turn/start', {'threadId': tid, 'model': 'gpt-5.6-luna', 'effort': 'low', 'serviceTierForTurn': 'default', 'input': [{'type': 'text', 'text': 'Reply exactly PROBE_TWO.'}]})
    log('trial_shape_result', result=b)
    if '__error__' not in b:
        ws.wait_turn_done()
    log('done')
finally:
    for c in children:
        try: os.killpg(c.pid, signal.SIGTERM)
        except Exception: pass
    time.sleep(0.5)
    for c in children:
        try: os.killpg(c.pid, signal.SIGKILL)
        except Exception: pass
    shutil.rmtree(ROOT, ignore_errors=True)
    log('cleanup', root_removed=not ROOT.exists())
