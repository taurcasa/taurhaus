//! Owned host process and bounded contract-v1 RPC over Unix WebSockets.

use crate::coordination::stores::lock::HostOperationLock;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::{ErrorKind, Read};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
use taurhaus_lib::session_scanner::launch::HostedLaunch;

#[path = "hosted_websocket.rs"]
mod websocket;
use websocket::WebSocket;

struct StderrTail {
    bytes: Arc<Mutex<VecDeque<u8>>>,
    stop: Arc<AtomicBool>,
    reader: Option<std::thread::JoinHandle<()>>,
}
impl StderrTail {
    fn start(mut pipe: std::process::ChildStderr) -> std::io::Result<Self> {
        // fcntl(O_NONBLOCK) also works on pipes; no raw descriptor ownership transfer.
        socket2::SockRef::from(&pipe).set_nonblocking(true)?;
        let bytes = Arc::new(Mutex::new(VecDeque::with_capacity(4096)));
        let stop = Arc::new(AtomicBool::new(false));
        let (buffer, stopping) = (bytes.clone(), stop.clone());
        let reader = std::thread::Builder::new()
            .name("host-stderr".into())
            .spawn(move || {
                let mut chunk = [0; 4096];
                let mut deadline = None;
                loop {
                    if stopping.load(Ordering::Relaxed) {
                        let end = deadline
                            .get_or_insert_with(|| Instant::now() + Duration::from_millis(50));
                        if Instant::now() >= *end {
                            break;
                        }
                    }
                    match pipe.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            let mut tail = buffer.lock().unwrap();
                            let excess = (tail.len() + n).saturating_sub(4096);
                            tail.drain(..excess);
                            tail.extend(&chunk[..n]);
                        }
                        Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                        Err(e) if e.kind() == ErrorKind::WouldBlock => {
                            if stopping.load(Ordering::Relaxed) {
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            })?;
        Ok(Self {
            bytes,
            stop,
            reader: Some(reader),
        })
    }
    fn sanitized(&self) -> String {
        let bytes: Vec<_> = self.bytes.lock().unwrap().iter().copied().collect();
        sanitize_stderr(&bytes)
    }
    fn finish(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
impl Drop for StderrTail {
    fn drop(&mut self) {
        self.finish();
    }
}

fn sanitize_stderr(bytes: &[u8]) -> String {
    // A ring cut can remove a credential's prefix: discard the leading fragment.
    let bytes = if bytes.len() == 4096 {
        let start = bytes
            .iter()
            .position(|b| b.is_ascii_whitespace() || b.is_ascii_control())
            .unwrap_or(bytes.len());
        &bytes[start..]
    } else {
        bytes
    };
    let mut text = String::new();
    let mut escape = 0;
    for c in String::from_utf8_lossy(bytes).chars() {
        match (escape, c) {
            (0, '\u{1b}') => escape = 1,
            (0, '\u{9b}') => escape = 2,
            (0, '\u{9d}') => escape = 3,
            (1, '[') => escape = 2,
            (1, ']' | 'P' | '^' | '_') => escape = 3,
            (1, ' '..='/') => {}
            (1, _) | (2, '@'..='~') | (3, '\u{7}') | (4, '\\') => escape = 0,
            (3, '\u{1b}') => escape = 4,
            (4, _) => escape = 3,
            (0, c) if c.is_whitespace() => text.push(' '),
            (0, c) if !c.is_control() => text.push(c),
            _ => {}
        }
    }
    let mut cursor = 0;
    while cursor < text.len() {
        let lower = text[cursor..].to_ascii_lowercase();
        let found = [
            "sk-",
            "bearer ",
            "\"access_token\"",
            "\"api_key\"",
            "'access_token'",
        ]
        .iter()
        .filter_map(|key| lower.find(key).map(|i| (i, *key)))
        .min_by_key(|v| v.0);
        let Some((offset, key)) = found else {
            break;
        };
        let start = cursor + offset;
        let value = if key == "sk-" {
            start
        } else {
            start + key.len() + text[start + key.len()..].len()
                - text[start + key.len()..]
                    .trim_start_matches([' ', ':', '='])
                    .len()
        };
        let rest = &text[value..];
        let end = if rest.starts_with('"') {
            let mut json = serde_json::Deserializer::from_str(rest).into_iter::<String>();
            if json.next().is_some_and(|v| v.is_ok()) {
                value + json.byte_offset()
            } else {
                text.len()
            }
        } else if let Some(quoted) = rest.strip_prefix('\'') {
            quoted.find('\'').map_or(text.len(), |i| value + i + 2)
        } else {
            rest.find([' ', '\"', '\'', ',', ';', '}', ']'])
                .map_or(text.len(), |i| value + i)
        };
        text.replace_range(start..end, "[redacted]");
        cursor = start + "[redacted]".len();
    }
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let start = text.char_indices().rev().nth(511).map_or(0, |(i, _)| i);
    text[start..].into()
}

pub(crate) struct HostProcess {
    child: Child,
    stderr: Option<StderrTail>,
    identity: (String, String),
    exit_logged: bool,
    rpc: Option<Rpc>,
    pub thread_id: String,
    pub attach_config: Value,
    pub instruction_sources: Vec<String>,
    pub build: String,
    pub process_start: String,
    socket: PathBuf,
    account_root: PathBuf,
    uncertain: bool,
    reconnect_failures: u32,
    reconnect_after: Option<Instant>,
    pub deferred_compaction: Option<chrono::DateTime<chrono::Utc>>,
}

impl HostProcess {
    pub fn launch(
        launch: &HostedLaunch,
        cwd: &Path,
        socket: &Path,
        resume: Option<&str>,
        guard: &HostOperationLock,
        identity: (&str, &str),
    ) -> Result<Self, String> {
        if !socket.is_absolute()
            || socket.as_os_str().len() > 100
            || std::fs::symlink_metadata(socket).is_ok()
        {
            return Err("host socket must be a new short absolute private path".into());
        }
        let timeout_seconds = guard.remaining().map_err(|e| e.to_string())?.as_secs_f64();
        let child = Command::new(&launch.program)
            .args(&launch.arguments)
            .args(["--listen", &format!("unix://{}", socket.display())])
            .envs(&launch.environment)
            .env_remove("TMUX")
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut host = Self {
            child,
            stderr: None,
            identity: (identity.0.into(), identity.1.into()),
            exit_logged: false,
            rpc: None,
            thread_id: String::new(),
            attach_config: Value::Null,
            instruction_sources: Vec::new(),
            build: String::new(),
            process_start: String::new(),
            socket: socket.into(),
            account_root: launch.account_root.clone(),
            uncertain: false,
            reconnect_failures: 0,
            reconnect_after: None,
            deferred_compaction: None,
        };
        host.stderr =
            Some(StderrTail::start(host.child.stderr.take().unwrap()).map_err(|e| e.to_string())?);
        host.process_start = taurhaus_lib::platform::process_start_ticks(host.child.id())
            .ok_or("host process identity unavailable")?
            .to_string();
        loop {
            if let Some(status) = host.child.try_wait().map_err(|e| e.to_string())? {
                let (status, tail) = host.exit_details(status);
                host.diagnostic(
                    "hosted.launch.failed",
                    json!({"exit_status":status,
                    "stderr_tail":tail, "reason":"exited_before_readiness"}),
                );
                return Err(format!(
                    "app-server exited before transport readiness (exit {status}): {tail}"
                ));
            }
            let remaining = guard.remaining().map_err(|e| {
                host.diagnostic(
                    "hosted.launch.timed_out",
                    json!({"timeout_seconds":timeout_seconds,
                    "stderr_tail":host.stderr.as_ref().unwrap().sanitized()}),
                );
                e.to_string()
            })?;
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
                    socket: Some(WebSocket::connect(stream, guard)?),
                    activity_socket: socket.into(),
                    events: VecDeque::new(),
                    compactions: VecDeque::new(),
                    thread_id: String::new(),
                    status: Value::Null,
                    active_turn: None,
                    requests: Vec::new(),
                    truncated: false,
                    policy: None,
                    repairing: false,
                    policy_dirty: false,
                    pending_read_logged: false,
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
        rpc.thread_id = host.thread_id.clone();
        rpc.status = result["thread"]["status"].clone();
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

    #[cfg(test)]
    pub fn disconnect_for_test(&mut self) {
        self.rpc.as_mut().unwrap().socket = None;
    }

    fn diagnostic(&self, event: &str, mut fields: Value) {
        fields["team"] = json!(self.identity.0);
        fields["member"] = json!(self.identity.1);
        tracing::warn!(event, fields = %fields, "Hosted child unavailable");
        taurhaus_lib::logging::emit_global(
            "warn",
            "coordination",
            event,
            Some("Hosted child unavailable".into()),
            fields.as_object().unwrap().clone(),
        );
    }

    fn exit_details(&mut self, status: std::process::ExitStatus) -> (String, String) {
        let stderr = self.stderr.as_mut().unwrap();
        stderr.finish();
        (
            status
                .code()
                .map_or_else(|| status.to_string(), |code| code.to_string()),
            stderr.sanitized(),
        )
    }

    pub fn outcome_unknown(&self) -> bool {
        self.uncertain
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }
    pub fn alive(&mut self) -> bool {
        if let Ok(Some(status)) = self.child.try_wait() {
            if !self.exit_logged {
                let (status, tail) = self.exit_details(status);
                self.diagnostic(
                    "hosted.process.exited",
                    json!({"exit_status":status, "stderr_tail":tail}),
                );
                self.exit_logged = true;
            }
            return false;
        }
        taurhaus_lib::platform::process_start_ticks(self.child.id())
            .map(|v| v.to_string())
            .as_deref()
            == Some(&self.process_start)
    }

    pub fn needs_reconnect(&self) -> bool {
        self.rpc.as_ref().is_some_and(|rpc| rpc.socket.is_none())
    }

    pub fn activity_retry_due(&self) -> bool {
        !self.needs_reconnect() || self.reconnect_after.is_none_or(|at| Instant::now() >= at)
    }

    /// Read the thread response and queued notifications, without cloning cached events/requests.
    pub fn refresh_activity(&mut self, guard: &HostOperationLock) -> Result<(), String> {
        self.reconnect(guard)?;
        self.rpc
            .as_mut()
            .ok_or("host connection unavailable")?
            .call("thread/read", json!({"threadId":self.thread_id}), guard)
            .map(|_| ())
            .map_err(String::from)
    }

    pub fn transcript(&mut self, guard: &HostOperationLock) -> Result<Value, String> {
        self.transcript_with_retry(guard, true)
    }

    pub fn transcript_with_retry(
        &mut self,
        guard: &HostOperationLock,
        retry: bool,
    ) -> Result<Value, String> {
        if !self.alive() {
            return Err("owned host stopped".into());
        }
        self.reconnect(guard)?;
        let rpc = self.rpc.as_mut().ok_or("host connection unavailable")?;
        let mut pending = false;
        let mut result = loop {
            if pending && guard.remaining().is_err() {
                return Err(RpcError::PendingRead.into());
            }
            match rpc.call("thread/read", json!({"threadId":self.thread_id}), guard) {
                Ok(result) => break result,
                Err(RpcError::PendingRead) if retry => {
                    pending = true;
                    if let Ok(remaining) = guard.remaining() {
                        std::thread::sleep(remaining.min(Duration::from_millis(50)));
                    }
                }
                Err(_) if pending && guard.remaining().is_err() => {
                    return Err(RpcError::PendingRead.into())
                }
                Err(error) => return Err(error.into()),
            }
        };
        if result["thread"]["id"] != self.thread_id {
            return Err("host thread identity changed".into());
        }
        result["thread"]["status"] = rpc.status.clone();
        result["thread"]["turns"] = json!(event_turns(&rpc.events, &self.thread_id));
        result["requests"] = json!(rpc.requests);
        result["events"] = json!(rpc.events);
        result["eventsTruncated"] = json!(rpc.truncated);
        result["outcomeUnknown"] = json!(self.uncertain);
        Ok(result)
    }

    /// A failed frame is never reused. Reinitialize and revalidate the same owned thread;
    /// keep history and the unknown-input fence, but never replay an input or approval.
    fn reconnect(&mut self, guard: &HostOperationLock) -> Result<(), String> {
        let rpc = self.rpc.as_mut().ok_or("host connection unavailable")?;
        if rpc.socket.is_some() {
            return Ok(());
        }
        // Old approval IDs cannot be answered on the new connection. Surface the
        // existing unknown-outcome fence so the operator can stop and re-trigger.
        self.uncertain |= !rpc.requests.is_empty();
        rpc.requests.clear();
        let thread = std::mem::take(&mut rpc.thread_id);
        rpc.repairing = true;
        let result = (|| -> Result<Value, String> {
            let connection =
                socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)
                    .map_err(|e| e.to_string())?;
            connection
                .connect_timeout(
                    &socket2::SockAddr::unix(&self.socket).map_err(|e| e.to_string())?,
                    guard.remaining().map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
            rpc.socket = Some(WebSocket::connect(connection.into(), guard)?);
            let handshake = rpc.call("initialize", json!({"clientInfo":{"name":"taurhaus_host","version":"1"},"capabilities":{"experimentalApi":true}}), guard)?;
            if handshake["codexHome"].as_str().map(Path::new) != Some(&self.account_root)
                || handshake["userAgent"]
                    .as_str()
                    .and_then(|s| s.strip_prefix("taurhaus_host/"))
                    .and_then(|s| s.split_whitespace().next())
                    != Some(&self.build)
            {
                return Err("app-server reconnect identity mismatch".into());
            }
            rpc.write(&json!({"method":"initialized"}), guard)?;
            let (expected, resume, sources) =
                rpc.policy.clone().ok_or("host policy unavailable")?;
            let resumed = rpc.call("thread/resume", resume, guard)?;
            if resumed["thread"]["id"] != thread
                || resumed["model"] != expected["model"]
                || resumed["reasoningEffort"] != expected["effort"]
                || resumed["approvalPolicy"] != expected["approvalPolicy"]
                || resumed["sandbox"] != expected["sandboxPolicy"]
                || instruction_sources(&resumed) != sources
            {
                return Err("host did not restore owned thread policy".into());
            }
            Ok(resumed["thread"]["status"].clone())
        })();
        rpc.thread_id = thread;
        rpc.repairing = false;
        match result {
            Ok(status) => {
                self.reconnect_failures = 0;
                self.reconnect_after = None;
                rpc.policy_dirty = false;
                rpc.set_status(&status);
                rpc.publish_activity();
                Ok(())
            }
            Err(error) => {
                self.reconnect_failures = self.reconnect_failures.saturating_add(1);
                let backoff =
                    Duration::from_secs(1 << self.reconnect_failures.min(4).saturating_sub(1))
                        .min(Duration::from_secs(5));
                self.reconnect_after = Some(Instant::now() + backoff);
                if self.reconnect_failures == 1 {
                    tracing::warn!(event = "hosted.rpc.reconnect_failed", thread_id = %rpc.thread_id,
                        "Host reconnect failed; background retries will back off");
                    taurhaus_lib::logging::emit_global(
                        "warn",
                        "coordination",
                        "hosted.rpc.reconnect_failed",
                        Some("Host reconnect failed; background retries will back off".into()),
                        serde_json::Map::from_iter([("thread_id".into(), json!(rpc.thread_id))]),
                    );
                }
                rpc.socket = None;
                rpc.set_status(&Value::Null);
                rpc.publish_activity();
                Err(error)
            }
        }
    }

    pub fn take_compactions(&mut self) -> VecDeque<Value> {
        self.rpc
            .as_mut()
            .map(|rpc| std::mem::take(&mut rpc.compactions))
            .unwrap_or_default()
    }

    pub fn input(&mut self, text: &str, guard: &HostOperationLock) -> Result<Value, String> {
        let state = self.transcript(guard)?;
        self.input_checked(text, &state, guard)
    }

    pub fn accepts_input(state: &Value) -> bool {
        let thread = &state["thread"];
        thread["canAcceptDirectInput"] == true
            && match thread["status"].get("activeFlags") {
                None => thread["status"]["type"] == "idle",
                Some(flags) => flags.as_array().is_some_and(Vec::is_empty),
            }
            && state["requests"].as_array().is_some_and(Vec::is_empty)
    }

    /// Reuse the state validated under this same host lock; recovery never steers.
    pub fn input_checked(
        &mut self,
        text: &str,
        state: &Value,
        guard: &HostOperationLock,
    ) -> Result<Value, String> {
        if self.uncertain {
            return Err(
                "outcome_unknown: reconcile previous input before another submission".into(),
            );
        }
        if text.trim().is_empty()
            || text.len() > 16_384
            || text.chars().count() > crate::coordination::recovery_card::CARD_BYTE_CAP
        {
            return Err("input must contain 1–8192 characters within 16 KiB".into());
        }
        let thread = &state["thread"];
        if !Self::accepts_input(state) {
            return Err(
                "pending: thread is waiting for permission/input or has unverified state".into(),
            );
        }
        let active = self.rpc.as_ref().unwrap().active_turn.clone();
        let mut params = json!({"threadId":self.thread_id,"input":[{"type":"text","text":text}]});
        let expected = match (thread["status"]["type"].as_str(), active) {
            (Some("idle"), None) => None,
            (Some("active"), Some(id)) => Some(id),
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
        let turn = self
            .rpc
            .as_ref()
            .unwrap()
            .active_turn
            .clone()
            .filter(|_| state["thread"]["status"]["type"] == "active")
            .ok_or("pending: no tracked active turn to cancel")?;
        self.rpc
            .as_mut()
            .unwrap()
            .call(
                "turn/interrupt",
                json!({"threadId":self.thread_id,"turnId":turn}),
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
        if let Some(stderr) = &mut self.stderr {
            stderr.finish();
        }
        let _ = std::fs::remove_file(&self.socket);
    }
}

#[derive(Debug)]
enum RpcError {
    Transport(String),
    Rejected(Value),
    PendingRead,
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
            RpcError::PendingRead => {
                "pending: host thread state is not yet readable; retry within the host deadline"
                    .into()
            }
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

// Rebuild only from the retained event window; no second, unbounded history.
fn event_turns(events: &VecDeque<Value>, thread_id: &str) -> Vec<Value> {
    let mut turns: Vec<Value> = Vec::new();
    for event in events {
        let params = &event["params"];
        if params["threadId"] != thread_id {
            continue;
        }
        let method = event["method"].as_str().unwrap_or_default();
        if !matches!(
            method,
            "turn/started"
                | "turn/completed"
                | "item/started"
                | "item/completed"
                | "item/agentMessage/delta"
        ) {
            continue;
        }
        let Some(id) = params["turnId"]
            .as_str()
            .or_else(|| params["turn"]["id"].as_str())
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        let index = turns
            .iter()
            .position(|turn| turn["id"] == id)
            .unwrap_or_else(|| {
                turns.push(json!({"id":id, "status":"inProgress", "items":[]}));
                turns.len() - 1
            });
        let turn = &mut turns[index];
        if let Some(fields) = params["turn"].as_object() {
            for (key, value) in fields.iter().filter(|(key, _)| key.as_str() != "items") {
                turn[key] = value.clone();
            }
        }
        let items = turn["items"].as_array_mut().unwrap();
        let incoming = params["turn"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| params.get("item").cloned().into_iter().collect());
        for item in incoming {
            if let Some(index) = items.iter().position(|old| old["id"] == item["id"]) {
                items[index] = item;
            } else {
                items.push(item);
            }
        }
        if method == "item/agentMessage/delta" {
            let Some(id) = params["itemId"].as_str() else {
                continue;
            };
            let index = items
                .iter()
                .position(|item| item["id"] == id)
                .unwrap_or_else(|| {
                    items.push(json!({"id":id,"type":"agentMessage","text":""}));
                    items.len() - 1
                });
            let mut text = items[index]["text"].as_str().unwrap_or_default().to_owned();
            text.push_str(params["delta"].as_str().unwrap_or_default());
            items[index]["text"] = json!(text);
        }
    }
    turns
}

struct Rpc {
    activity_socket: PathBuf,
    socket: Option<WebSocket>,
    events: VecDeque<Value>,
    compactions: VecDeque<Value>,
    thread_id: String,
    status: Value,
    active_turn: Option<String>,
    requests: Vec<Value>,
    truncated: bool,
    policy: Option<(Value, Value, Vec<String>)>,
    repairing: bool,
    policy_dirty: bool,
    pending_read_logged: bool,
}
impl Rpc {
    fn publish_activity(&self) {
        taurhaus_lib::daemon::session_activity::SessionActivityHub::shared().publish_host_status(
            &self.activity_socket,
            &self.thread_id,
            &self.status,
        );
    }

    fn observe(&mut self, frame: &Value) {
        let params = &frame["params"];
        if params["threadId"].as_str() != Some(self.thread_id.as_str()) {
            return;
        }
        match frame["method"].as_str() {
            Some("thread/status/changed") => self.set_status(&params["status"]),
            Some("turn/started") => {
                self.active_turn = params["turn"]["id"]
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .map(str::to_owned);
                if self.status["type"] != "active" {
                    self.status = json!({"type":"active", "activeFlags":[]});
                }
            }
            // This notification also carries failed/interrupted terminal statuses.
            Some("turn/completed")
                if self
                    .active_turn
                    .as_deref()
                    .is_none_or(|id| params["turn"]["id"] == id) =>
            {
                self.set_status(&json!({"type":"idle"}));
            }
            _ => {}
        }
        self.publish_activity();
    }

    fn set_status(&mut self, status: &Value) {
        self.status = status.clone();
        if status["type"] == "idle" {
            self.active_turn = None;
        }
    }

    fn write(&mut self, value: &Value, guard: &HostOperationLock) -> Result<(), String> {
        self.socket
            .as_mut()
            .ok_or("host connection unavailable")?
            .send(
                1,
                &serde_json::to_vec(value).map_err(|e| e.to_string())?,
                guard,
            )
    }
    fn read(&mut self, guard: &HostOperationLock) -> Result<Value, String> {
        let message = self
            .socket
            .as_mut()
            .ok_or("host connection unavailable")?
            .read(guard)?;
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
        let result = self.call_inner(method, params, guard);
        if matches!(result, Err(RpcError::Transport(_))) {
            // Header/payload bytes or an RPC reply may already be consumed. Close
            // the transport on every ambiguous failure; never parse its tail again.
            self.socket = None;
            self.set_status(&Value::Null);
            self.publish_activity();
        }
        result
    }

    fn call_inner(
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
                    if frame["method"] == "item/completed"
                        && frame["params"]["item"]["type"] == "contextCompaction"
                    {
                        if self.compactions.len() == 64 {
                            self.compactions.pop_front();
                        }
                        let p = &frame["params"];
                        self.compactions
                            .push_back(json!({"threadId":p["threadId"], "turnId":p["turnId"],
                            "itemId":p["item"]["id"], "completedAtMs":p["completedAtMs"]}));
                    }
                    self.observe(&frame);
                    if self.events.len() == 64 {
                        self.events.pop_front();
                        self.truncated = true;
                    }
                    self.events.push_back(frame);
                }
            } else if frame["id"] == id {
                if frame.get("error").is_some() {
                    let error = &frame["error"];
                    let pending = method == "thread/read" && error["code"] == -32603;
                    if pending && self.pending_read_logged {
                        return Err(RpcError::PendingRead);
                    }
                    self.pending_read_logged |= pending;
                    let event = if pending {
                        "hosted.rpc.pending"
                    } else {
                        "hosted.rpc.rejected"
                    };
                    let code = error["code"].as_i64();
                    let message: String = error["message"]
                        .as_str()
                        .unwrap_or_default()
                        .chars()
                        .take(256)
                        .collect();
                    tracing::warn!(event, method, code, message = %message);
                    taurhaus_lib::logging::emit_global(
                        "warn",
                        "coordination",
                        event,
                        Some(message),
                        serde_json::Map::from_iter([
                            ("method".into(), json!(method)),
                            ("code".into(), json!(code)),
                        ]),
                    );
                    return Err(if pending {
                        RpcError::PendingRead
                    } else {
                        RpcError::Rejected(error.clone())
                    });
                }
                let result = frame.get("result").cloned().ok_or("missing host result")?;
                if method == "thread/read" {
                    self.pending_read_logged = false;
                }
                if matches!(method, "thread/read" | "thread/resume")
                    && result["thread"]["id"] == self.thread_id
                    // Snapshots can lag a start receipt/notification. Only live
                    // notifications may retire the connection's tracked turn.
                    && self.active_turn.is_none()
                {
                    self.set_status(&result["thread"]["status"]);
                }
                if method == "turn/start" {
                    if let Some(turn) = result["turn"]["id"].as_str().filter(|id| !id.is_empty()) {
                        // A start receipt can precede turn/started. Never overwrite a
                        // completion already observed before this correlated result.
                        if !self.events.iter().any(|event| {
                            event["method"] == "turn/completed"
                                && event["params"]["threadId"] == self.thread_id
                                && event["params"]["turn"]["id"] == turn
                        }) {
                            self.active_turn = Some(turn.to_owned());
                            self.status = json!({"type":"active", "activeFlags":[]});
                        }
                    }
                }
                self.publish_activity();
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

    #[test]
    fn stderr_sanitizer_bounds_unicode_and_redacts_quoted_credentials() {
        // Regression: cadd533e discarded stderr; a8cb4821 missed single-quoted tokens.
        assert_eq!(
            sanitize_stderr("é".repeat(600).as_bytes()).chars().count(),
            512
        );
        assert!(!sanitize_stderr(b"'access_token': 'fake-secret'").contains("fake"));
        assert_eq!(
            sanitize_stderr(b"\x1b]0;hidden\x07visible\x1b[0m"),
            "visible"
        );
    }

    #[test]
    fn stderr_reader_drains_and_keeps_last_4k() {
        // Regression: cadd533e discarded child stderr, hiding launch failures.
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("writer.sh");
        std::fs::write(
            &script,
            "head -c 1048576 /dev/zero >&2; printf END >&2; exit 23",
        )
        .unwrap();
        let mut child = Command::new("/bin/sh")
            .arg(script)
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut tail = StderrTail::start(child.stderr.take().unwrap()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let status = child.try_wait().unwrap();
        if status.is_none() {
            child.kill().unwrap();
        }
        child.wait().unwrap();
        tail.finish();
        assert_eq!(status.unwrap().code(), Some(23));
        assert!(tail.reader.is_none());
        let bytes = tail.bytes.lock().unwrap();
        assert_eq!(bytes.len(), 4096);
        assert!(bytes.iter().copied().collect::<Vec<_>>().ends_with(b"END"));
    }

    #[test]
    fn launch_error_has_sanitized_stderr_and_status() {
        // Regression: cadd533e returned a bare readiness error and discarded stderr.
        let _logs = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().unwrap();
        let sink =
            taurhaus_lib::logging::LogFileState::new(tmp.path().join("events.jsonl")).unwrap();
        taurhaus_lib::logging::install_global_sink(&sink);
        let launch = fixture(tmp.path());
        std::fs::write(&launch.program, r#"#!/bin/sh
head -c 5000 /dev/zero >&2
printf '\033[31mfailed\033[0m sk-fake-secret Bearer fake-bearer "access_token": "fake-access"\nfinal reason\001' >&2
exit 23
"#.replace("failed", &format!("{}failed", "é".repeat(600)))).unwrap();
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let error = spawn(&launch, tmp.path(), None, &guard).err().unwrap();
        assert!(error.contains("(exit 23):"), "{error}");
        assert_eq!(error.split_once(": ").unwrap().1.chars().count(), 512);
        assert!(error.contains("failed") && error.ends_with("final reason"));
        for forbidden in ["fake-", "\u{1b}", "\n", "\u{1}"] {
            assert!(!error.contains(forbidden));
        }
        drop(guard);
        std::fs::write(
            &launch.program,
            "#!/bin/sh\nprintf waiting >&2; exec sleep 60",
        )
        .unwrap();
        let guard = HostOperationLock::acquire_for_activity(tmp.path(), "team", "seat").unwrap();
        assert!(spawn(&launch, tmp.path(), None, &guard).is_err());
        drop(guard);
        let launch = fixture(tmp.path());
        let script = std::fs::read_to_string(&launch.program).unwrap().replace(
            "root = os.environ",
            "sys.stderr.write('mid-run reason\\n'); sys.stderr.flush()\nroot = os.environ",
        );
        std::fs::write(&launch.program, script).unwrap();
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let mut host = spawn(&launch, tmp.path(), None, &guard).unwrap();
        host.child.kill().unwrap();
        host.child.wait().unwrap();
        assert!(!host.alive());
        assert!(!host.alive());
        drop(host);
        sink.flush_for_test().unwrap();
        let events: Vec<Value> = std::fs::read_to_string(tmp.path().join("events.jsonl"))
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        for (name, tail) in [
            ("hosted.launch.failed", error.split_once(": ").unwrap().1),
            ("hosted.launch.timed_out", "waiting"),
            ("hosted.process.exited", "mid-run reason"),
        ] {
            let rows: Vec<_> = events.iter().filter(|e| e["event"] == name).collect();
            assert_eq!(rows.len(), 1, "{name}");
            assert_eq!(rows[0]["level"], "WARN");
            let fields = rows[0];
            assert_eq!(fields["team"], "team");
            assert_eq!(fields["member"], "seat");
            assert_eq!(fields["stderr_tail"], tail);
            if name.ends_with("failed") {
                assert_eq!(fields["exit_status"], "23");
                assert_eq!(fields["reason"], "exited_before_readiness");
            } else if name.ends_with("exited") {
                assert!(fields["exit_status"].as_str().unwrap().contains("signal"));
            } else {
                assert!(fields["timeout_seconds"].as_f64().unwrap() > 0.0);
            }
        }
    }

    pub(crate) fn fixture(
        root: &std::path::Path,
    ) -> taurhaus_lib::session_scanner::launch::HostedLaunch {
        let executable = root.join("codex");
        std::fs::write(&executable, r#"#!/usr/bin/python3
import json, os, socket, sys, threading, fcntl, base64, hashlib, struct, time
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
with open(os.path.join(root, 'host-argv.json'), 'w') as output: json.dump(sys.argv, output)
address = sys.argv[sys.argv.index('--listen')+1].removeprefix('unix://')
saved = os.path.join(root, 'thread.json')
thread = json.load(open(saved)) if os.path.exists(saved) else None
# A killed host cannot retain a running model turn in the new process.
if thread and thread['status']['type'] == 'active':
    thread['status'] = {'type':'idle'}
    thread['canAcceptDirectInput'] = True
    thread['turns'][-1]['status'] = 'interrupted'
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
        def notify(method, **params):
            emit({'method':method, 'params':dict(threadId=thread['id'], **params)})
        def items_and_completion(turn):
            for item in turn['items']:
                notify('item/started', turnId=turn['id'], item=item)
                notify('item/completed', turnId=turn['id'], item=item)
            agent = {'id':'agent-'+turn['id'], 'type':'agentMessage', 'text':''}
            notify('item/started', turnId=turn['id'], item=agent)
            notify('item/agentMessage/delta', turnId=turn['id'], itemId=agent['id'], delta='fixture reply')
            agent['text'] = 'fixture reply'
            notify('item/completed', turnId=turn['id'], item=agent)
            if turn['status'] != 'inProgress':
                thread['status'] = {'type':'idle'}
                with open(saved, 'w') as output: json.dump(thread, output)
                notify('thread/status/changed', status={'type':'idle'})
                # Like the probe, completion summarizes the agent item only.
                notify('turn/completed', turn=dict(turn, items=[agent]))
        pending_items = None
        expect_card = False
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
                notify('thread/status/changed', status=thread['status'])
                continue
            method, params = request.get('method'), request.get('params', {})
            if expect_card:
                assert method == 'turn/start', 'recovery must be the next request after idle'
                assert params['input'][0]['text'].startswith('[taurhaus] recovery_card')
                expect_card = False
            result, error, approval = {}, None, None
            with open(os.path.join(root, 'requests.jsonl'), 'a') as output:
                output.write(json.dumps(request)+'\n')
            with lock:
                if method == 'initialize':
                    if os.path.exists(os.path.join(root, 'fail-reconnect')): return
                    result = {'userAgent':'taurhaus_host/'+os.environ.get('FAKE_BUILD','0.153.4'), 'codexHome':root}
                    if os.environ.get('FAKE_RUNTIME'):
                        with open(os.environ['FAKE_RUNTIME'], 'r+') as runtime:
                            fcntl.flock(runtime, fcntl.LOCK_EX)
                            record = json.load(runtime); record['foreignClaim'] = 'concurrent'
                            runtime.seek(0); json.dump(record, runtime); runtime.truncate()

                elif method == 'thread/start':
                    assert thread is None
                    with open(os.path.join(root, 'start.json'), 'w') as output: json.dump(params, output)
                    thread = {'id':'owned-thread', 'status':{'type':'idle'}, 'canAcceptDirectInput':True, 'turns':[]}
                    result = dict(policy, thread=dict(thread, turns=[]))
                elif method in ('thread/resume', 'thread/read'):
                    refusal = os.path.join(root, 'refuse-input')
                    reason = open(refusal).read() if os.path.exists(refusal) else ''
                    if os.path.exists(refusal):
                        thread['canAcceptDirectInput'] = reason != 'blocked'
                        if not reason: notify('thread/status/changed', status=thread['status'])
                    if reason == 'requests' and os.path.exists(os.path.join(root, 'compact.json')): emit({'id':'lingering','method':'item/commandExecution/requestApproval','params':{'threadId':thread['id']}})
                    boundary = os.path.join(root, 'compact.json')
                    if os.path.exists(boundary):
                        compact = json.load(open(boundary)); os.unlink(boundary)
                        expect_card = compact.get('expectCard', False)
                        tid = compact.get('threadId', thread['id'])
                        def boundary_event(method, **params):
                            emit({'method':method, 'params':dict(threadId=tid, **params)})
                        for i in range(compact.get('backlog', 0)):
                            boundary_event('item/completed', turnId=str(i), item={'id':str(i),'type':'contextCompaction'}, completedAtMs=i)
                        turn_id = compact.get('turnId', 'compact-turn')
                        item = {'id':'compact-item', 'type':'contextCompaction'}
                        boundary_event('thread/status/changed', status={'type':'active','activeFlags':[]})
                        boundary_event('turn/started', turn={'id':turn_id,'status':'inProgress','items':[]})
                        boundary_event('item/started', turnId=turn_id, item=item)
                        boundary_event('thread/tokenUsage/updated', turnId=turn_id, tokenUsage={'last':{'totalTokens':6344}})
                        boundary_event('item/completed', turnId=turn_id, item=item, completedAtMs=compact.get("completedAtMs", int(time.time()*1000)-1000))
                        if not compact.get('busy'):
                            boundary_event('thread/status/changed', status={'type':'idle'})
                            boundary_event('turn/completed', turn={'id':turn_id,'status':'completed','items':[]})
                        with open(os.path.join(root, 'compact-emitted'), 'w') as output: output.write('1')
                    if 'includeTurns' in params:
                        error = {'code':-32601,'message':'list_turns is not supported yet'}
                    elif method == 'thread/read' and (pending_items or any(os.path.exists(os.path.join(root, name)) for name in ('pending-read', 'pending-read-once'))):
                        error = {'code':-32603,'message':os.environ.get('FAKE_ERROR_MESSAGE', 'failed to read thread: rollout at fixture/sessions/first.jsonl is empty')}
                        once = os.path.join(root, 'pending-read-once')
                        if os.path.exists(once): os.unlink(once)
                    elif thread is None or params['threadId'] != thread['id']: error = {'code':-32600,'message':'unknown thread'}
                    else:
                        result = dict(policy, thread=dict(thread, turns=[]))
                        if os.environ.get('FAKE_STALE_IDLE') and thread['status']['type'] == 'active':
                            result['thread']['status'] = {'type':'idle'}
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
                    if os.path.exists(os.path.join(root, 'compact-emitted')):
                        state = json.load(open(os.path.join(root, 'team/state/compaction/seat.json')))
                        assert state['pending'] and state['pending_obligation'][1][1] == 1
                        with open(os.path.join(root, 'boundary-submission.json'), 'w') as output:
                            json.dump({'state':state,'input':params['input'],'turnId':str(len(thread['turns'])+1)}, output)
                        os.unlink(os.path.join(root, 'compact-emitted'))
                    turn = {'id':str(len(thread['turns'])+1), 'status':'completed', 'items':[{'id':'user-'+str(len(thread['turns'])+1),'type':'userMessage','content':params['input']}]}
                    thread['turns'].append(turn)
                    thread['status'] = {'type':'active','activeFlags':[]}
                    text = params['input'][0]['text']
                    if text == 'active':
                        turn['status'] = 'inProgress'
                        thread['status'] = {'type':'active','activeFlags':[]}
                    if text in ('approval', 'foreign approval'):
                        turn['status'] = 'inProgress'
                        thread['status'] = {'type':'active','activeFlags':['waitingOnApproval']}
                        thread['canAcceptDirectInput'] = False
                        approval = {'id':'permission-1','method':'item/commandExecution/requestApproval','params':{'threadId':thread['id'] if text == 'approval' else 'foreign-thread','command':'echo marker'}}
                    result = {'turn':dict(turn, status='inProgress', items=[], itemsView='notLoaded')}
                elif method == 'turn/steer':
                    assert params['expectedTurnId'] == thread['turns'][-1]['id']
                    text = params['input'][0]['text']
                    if text == 'completion race':
                        error = {'code':-32600, 'message':'no active turn to steer'}
                        thread['turns'][-1]['status'] = 'completed'
                        thread['status'] = {'type':'idle'}
                        notify('thread/status/changed', status=thread['status'])
                        notify('turn/completed', turn=thread['turns'][-1])
                    elif text == 'wrong turn':
                        error = {'code':-32600, 'message':'expected turn id 1 but actual turn id 2'}
                    elif text == 'unclassified':
                        error = {'code':-32600, 'message':'unclassified rejection'}
                    else: result = {'turnId':params['expectedTurnId']}
                elif method == 'turn/interrupt':
                    assert params['turnId'] == thread['turns'][-1]['id']
                    thread['turns'][-1]['status'] = 'interrupted'
                    thread['status'] = {'type':'idle'}
                    thread['canAcceptDirectInput'] = True
                    notify('thread/status/changed', status=thread['status'])
                    notify('turn/completed', turn=thread['turns'][-1])
                    result = {}
                else: error = {'code':-32601,'message':os.environ.get('FAKE_ERROR_MESSAGE','unsupported fake method')}
                if thread is not None:
                    with open(saved, 'w') as output: json.dump(thread, output)
                if method == 'turn/start' and params['input'][0]['text'] == 'disconnect': return
                reply = {'id':request['id'], 'error':error} if error else {'id':request['id'], 'result':result}
                emit(reply)
                if method == 'turn/start':
                    notify('thread/status/changed', status=thread['status'])
                    notify('turn/started', turn=dict(turn, status='inProgress', items=[], itemsView='notLoaded'))
                    if len(thread['turns']) == 1:
                        pending_items = turn
                    else: items_and_completion(turn)
                elif method == 'thread/read' and error and error['code'] == -32603 and pending_items:
                    items_and_completion(pending_items)
                    pending_items = None
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
        HostProcess::launch(launch, root, &socket, resume, guard, ("team", "seat"))
    }

    #[test]
    fn hosted_idle_startup_card_and_transcript_use_real_thread_state() {
        // Regression: cadd533e (still present in 80290f36) rejected startup recovery by requesting unsupported includeTurns.
        let tmp = tempfile::tempdir().unwrap();
        let launch = fixture(tmp.path());
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let mut host = spawn(&launch, tmp.path(), None, &guard).unwrap();
        // Regression: 04128879 did not retry pre-card reads within the launch deadline.
        std::fs::write(tmp.path().join("pending-read-once"), "").unwrap();
        // Regression: cadd533e's 8000-char limit rejected the 8192-byte recovery cap.
        let card = format!(
            "[taurhaus] recovery_card {}",
            "x".repeat(crate::coordination::recovery_card::CARD_BYTE_CAP - 25)
        );
        assert_eq!(
            card.len(),
            crate::coordination::recovery_card::CARD_BYTE_CAP
        );
        assert_eq!(host.input(&card, &guard).unwrap()["turn"]["id"], "1");
        let state = host.transcript(&guard).unwrap();
        let turn = &state["thread"]["turns"][0];
        assert_eq!(turn["status"], "completed");
        assert_eq!(turn["items"][0]["content"][0]["text"], card);
        assert_eq!(turn["items"][1]["text"], "fixture reply");
        let requests = std::fs::read_to_string(tmp.path().join("requests.jsonl")).unwrap();
        assert!(!requests.contains("includeTurns"));
        assert!(requests.contains("turn/start"));
        assert!(!requests.contains("turn/steer"));
    }

    #[test]
    fn hosted_first_turn_read_is_pending_then_steers_tracked_id() {
        // Regression: cadd533e (still present in 80290f36) rejected transient rollout reads and derived active IDs from empty turns.
        let tmp = tempfile::tempdir().unwrap();
        let launch = fixture(tmp.path());
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let mut host = spawn(&launch, tmp.path(), None, &guard).unwrap();
        host.input("active", &guard).unwrap();
        let error = host
            .rpc
            .as_mut()
            .unwrap()
            .call("thread/read", json!({"threadId":host.thread_id}), &guard)
            .unwrap_err();
        assert!(matches!(error, RpcError::PendingRead));
        assert!(!host.outcome_unknown());
        assert_eq!(host.input("steer marker", &guard).unwrap()["turnId"], "1");
        let state = host.transcript(&guard).unwrap();
        assert_eq!(
            state["thread"]["turns"][0]["items"][1]["text"],
            "fixture reply"
        );
        host.interrupt(&guard).unwrap();
        assert_eq!(
            host.transcript(&guard).unwrap()["thread"]["status"]["type"],
            "idle"
        );
        let requests = std::fs::read_to_string(tmp.path().join("requests.jsonl")).unwrap();
        assert!(!requests.contains("not submitted"));
        assert!(!requests.contains("includeTurns"));
        assert!(requests.contains("expectedTurnId"));
    }

    #[test]
    fn hosted_transcript_window_does_not_forget_active_turn() {
        // Regression: cadd533e (still present in 80290f36) used read history instead of connection state and events.
        let tmp = tempfile::tempdir().unwrap();
        let launch = fixture(tmp.path());
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let mut host = spawn(&launch, tmp.path(), None, &guard).unwrap();
        host.input("active", &guard).unwrap();
        host.transcript(&guard).unwrap();
        let rpc = host.rpc.as_mut().unwrap();
        // Model an evicted history window without losing connection activity.
        rpc.events.clear();
        rpc.truncated = true;
        for n in 0..64 {
            rpc.events
                .push_back(json!({"method":"item/completed", "params":{
                "threadId":"owned-thread", "turnId":format!("old-{n}"),
                "item":{"id":"agent", "type":"agentMessage", "text":"retained"}}}));
        }
        let state = host.transcript(&guard).unwrap();
        assert_eq!(state["thread"]["turns"].as_array().unwrap().len(), 64);
        assert_eq!(state["eventsTruncated"], true);
        assert_eq!(
            host.input("steer after eviction", &guard).unwrap()["turnId"],
            "1"
        );
    }

    #[test]
    fn hosted_stale_idle_snapshot_preserves_steer_and_interrupt() {
        // Regression: 04128879 let lagging read/resume snapshots erase a tracked turn.
        let tmp = tempfile::tempdir().unwrap();
        let mut launch = fixture(tmp.path());
        launch
            .environment
            .insert("FAKE_STALE_IDLE".into(), "1".into());
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let mut host = spawn(&launch, tmp.path(), None, &guard).unwrap();
        host.input("first", &guard).unwrap();
        host.transcript(&guard).unwrap();
        host.input("active", &guard).unwrap();
        assert_eq!(
            host.transcript(&guard).unwrap()["thread"]["status"]["type"],
            "active"
        );
        let rpc = host.rpc.as_mut().unwrap();
        rpc.call(
            "thread/resume",
            rpc.policy.as_ref().unwrap().1.clone(),
            &guard,
        )
        .unwrap();
        assert_eq!(host.input("steer", &guard).unwrap()["turnId"], "2");
        host.interrupt(&guard).unwrap();
        assert_eq!(
            host.transcript(&guard).unwrap()["thread"]["status"]["type"],
            "idle"
        );
    }

    #[test]
    fn hosted_rpc_rejection_is_logged_without_params() {
        // Regression: cadd533e (still present in 80290f36) discarded the host error behind an opaque public refusal.
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().unwrap();
        let sink =
            taurhaus_lib::logging::LogFileState::new(tmp.path().join("events.jsonl")).unwrap();
        taurhaus_lib::logging::install_global_sink(&sink);
        let mut launch = fixture(tmp.path());
        launch
            .environment
            .insert("FAKE_ERROR_MESSAGE".into(), "é".repeat(300));
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let mut host = spawn(&launch, tmp.path(), None, &guard).unwrap();
        // Regression: 04128879 selected a foreign rejection from the global sink.
        let rpc = host.rpc.as_mut().unwrap();
        let _ = rpc.call("thread/read", json!({"threadId":"foreign"}), &guard);
        let error = rpc
            .call("unsupported", json!({"private":"do not log"}), &guard)
            .unwrap_err();
        assert_eq!(
            String::from(error),
            "host rejected request; reconcile before retrying"
        );
        // Regression: 04128879 surfaced the first transient without using the host deadline.
        std::fs::write(tmp.path().join("pending-read"), "").unwrap();
        let started = std::time::Instant::now();
        assert!(host
            .input("not submitted", &guard)
            .unwrap_err()
            .starts_with("pending:"));
        assert!(started.elapsed() >= Duration::from_secs(4));
        assert!(!host.outcome_unknown());
        std::fs::remove_file(tmp.path().join("pending-read")).unwrap();
        drop(guard);
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        assert_eq!(
            host.input("recovery card", &guard).unwrap()["turn"]["id"],
            "1"
        );
        let requests = std::fs::read_to_string(tmp.path().join("requests.jsonl")).unwrap();
        assert!(!requests.contains("not submitted"));
        sink.flush_for_test().unwrap();
        let logs = std::fs::read_to_string(tmp.path().join("events.jsonl")).unwrap();
        let event: Value = serde_json::from_str(
            logs.lines()
                .find(|line| {
                    line.contains("hosted.rpc.rejected")
                        && line.contains("\"method\":\"unsupported\"")
                })
                .expect("rejection must be logged"),
        )
        .unwrap();
        assert_eq!(event["method"], "unsupported");
        assert_eq!(event["code"], -32601);
        assert_eq!(event["message"], "é".repeat(256));
        assert!(!logs.contains("do not log"));
        let pending: Vec<Value> = logs
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .filter(|event: &Value| {
                event["event"] == "hosted.rpc.pending" && event["message"] == "é".repeat(256)
            })
            .collect();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0]["code"], -32603);
    }
    #[test]
    fn hosted_workspace_write_config_round_trips_optional_fields() {
        // Serialize with the global-sink logging tests: this spawn emits hosted.instruction_sources.loaded.
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
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
        // Serialize with the global-sink logging tests: this spawn emits hosted.instruction_sources.loaded.
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
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
        // Serialize with the global-sink logging tests: this spawn emits hosted.instruction_sources.loaded.
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
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
        // Serialize with the global-sink logging tests: this spawn emits hosted.instruction_sources.loaded.
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
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
        // Serialize with the global-sink logging tests: this spawn emits hosted.instruction_sources.loaded.
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
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
            resumed.transcript(&guard).unwrap()["thread"]["id"],
            original["thread"]["id"]
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
