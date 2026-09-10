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
    pub attach_config: Value,
    pub instruction_sources: Vec<String>,
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
            attach_config: Value::Null,
            instruction_sources: Vec::new(),
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
                    policy: None,
                    repairing: false,
                    policy_dirty: false,
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
        if host.build != taurhaus_lib::session_scanner::launch::HostedDescriptor::codex().build {
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
        host.instruction_sources = instruction_sources(&result);
        if !host.instruction_sources.is_empty() {
            tracing::info!(event = "hosted.instruction_sources.loaded",
                thread_id = %host.thread_id, count = host.instruction_sources.len(),
                "Host loaded project instructions; strict TUI policy remains enforced");
            taurhaus_lib::logging::emit_global(
                "info",
                "coordination",
                "hosted.instruction_sources.loaded",
                Some("Host loaded project instructions; strict TUI policy remains enforced".into()),
                serde_json::Map::from_iter([
                    ("thread_id".into(), json!(host.thread_id)),
                    ("count".into(), json!(host.instruction_sources.len())),
                ]),
            );
        }
        let sandbox = match result["sandbox"]["type"].as_str() {
            Some("readOnly") => "read-only",
            Some("workspaceWrite") => "workspace-write",
            Some("dangerFullAccess") => "danger-full-access",
            _ => return Err("unsupported effective host sandbox".into()),
        };
        let model = result["model"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("missing effective host model")?;
        let effort = result["reasoningEffort"]
            .as_str()
            .ok_or("missing effective host effort")?;
        // RPC enums and config enums need not share their spelling. Keep the
        // original response for settings comparisons; use config/request values below.
        let approval = match result["approvalPolicy"].as_str() {
            Some("untrusted" | "unlessTrusted") => "untrusted",
            Some("onFailure" | "on-failure") => "on-failure",
            Some("onRequest" | "on-request") => "on-request",
            Some("never") => "never",
            _ => return Err("unsupported effective host approval policy".into()),
        };
        host.attach_config = json!({"model":model, "model_reasoning_effort":effort,
            "sandbox_mode":sandbox, "approval_policy":approval});
        if sandbox == "workspace-write" {
            let mut workspace = serde_json::Map::new();
            for (wire, config) in [
                ("networkAccess", "network_access"),
                ("writableRoots", "writable_roots"),
                ("excludeTmpdirEnvVar", "exclude_tmpdir_env_var"),
                ("excludeSlashTmp", "exclude_slash_tmp"),
            ] {
                let value = &result["sandbox"][wire];
                if !value.is_null() {
                    workspace.insert(config.into(), value.clone());
                }
            }
            host.attach_config["sandbox_workspace_write"] = workspace.into();
        }
        let settings = json!({"model":model, "effort":effort,
            "approvalPolicy":result["approvalPolicy"], "sandboxPolicy":result["sandbox"]});
        let resume = json!({"threadId":host.thread_id, "cwd":cwd, "model":model,
            "approvalPolicy":approval, "sandbox":sandbox,
            "config":host.attach_config});
        // Only the attached view suppresses its own project-document discovery.
        // Never push that suppression back into the daemon-owned thread.
        host.attach_config["project_doc_max_bytes"] = json!(0);
        rpc.policy = Some((settings, resume, host.instruction_sources.clone()));
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

fn instruction_sources(result: &Value) -> Vec<String> {
    result["instructionSources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

struct Rpc {
    socket: WebSocket,
    events: VecDeque<Value>,
    requests: Vec<Value>,
    truncated: bool,
    policy: Option<(Value, Value, Vec<String>)>,
    repairing: bool,
    policy_dirty: bool,
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
                    if frame["method"] == "thread/settings/updated" {
                        if let Some((expected, resume, _)) = &self.policy {
                            if frame["params"]["threadId"] == resume["threadId"] {
                                let settings = &frame["params"]["threadSettings"];
                                let differs = expected
                                    .as_object()
                                    .unwrap()
                                    .iter()
                                    .any(|(k, v)| settings[k] != *v);
                                if differs {
                                    if self.repairing {
                                        return Err(
                                            "host settings diverged during reassertion".into()
                                        );
                                    }
                                    self.policy_dirty = true;
                                    tracing::warn!(event = "hosted.settings.diverged", thread_id = %resume["threadId"], "attached TUI changed owned thread policy; reasserting");
                                    taurhaus_lib::logging::emit_global(
                                        "warn",
                                        "coordination",
                                        "hosted.settings.diverged",
                                        Some(
                                            "Attached TUI changed owned thread policy; reasserting"
                                                .into(),
                                        ),
                                        serde_json::Map::from_iter([(
                                            "thread_id".into(),
                                            resume["threadId"].clone(),
                                        )]),
                                    );
                                }
                            }
                        }
                    }
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
                let result = frame.get("result").cloned().ok_or("missing host result")?;
                if self.policy_dirty && !self.repairing {
                    // Finish the in-flight response first: never lose correlation to a
                    // nested repair. One repair only, under this same bounded deadline.
                    let params = self.policy.as_ref().unwrap().1.clone();
                    self.repairing = true;
                    let repaired = self.call("thread/resume", params, guard);
                    self.repairing = false;
                    let repaired = repaired?;
                    let (expected, resume, sources) = self.policy.as_ref().unwrap();
                    if repaired["thread"]["id"] != resume["threadId"]
                        || repaired["model"] != expected["model"]
                        || repaired["reasoningEffort"] != expected["effort"]
                        || repaired["approvalPolicy"] != expected["approvalPolicy"]
                        || repaired["sandbox"] != expected["sandboxPolicy"]
                        || instruction_sources(&repaired) != *sources
                    {
                        return Err("host did not restore owned thread policy".into());
                    }
                    self.policy_dirty = false;
                }
                return Ok(result);
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
policy = {'model':'fake-model', 'reasoningEffort':'low', 'approvalPolicy':'never', 'sandbox':{'type':'readOnly','networkAccess':False}, 'instructionSources':[]}
policy.update(json.loads(os.environ.get('FAKE_POLICY', '{}')))
for i, arg in enumerate(sys.argv[:-1]):
    if arg == '-c':
        key, value = sys.argv[i+1].split('=',1)
        if key in ('model','model_reasoning_effort'):
            policy['model' if key == 'model' else 'reasoningEffort'] = json.loads(value)
if '--remote' in sys.argv:
    import signal, tomllib
    assert 'TMUX' not in os.environ
    assert '--strict-config' in sys.argv
    config = tomllib.load(open(os.path.join(root, 'config.toml'), 'rb'))
    assert config['model'] == policy['model']
    assert config['model_reasoning_effort'] == policy['reasoningEffort']
    assert config['project_doc_max_bytes'] == 0
    assert not {'personality', 'developer_instructions', 'projects'} & config.keys()
    account_root = os.path.dirname(os.path.realpath(sys.argv[0]))
    thread_id = sys.argv[sys.argv.index('resume')+1]
    assert json.load(open(os.path.join(account_root, 'thread.json')))['id'] == thread_id
    with open(os.path.join(account_root, 'attach-events.jsonl'), 'a') as output:
        output.write(json.dumps({'argv':sys.argv, 'codexHome':root, 'tmux':os.environ.get('TMUX')})+'\n')
    signal.pause()
    sys.exit(0)
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
                    with open(os.path.join(root, 'start.json'), 'w') as output: json.dump(params, output)
                    thread = {'id':'owned-thread', 'status':{'type':'idle','activeFlags':[]}, 'canAcceptDirectInput':True, 'turns':[]}
                    result = dict(policy, thread=thread)
                elif method in ('thread/resume', 'thread/read'):
                    if thread is None or params['threadId'] != thread['id']: error = {'code':-32600,'message':'unknown thread'}
                    else:
                        result = dict(policy, thread=thread)
                        marker = os.path.join(root, 'drift.json')
                        if method == 'thread/read' and os.path.exists(marker):
                            settings = json.load(open(marker)); os.unlink(marker)
                            emit({'method':'thread/settings/updated', 'params':{'threadId':thread['id'], 'threadSettings':settings}})
                        if method == 'thread/resume':
                            if os.path.exists(os.path.join(root, 'changed-instructions')): result['instructionSources'] = []
                            if os.path.exists(os.path.join(root, 'reject-repair')): error = {'code':-32600,'message':'repair refused'}
                            with open(os.path.join(root, 'reassert.json'), 'w') as output: json.dump(params, output)
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
    fn hosted_workspace_write_config_round_trips_optional_fields() {
        // Regression: efb1ddb8 copied missing workspace sandbox fields as TOML nulls.
        for sandbox in [
            json!({"type":"workspaceWrite", "networkAccess":false}),
            json!({"type":"workspaceWrite", "networkAccess":true, "writableRoots":[],
                "excludeTmpdirEnvVar":true, "excludeSlashTmp":false}),
            json!({"type":"workspaceWrite", "networkAccess":null, "writableRoots":null}),
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let mut launch = fixture(tmp.path());
            launch
                .environment
                .insert("FAKE_POLICY".into(), json!({"sandbox":sandbox}).to_string());
            let guard =
                HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
            let host = spawn(&launch, tmp.path(), None, &guard).unwrap();
            let home = tmp.path().join("tui");
            launch
                .prepare_attach_home(&home, &host.attach_config)
                .unwrap();
            let config: toml::Value =
                toml::from_str(&std::fs::read_to_string(home.join("config.toml")).unwrap())
                    .unwrap();
            assert_eq!(config["sandbox_mode"].as_str(), Some("workspace-write"));
            let table = config["sandbox_workspace_write"].as_table().unwrap();
            for (wire, key) in [
                ("networkAccess", "network_access"),
                ("writableRoots", "writable_roots"),
                ("excludeTmpdirEnvVar", "exclude_tmpdir_env_var"),
                ("excludeSlashTmp", "exclude_slash_tmp"),
            ] {
                if sandbox[wire].is_null() {
                    assert!(!table.contains_key(key));
                } else {
                    assert_eq!(serde_json::to_value(&table[key]).unwrap(), sandbox[wire]);
                }
            }
        }
    }

    #[test]
    fn hosted_approval_policy_normalizes_config_and_repair() {
        // Regression: efb1ddb8 wrote unchecked RPC approval enums straight into TOML.
        for (wire, config) in [
            ("never", "never"),
            ("untrusted", "untrusted"),
            ("onRequest", "on-request"),
            ("onFailure", "on-failure"),
            ("on-request", "on-request"),
            ("on-failure", "on-failure"),
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let mut launch = fixture(tmp.path());
            launch.environment.insert(
                "FAKE_POLICY".into(),
                json!({"approvalPolicy":wire}).to_string(),
            );
            let guard =
                HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
            let mut host = spawn(&launch, tmp.path(), None, &guard).unwrap();
            assert_eq!(host.attach_config["approval_policy"], config);
            std::fs::write(tmp.path().join("drift.json"), "{}").unwrap();
            host.transcript(&guard).unwrap();
            let params: Value = serde_json::from_str(
                &std::fs::read_to_string(tmp.path().join("reassert.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(params["approvalPolicy"], config);
            assert!(!host.rpc.as_ref().unwrap().policy_dirty);
        }
    }

    #[test]
    fn hosted_approval_policy_refuses_unknown_variant() {
        // Regression: efb1ddb8 accepted arbitrary approval strings into strict TUI config.
        let tmp = tempfile::tempdir().unwrap();
        let mut launch = fixture(tmp.path());
        launch.environment.insert(
            "FAKE_POLICY".into(),
            json!({"approvalPolicy":"futurePolicy"}).to_string(),
        );
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        assert_eq!(
            spawn(&launch, tmp.path(), None, &guard).err().as_deref(),
            Some("unsupported effective host approval policy")
        );
    }

    #[test]
    fn hosted_attach_config_omits_unattested_settings_and_rpc_overrides() {
        // Regression: efb1ddb8 added unprobed strict-config keys and host instruction overrides.
        let tmp = tempfile::tempdir().unwrap();
        let launch = fixture(tmp.path());
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let host = spawn(&launch, tmp.path(), None, &guard).unwrap();
        for key in ["personality", "developer_instructions", "projects"] {
            assert!(
                host.attach_config.get(key).is_none(),
                "unattested config key: {key}"
            );
        }
        assert_eq!(host.attach_config["project_doc_max_bytes"], 0);
        let params: Value =
            serde_json::from_str(&std::fs::read_to_string(tmp.path().join("start.json")).unwrap())
                .unwrap();
        assert!(params.get("developerInstructions").is_none());
        assert!(params.get("config").is_none());
    }

    #[test]
    fn hosted_instruction_sources_log_info_and_continue() {
        // Regression: 9d358935 refused project instructions; ef8f6ce9 logged expected loading at WARN.
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().unwrap();
        let sink =
            taurhaus_lib::logging::LogFileState::new(tmp.path().join("events.jsonl")).unwrap();
        taurhaus_lib::logging::install_global_sink(&sink);
        let mut launch = fixture(tmp.path());
        launch.environment.insert(
            "FAKE_POLICY".into(),
            json!({"instructionSources":[tmp.path().join("AGENTS.md")]}).to_string(),
        );
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let host =
            spawn(&launch, tmp.path(), None, &guard).expect("project instructions must be allowed");
        assert_eq!(
            host.instruction_sources,
            vec![tmp.path().join("AGENTS.md").to_string_lossy().into_owned()]
        );
        assert_eq!(host.attach_config["model"], "fake-model");
        assert_eq!(host.attach_config["model_reasoning_effort"], "low");
        assert_eq!(host.attach_config["sandbox_mode"], "read-only");
        assert_eq!(host.attach_config["approval_policy"], "never");
        sink.flush_for_test().unwrap();
        let events = std::fs::read_to_string(tmp.path().join("events.jsonl")).unwrap();
        assert_eq!(
            events.matches("hosted.instruction_sources.loaded").count(),
            1
        );
        let event: Value = serde_json::from_str(
            events
                .lines()
                .find(|line| line.contains("hosted.instruction_sources.loaded"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(event["level"], "INFO");
        assert!(
            !events.contains("AGENTS.md"),
            "source contents/paths are not logged"
        );
    }

    #[test]
    fn hosted_instruction_sources_survive_settings_repair() {
        // Regression: ef8f6ce9 allowed instructions but repair reused the TUI's discovery suppression.
        for changed in [false, true] {
            let tmp = tempfile::tempdir().unwrap();
            let mut launch = fixture(tmp.path());
            launch.environment.insert(
                "FAKE_POLICY".into(),
                json!({"instructionSources":["AGENTS.md"]}).to_string(),
            );
            let guard =
                HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
            let mut host = spawn(&launch, tmp.path(), None, &guard).unwrap();
            std::fs::write(
                tmp.path().join("drift.json"),
                json!({"personality":"friendly"}).to_string(),
            )
            .unwrap();
            if changed {
                std::fs::write(tmp.path().join("changed-instructions"), "").unwrap();
            }
            let result = host.transcript(&guard);
            if changed {
                assert!(
                    result.is_err(),
                    "changed instruction evidence must fail closed"
                );
                assert!(host.rpc.as_ref().unwrap().policy_dirty);
            } else {
                assert_eq!(result.unwrap()["instructionSources"], json!(["AGENTS.md"]));
                assert_eq!(
                    serde_json::to_value(&host.instruction_sources).unwrap(),
                    json!(["AGENTS.md"])
                );
                let params: Value = serde_json::from_str(
                    &std::fs::read_to_string(tmp.path().join("reassert.json")).unwrap(),
                )
                .unwrap();
                assert!(params["config"].get("project_doc_max_bytes").is_none());
                assert_eq!(host.attach_config["project_doc_max_bytes"], 0);
            }
        }
    }

    #[test]
    fn hosted_instruction_sources_drop_non_string_entries() {
        // Regression: ef8f6ce9 copied untyped host objects into the string-array record contract.
        let tmp = tempfile::tempdir().unwrap();
        let mut launch = fixture(tmp.path());
        launch.environment.insert(
            "FAKE_POLICY".into(),
            json!({"instructionSources":["AGENTS.md", {"path":"other"}, 7]}).to_string(),
        );
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let host = spawn(&launch, tmp.path(), None, &guard).unwrap();
        assert_eq!(
            serde_json::to_value(&host.instruction_sources).unwrap(),
            json!(["AGENTS.md"])
        );
    }

    #[test]
    fn hosted_settings_drift_reasserts_policy_before_returning() {
        // Regression: b4a4b2dd silently queued settings pushes from the attached TUI.
        let tmp = tempfile::tempdir().unwrap();
        let launch = fixture(tmp.path());
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let mut host = spawn(&launch, tmp.path(), None, &guard).unwrap();
        std::fs::write(tmp.path().join("drift.json"), json!({"model":"operator-model", "effort":"high", "approvalPolicy":"on-request", "sandboxPolicy":{"type":"dangerFullAccess"}}).to_string()).unwrap();
        host.transcript(&guard).unwrap();
        let path = tmp.path().join("reassert.json");
        assert!(path.exists(), "settings drift must be reasserted");
        let params: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(params["threadId"], "owned-thread");
        assert_eq!(params["model"], "fake-model");
        assert_eq!(params["config"]["model_reasoning_effort"], "low");
        assert_eq!(params["approvalPolicy"], "never");
        assert_eq!(params["sandbox"], "read-only");
    }

    #[test]
    fn hosted_settings_parity_is_inert_and_failed_repair_stays_gated() {
        // Regression: b4a4b2dd treated attached-client policy drift as an inert event.
        let tmp = tempfile::tempdir().unwrap();
        let launch = fixture(tmp.path());
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let mut host = spawn(&launch, tmp.path(), None, &guard).unwrap();
        let policy = json!({"model":"fake-model", "effort":"low", "approvalPolicy":"never", "sandboxPolicy":{"type":"readOnly","networkAccess":false}});
        std::fs::write(tmp.path().join("drift.json"), policy.to_string()).unwrap();
        host.transcript(&guard).unwrap();
        assert!(!tmp.path().join("reassert.json").exists());
        std::fs::write(tmp.path().join("drift.json"), "{}").unwrap();
        std::fs::write(tmp.path().join("reject-repair"), "").unwrap();
        assert!(host.input("must not submit", &guard).is_err());
        assert!(host.input("still must not submit", &guard).is_err());
        std::fs::remove_file(tmp.path().join("reject-repair")).unwrap();
        let state = host.transcript(&guard).unwrap();
        assert_eq!(state["thread"]["turns"], json!([]));
        assert!(!host.rpc.as_ref().unwrap().policy_dirty);
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
