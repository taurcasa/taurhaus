use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, SystemTime};

use chrono::DateTime;
use serde_json::{Map, Value};

use super::{
    WorkflowActivity, WorkflowAgent, WorkflowAgentState, WorkflowRun, WorkflowRunStatus,
    WorkflowRunTotals,
};

const SCRIPT_META_READ_LIMIT: u64 = 64 * 1024;
const TRANSCRIPT_PREFIX_LIMIT: usize = 16 * 1024;
const TRANSCRIPT_TAIL_LIMIT: usize = 256 * 1024;
const TRANSCRIPT_CACHE_ENTRIES: usize = 256;
const PROMPT_PREVIEW_CHARS: usize = 200;
const ACTIVITY_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileStamp {
    len: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Clone, Default)]
struct TranscriptFacts {
    prompt_preview: String,
    model: Option<String>,
    last_tool: Option<String>,
    tokens: Option<u64>,
    tool_calls: Option<u32>,
    last_write_at: i64,
}

#[derive(Debug, Clone)]
struct CachedTranscript {
    stamp: FileStamp,
    facts: TranscriptFacts,
}

static TRANSCRIPT_CACHE: LazyLock<Mutex<HashMap<PathBuf, CachedTranscript>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Default)]
struct ScriptMeta {
    name: String,
    description: String,
    phases: Vec<String>,
}

#[derive(Debug, Clone)]
struct JournalAgent {
    agent_id: String,
    state: WorkflowAgentState,
    result: Option<Value>,
}

pub fn scan_session_runs(session_dir: &Path) -> Vec<WorkflowRun> {
    let run_root = session_dir.join("subagents/workflows");
    let Ok(entries) = fs::read_dir(run_root) else {
        return Vec::new();
    };
    let mut runs = entries
        .flatten()
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            let run_id = entry.file_name().to_str()?.to_string();
            (file_type.is_dir() && safe_id(&run_id))
                .then(|| read_run(session_dir, &run_id))
                .flatten()
        })
        .collect::<Vec<_>>();
    runs.sort_by(|left, right| {
        right
            .started_at
            .cmp(&left.started_at)
            .then_with(|| left.run_id.cmp(&right.run_id))
    });
    runs
}

pub fn read_run(session_dir: &Path, run_id: &str) -> Option<WorkflowRun> {
    if !safe_id(run_id) {
        return None;
    }
    let run_dir = session_dir.join("subagents/workflows").join(run_id);
    if !run_dir.is_dir() {
        return None;
    }

    let script_path = find_script(session_dir, run_id);
    let script_meta = script_path
        .as_deref()
        .and_then(read_script_meta)
        .unwrap_or_default();
    let summary_path = session_dir.join("workflows").join(format!("{run_id}.json"));
    let summary = fs::read_to_string(&summary_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .filter(Value::is_object);

    match summary {
        Some(summary) => completed_run(run_id, &run_dir, script_path, script_meta, &summary),
        None => Some(live_run(run_id, &run_dir, script_path, script_meta)),
    }
}

pub fn workflow_activity(session_dir: &Path, now: SystemTime) -> Option<WorkflowActivity> {
    let run_root = session_dir.join("subagents/workflows");
    let entries = fs::read_dir(run_root).ok()?;
    let mut live_runs = 0_u32;
    let mut latest = None;

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let Some(run_id) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !file_type.is_dir()
            || !safe_id(&run_id)
            || session_dir
                .join("workflows")
                .join(format!("{run_id}.json"))
                .is_file()
        {
            continue;
        }
        live_runs = live_runs.saturating_add(1);
        let Ok(files) = fs::read_dir(entry.path()) else {
            continue;
        };
        for file in files.flatten() {
            let name = file.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Ok(metadata) = fs::symlink_metadata(file.path()) else {
                continue;
            };
            if !metadata.file_type().is_file()
                || !name.starts_with("agent-")
                || !name.ends_with(".jsonl")
            {
                continue;
            }
            if let Ok(modified) = metadata.modified() {
                latest = Some(latest.map_or(modified, |current: SystemTime| current.max(modified)));
            }
        }
    }

    let latest = latest?;
    if now.duration_since(latest).unwrap_or_default() > ACTIVITY_WINDOW {
        return None;
    }
    Some(WorkflowActivity {
        live_runs,
        last_write_at: system_time_ms(latest),
    })
}

fn live_run(
    run_id: &str,
    run_dir: &Path,
    script_path: Option<PathBuf>,
    script_meta: ScriptMeta,
) -> WorkflowRun {
    let journal_path = run_dir.join("journal.jsonl");
    let journal = read_journal(&journal_path);
    let mut agents = Vec::with_capacity(journal.len());
    for journal_agent in journal {
        let transcript_path = run_dir.join(format!("agent-{}.jsonl", journal_agent.agent_id));
        let facts = read_transcript(&transcript_path);
        agents.push(WorkflowAgent {
            agent_id: journal_agent.agent_id,
            label: None,
            phase: None,
            model: facts.model,
            state: journal_agent.state,
            prompt_preview: facts.prompt_preview,
            last_tool: facts.last_tool,
            tokens: facts.tokens,
            tool_calls: facts.tool_calls,
            last_write_at: facts.last_write_at,
            result_preview: journal_agent.result,
        });
    }

    let started_at = script_path
        .as_deref()
        .and_then(file_mtime_ms)
        .into_iter()
        .chain(file_mtime_ms(&journal_path))
        .chain(
            agents
                .iter()
                .map(|agent| agent.last_write_at)
                .filter(|timestamp| *timestamp > 0),
        )
        .min()
        .unwrap_or_default();
    let totals = totals_from_agents(&agents, None);

    WorkflowRun {
        run_id: run_id.to_string(),
        name: non_empty(&script_meta.name).unwrap_or(run_id).to_string(),
        description: script_meta.description,
        phases: script_meta.phases,
        status: WorkflowRunStatus::Live,
        started_at,
        finished_at: None,
        agents,
        totals,
        result: None,
        script_path: script_path.unwrap_or_default(),
    }
}

fn completed_run(
    run_id: &str,
    run_dir: &Path,
    script_path: Option<PathBuf>,
    script_meta: ScriptMeta,
    summary: &Value,
) -> Option<WorkflowRun> {
    let object = summary.as_object()?;
    let started_at = integer(object.get("startTime")).unwrap_or_else(|| {
        script_path
            .as_deref()
            .and_then(file_mtime_ms)
            .unwrap_or_default()
    });
    let duration_ms = unsigned(object.get("durationMs"));
    let finished_at = object
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(timestamp_ms)
        .or_else(|| duration_ms.map(|duration| started_at.saturating_add(duration as i64)));
    let status = match object.get("status").and_then(Value::as_str) {
        Some("completed" | "success") => WorkflowRunStatus::Completed,
        Some("failed" | "error") => WorkflowRunStatus::Failed,
        _ => WorkflowRunStatus::Unknown,
    };
    let agents = object
        .get("workflowProgress")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|progress| progress.get("type").and_then(Value::as_str) == Some("workflow_agent"))
        .filter_map(|progress| summary_agent(run_dir, progress))
        .collect::<Vec<_>>();
    let done = agents
        .iter()
        .filter(|agent| {
            matches!(
                agent.state,
                WorkflowAgentState::Done | WorkflowAgentState::Failed
            )
        })
        .count() as u32;
    let phases = object
        .get("phases")
        .and_then(Value::as_array)
        .map(|phases| {
            phases
                .iter()
                .filter_map(|phase| phase.get("title").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|phases| !phases.is_empty())
        .unwrap_or(script_meta.phases);
    let summary_script_path = object
        .get("scriptPath")
        .and_then(Value::as_str)
        .map(PathBuf::from);

    Some(WorkflowRun {
        run_id: object
            .get("runId")
            .and_then(Value::as_str)
            .unwrap_or(run_id)
            .to_string(),
        name: object
            .get("workflowName")
            .and_then(Value::as_str)
            .or_else(|| non_empty(&script_meta.name))
            .unwrap_or(run_id)
            .to_string(),
        description: script_meta.description,
        phases,
        status,
        started_at,
        finished_at,
        totals: WorkflowRunTotals {
            agents: unsigned(object.get("agentCount"))
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(agents.len() as u32),
            done,
            tokens: unsigned(object.get("totalTokens")),
            tool_calls: unsigned(object.get("totalToolCalls")),
            duration_ms,
        },
        agents,
        result: object
            .get("result")
            .filter(|value| !value.is_null())
            .cloned(),
        script_path: script_path.or(summary_script_path).unwrap_or_default(),
    })
}

fn summary_agent(run_dir: &Path, progress: &Value) -> Option<WorkflowAgent> {
    let object = progress.as_object()?;
    let agent_id = object.get("agentId")?.as_str()?.to_string();
    let transcript_mtime = file_mtime_ms(&run_dir.join(format!("agent-{agent_id}.jsonl")));
    Some(WorkflowAgent {
        agent_id,
        label: string(object.get("label")),
        phase: string(object.get("phaseTitle")),
        model: string(object.get("model")),
        state: match object.get("state").and_then(Value::as_str) {
            Some("queued") => WorkflowAgentState::Queued,
            Some("done" | "completed" | "success") => WorkflowAgentState::Done,
            Some("failed" | "error") => WorkflowAgentState::Failed,
            _ => WorkflowAgentState::Running,
        },
        prompt_preview: object
            .get("promptPreview")
            .and_then(Value::as_str)
            .map(preview)
            .unwrap_or_default(),
        last_tool: string(object.get("lastToolName")),
        tokens: unsigned(object.get("tokens")),
        tool_calls: unsigned(object.get("toolCalls")).and_then(|value| u32::try_from(value).ok()),
        last_write_at: integer(object.get("lastProgressAt"))
            .or_else(|| integer(object.get("startedAt")))
            .or(transcript_mtime)
            .unwrap_or_default(),
        result_preview: object
            .get("resultPreview")
            .filter(|value| !value.is_null())
            .cloned(),
    })
}

fn read_journal(path: &Path) -> Vec<JournalAgent> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut agents = Vec::<JournalAgent>::new();
    let mut positions = HashMap::<String, usize>::new();
    for value in raw
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    {
        let Some(object) = value.as_object() else {
            continue;
        };
        let Some(agent_id) = object.get("agentId").and_then(Value::as_str) else {
            continue;
        };
        let entry = match positions.get(agent_id).copied() {
            Some(index) => &mut agents[index],
            None => {
                positions.insert(agent_id.to_string(), agents.len());
                agents.push(JournalAgent {
                    agent_id: agent_id.to_string(),
                    state: WorkflowAgentState::Queued,
                    result: None,
                });
                agents.last_mut().expect("agent was pushed")
            }
        };
        match object.get("type").and_then(Value::as_str) {
            Some("started") => entry.state = WorkflowAgentState::Running,
            Some("result") => {
                entry.state = WorkflowAgentState::Done;
                entry.result = object
                    .get("result")
                    .filter(|value| !value.is_null())
                    .cloned();
            }
            _ => {}
        }
    }
    agents
}

fn read_transcript(path: &Path) -> TranscriptFacts {
    let Ok(metadata) = fs::metadata(path) else {
        return TranscriptFacts::default();
    };
    let stamp = FileStamp {
        len: metadata.len(),
        modified: metadata.modified().ok(),
    };
    if let Some(facts) = TRANSCRIPT_CACHE
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(path)
        .filter(|cached| cached.stamp == stamp)
        .map(|cached| cached.facts.clone())
    {
        return facts;
    }

    let facts = read_transcript_uncached(path, &stamp).unwrap_or_default();
    let mut cache = TRANSCRIPT_CACHE
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if cache.len() >= TRANSCRIPT_CACHE_ENTRIES && !cache.contains_key(path) {
        cache.clear();
    }
    cache.insert(
        path.to_path_buf(),
        CachedTranscript {
            stamp,
            facts: facts.clone(),
        },
    );
    facts
}

fn read_transcript_uncached(path: &Path, stamp: &FileStamp) -> Option<TranscriptFacts> {
    let mut file = File::open(path).ok()?;
    let len = usize::try_from(stamp.len).unwrap_or(usize::MAX);
    let (prefix, tail, complete) = if len <= TRANSCRIPT_TAIL_LIMIT {
        let mut bytes = Vec::with_capacity(len);
        file.read_to_end(&mut bytes).ok()?;
        (bytes.clone(), bytes, true)
    } else {
        let mut prefix = vec![0; TRANSCRIPT_PREFIX_LIMIT];
        let prefix_len = file.read(&mut prefix).ok()?;
        prefix.truncate(prefix_len);
        file.seek(SeekFrom::End(-(TRANSCRIPT_TAIL_LIMIT as i64)))
            .ok()?;
        let mut tail = Vec::with_capacity(TRANSCRIPT_TAIL_LIMIT);
        file.read_to_end(&mut tail).ok()?;
        if let Some(newline) = tail.iter().position(|byte| *byte == b'\n') {
            tail.drain(..=newline);
        }
        (prefix, tail, false)
    };

    let mut facts = TranscriptFacts {
        prompt_preview: first_user_prompt(&prefix),
        last_write_at: stamp.modified.map(system_time_ms).unwrap_or_default(),
        tokens: complete.then_some(0),
        tool_calls: complete.then_some(0),
        ..Default::default()
    };
    let mut message_ids = HashSet::new();
    for (line_number, line) in tail.split(|byte| *byte == b'\n').enumerate() {
        let Ok(value) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        let Some(message) = value.get("message").and_then(Value::as_object) else {
            continue;
        };
        if message.get("role").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        if let Some(model) = message.get("model").and_then(Value::as_str) {
            facts.model = Some(model.to_string());
        }
        if let Some(content) = message.get("content").and_then(Value::as_array) {
            for block in content {
                if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                    if let Some(name) = block.get("name").and_then(Value::as_str) {
                        facts.last_tool = Some(name.to_string());
                    }
                    if let Some(tool_calls) = &mut facts.tool_calls {
                        *tool_calls = tool_calls.saturating_add(1);
                    }
                }
            }
        }
        let message_key = message
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("line-{line_number}"));
        if message_ids.insert(message_key) {
            if let (Some(total), Some(usage)) = (
                &mut facts.tokens,
                message.get("usage").and_then(Value::as_object),
            ) {
                for key in [
                    "input_tokens",
                    "output_tokens",
                    "cache_read_input_tokens",
                    "cache_creation_input_tokens",
                ] {
                    *total = total.saturating_add(unsigned(usage.get(key)).unwrap_or_default());
                }
            }
        }
    }
    Some(facts)
}

fn first_user_prompt(bytes: &[u8]) -> String {
    bytes
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
        .find_map(|value| {
            let message = value.get("message")?.as_object()?;
            (message.get("role")?.as_str()? == "user")
                .then(|| content_text(message.get("content")))
                .flatten()
        })
        .map(|prompt| preview(&prompt))
        .unwrap_or_default()
}

fn content_text(content: Option<&Value>) -> Option<String> {
    match content? {
        Value::String(text) => Some(text.clone()),
        Value::Array(blocks) => Some(
            blocks
                .iter()
                .filter_map(|block| {
                    (block.get("type").and_then(Value::as_str) == Some("text"))
                        .then(|| block.get("text").and_then(Value::as_str))
                        .flatten()
                })
                .collect::<Vec<_>>()
                .join(" "),
        ),
        _ => None,
    }
}

fn totals_from_agents(agents: &[WorkflowAgent], duration_ms: Option<u64>) -> WorkflowRunTotals {
    let metrics_complete = agents
        .iter()
        .all(|agent| agent.tokens.is_some() && agent.tool_calls.is_some());
    WorkflowRunTotals {
        agents: agents.len() as u32,
        done: agents
            .iter()
            .filter(|agent| {
                matches!(
                    agent.state,
                    WorkflowAgentState::Done | WorkflowAgentState::Failed
                )
            })
            .count() as u32,
        tokens: metrics_complete.then(|| {
            agents
                .iter()
                .filter_map(|agent| agent.tokens)
                .fold(0_u64, u64::saturating_add)
        }),
        tool_calls: metrics_complete.then(|| {
            agents
                .iter()
                .filter_map(|agent| agent.tool_calls)
                .map(u64::from)
                .fold(0_u64, u64::saturating_add)
        }),
        duration_ms,
    }
}

fn find_script(session_dir: &Path, run_id: &str) -> Option<PathBuf> {
    let suffix = format!("-{run_id}.js");
    let mut scripts = fs::read_dir(session_dir.join("workflows/scripts"))
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            let name = entry.file_name();
            let name = name.to_str()?;
            (file_type.is_file() && name.ends_with(&suffix)).then(|| entry.path())
        })
        .collect::<Vec<_>>();
    scripts.sort();
    scripts.into_iter().next()
}

fn read_script_meta(path: &Path) -> Option<ScriptMeta> {
    let mut source = String::new();
    File::open(path)
        .ok()?
        .take(SCRIPT_META_READ_LIMIT)
        .read_to_string(&mut source)
        .ok()?;
    let declaration = source.find("export const meta")?;
    let equals = source[declaration..].find('=')? + declaration;
    let mut parser = JsLiteralParser::new(&source[equals + 1..]);
    let object = parser.parse_value()?.as_object()?.clone();
    Some(ScriptMeta {
        name: object
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        description: object
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        phases: object
            .get("phases")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|phase| phase.get("title").and_then(Value::as_str))
            .map(str::to_string)
            .collect(),
    })
}

struct JsLiteralParser<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl<'a> JsLiteralParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            bytes: source.as_bytes(),
            index: 0,
        }
    }

    fn parse_value(&mut self) -> Option<Value> {
        self.skip_trivia();
        match self.peek()? {
            b'{' => self.parse_object(),
            b'[' => self.parse_array(),
            b'\'' | b'"' | b'`' => self.parse_string().map(Value::String),
            b'-' | b'0'..=b'9' => self.parse_number(),
            _ => match self.parse_identifier()?.as_str() {
                "true" => Some(Value::Bool(true)),
                "false" => Some(Value::Bool(false)),
                "null" => Some(Value::Null),
                _ => None,
            },
        }
    }

    fn parse_object(&mut self) -> Option<Value> {
        self.expect(b'{')?;
        let mut object = Map::new();
        loop {
            self.skip_trivia();
            if self.consume(b'}') {
                break;
            }
            let key = match self.peek()? {
                b'\'' | b'"' | b'`' => self.parse_string()?,
                _ => self.parse_identifier()?,
            };
            self.skip_trivia();
            self.expect(b':')?;
            let value = self.parse_value()?;
            object.insert(key, value);
            self.skip_trivia();
            if self.consume(b'}') {
                break;
            }
            self.expect(b',')?;
        }
        Some(Value::Object(object))
    }

    fn parse_array(&mut self) -> Option<Value> {
        self.expect(b'[')?;
        let mut values = Vec::new();
        loop {
            self.skip_trivia();
            if self.consume(b']') {
                break;
            }
            values.push(self.parse_value()?);
            self.skip_trivia();
            if self.consume(b']') {
                break;
            }
            self.expect(b',')?;
        }
        Some(Value::Array(values))
    }

    fn parse_string(&mut self) -> Option<String> {
        let quote = self.next()?;
        let mut output = Vec::new();
        loop {
            let byte = self.next()?;
            if byte == quote {
                return String::from_utf8(output).ok();
            }
            if quote == b'`' && byte == b'$' && self.peek() == Some(b'{') {
                return None;
            }
            if byte != b'\\' {
                output.push(byte);
                continue;
            }
            let escaped = self.next()?;
            match escaped {
                b'n' => output.push(b'\n'),
                b'r' => output.push(b'\r'),
                b't' => output.push(b'\t'),
                b'b' => output.push(8),
                b'f' => output.push(12),
                b'v' => output.push(11),
                b'0' => output.push(0),
                b'\n' => {}
                other => output.push(other),
            }
        }
    }

    fn parse_number(&mut self) -> Option<Value> {
        let start = self.index;
        while self.peek().is_some_and(|byte| {
            byte.is_ascii_digit() || matches!(byte, b'-' | b'+' | b'.' | b'e' | b'E')
        }) {
            self.index += 1;
        }
        serde_json::from_slice(&self.bytes[start..self.index]).ok()
    }

    fn parse_identifier(&mut self) -> Option<String> {
        let start = self.index;
        if !self
            .peek()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$'))
        {
            return None;
        }
        self.index += 1;
        while self
            .peek()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
        {
            self.index += 1;
        }
        std::str::from_utf8(&self.bytes[start..self.index])
            .ok()
            .map(str::to_string)
    }

    fn skip_trivia(&mut self) {
        loop {
            while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
                self.index += 1;
            }
            if self.bytes.get(self.index..self.index + 2) == Some(b"//") {
                self.index += 2;
                while self.peek().is_some_and(|byte| byte != b'\n') {
                    self.index += 1;
                }
                continue;
            }
            if self.bytes.get(self.index..self.index + 2) == Some(b"/*") {
                self.index += 2;
                while self.index + 1 < self.bytes.len()
                    && self.bytes.get(self.index..self.index + 2) != Some(b"*/")
                {
                    self.index += 1;
                }
                self.index = (self.index + 2).min(self.bytes.len());
                continue;
            }
            break;
        }
    }

    fn expect(&mut self, byte: u8) -> Option<()> {
        self.skip_trivia();
        self.consume(byte).then_some(())
    }

    fn consume(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.index += 1;
        Some(byte)
    }
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn file_mtime_ms(path: &Path) -> Option<i64> {
    fs::metadata(path).ok()?.modified().ok().map(system_time_ms)
}

fn system_time_ms(time: SystemTime) -> i64 {
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_millis()).unwrap_or(i64::MAX),
        Err(error) => -i64::try_from(error.duration().as_millis()).unwrap_or(i64::MAX),
    }
}

fn timestamp_ms(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.timestamp_millis())
}

fn integer(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
    })
}

fn unsigned(value: Option<&Value>) -> Option<u64> {
    value.and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
    })
}

fn string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_string)
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn preview(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(PROMPT_PREVIEW_CHARS)
        .collect()
}
