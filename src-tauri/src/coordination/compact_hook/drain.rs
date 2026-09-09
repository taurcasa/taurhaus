//! mesh-hook-drain/1: output executor only; Mesh owns claims and suppression.
use super::*;
use std::time::{Duration, Instant};

pub const PROTOCOL: &str = "mesh-hook-drain/1";
const INPUT_LIMIT: usize = 16 * 1024;
const OUTPUT_LIMIT: usize = 64 * 1024;
const DEADLINE: Duration = Duration::from_secs(2);
const SECTION: &str = "\n\n## Mesh pending messages (attributed data)\n";

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Descriptor {
    pub id: String,
    pub harness: String,
    pub build: String,
    pub host: String,
    pub event: String,
    pub source: String,
    pub matcher: String,
    pub envelope: String,
    pub config_trust: String,
    pub context_entry: String,
    pub drop_rules: String,
    pub max_bytes: usize,
    pub max_chars: usize,
    pub continuation_budget: u8,
    pub enabled: bool,
}
impl Descriptor {
    fn supported(&self) -> bool {
        self.enabled
            && matches!(self.harness.as_str(), "claude" | "codex")
            && matches!(
                self.event.as_str(),
                "SessionStart"
                    | "UserPromptSubmit"
                    | "PreToolUse"
                    | "PostToolUse"
                    | "PermissionRequest"
            )
            && self
                .envelope
                .starts_with("hookSpecificOutput.additionalContext")
            && self.continuation_budget == 0
            && !self.id.is_empty()
            && !self.build.is_empty()
            && !self.host.is_empty()
            && !self.config_trust.is_empty()
            && !self.context_entry.is_empty()
            && !self.drop_rules.is_empty()
            && self.max_bytes > 128
            && self.max_chars > 128
            && self.max_bytes <= 8192
            && self.max_chars <= 8000
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Offer {
    executable: PathBuf,
    root: PathBuf,
    team: String,
    member: String,
    request: Value,
    id: Option<String>,
    rendered: bool,
    reason: &'static str,
}

fn read_json(path: &Path, limit: usize) -> Option<Value> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= limit)
        .then(|| serde_json::from_slice(&bytes).ok())
        .flatten()
}

fn executable() -> Option<PathBuf> {
    // Windows hooks execute through the native WSL daemon. Never discover a shell here.
    if cfg!(windows) {
        return None;
    }
    let path = PathBuf::from(crate::coordination::mesh_cli::mesh_binary_path()?);
    path.is_absolute().then_some(path)
}

fn descriptors(executable: &Path, root: &Path, team: &str, member: &str) -> Vec<Descriptor> {
    let result = exchange(
        executable,
        root,
        team,
        member,
        "capabilities",
        &json!({"protocol":PROTOCOL}),
    );
    if result.failed {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_slice::<Value>(&result.bytes) else {
        return Vec::new();
    };
    if value["protocol"] != PROTOCOL {
        return Vec::new();
    }
    value["descriptors"]
        .as_array()
        .filter(|rows| rows.len() <= 64)
        .into_iter()
        .flatten()
        .filter_map(|row| serde_json::from_value(row.clone()).ok())
        .filter(Descriptor::supported)
        .collect()
}

/// Only exact runtime session identity can authorize a drain. The compaction
/// resolver's historical unique-cwd fallback is deliberately insufficient here.
fn candidate(teams: &Path, payload: &CompactHookInput) -> Option<(HookMemberMatch, Value, Value)> {
    let tool = payload.inferred_tool()?;
    if !matches!(tool, CliTool::Claude | CliTool::Codex) {
        return None;
    }
    let matched = resolve_member_match(teams, tool, payload).ok()?.ok()?;
    let runtime =
        MemberRuntimeStore::load(&matched.teams_dir, &matched.team_name, &matched.member.name)
            .ok()?;
    if runtime.session_id.as_deref() != Some(&payload.session_id)
        || runtime.health == crate::coordination::domain::HealthState::SessionDead
        || runtime.terminal_contract != 1
        || runtime.harness != Some(tool)
    {
        return None;
    }
    // A serde default is compatibility for old readers, never verified hook authority.
    let published = read_json(
        &matched
            .teams_dir
            .join(&matched.team_name)
            .join("runtime")
            .join(format!("{}.json", matched.member.name)),
        1024 * 1024,
    )?;
    if published["hookSessionId"].as_str()? != payload.session_id
        || published["contextGeneration"].as_str()? != runtime.context_generation.to_string()
    {
        return None;
    }
    let root = fs::canonicalize(matched.teams_dir.parent()?).ok()?;
    let launch = runtime.launch_root.as_ref()?;
    if launch.claude_dir != root
        || launch.teams_dir != root.join("teams")
        || launch.root_authority_revision
            != TeamRootRegistry::new(teams.into())
                .revision(&matched.team_name)
                .ok()?
    {
        return None;
    }
    let config = read_json(
        &matched
            .teams_dir
            .join(&matched.team_name)
            .join("config.json"),
        1024 * 1024,
    )?;
    if config["delivery_owner"] != "team"
        || config["messaging_format"] != 2
        || config["team_incarnation_id"].as_str()? != launch.team_incarnation_id.as_deref()?
    {
        return None;
    }
    let member = config["members"]
        .as_array()?
        .iter()
        .find(|m| m["name"] == matched.member.name)?;
    if member["isActive"] == false {
        return None;
    }
    let member_id = member["agentId"].as_str()?;
    let selection = read_json(
        &matched.teams_dir.join(&matched.team_name).join(format!(
            "state/delivery/adapter-{}.json",
            matched.member.name
        )),
        4 * 1024 * 1024,
    )?;
    if selection["mode"] != "hook" || selection["revision"].as_u64()? == 0 {
        return None;
    }
    let socket = runtime.tmux_socket.as_ref()?;
    if !socket.is_absolute() || member_id.is_empty() || payload.session_id.is_empty() {
        return None;
    }
    let session = runtime.tmux_session_id.as_ref()?;
    let attachment = json!({
        "launch_root":launch, "attachment_generation":runtime.attachment_generation,
        "root":root,"team_incarnation":launch.team_incarnation_id,"member":matched.member.name,
        "session":session,"context_generation":runtime.context_generation.to_string(),
        "generation":runtime.attachment_generation.to_string(),"socket":socket,
        "pane":runtime.pane_id?,"pane_pid":runtime.pane_pid?,
        "pane_start":runtime.pane_start_time?.to_string(),"tmux_session":session,"harness":tool,
    });
    Some((
        matched,
        json!({"runtime":attachment,"member_id":member_id}),
        selection,
    ))
}

pub(super) fn append(teams: &Path, payload: &CompactHookInput, response: &mut CompactHookResponse) {
    let Some((matched, identity, selection)) = candidate(teams, payload) else {
        return;
    };
    let Some(executable) = executable() else {
        return;
    };
    let root = PathBuf::from(identity["runtime"]["root"].as_str().unwrap_or_default());
    let source = payload.source.as_deref().unwrap_or("ordinary");
    let pins = descriptors(&executable, &root, &matched.team_name, &matched.member.name);
    let matching: Vec<_> = pins
        .iter()
        .filter(|pin| {
            pin.harness == matched.member.cli_tool.to_string()
                && pin.event == payload.hook_event_name
                && pin.source == source
        })
        .collect();
    let [pin] = matching.as_slice() else {
        return;
    };
    let compact = pin.event == SESSION_START_HOOK_EVENT && source == COMPACT_SOURCE;
    let card = response
        .hook_specific_output
        .as_ref()
        .map(|o| o.additional_context.as_str())
        .unwrap_or_default();
    let prefix = if card.is_empty() {
        String::new()
    } else {
        format!("{card}{SECTION}")
    };
    let encoded = serde_json::to_string(&prefix).expect("string serialization");
    let reserved_bytes = encoded.len();
    let reserved_chars = encoded.chars().count();
    if reserved_bytes + 128 >= pin.max_bytes || reserved_chars + 128 >= pin.max_chars {
        return;
    }
    let request = json!({"protocol":PROTOCOL,"runtime":identity["runtime"],"member_id":identity["member_id"],
        "session":payload.session_id,"context":identity["runtime"]["context_generation"],
        "descriptor":pin.id,"build":pin.build,"host":pin.host,"event":pin.event,"source":pin.source,
        "max_bytes":pin.max_bytes,"max_chars":pin.max_chars,"reserved_bytes":reserved_bytes,
        "reserved_chars":reserved_chars,"compose_compaction":compact});
    let result = exchange(
        &executable,
        &root,
        &matched.team_name,
        &matched.member.name,
        "drain",
        &json!({"protocol":PROTOCOL,"request":request}),
    );
    let value = serde_json::from_slice::<Value>(&result.bytes).unwrap_or(Value::Null);
    // Even a refused or transport-failed response can identify a saved reservation.
    let id = value["offer_id"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 256)
        .map(str::to_owned);
    let mut offer = Offer {
        executable,
        root,
        team: matched.team_name,
        member: matched.member.name,
        request,
        id,
        rendered: false,
        reason: "invalid_output",
    };
    if !result.failed && valid_outcome(&value) {
        if value["status"] == "offered" && valid_batch(&value, &selection, pin, &prefix) {
            response.hook_specific_output = Some(CompactHookSpecificOutput {
                hook_event_name: pin.event.clone(),
                additional_context: format!("{prefix}{}", value["text"].as_str().unwrap()),
            });
            offer.rendered = true;
            offer.reason = "rendered";
        } else if offer.id.is_none() && value["status"] != "offered" {
            return;
        }
    }
    response.drain_receipt = Some(offer);
}

fn valid_outcome(value: &Value) -> bool {
    value["protocol"] == PROTOCOL
        && value["continue"] == false
        && matches!(
            value["status"].as_str(),
            Some("offered" | "empty" | "unsupported" | "refused" | "unknown" | "error")
        )
        && value["text"].is_string()
        && value["deliveries"].is_array()
        && value.get("offer_id").is_some_and(|id| {
            id.is_null()
                || id
                    .as_str()
                    .is_some_and(|id| !id.is_empty() && id.len() <= 256)
        })
        && (value["status"] == "offered"
            || (value["text"] == "" && value["deliveries"].as_array().is_some_and(Vec::is_empty)))
}
fn valid_batch(value: &Value, selection: &Value, pin: &Descriptor, prefix: &str) -> bool {
    let Some(items) = value["deliveries"].as_array() else {
        return false;
    };
    let Some(attempts) = value["attempt_ids"].as_array() else {
        return false;
    };
    let text = value["text"].as_str().unwrap_or_default();
    let encoded = serde_json::to_string(&format!("{prefix}{text}")).expect("string serialization");
    let mut ids = std::collections::HashSet::new();
    let mut last_sequence = 0;
    value["stage"] == "bridge_rendered"
        && value["offer_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty() && id.len() <= 256)
        && value["owner_fence"].as_u64().is_some_and(|n| n > 0)
        && value["selection_revision"] == selection["revision"]
        && !text.is_empty()
        && !items.is_empty()
        && items.len() <= 16
        && attempts.len() == items.len()
        && encoded.len() + 128 <= pin.max_bytes
        && encoded.chars().count() + 128 <= pin.max_chars
        && attempts.iter().all(|id| {
            id.as_str()
                .is_some_and(|id| !id.is_empty() && ids.insert(format!("a:{id}")))
        })
        && items.iter().all(|item| {
            let sequence = item["sequence"].as_u64().unwrap_or(0);
            let ordered = sequence > last_sequence;
            last_sequence = sequence;
            ordered
                && item["coverage"] == "full_body"
                && item["body_bytes"].as_u64().is_some_and(|n| n <= 16 * 1024)
                && item["body_chars"]
                    .as_u64()
                    .is_some_and(|n| n <= item["body_bytes"].as_u64().unwrap_or(0))
                && ["message_id", "delivery_id"].iter().all(|key| {
                    item[*key]
                        .as_str()
                        .is_some_and(|id| !id.is_empty() && ids.insert(format!("{key}:{id}")))
                })
        })
}

impl Offer {
    /// Called only after the harness output executor is dropped. Never retry drain.
    pub(super) fn finish(self, flushed: bool) {
        let stage = if self.rendered && flushed {
            "hook_response_offered"
        } else {
            "outcome_unknown"
        };
        let evidence = if self.rendered && flushed {
            "final event response written and flushed; executor closed"
        } else {
            "executor closed; no full output evidence; no automatic replay"
        };
        let result = exchange(
            &self.executable,
            &self.root,
            &self.team,
            &self.member,
            "receipt",
            &json!({"protocol":PROTOCOL,"request":self.request,"offer_id":self.id,"stage":stage,"evidence":evidence}),
        );
        let receipt = serde_json::from_slice::<Value>(&result.bytes).unwrap_or(Value::Null);
        let recorded =
            !result.failed && receipt["protocol"] == PROTOCOL && receipt["status"] == "recorded";
        emit_global(if recorded { "debug" } else { "warn" }, "coordination", "delivery.hook.receipt", None,
            json!({"team":self.team,"member":self.member,"offer_id":self.id,"stage":stage,"receipt_recorded":recorded,"reason":self.reason})
                .as_object().unwrap().clone());
    }
}

struct Exchange {
    bytes: Vec<u8>,
    failed: bool,
}
fn exchange(
    executable: &Path,
    root: &Path,
    team: &str,
    member: &str,
    verb: &str,
    input: &Value,
) -> Exchange {
    let input = serde_json::to_vec(input).unwrap_or_default();
    let mut output = Vec::new();
    // Reserve room for Mesh's additive RPC routing envelope.
    let routing = json!({"root":root,"team":team,"name":member,"op":verb});
    let routing_size = serde_json::to_vec(&routing).map_or(INPUT_LIMIT, |b| b.len());
    let result = if input.len() + routing_size + 128 > INPUT_LIMIT
        || !executable.is_absolute()
        || !root.is_absolute()
    {
        Err(std::io::Error::other("request_budget_or_path"))
    } else {
        child_exchange(executable, root, team, member, verb, &input, &mut output)
    };
    Exchange {
        bytes: output,
        failed: result.is_err(),
    }
}

#[cfg(unix)]
fn child_exchange(
    executable: &Path,
    root: &Path,
    team: &str,
    member: &str,
    verb: &str,
    input: &[u8],
    output: &mut Vec<u8>,
) -> std::io::Result<()> {
    use std::os::{fd::OwnedFd, unix::net::UnixStream};
    use std::process::{Child, Command, Stdio};
    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let end = Instant::now() + DEADLINE;
    let (mut stdin, input_fd) = UnixStream::pair()?;
    let (mut stdout, output_fd) = UnixStream::pair()?;
    let (mut stderr, error_fd) = UnixStream::pair()?;
    for stream in [&stdin, &stdout, &stderr] {
        stream.set_nonblocking(true)?;
    }
    let mut child = OwnedChild(
        Command::new(executable)
            .args(["--claude-dir"])
            .arg(root)
            .args(["--team", team, "--name", member, "delivery", verb])
            .env_clear()
            .env("LANG", "C.UTF-8")
            .current_dir(root)
            .stdin(Stdio::from(OwnedFd::from(input_fd)))
            .stdout(Stdio::from(OwnedFd::from(output_fd)))
            .stderr(Stdio::from(OwnedFd::from(error_fd)))
            .spawn()?,
    );
    let mut offset = 0;
    let mut stdout_eof = false;
    let mut stderr_eof = false;
    let mut captured_error = Vec::new();
    loop {
        if Instant::now() >= end {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "hook child deadline",
            ));
        }
        if offset < input.len() {
            match stdin.write(&input[offset..]) {
                Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
                Ok(n) => {
                    offset += n;
                    if offset == input.len() {
                        stdin.shutdown(std::net::Shutdown::Write)?;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
        }
        for (pipe, bytes, eof, limit, strict) in [
            (
                &mut stdout,
                &mut *output,
                &mut stdout_eof,
                OUTPUT_LIMIT,
                true,
            ),
            (
                &mut stderr,
                &mut captured_error,
                &mut stderr_eof,
                4096,
                false,
            ),
        ] {
            // One chunk per pipe per cycle: even a flooding child cannot starve the deadline.
            let mut buffer = [0; 4096];
            match pipe.read(&mut buffer) {
                Ok(0) => *eof = true,
                Ok(n) => {
                    if strict && bytes.len() + n > limit {
                        return Err(std::io::Error::other("response_budget"));
                    }
                    bytes.extend_from_slice(&buffer[..n.min(limit.saturating_sub(bytes.len()))]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
        }
        if let Some(status) = child.0.try_wait()? {
            if stdout_eof && stderr_eof {
                return if status.success() {
                    Ok(())
                } else {
                    Err(std::io::Error::other("child_nonzero"))
                };
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[cfg(not(unix))]
fn child_exchange(
    _: &Path,
    _: &Path,
    _: &str,
    _: &str,
    _: &str,
    _: &[u8],
    _: &mut Vec<u8>,
) -> std::io::Result<()> {
    Err(std::io::Error::other("native hook transport requires Unix"))
}

/// Explicit live-home bindings supplied by the account reconciler. Shared homes
/// receive the union; an unresolved live account is never guessed from defaults.
pub type Binding = (PathBuf, String, String);
const SCRIPT_PREFIX: &str = "taurhaus-delivery-drain-";

pub fn reconcile_home(
    home: &Path,
    tool: CliTool,
    bindings: &[Binding],
    exe: &Path,
) -> Result<bool, CoordinationError> {
    if !matches!(tool, CliTool::Claude | CliTool::Codex) {
        return Ok(false);
    }
    let filename = if tool == CliTool::Claude {
        CLAUDE_SETTINGS_FILENAME
    } else {
        CODEX_HOOKS_FILENAME
    };
    let settings_path = home.join(filename);
    let mut settings = load_settings_json(&settings_path)?;
    let original = settings.clone();
    let runtime = detect_hook_runtime(home);
    let mut desired = std::collections::BTreeMap::new();
    if settings["disableAllHooks"] != true
        && hook_executable_exists(home, &runtime_path_string(exe, runtime)?)
    {
        if let Some(mesh) = executable() {
            for (teams, team, member) in bindings {
                let Some(config) = read_json(&teams.join(team).join("config.json"), 1024 * 1024)
                else {
                    continue;
                };
                if config["delivery_owner"] != "team" || config["messaging_format"] != 2 {
                    continue;
                }
                if !config["members"].as_array().is_some_and(|members| {
                    members
                        .iter()
                        .any(|m| m["name"] == *member && m["isActive"] != false)
                }) {
                    continue;
                }
                let Some(state) = read_json(
                    &teams
                        .join(team)
                        .join(format!("state/delivery/adapter-{member}.json")),
                    4 * 1024 * 1024,
                ) else {
                    continue;
                };
                if state["mode"] != "hook" {
                    continue;
                }
                let Some(root) = teams.parent() else {
                    continue;
                };
                for pin in descriptors(&mesh, root, team, member) {
                    if pin.harness != tool.to_string() || pin.source == COMPACT_SOURCE {
                        continue;
                    }
                    // The ordinary SessionStart descriptor has an exact source matcher;
                    // prose from a disabled discovery descriptor never becomes config.
                    if pin.event == SESSION_START_HOOK_EVENT && pin.matcher != pin.source {
                        continue;
                    }
                    use sha2::{Digest, Sha256};
                    let key = hex::encode(Sha256::digest(root.to_string_lossy().as_bytes()));
                    desired.insert(
                        (key, pin.event.clone(), pin.matcher.clone()),
                        (root.to_path_buf(), pin),
                    );
                }
            }
        }
    }
    let owned = |hook: &Value| {
        let Some(command) = hook["command"].as_str() else {
            return false;
        };
        let Some(suffix) = command.split_once(SCRIPT_PREFIX).map(|(_, suffix)| suffix) else {
            return false;
        };
        let Some(hash) = suffix
            .get(..64)
            .filter(|hash| hash.bytes().all(|b| b.is_ascii_hexdigit()))
        else {
            return false;
        };
        let extension = if runtime == HookRuntime::Windows {
            "cmd"
        } else {
            "sh"
        };
        let script = home
            .join("hooks")
            .join(format!("{SCRIPT_PREFIX}{hash}.{extension}"));
        settings_command_for_script(&script, runtime).is_ok_and(|expected| command == expected)
    };
    // Do not erase per-hook user disablement when reconciling a shared home.
    if settings["hooks"]
        .as_object()
        .into_iter()
        .flat_map(|events| events.values())
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|entry| entry["hooks"].as_array())
        .flatten()
        .any(|hook| owned(hook) && (hook["disabled"] == true || hook["enabled"] == false))
    {
        return Ok(false);
    }
    if settings.get("hooks").is_none() && desired.is_empty() {
        return Ok(false);
    }
    let hooks = settings
        .as_object_mut()
        .ok_or_else(|| CoordinationError::Validation("hook settings object required".into()))?
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| CoordinationError::Validation("hook events object required".into()))?;
    let mut old_scripts = Vec::new();
    for entries in hooks.values_mut() {
        let entries = entries
            .as_array_mut()
            .ok_or_else(|| CoordinationError::Validation("hook entries array required".into()))?;
        for entry in entries.iter_mut() {
            let commands = entry
                .get_mut("hooks")
                .and_then(Value::as_array_mut)
                .ok_or_else(|| {
                    CoordinationError::Validation("hook commands array required".into())
                })?;
            commands.retain(|hook| {
                if owned(hook) {
                    old_scripts.push(hook["command"].clone());
                    false
                } else {
                    true
                }
            });
        }
        entries.retain(|entry| {
            entry["hooks"]
                .as_array()
                .is_none_or(|commands| !commands.is_empty())
        });
    }
    hooks.retain(|_, entries| entries.as_array().is_none_or(|entries| !entries.is_empty()));
    let mut scripts = Vec::new();
    for ((key, event, matcher), (root, pin)) in desired {
        let extension = if runtime == HookRuntime::Windows {
            "cmd"
        } else {
            "sh"
        };
        let script = home
            .join("hooks")
            .join(format!("{SCRIPT_PREFIX}{key}.{extension}"));
        let command = settings_command_for_script(&script, runtime)?;
        let hook = command_hook_value(
            &command,
            (tool == CliTool::Codex).then_some(pin.max_chars as u64),
        );
        hooks
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .expect("validated entries")
            .push(json!({"matcher":matcher,"hooks":[hook]}));
        scripts.push((script, root));
    }
    // Validate everything before mutation, install every target before config teardown.
    let mut changed = false;
    for (script, root) in &scripts {
        fs::create_dir_all(script.parent().expect("script parent"))?;
        changed |= write_hook_script_with_claude_dir(script, exe, runtime, Some(root))?;
    }
    if settings != original {
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_atomic_settings_file(
            &settings_path,
            &serde_json::to_vec_pretty(&settings)
                .map_err(|e| CoordinationError::StoreError(e.to_string()))?,
        )?;
        changed = true;
    }
    // Remove only the exact generated names, after their registrations are gone.
    if let Ok(entries) = fs::read_dir(home.join("hooks")) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(hash) = name.strip_prefix(SCRIPT_PREFIX) else {
                continue;
            };
            if hash.len() == 64
                && hash.bytes().all(|b| b.is_ascii_hexdigit())
                && !scripts.iter().any(|(active, _)| active == &path)
                && old_scripts
                    .iter()
                    .any(|c| c.as_str().is_some_and(|c| c.contains(name)))
            {
                fs::remove_file(path)?;
                changed = true;
            }
        }
    }
    Ok(changed)
}
