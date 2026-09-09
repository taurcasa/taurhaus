//! Owned host process and bounded contract-v1 RPC over Unix WebSockets.

use crate::coordination::stores::lock::HostOperationLock;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use taurhaus_lib::session_scanner::launch::HostedLaunch;

#[path = "hosted_websocket.rs"]
mod websocket;
use websocket::WebSocket;

pub(crate) struct HostProcess {
    child: Child,
    rpc: Option<Rpc>,
    pub thread_id: String,
    pub build: String,
    pub process_start: String,
    socket: PathBuf,
    uncertain: bool,
}

impl HostProcess {
    pub fn launch(
        launch: &HostedLaunch,
        cwd: &Path,
        socket: &Path,
        resume: Option<&str>,
        guard: &HostOperationLock,
    ) -> Result<Self, String> {
        if !socket.is_absolute()
            || socket.as_os_str().len() > 100
            || std::fs::symlink_metadata(socket).is_ok()
        {
            return Err("host socket must be a new short absolute private path".into());
        }
        let child = Command::new(&launch.program)
            .args(&launch.arguments)
            .args(["--listen", &format!("unix://{}", socket.display())])
            .envs(&launch.environment)
            .env_remove("TMUX")
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut host = Self {
            child,
            rpc: None,
            thread_id: String::new(),
            build: String::new(),
            process_start: String::new(),
            socket: socket.into(),
            uncertain: false,
        };
        host.process_start = taurhaus_lib::platform::process_start_ticks(host.child.id())
            .ok_or("host process identity unavailable")?
            .to_string();
        loop {
            let remaining = guard.remaining().map_err(|e| e.to_string())?;
            if host.child.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Err("app-server exited before transport readiness".into());
            }
            let connection =
                socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)
                    .map_err(|e| e.to_string())?;
            let address = socket2::SockAddr::unix(socket).map_err(|e| e.to_string())?;
            if connection
                .connect_timeout(&address, remaining.min(Duration::from_millis(20)))
                .is_ok()
            {
                let stream: UnixStream = connection.into();
                host.rpc = Some(Rpc {
                    socket: WebSocket::connect(stream, guard)?,
                    events: VecDeque::new(),
                    requests: Vec::new(),
                    truncated: false,
                });
                break;
            }
            std::thread::sleep(remaining.min(Duration::from_millis(10)));
        }
        let rpc = host.rpc.as_mut().unwrap();
        let handshake = rpc.call("initialize", json!({"clientInfo":{"name":"taurhaus_host","version":"1"},"capabilities":{"experimentalApi":true}}), guard)?;
        if handshake["codexHome"].as_str().map(Path::new) != Some(launch.account_root.as_path()) {
            return Err("app-server account root mismatch".into());
        }
        host.build = handshake["userAgent"]
            .as_str()
            .and_then(|s| s.strip_prefix("taurhaus_host/"))
            .and_then(|s| s.split_whitespace().next())
            .filter(|s| !s.is_empty())
            .ok_or("unsupported app-server handshake")?
            .into();
        if host.build != "0.153.4" {
            return Err("unsupported app-server build; native input refused".into());
        }
        rpc.write(&json!({"method":"initialized"}), guard)?;
        let (method, params) = match resume {
            Some(id) if !id.is_empty() => ("thread/resume", json!({"threadId":id, "cwd":cwd})),
            Some(_) => return Err("empty resume identity".into()),
            None => ("thread/start", json!({"cwd":cwd, "ephemeral":false})),
        };
        let result = rpc.call(method, params, guard)?;
        host.thread_id = result["thread"]["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("host omitted thread identity")?
            .into();
        if resume.is_some_and(|id| id != host.thread_id) {
            return Err("host resumed a different thread".into());
        }
        Ok(host)
    }

    pub fn outcome_unknown(&self) -> bool {
        self.uncertain
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }
    pub fn alive(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
            && taurhaus_lib::platform::process_start_ticks(self.child.id())
                .map(|v| v.to_string())
                .as_deref()
                == Some(&self.process_start)
    }

    pub fn transcript(&mut self, guard: &HostOperationLock) -> Result<Value, String> {
        if !self.alive() {
            return Err("owned host stopped".into());
        }
        let rpc = self.rpc.as_mut().ok_or("host connection unavailable")?;
        let mut result = rpc.call(
            "thread/read",
            json!({"threadId":self.thread_id,"includeTurns":true}),
            guard,
        )?;
        if result["thread"]["id"] != self.thread_id {
            return Err("host thread identity changed".into());
        }
        result["requests"] = json!(rpc.requests);
        result["events"] = json!(rpc.events);
        result["eventsTruncated"] = json!(rpc.truncated);
        result["outcomeUnknown"] = json!(self.uncertain);
        Ok(result)
    }

    pub fn input(&mut self, text: &str, guard: &HostOperationLock) -> Result<Value, String> {
        if self.uncertain {
            return Err(
                "outcome_unknown: reconcile previous input before another submission".into(),
            );
        }
        if text.trim().is_empty() || text.len() > 16_384 || text.chars().count() > 8000 {
            return Err("input must contain 1–8000 characters within 16 KiB".into());
        }
        let state = self.transcript(guard)?;
        let thread = &state["thread"];
        if thread["canAcceptDirectInput"] != true
            || !thread["status"]["activeFlags"]
                .as_array()
                .is_some_and(Vec::is_empty)
            || !state["requests"].as_array().is_some_and(Vec::is_empty)
        {
            return Err(
                "pending: thread is waiting for permission/input or has unverified state".into(),
            );
        }
        let active: Vec<_> = thread["turns"]
            .as_array()
            .ok_or("unverified turns")?
            .iter()
            .filter(|t| t["status"] == "inProgress")
            .collect();
        let mut params = json!({"threadId":self.thread_id,"input":[{"type":"text","text":text}]});
        let expected = match (thread["status"]["type"].as_str(), active.as_slice()) {
            (Some("idle"), []) => None,
            (Some("active"), [turn]) => Some(
                turn["id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or("missing active turn")?
                    .to_string(),
            ),
            _ => return Err("pending: inconsistent thread state".into()),
        };
        let method = if let Some(id) = &expected {
            params["expectedTurnId"] = json!(id);
            "turn/steer"
        } else {
            "turn/start"
        };
        self.uncertain = true; // A lost/partial response never authorizes a resend.
        let result = match self.rpc.as_mut().unwrap().call(method, params, guard) {
            Ok(result) => result,
            Err(error) => {
                if method == "turn/steer" && error.definite_rejection() {
                    self.uncertain = false;
                    return Err("failed: turn changed before input; refresh and retry".into());
                }
                return Err(format!("outcome_unknown: {}", String::from(error)));
            }
        };
        let turn = result["turn"]["id"]
            .as_str()
            .or_else(|| result["turnId"].as_str())
            .filter(|s| !s.is_empty())
            .ok_or("outcome_unknown: missing turn receipt")?;
        if expected.as_ref().is_some_and(|id| id != turn)
            || result
                .get("threadId")
                .is_some_and(|id| id != &self.thread_id)
        {
            return Err("outcome_unknown: receipt identity mismatch".into());
        }
        self.uncertain = false;
        Ok(result)
    }

    pub fn interrupt(&mut self, guard: &HostOperationLock) -> Result<Value, String> {
        let state = self.transcript(guard)?;
        let turns = state["thread"]["turns"]
            .as_array()
            .ok_or("unverified turns")?;
        let active: Vec<_> = turns
            .iter()
            .filter(|t| t["status"] == "inProgress")
            .collect();
        let [turn] = active.as_slice() else {
            return Err("no single active turn to cancel".into());
        };
        self.rpc
            .as_mut()
            .unwrap()
            .call(
                "turn/interrupt",
                json!({"threadId":self.thread_id,"turnId":turn["id"]}),
                guard,
            )
            .map_err(String::from)
    }

    pub fn approval(
        &mut self,
        request: &Value,
        accept: bool,
        guard: &HostOperationLock,
    ) -> Result<Value, String> {
        let rpc = self.rpc.as_mut().ok_or("host connection unavailable")?;
        let index = rpc
            .requests
            .iter()
            .position(|r| &r["id"] == request)
            .ok_or("approval is no longer pending")?;
        if rpc.requests[index]["params"]["threadId"].as_str() != Some(&self.thread_id) {
            return Err("approval thread identity mismatch".into());
        }
        if !matches!(
            rpc.requests[index]["method"].as_str(),
            Some("item/commandExecution/requestApproval" | "item/fileChange/requestApproval")
        ) {
            return Err("unsupported interactive request".into());
        }
        rpc.write(
            &json!({"id":request,"result":{"decision":if accept {"accept"} else {"decline"}}}),
            guard,
        )?;
        rpc.requests.remove(index);
        Ok(json!({"submitted":true}))
    }
}

impl Drop for HostProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let deadline = Instant::now() + Duration::from_secs(1);
            while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        let _ = std::fs::remove_file(&self.socket);
    }
}

#[derive(Debug)]
enum RpcError {
    Transport(String),
    Rejected(Value),
}
impl From<String> for RpcError {
    fn from(message: String) -> Self {
        Self::Transport(message)
    }
}
impl From<&str> for RpcError {
    fn from(message: &str) -> Self {
        Self::Transport(message.into())
    }
}
impl From<RpcError> for String {
    fn from(error: RpcError) -> Self {
        match error {
            RpcError::Transport(message) => message,
            // Preserve the error object for classification; never expose arbitrary host text.
            RpcError::Rejected(_) => "host rejected request; reconcile before retrying".into(),
        }
    }
}
impl RpcError {
    fn definite_rejection(&self) -> bool {
        let Self::Rejected(error) = self else {
            return false;
        };
        let message = error["message"]
            .as_str()
            .unwrap_or_default()
            .to_ascii_lowercase();
        error["code"] == -32600
            && (message == "no active turn to steer"
                || (message.contains("expected turn id") && message.contains("actual turn id")))
    }
}

struct Rpc {
    socket: WebSocket,
    events: VecDeque<Value>,
    requests: Vec<Value>,
    truncated: bool,
}
impl Rpc {
    fn write(&mut self, value: &Value, guard: &HostOperationLock) -> Result<(), String> {
        self.socket.send(
            1,
            &serde_json::to_vec(value).map_err(|e| e.to_string())?,
            guard,
        )
    }
    fn read(&mut self, guard: &HostOperationLock) -> Result<Value, String> {
        let message = self.socket.read(guard)?;
        let value: Value = serde_json::from_slice(&message).map_err(|_| "malformed host frame")?;
        if !value.is_object() {
            return Err("host frame must contain one JSON object".into());
        }
        Ok(value)
    }
    fn call(
        &mut self,
        method: &str,
        params: Value,
        guard: &HostOperationLock,
    ) -> Result<Value, RpcError> {
        let id = uuid::Uuid::new_v4().to_string();
        self.write(&json!({"id":id,"method":method,"params":params}), guard)?;
        for _ in 0..128 {
            let frame = self.read(guard)?;
            if frame.get("method").is_some() {
                if frame.get("id").is_some() {
                    if self.requests.len() >= 16 {
                        return Err("host interactive request limit reached".into());
                    }
                    self.requests.push(frame);
                } else {
                    if self.events.len() == 64 {
                        self.events.pop_front();
                        self.truncated = true;
                    }
                    self.events.push_back(frame);
                }
            } else if frame["id"] == id {
                if frame.get("error").is_some() {
                    return Err(RpcError::Rejected(frame["error"].clone()));
                }
                return frame
                    .get("result")
                    .cloned()
                    .ok_or("missing host result".into());
            } else {
                return Err("uncorrelated host response".into());
            }
        }
        Err("host event limit reached".into())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    pub(crate) fn fixture(
        root: &std::path::Path,
    ) -> taurhaus_lib::session_scanner::launch::HostedLaunch {
        let executable = root.join("codex");
        std::fs::write(&executable, r#"#!/usr/bin/python3
import json, os, socket, sys, threading, fcntl, base64, hashlib, struct
root = os.environ['CODEX_HOME']
address = sys.argv[sys.argv.index('--listen')+1].removeprefix('unix://')
saved = os.path.join(root, 'thread.json')
thread = json.load(open(saved)) if os.path.exists(saved) else None
lock = threading.Lock()
def client(connection):
    global thread
    with connection, connection.makefile('rwb') as stream:
        # Regression: cadd533e spoke NDJSON; Codex 0.153.4 closes it with EOF.
        if stream.readline() != b'GET / HTTP/1.1\r\n': return
        headers = {}
        while True:
            line = stream.readline()
            if line == b'\r\n': break
            key, value = line.decode().split(':', 1)
            headers[key.lower()] = value.strip()
        assert headers['upgrade'].lower() == 'websocket'
        assert headers['connection'].lower() == 'upgrade'
        assert headers['sec-websocket-version'] == '13'
        assert len(base64.b64decode(headers['sec-websocket-key'])) == 16
        accept = base64.b64encode(hashlib.sha1((headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode()
        if os.environ.get('FAKE_WIRE') == 'bad_accept': accept = 'wrong'
        stream.write(('HTTP/1.1 101 Switching Protocols\r\nconnection: Upgrade\r\nupgrade: websocket\r\nsec-websocket-accept: '+accept+'\r\n\r\n').encode()); stream.flush()
        def frame(opcode, payload, fin=True):
            n = len(payload)
            header = bytes([(128 if fin else 0) | opcode])
            header += bytes([n]) if n < 126 else (b'\x7e'+struct.pack('!H',n) if n <= 65535 else b'\x7f'+struct.pack('!Q',n))
            stream.write(header+payload); stream.flush()
        def receive():
            header = stream.read(2)
            if not header: return 8, b''
            assert header[0] & 128 and header[1] & 128, 'client must mask'
            n = header[1] & 127
            if n == 126: n = struct.unpack('!H', stream.read(2))[0]
            if n == 127: n = struct.unpack('!Q', stream.read(8))[0]
            assert n <= 65536
            mask, payload = stream.read(4), stream.read(n)
            return header[0] & 15, bytes(b ^ mask[i%4] for i,b in enumerate(payload))
        def emit(value):
            payload = json.dumps(value).encode()
            if os.environ.get('FAKE_WIRE') == 'fragment_ping':
                frame(1, payload[:3], False); frame(9, b'probe')
                assert receive() == (10, b'probe')
                frame(0, payload[3:])
            else: frame(1, payload)
        initialized = False
        while True:
            opcode, payload = receive()
            if opcode == 8:
                if payload: frame(8, payload)
                return
            assert opcode == 1
            request = json.loads(payload)
            if not initialized:
                assert request['method'] == 'initialize'
                assert request['params']['capabilities']['experimentalApi'] is True
                initialized = True
                wire = os.environ.get('FAKE_WIRE')
                if wire == 'close':
                    frame(8, struct.pack('!H',1000)); assert receive() == (8, struct.pack('!H',1000)); return
                if wire == 'oversize':
                    stream.write(b'\x81\x7f'+struct.pack('!Q',65537)); stream.flush(); return
            assert 'jsonrpc' not in request
            if 'id' not in request: continue
            if 'method' not in request:
                thread['approvalDecision'] = request['result']['decision']
                thread['canAcceptDirectInput'] = True
                thread['status']['activeFlags'] = []
                continue
            method, params = request.get('method'), request.get('params', {})
            result, error, approval = {}, None, None
            with lock:
                if method == 'initialize':
                    result = {'userAgent':'taurhaus_host/'+os.environ.get('FAKE_BUILD','0.153.4'), 'codexHome':root}
                    if os.environ.get('FAKE_RUNTIME'):
                        with open(os.environ['FAKE_RUNTIME'], 'r+') as runtime:
                            fcntl.flock(runtime, fcntl.LOCK_EX)
                            record = json.load(runtime); record['foreignClaim'] = 'concurrent'
                            runtime.seek(0); json.dump(record, runtime); runtime.truncate()

                elif method == 'thread/start':
                    assert thread is None
                    thread = {'id':'owned-thread', 'status':{'type':'idle','activeFlags':[]}, 'canAcceptDirectInput':True, 'turns':[]}
                    result = {'thread':thread}
                elif method in ('thread/resume', 'thread/read'):
                    if thread is None or params['threadId'] != thread['id']: error = {'code':-32600,'message':'unknown thread'}
                    else: result = {'thread':thread}
                elif method == 'turn/start':
                    assert params['threadId'] == thread['id']
                    turn = {'id':str(len(thread['turns'])+1), 'status':'completed', 'items':[{'type':'userMessage','content':params['input']}]}
                    thread['turns'].append(turn)
                    text = params['input'][0]['text']
                    if text == 'active':
                        turn['status'] = 'inProgress'
                        thread['status']['type'] = 'active'
                    if text in ('approval', 'foreign approval'):
                        turn['status'] = 'inProgress'
                        thread['status'] = {'type':'active','activeFlags':['waitingOnApproval']}
                        thread['canAcceptDirectInput'] = False
                        approval = {'id':'permission-1','method':'item/commandExecution/requestApproval','params':{'threadId':thread['id'] if text == 'approval' else 'foreign-thread','command':'echo marker'}}
                    result = {'turn':turn, 'threadId':thread['id']}
                elif method == 'turn/steer':
                    assert params['expectedTurnId'] == thread['turns'][-1]['id']
                    text = params['input'][0]['text']
                    if text == 'completion race':
                        error = {'code':-32600, 'message':'no active turn to steer'}
                        thread['turns'][-1]['status'] = 'completed'
                        thread['status']['type'] = 'idle'
                    elif text == 'wrong turn':
                        error = {'code':-32600, 'message':'expected turn id 1 but actual turn id 2'}
                    elif text == 'unclassified':
                        error = {'code':-32600, 'message':'unclassified rejection'}
                    else: result = {'turnId':params['expectedTurnId']}
                elif method == 'turn/interrupt':
                    thread['turns'][-1]['status'] = 'interrupted'
                    thread['status'] = {'type':'idle','activeFlags':[]}
                    thread['canAcceptDirectInput'] = True
                    result = {}
                else: error = {'code':-32601,'message':'unsupported fake method'}
                if thread is not None:
                    with open(saved, 'w') as output: json.dump(thread, output)
                if method == 'turn/start' and params['input'][0]['text'] == 'disconnect': return
                reply = {'id':request['id'], 'error':error} if error else {'id':request['id'], 'result':result}
                emit(reply)
                if approval: emit(approval)
with socket.socket(socket.AF_UNIX) as listener:
    listener.bind(address); listener.listen(4)
    while True:
        connection, _ = listener.accept()
        threading.Thread(target=client, args=(connection,), daemon=True).start()
"#).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let command = format!(
            "CODEX_HOME='{}' '{}' --sandbox read-only --ask-for-approval never",
            root.display(),
            executable.display()
        );
        taurhaus_lib::session_scanner::launch::HostedLaunch::from_rendered(&command, root, None)
            .unwrap()
    }
    fn spawn(
        launch: &HostedLaunch,
        root: &Path,
        resume: Option<&str>,
        guard: &HostOperationLock,
    ) -> Result<HostProcess, String> {
        let socket = root.join(format!("{}.sock", uuid::Uuid::new_v4().simple()));
        HostProcess::launch(launch, root, &socket, resume, guard)
    }
    #[test]
    fn fake_host_round_trip_named_resume_and_owned_cleanup() {
        let tmp = tempfile::tempdir().unwrap();
        let launch = fixture(tmp.path());
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let mut host = spawn(&launch, tmp.path(), None, &guard).unwrap();
        assert_eq!(host.thread_id, "owned-thread");
        assert_eq!(
            host.input("operator marker", &guard).unwrap()["turn"]["id"],
            "1"
        );
        let original = host.transcript(&guard).unwrap();
        assert!(original.to_string().contains("operator marker"));
        let pid = host.child.id();
        drop(host);
        assert!(taurhaus_lib::platform::process_start_ticks(pid).is_none());
        let mut resumed = spawn(&launch, tmp.path(), Some("owned-thread"), &guard).unwrap();
        assert_eq!(
            resumed.transcript(&guard).unwrap()["thread"],
            original["thread"]
        );
        assert!(spawn(&launch, tmp.path(), Some("wrong-thread"), &guard).is_err());
    }
    #[test]
    fn fake_host_lost_input_response_never_replays() {
        let tmp = tempfile::tempdir().unwrap();
        let launch = fixture(tmp.path());
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let mut host = spawn(&launch, tmp.path(), None, &guard).unwrap();
        assert!(host.input("disconnect", &guard).is_err());
        assert!(host.input("must not replay", &guard).is_err());
        let persisted = std::fs::read_to_string(tmp.path().join("thread.json")).unwrap();
        assert!(persisted.contains("disconnect"));
        assert!(!persisted.contains("must not replay"));
    }
    #[test]
    fn fake_host_permission_wait_deny_steer_and_cancel_preserve_thread() {
        let tmp = tempfile::tempdir().unwrap();
        let launch = fixture(tmp.path());
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let mut host = spawn(&launch, tmp.path(), None, &guard).unwrap();
        host.input("approval", &guard).unwrap();
        assert!(host.input("blocked by approval", &guard).is_err());
        host.approval(&json!("permission-1"), false, &guard)
            .unwrap();
        assert_eq!(
            host.transcript(&guard).unwrap()["thread"]["approvalDecision"],
            "decline"
        );
        assert_eq!(host.input("steer marker", &guard).unwrap()["turnId"], "1");
        host.interrupt(&guard).unwrap();
        assert_eq!(
            host.transcript(&guard).unwrap()["thread"]["status"]["type"],
            "idle"
        );
        host.input("foreign approval", &guard).unwrap();
        host.transcript(&guard).unwrap();
        // Regression: 9b50346b checked the approval ID but not its thread identity.
        assert!(host.approval(&json!("permission-1"), true, &guard).is_err());
    }
    #[test]
    fn websocket_upgrade_masking_fragmentation_and_ping() {
        // Regression: cadd533e used NDJSON, which the pinned Unix endpoint closes with EOF.
        let tmp = tempfile::tempdir().unwrap();
        let mut launch = fixture(tmp.path());
        launch
            .environment
            .insert("FAKE_WIRE".into(), "fragment_ping".into());
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let mut host = spawn(&launch, tmp.path(), None, &guard).unwrap();
        assert_eq!(
            host.transcript(&guard).unwrap()["thread"]["id"],
            "owned-thread"
        );
    }

    #[test]
    fn websocket_refuses_bad_accept_close_and_oversize_before_thread_start() {
        for wire in ["bad_accept", "close", "oversize"] {
            let tmp = tempfile::tempdir().unwrap();
            let mut launch = fixture(tmp.path());
            launch.environment.insert("FAKE_WIRE".into(), wire.into());
            let guard =
                HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
            assert!(spawn(&launch, tmp.path(), None, &guard).is_err(), "{wire}");
            assert!(!tmp.path().join("thread.json").exists());
        }
    }

    #[test]
    fn fake_host_unknown_build_refuses_before_thread_creation() {
        let tmp = tempfile::tempdir().unwrap();
        let mut launch = fixture(tmp.path());
        launch
            .environment
            .insert("FAKE_BUILD".into(), "unreviewed".into());
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        // Regression: 9b50346b parsed any nonempty build as usable for native input.
        assert!(spawn(&launch, tmp.path(), None, &guard).is_err());
        assert!(!tmp.path().join("thread.json").exists());
    }
}
