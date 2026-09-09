//! Owned host process and bounded contract-v1 RPC. Never used for TUI seats.

use crate::coordination::stores::lock::HostOperationLock;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use taurhaus_lib::session_scanner::launch::HostedLaunch;

const FRAME_LIMIT: usize = 65_536;

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
                    reader: BufReader::new(stream),
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
        let result = self.rpc.as_mut().unwrap().call(method, params, guard)?;
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
        self.rpc.as_mut().unwrap().call(
            "turn/interrupt",
            json!({"threadId":self.thread_id,"turnId":turn["id"]}),
            guard,
        )
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

struct Rpc {
    reader: BufReader<UnixStream>,
    events: VecDeque<Value>,
    requests: Vec<Value>,
    truncated: bool,
}
impl Rpc {
    fn write(&mut self, value: &Value, guard: &HostOperationLock) -> Result<(), String> {
        let mut frame = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        frame.push(b'\n');
        if frame.len() > FRAME_LIMIT {
            return Err("host frame exceeds 64 KiB".into());
        }
        let mut remaining = frame.as_slice();
        while !remaining.is_empty() {
            self.reader
                .get_mut()
                .set_write_timeout(Some(guard.remaining().map_err(|e| e.to_string())?))
                .map_err(|e| e.to_string())?;
            let n = self
                .reader
                .get_mut()
                .write(remaining)
                .map_err(|_| "host write failed; outcome may be unknown")?;
            if n == 0 {
                return Err("host connection closed during write".into());
            }
            remaining = &remaining[n..];
        }
        Ok(())
    }
    fn read(&mut self, guard: &HostOperationLock) -> Result<Value, String> {
        let mut frame = Vec::new();
        loop {
            self.reader
                .get_mut()
                .set_read_timeout(Some(guard.remaining().map_err(|e| e.to_string())?))
                .map_err(|e| e.to_string())?;
            let chunk = self
                .reader
                .fill_buf()
                .map_err(|_| "host read failed; outcome may be unknown")?;
            if chunk.is_empty() {
                return Err("host connection closed".into());
            }
            let count = chunk
                .iter()
                .position(|b| *b == b'\n')
                .map_or(chunk.len(), |n| n + 1);
            if frame.len() + count > FRAME_LIMIT {
                return Err("host frame exceeds 64 KiB".into());
            }
            frame.extend_from_slice(&chunk[..count]);
            self.reader.consume(count);
            if frame.last() == Some(&b'\n') {
                return serde_json::from_slice(&frame).map_err(|_| "malformed host frame".into());
            }
        }
    }
    fn call(
        &mut self,
        method: &str,
        params: Value,
        guard: &HostOperationLock,
    ) -> Result<Value, String> {
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
                    return Err("host rejected request; reconcile before retrying".into());
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
import json, os, socket, sys, threading, fcntl
root = os.environ['CODEX_HOME']
address = sys.argv[sys.argv.index('--listen')+1].removeprefix('unix://')
saved = os.path.join(root, 'thread.json')
thread = json.load(open(saved)) if os.path.exists(saved) else None
lock = threading.Lock()
def client(connection):
    global thread
    with connection, connection.makefile('rwb') as stream:
        for line in stream:
            request = json.loads(line)
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
                    result = {'userAgent':'taurhaus_host/0.153.4', 'codexHome':root}
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
                    if text in ('approval', 'foreign approval'):
                        turn['status'] = 'inProgress'
                        thread['status'] = {'type':'active','activeFlags':['waitingOnApproval']}
                        thread['canAcceptDirectInput'] = False
                        approval = {'id':'permission-1','method':'item/commandExecution/requestApproval','params':{'threadId':thread['id'] if text == 'approval' else 'foreign-thread','command':'echo marker'}}
                    result = {'turn':turn, 'threadId':thread['id']}
                elif method == 'turn/steer':
                    assert params['expectedTurnId'] == thread['turns'][-1]['id']
                    result = {'turnId':params['expectedTurnId']}
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
                stream.write((json.dumps(reply)+'\n').encode())
                if approval: stream.write((json.dumps(approval)+'\n').encode())
                stream.flush()
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

    #[test]
    fn fake_host_round_trip_named_resume_and_owned_cleanup() {
        let tmp = tempfile::tempdir().unwrap();
        let launch = fixture(tmp.path());
        let guard = crate::coordination::stores::lock::HostOperationLock::acquire(
            tmp.path(),
            "team",
            "seat",
            std::time::Duration::ZERO,
        )
        .unwrap();
        let mut host = HostProcess::launch(
            &launch,
            tmp.path(),
            &tmp.path().join("a.sock"),
            None,
            &guard,
        )
        .unwrap();
        assert_eq!(host.thread_id, "owned-thread");
        assert_eq!(
            host.input("operator marker", &guard).unwrap()["turn"]["id"],
            "1"
        );
        assert!(host
            .transcript(&guard)
            .unwrap()
            .to_string()
            .contains("operator marker"));
        let pid = host.child.id();
        drop(host);
        assert!(taurhaus_lib::platform::process_start_ticks(pid).is_none());
        let mut resumed = HostProcess::launch(
            &launch,
            tmp.path(),
            &tmp.path().join("b.sock"),
            Some("owned-thread"),
            &guard,
        )
        .unwrap();
        assert!(resumed
            .transcript(&guard)
            .unwrap()
            .to_string()
            .contains("operator marker"));
        assert!(HostProcess::launch(
            &launch,
            tmp.path(),
            &tmp.path().join("c.sock"),
            Some("wrong-thread"),
            &guard
        )
        .is_err());
    }

    #[test]
    fn fake_host_lost_input_response_never_replays() {
        let tmp = tempfile::tempdir().unwrap();
        let launch = fixture(tmp.path());
        let guard = crate::coordination::stores::lock::HostOperationLock::acquire(
            tmp.path(),
            "team",
            "seat",
            std::time::Duration::ZERO,
        )
        .unwrap();
        let mut host = HostProcess::launch(
            &launch,
            tmp.path(),
            &tmp.path().join("a.sock"),
            None,
            &guard,
        )
        .unwrap();
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
        let mut host = HostProcess::launch(&launch, tmp.path(), &tmp.path().join("rpc.sock"), None, &guard).unwrap();
        host.input("approval", &guard).unwrap();
        assert!(host.input("blocked by approval", &guard).is_err());
        host.approval(&json!("permission-1"), false, &guard).unwrap();
        assert_eq!(host.transcript(&guard).unwrap()["thread"]["approvalDecision"], "decline");
        assert_eq!(host.input("steer marker", &guard).unwrap()["turnId"], "1");
        host.interrupt(&guard).unwrap();
        assert_eq!(host.transcript(&guard).unwrap()["thread"]["status"]["type"], "idle");
        host.input("foreign approval", &guard).unwrap();
        host.transcript(&guard).unwrap();
        // Regression: 9b50346b checked the approval ID but not its thread identity.
        assert!(host.approval(&json!("permission-1"), true, &guard).is_err());
    }

}
