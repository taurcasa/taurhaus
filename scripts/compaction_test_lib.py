#!/usr/bin/env python3
from __future__ import annotations

import json
import fcntl
import os
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Dict, Iterable, Iterator, List, Optional, Tuple

DEFAULT_TEAMS_DIR = Path.home() / ".claude" / "teams"
DEFAULT_DEBUG_DIR = Path.home() / ".claude" / "debug"


class CompactionTestError(RuntimeError):
    pass


@dataclass
class MemberTarget:
    teams_dir: Path
    tmux_socket: Path
    attachment_generation: int
    team_name: str
    member_name: str
    model: Optional[str]
    cli_tool: str
    pane_id: str
    project_path: Path
    runtime_session_id: Optional[str]
    runtime_health: Optional[str]
    runtime_jsonl_path: Optional[Path]
    operational_snapshot_path: Path


@dataclass
class ManualRun:
    run_id: str
    tool: str
    team_name: str
    member_name: str
    pane_id: str
    project_path: str
    session_id: Optional[str]
    jsonl_path: Optional[str]
    debug_log_path: Optional[str]
    trigger_command: str
    trigger_mode: str
    triggered_at: str

    def to_dict(self) -> Dict[str, Any]:
        return {
            "run_id": self.run_id,
            "tool": self.tool,
            "team_name": self.team_name,
            "member_name": self.member_name,
            "pane_id": self.pane_id,
            "project_path": self.project_path,
            "session_id": self.session_id,
            "jsonl_path": self.jsonl_path,
            "debug_log_path": self.debug_log_path,
            "trigger_command": self.trigger_command,
            "trigger_mode": self.trigger_mode,
            "triggered_at": self.triggered_at,
        }


def now_utc() -> datetime:
    return datetime.now(timezone.utc)


def to_iso(dt: datetime) -> str:
    return dt.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


def parse_iso(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00")).astimezone(timezone.utc)


def infer_tool(cli_tool: Any, model: Any) -> Optional[str]:
    if isinstance(cli_tool, str) and cli_tool.strip():
        return cli_tool.strip().lower()
    if not isinstance(model, str) or not model.strip():
        return None
    lower = model.strip().lower()
    if "claude" in lower:
        return "claude"
    if "gpt" in lower or "codex" in lower:
        return "codex"
    # Antigravity roles run gemini-* model ids (see the bundled
    # antigravity-* role templates). The retired Gemini CLI is not a tool
    # value any more, so a gemini-* model means agy.
    if "gemini" in lower:
        return "agy"
    if "grok" in lower:
        return "grok"
    return None


def load_json(path: Path) -> Dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise CompactionTestError(f"missing file: {path}") from exc
    except json.JSONDecodeError as exc:
        raise CompactionTestError(f"invalid JSON in {path}: {exc}") from exc


def team_paths(teams_dir: Path, team_name: str) -> Tuple[Path, Path, Path, Path]:
    team_dir = teams_dir / team_name
    return (
        team_dir,
        team_dir / "config.json",
        team_dir / "runtime",
        team_dir / "state" / "operational",
    )


def resolve_member_target(teams_dir: Path, team_name: str, member_name: str) -> MemberTarget:
    team_dir, config_path, runtime_dir, operational_dir = team_paths(teams_dir, team_name)
    config = load_json(config_path)
    members = config.get("members")
    if not isinstance(members, list):
        raise CompactionTestError(f"invalid members list in {config_path}")

    member = next((m for m in members if isinstance(m, dict) and m.get("name") == member_name), None)
    if member is None:
        raise CompactionTestError(f"member '{member_name}' not found in {config_path}")

    model = member.get("model")
    cli_tool = infer_tool(member.get("cliTool"), model)
    if cli_tool is None:
        raise CompactionTestError(f"could not infer cli tool for member '{member_name}'")

    pane_id = member.get("tmuxPaneId")
    project_path = member.get("projectPath")
    if not isinstance(pane_id, str) or not pane_id:
        raise CompactionTestError(f"member '{member_name}' has no tmuxPaneId")
    if not isinstance(project_path, str) or not project_path:
        raise CompactionTestError(f"member '{member_name}' has no projectPath")

    runtime_path = runtime_dir / f"{member_name}.json"
    runtime = load_json(runtime_path)
    operational_snapshot_path = operational_dir / f"{member_name}.json"

    runtime_jsonl_path = runtime.get("jsonl_path")
    socket = runtime.get("tmuxSocket")
    if not isinstance(socket, str) or not Path(socket).is_absolute() or runtime.get("terminalContract") != 1:
        raise CompactionTestError("member has no terminal contract/socket; relaunch before injection")
    return MemberTarget(
        teams_dir=teams_dir,
        tmux_socket=Path(socket),
        attachment_generation=runtime.get("attachmentGeneration", 0),
        team_name=team_name,
        member_name=member_name,
        model=model if isinstance(model, str) else None,
        cli_tool=cli_tool,
        pane_id=runtime.get("paneId") or pane_id,
        project_path=Path(project_path),
        runtime_session_id=runtime.get("session_id") if isinstance(runtime.get("session_id"), str) else None,
        runtime_health=runtime.get("health") if isinstance(runtime.get("health"), str) else None,
        runtime_jsonl_path=Path(runtime_jsonl_path) if isinstance(runtime_jsonl_path, str) and runtime_jsonl_path else None,
        operational_snapshot_path=operational_snapshot_path,
    )


def ensure_resumable_task(snapshot_path: Path) -> Dict[str, Any]:
    snapshot = load_json(snapshot_path)
    task = snapshot.get("task")
    if not isinstance(task, dict):
        raise CompactionTestError(f"operational snapshot has no task: {snapshot_path}")
    task_id = task.get("id")
    status = task.get("status")
    if not ((isinstance(task_id, int) and task_id >= 0) or (isinstance(task_id, str) and task_id.strip())):
        raise CompactionTestError(f"operational snapshot has no active task id: {snapshot_path}")
    if status in {"completed", "deleted"}:
        raise CompactionTestError(
            f"operational snapshot task is not resumable ({status}) in {snapshot_path}"
        )
    return snapshot


def tmux_send_literal(target: MemberTarget, text: str) -> None:
    for name in (target.team_name, target.member_name):
        if not name or name in (".", "..") or "/" in name or "\\" in name:
            raise CompactionTestError("invalid terminal member path")
    if not target.tmux_socket.is_absolute():
        raise CompactionTestError("terminal socket must be absolute")
    terminal = target.teams_dir / target.team_name / "state/terminal"
    terminal.mkdir(parents=True, exist_ok=True)
    holder = terminal / f"{target.member_name}.holder.json"
    with (terminal / f"{target.member_name}.lock").open("a+") as lock:
        deadline = time.monotonic() + 2
        while True:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise CompactionTestError("terminal write deferred: lock busy")
                time.sleep(0.01)
            except OSError as error:
                raise CompactionTestError(f"terminal writes disabled: flock unavailable: {error}") from error
        try:
            holder.write_text(json.dumps({"owner": "taurhaus", "op": "compaction-test", "epoch": target.attachment_generation, "since": to_iso(now_utc())}))
            deadline = time.monotonic() + 10
            env = dict(os.environ)
            env.pop("TMUX", None)
            for keys in (["-l", text], ["Enter"]):
                result = subprocess.run([sys.executable, str(Path(__file__).resolve()), "--terminal-child", str(deadline - 0.05), "tmux", "-S", str(target.tmux_socket), "send-keys", "-t", target.pane_id, *keys], stdin=lock, env=env, text=True, capture_output=True, timeout=max(0.001, deadline - time.monotonic()))
                if result.returncode:
                    raise CompactionTestError(f"terminal injection failed: {result.stderr}")
        finally:
            holder.unlink(missing_ok=True)


def run_checked(argv: List[str]) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(argv, text=True, capture_output=True)
    if result.returncode != 0:
        stderr = (result.stderr or result.stdout or "").strip()
        raise CompactionTestError(f"command failed ({result.returncode}): {' '.join(argv)} :: {stderr}")
    return result


def build_run_id(tool: str, team_name: str, member_name: str) -> str:
    token = uuid.uuid4().hex[:8]
    return f"{tool}-{team_name}-{member_name}-{token}"


def manual_run_dir(teams_dir: Path, team_name: str) -> Path:
    return teams_dir / team_name / "state" / "compaction" / "manual-runs"


def write_manual_run(teams_dir: Path, run: ManualRun) -> Path:
    target_dir = manual_run_dir(teams_dir, run.team_name)
    target_dir.mkdir(parents=True, exist_ok=True)
    path = target_dir / f"{run.run_id}.json"
    path.write_text(json.dumps(run.to_dict(), indent=2) + "\n", encoding="utf-8")
    return path


def iter_jsonl_records(path: Path) -> Iterator[Dict[str, Any]]:
    if not path.exists():
        return
    with path.open("r", encoding="utf-8", errors="replace") as handle:
        for raw in handle:
            line = raw.strip()
            if not line:
                continue
            try:
                yield json.loads(line)
            except json.JSONDecodeError:
                continue


def discover_log_candidates() -> List[Path]:
    override = os.environ.get("TAURHAUS_DATA_DIR")
    candidates: List[Path] = []
    if override:
        candidates.append(Path(override) / "taurhaus.log.jsonl")
    candidates.append(Path.home() / ".local" / "share" / "com.taurhaus.dev" / "taurhaus.log.jsonl")
    candidates.extend(sorted(Path("/mnt/c/Users").glob("*/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl")))
    return candidates


def resolve_active_log_path() -> Path:
    existing = []
    for candidate in discover_log_candidates():
        try:
            stat = candidate.stat()
        except FileNotFoundError:
            continue
        existing.append((stat.st_mtime, stat.st_size, str(candidate), candidate))
    if not existing:
        raise CompactionTestError("no taurhaus.log.jsonl candidates found")
    existing.sort(reverse=True)
    return existing[0][3]


def latest_claude_debug_log(debug_dir: Path = DEFAULT_DEBUG_DIR) -> Path:
    latest = debug_dir / "latest"
    if latest.exists():
        return latest.resolve()
    files = sorted(debug_dir.glob("*.txt"), key=lambda p: p.stat().st_mtime)
    if not files:
        raise CompactionTestError(f"no Claude debug logs in {debug_dir}")
    return files[-1]


def wait_for(predicate: Callable[[], Optional[Dict[str, Any]]], timeout_seconds: int, interval_seconds: float = 1.0) -> Dict[str, Any]:
    deadline = time.time() + timeout_seconds
    last_error: Optional[Exception] = None
    while time.time() < deadline:
        try:
            payload = predicate()
        except Exception as exc:  # pragma: no cover - best-effort polling helper
            last_error = exc
            payload = None
        if payload is not None:
            return payload
        time.sleep(interval_seconds)
    if last_error is not None:
        raise CompactionTestError(f"timed out while polling: {last_error}")
    raise CompactionTestError("timed out waiting for expected compaction evidence")


def event_matches(record: Dict[str, Any], *, event: str, team_name: Optional[str], member_name: Optional[str], session_id: Optional[str], since: datetime) -> bool:
    if record.get("event") != event:
        return False
    ts = record.get("ts")
    if not isinstance(ts, str) or parse_iso(ts) < since:
        return False
    if team_name and record.get("team_name") != team_name:
        return False
    if member_name and record.get("member_name") != member_name:
        return False
    if session_id and record.get("session_id") != session_id:
        return False
    return True


def find_log_event(log_path: Path, *, event: str, team_name: Optional[str], member_name: Optional[str], session_id: Optional[str], since: datetime) -> Optional[Dict[str, Any]]:
    found: Optional[Dict[str, Any]] = None
    for record in iter_jsonl_records(log_path):
        if event_matches(record, event=event, team_name=team_name, member_name=member_name, session_id=session_id, since=since):
            found = record
    return found


def find_any_log_event(log_path: Path, *, events: Iterable[str], team_name: Optional[str], member_name: Optional[str], session_id: Optional[str], since: datetime) -> Optional[Dict[str, Any]]:
    for name in events:
        record = find_log_event(log_path, event=name, team_name=team_name, member_name=member_name, session_id=session_id, since=since)
        if record is not None:
            return record
    return None


def read_lines_after(path: Path, since: datetime) -> List[str]:
    lines: List[str] = []
    if not path.exists():
        return lines
    with path.open("r", encoding="utf-8", errors="replace") as handle:
        for raw in handle:
            line = raw.rstrip("\n")
            if len(line) < 24:
                continue
            try:
                ts = parse_iso(line[:24])
            except Exception:
                continue
            if ts >= since:
                lines.append(line)
    return lines


def find_debug_line(path: Path, since: datetime, needle: str) -> Optional[str]:
    matched: Optional[str] = None
    for line in read_lines_after(path, since):
        if needle in line:
            matched = line
    return matched


def find_codex_boundary(jsonl_path: Path, since: datetime) -> Optional[Dict[str, Any]]:
    matched: Optional[Dict[str, Any]] = None
    for record in iter_jsonl_records(jsonl_path):
        timestamp = record.get("timestamp")
        if not isinstance(timestamp, str):
            continue
        try:
            event_ts = parse_iso(timestamp)
        except Exception:
            continue
        if event_ts < since:
            continue
        record_type = record.get("type")
        payload = record.get("payload")
        payload_type = payload.get("type") if isinstance(payload, dict) else None
        if record_type == "compacted" or payload_type == "context_compacted":
            matched = record
    return matched


# An independent child owns the inherited fd and enforces its lifetime even
# when the diagnostic injector process is killed. No team files are read here.
if __name__ == "__main__" and len(sys.argv) > 3 and sys.argv[1] == "--terminal-child":
    remaining = min(10.0, float(sys.argv[2]) - time.monotonic())
    if remaining <= 0:
        sys.exit(124)
    try:
        sys.exit(subprocess.run(sys.argv[3:], stdin=sys.stdin, timeout=remaining).returncode)
    except subprocess.TimeoutExpired:
        sys.exit(124)
