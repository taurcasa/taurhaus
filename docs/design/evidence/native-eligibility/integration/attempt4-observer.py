"""Read-only Unix WebSocket observer; sanitizes before retaining any host data."""
import base64, hashlib, json, os, socket, struct, threading, time
from pathlib import Path


def clean(value):
    if isinstance(value, dict):
        return {k: ('<redacted>' if any(s in k.lower() for s in ['installation', 'accountid', 'auth', 'accesstoken', 'refreshtoken', 'idtoken']) else clean(v)) for k,v in value.items()}
    if isinstance(value, list): return [clean(v) for v in value]
    return value


class Observer:
    def __init__(self, root, out):
        self.root, self.out = root, out
        self.stop = threading.Event()
        self.rows = []
        self.thread = threading.Thread(target=self.watch, daemon=True)
        self.thread.start()

    def log(self, kind, **fields):
        row = clean({'at':time.time(), 'kind':kind, **fields})
        self.rows.append(row)
        with (self.out/'host-events.jsonl').open('a') as f: f.write(json.dumps(row)+'\n')

    def send(self, obj, opcode=1):
        data = json.dumps(obj).encode() if opcode == 1 else obj
        n=len(data); mask=os.urandom(4)
        size=bytes([n]) if n<126 else b'\x7e'+struct.pack('!H',n) if n<65536 else b'\x7f'+struct.pack('!Q',n)
        self.s.sendall(bytes([128|opcode,128|size[0]])+size[1:]+mask+bytes(v^mask[i%4] for i,v in enumerate(data)))
        if opcode==1:self.log('observer_request', message=obj)

    def readn(self,n):
        data=b''
        while len(data)<n:
            chunk=self.s.recv(n-len(data))
            if not chunk:raise EOFError('host closed')
            data+=chunk
        return data

    def recv(self):
        parts=b''
        while True:
            a,b=self.readn(2); n=b&127
            if n==126:n=struct.unpack('!H',self.readn(2))[0]
            elif n==127:n=struct.unpack('!Q',self.readn(8))[0]
            assert n<=1048576
            assert not b&128
            data=self.readn(n); op=a&15
            if op==9:self.send(data,10);continue
            if op==8:raise EOFError('host close frame')
            assert op in [0,1]
            parts+=data
            assert len(parts)<=1048576
            if a&128:return json.loads(parts)

    def watch(self):
        seen=set()
        while not self.stop.is_set():
            sockets=list((self.root/'tmp').glob('th-host-*/*.sock'))
            for path in sockets:
                if str(path) in seen:continue
                seen.add(str(path))
                try:
                    self.s=socket.socket(socket.AF_UNIX);self.s.settimeout(1);self.s.connect(str(path))
                    key=base64.b64encode(os.urandom(16))
                    req=b'GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: '+key+b'\r\nSec-WebSocket-Version: 13\r\n\r\n'
                    self.s.sendall(req);resp=b''
                    while not resp.endswith(b'\r\n\r\n'):resp+=self.readn(1)
                    accept=base64.b64encode(hashlib.sha1(key+b'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest())
                    assert resp.startswith(b'HTTP/1.1 101') and accept in resp
                    self.log('handshake',socket=str(path),request=req.decode(),response=resp.decode())
                    self.send({'id':'observer-init','method':'initialize','params':{'clientInfo':{'name':'trial_observer','version':'4'},'capabilities':{'experimentalApi':True}}})
                    while not self.stop.is_set():
                        try:row=self.recv()
                        except socket.timeout:continue
                        # Never retain account usage rows or installation identifiers.
                        if row.get('method','').startswith('account/'):continue
                        if row.get('id')=='observer-init':
                            self.log('initialized',codexHome=row.get('result',{}).get('codexHome'))
                            self.send({'method':'initialized'});continue
                        self.log('host_event',message=row)
                except Exception as e:self.log('observer_closed',error=str(e))
                finally:
                    if hasattr(self,'s'):self.s.close()
            self.stop.wait(.01)

    def ledger(self):
        generations=[]; starts=set()
        for row in self.rows:
            ev=row.get('message',{});p=ev.get('params',{})
            if ev.get('method')=='turn/started':starts.add(p['turn']['id'])
            if ev.get('method')!='thread/tokenUsage/updated':continue
            usage=p['tokenUsage']['last'];i=usage.get('inputTokens',0);c=usage.get('cachedInputTokens',0);o=usage.get('outputTokens',0)
            item={'thread_id':p.get('threadId'),'turn_id':p.get('turnId'),'input':i,'cached_input':c,'output':o,'reasoning_output':usage.get('reasoningOutputTokens',0),
                  'api_equivalent_usd':round(((i-c)*.20+c*.02+o*1.20)/1e6,9),'conservative_usd':round((i+o)*1.20/1e6,9)}
            if item not in generations:generations.append(item)
        return {'model':'gpt-5.6-luna','effort':'low','turn_ids':sorted(starts),'paid_inputs':len(starts),'generations':generations,
                'api_equivalent_usd':round(sum(g['api_equivalent_usd'] for g in generations),9),'conservative_usd':round(sum(g['conservative_usd'] for g in generations),9),
                'basis':'Observed host tokenUsage.last; packet rates $0.20/$0.02/$1.20 per million input/cached/output. API-equivalent estimate, not a billing invoice.'}
