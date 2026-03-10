#!/usr/bin/env python3
"""analyze-compaction.py

One-shot compaction reinjection pipeline analyzer for taurhaus.

Primary source:
  - structured JSONL app logs (`taurhaus.log.jsonl` plus rotated siblings)

Current-state supplements:
  - `~/.claude/teams/*/runtime/*.json` for session_id population health
  - `~/.claude/settings.json` + `~/.claude/hooks/` for Claude compact hook installation

Typical usage:
  python3 scripts/analyze-compaction.py --team taurhaus-team --last 24h
  python3 scripts/analyze-compaction.py --since 2026-03-08T12:00:00Z
  just analyze-compaction --team taurhaus-team --last 6h

Output is a human-readable stdout report. This is intentionally a post-hoc
analysis tool, not a continuous monitor.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from statistics import median
from typing import Dict, Iterable, Iterator, List, Optional, Tuple


DEFAULT_TEAMS_DIR = Path.home() / ".claude" / "teams"
DEFAULT_CLAUDE_SETTINGS = Path.home() / ".claude" / "settings.json"
DEFAULT_HOOKS_DIR = Path.home() / ".claude" / "hooks"
COMPACTION_SIGNAL_FILENAME = "codex-compaction-signals.jsonl"
EXTRACTOR_STATE_FILENAME = "extractor-state.json"
WATCHER_STATE_FILENAME = "signal-watcher-state.json"
COMPACTION_EVENTS = {
    "compaction.detected",
    "compaction.injected",
    "compaction.skipped",
    "compaction.stale",
    "compaction.failed",
}
TERMINAL_COMPACTION_EVENTS = {
    "compaction.injected",
    "compaction.skipped",
    "compaction.stale",
    "compaction.failed",
}
HOOK_TEXT_PATTERNS = (
    "--claude-compact-hook",
    '"hookEventName":"SessionStart"',
    '"source":"compact"',
    "source=compact",
)


@dataclass(frozen=True)
class CompactionKey:
    team_name: str
    member_name: str
    tool: str
    session_id: str
    compaction_timestamp: str


@dataclass
class AnalyzerState:
    total_lines: int = 0
    parsed_lines: int = 0
    invalid_lines: int = 0
    compaction_events: List[dict] = None  # type: ignore[assignment]
    scanner_events: List[dict] = None  # type: ignore[assignment]
    hook_log_hits: int = 0
    hook_log_examples: List[str] = None  # type: ignore[assignment]

    def __post_init__(self) -> None:
        self.compaction_events = []
        self.scanner_events = []
        self.hook_log_examples = []


@dataclass(frozen=True)
class LogCandidate:
    path: Path
    source: str
    mtime: float
    size: int


@dataclass
class ProtocolTelemetryState:
    total_lines: int = 0
    parsed_lines: int = 0
    invalid_lines: int = 0
    wake_events: List[dict] = None  # type: ignore[assignment]
    surfaced_events: List[dict] = None  # type: ignore[assignment]
    files: List[Path] = None  # type: ignore[assignment]

    def __post_init__(self) -> None:
        self.wake_events = []
        self.surfaced_events = []
        self.files = []


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Analyze compaction reinjection pipeline health from taurhaus JSONL logs."
    )
    parser.add_argument(
        "--log",
        type=Path,
        default=None,
        help="Path to current taurhaus.log.jsonl (default: auto-detect latest active log root).",
    )
    parser.add_argument(
        "--team",
        help="Restrict report to one team name (default: all teams in logs/runtime files).",
    )
    parser.add_argument(
        "--since",
        help="Only include log events at or after this ISO timestamp.",
    )
    parser.add_argument(
        "--last",
        help="Only include log events within the last duration (e.g. 30m, 6h, 1d).",
    )
    parser.add_argument(
        "--teams-dir",
        type=Path,
        default=DEFAULT_TEAMS_DIR,
        help=f"Teams directory for current runtime snapshots (default: {DEFAULT_TEAMS_DIR})",
    )
    parser.add_argument(
        "--claude-settings",
        type=Path,
        default=DEFAULT_CLAUDE_SETTINGS,
        help=f"Claude settings.json path (default: {DEFAULT_CLAUDE_SETTINGS})",
    )
    parser.add_argument(
        "--hooks-dir",
        type=Path,
        default=DEFAULT_HOOKS_DIR,
        help=f"Claude hooks directory (default: {DEFAULT_HOOKS_DIR})",
    )
    return parser.parse_args()


def classify_log_source(path: Path) -> str:
    path_str = str(path)
    if path_str.startswith("/mnt/") and "AppData/Roaming/com.taurhaus.dev" in path_str:
        return "windows-roaming-via-wsl"
    if "Library/Application Support/com.taurhaus.dev" in path_str:
        return "macos-app-support"
    return "wsl-local"


def discover_default_log_candidates() -> List[LogCandidate]:
    override = os.environ.get("TAURHAUS_DATA_DIR")
    if override:
        path = Path(override) / "taurhaus.log.jsonl"
        try:
            stat = path.stat()
            return [LogCandidate(path=path, source="override", mtime=stat.st_mtime, size=stat.st_size)]
        except FileNotFoundError:
            return [LogCandidate(path=path, source="override", mtime=0.0, size=0)]

    candidates = [
        Path.home() / ".local" / "share" / "com.taurhaus.dev" / "taurhaus.log.jsonl",
        Path.home() / "Library" / "Application Support" / "com.taurhaus.dev" / "taurhaus.log.jsonl",
    ]

    windows_candidates = sorted(
        Path("/mnt/c/Users").glob("*/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl")
    )
    candidates.extend(windows_candidates)
    resolved: List[LogCandidate] = []
    for candidate in candidates:
        try:
            stat = candidate.stat()
        except FileNotFoundError:
            resolved.append(
                LogCandidate(
                    path=candidate,
                    source=classify_log_source(candidate),
                    mtime=0.0,
                    size=0,
                )
            )
            continue
        resolved.append(
            LogCandidate(
                path=candidate,
                source=classify_log_source(candidate),
                mtime=stat.st_mtime,
                size=stat.st_size,
            )
        )
    return resolved


def select_best_log_candidate(candidates: Iterable[LogCandidate]) -> Optional[LogCandidate]:
    existing: List[LogCandidate] = []
    for candidate in candidates:
        if candidate.mtime <= 0:
            continue
        existing.append(candidate)

    if not existing:
        return None

    existing.sort(key=lambda candidate: (candidate.mtime, candidate.size, str(candidate.path)), reverse=True)
    return existing[0]


def resolve_log_selection(explicit_log: Optional[Path]) -> Tuple[Path, List[LogCandidate], List[str]]:
    if explicit_log is not None:
        return explicit_log, [], []

    candidates = discover_default_log_candidates()
    selected = select_best_log_candidate(candidates)
    warnings: List[str] = []
    existing = [candidate for candidate in candidates if candidate.mtime > 0]
    sources = sorted({candidate.source for candidate in existing})
    if len(sources) > 1:
        rendered = ", ".join(
            f"{candidate.source}:{candidate.path}"
            for candidate in sorted(existing, key=lambda candidate: candidate.mtime, reverse=True)
        )
        warnings.append(
            "mixed log roots detected; auto-selection may pick a different active run than expected "
            f"({rendered})"
        )

    if selected is not None:
        return selected.path, candidates, warnings

    return candidates[0].path, candidates, warnings


def parse_iso_timestamp(value: str) -> datetime:
    normalized = value.replace("Z", "+00:00")
    parsed = datetime.fromisoformat(normalized)
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def parse_duration(value: str) -> timedelta:
    match = re.fullmatch(r"(?i)\s*(\d+)\s*([smhdw])\s*", value)
    if not match:
        raise ValueError(f"invalid duration '{value}' (expected 30m, 6h, 1d, 1w)")
    amount = int(match.group(1))
    unit = match.group(2).lower()
    seconds = {
        "s": 1,
        "m": 60,
        "h": 3600,
        "d": 86400,
        "w": 604800,
    }[unit]
    return timedelta(seconds=amount * seconds)


def resolve_window(args: argparse.Namespace) -> Tuple[Optional[datetime], Optional[datetime], str]:
    if args.since and args.last:
        raise SystemExit("Use only one of --since or --last.")
    if args.since:
        since = parse_iso_timestamp(args.since)
        return since, None, f"since {since.isoformat()}"
    if args.last:
        delta = parse_duration(args.last)
        since = datetime.now(timezone.utc) - delta
        return since, None, f"last {args.last}"
    return None, None, "entire available log set"


def discover_log_files(log_path: Path) -> List[Path]:
    if log_path.name == "taurhaus.log.jsonl" and log_path.parent.is_dir():
        rotated = sorted(
            log_path.parent.glob("taurhaus.log*.jsonl"),
            key=lambda path: (path.name == "taurhaus.log.jsonl", path.name),
        )
        return rotated
    return [log_path]


def iter_jsonl_records(files: Iterable[Path]) -> Iterator[Tuple[Path, int, str, dict]]:
    for path in files:
        if not path.exists():
            continue
        with path.open("r", encoding="utf-8", errors="replace") as handle:
            for line_no, raw in enumerate(handle, start=1):
                line = raw.strip()
                if not line:
                    continue
                try:
                    payload = json.loads(line)
                except json.JSONDecodeError:
                    yield path, line_no, line, {"__invalid_json__": True}
                    continue
                yield path, line_no, line, payload


def iter_telemetry_records(path: Path) -> Iterator[Tuple[int, str, dict]]:
    if not path.exists():
        return
    with path.open("r", encoding="utf-8", errors="replace") as handle:
        for line_no, raw in enumerate(handle, start=1):
            line = raw.strip()
            if not line:
                continue
            try:
                payload = json.loads(line)
            except json.JSONDecodeError:
                yield line_no, line, {"__invalid_json__": True}
                continue
            yield line_no, line, payload


def record_in_window(payload: dict, since: Optional[datetime]) -> bool:
    if since is None:
        return True
    ts_value = payload.get("ts")
    if not isinstance(ts_value, str):
        return False
    try:
        event_ts = parse_iso_timestamp(ts_value)
    except ValueError:
        return False
    return event_ts >= since


def telemetry_record_in_window(payload: dict, since: Optional[datetime]) -> bool:
    if since is None:
        return True
    ts_value = payload.get("timestamp")
    if not isinstance(ts_value, str):
        return False
    try:
        event_ts = parse_iso_timestamp(ts_value)
    except ValueError:
        return False
    return event_ts >= since


def compaction_key(payload: dict) -> Optional[CompactionKey]:
    team = payload.get("team_name")
    member = payload.get("member_name")
    tool = payload.get("tool")
    session_id = payload.get("session_id")
    compaction_timestamp = payload.get("compaction_timestamp")
    if not all(isinstance(value, str) for value in (team, member, tool, session_id, compaction_timestamp)):
        return None
    return CompactionKey(team, member, tool, session_id, compaction_timestamp)


def format_duration_ms(value: Optional[float]) -> str:
    if value is None:
        return "n/a"
    if value >= 1000:
        return f"{value / 1000:.2f}s"
    return f"{value:.0f}ms"


def status_label(level: str) -> str:
    return {
        "ok": "OK",
        "warn": "WARN",
        "fail": "FAIL",
        "unknown": "UNKNOWN",
    }[level]


def infer_cli_tool(cli_tool: object, model: object) -> Optional[str]:
    if isinstance(cli_tool, str) and cli_tool.strip():
        return cli_tool.strip().lower()
    if not isinstance(model, str) or not model.strip():
        return None
    lower = model.strip().lower()
    if "claude" in lower:
        return "claude"
    if "gpt" in lower or "codex" in lower:
        return "codex"
    if "gemini" in lower:
        return "gemini"
    return None


def load_team_member_tools(teams_dir: Path, team_filter: Optional[str]) -> Dict[Tuple[str, str], str]:
    tool_by_member: Dict[Tuple[str, str], str] = {}
    if not teams_dir.exists():
        return tool_by_member

    team_dirs = [teams_dir / team_filter] if team_filter else [p for p in teams_dir.iterdir() if p.is_dir()]
    for team_dir in team_dirs:
        config_path = team_dir / "config.json"
        if not config_path.exists():
            continue
        try:
            config = json.loads(config_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        team_name = config.get("name") or team_dir.name
        members = config.get("members")
        if not isinstance(members, list):
            continue
        for member in members:
            if not isinstance(member, dict):
                continue
            member_name = member.get("name")
            cli_tool = infer_cli_tool(member.get("cliTool"), member.get("model"))
            if isinstance(member_name, str) and cli_tool:
                tool_by_member[(team_name, member_name)] = cli_tool
    return tool_by_member


def iter_team_dirs(teams_dir: Path, team_filter: Optional[str]) -> List[Path]:
    if not teams_dir.exists():
        return []
    if team_filter:
        candidate = teams_dir / team_filter
        return [candidate] if candidate.is_dir() else []
    return sorted((path for path in teams_dir.iterdir() if path.is_dir()), key=lambda path: path.name)


def load_json_file(path: Path) -> Optional[dict]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None


def inspect_signal_log_file(
    signal_path: Path, last_consumed_offset: int, recent_limit: int
) -> Dict[str, object]:
    summary: Dict[str, object] = {
        "path": signal_path,
        "exists": signal_path.exists(),
        "file_size_bytes": 0,
        "total_signals": 0,
        "unconsumed_count": 0,
        "recent_signals": [],
    }
    if not signal_path.exists():
        return summary

    try:
        summary["file_size_bytes"] = signal_path.stat().st_size
    except OSError:
        return summary

    recent_signals: List[dict] = []
    offset = 0
    try:
        with signal_path.open("rb") as handle:
            while True:
                line = handle.readline()
                if not line:
                    break
                line_start = offset
                offset += len(line)
                if not line.endswith(b"\n"):
                    break
                stripped = line.strip()
                if not stripped:
                    continue
                try:
                    payload = json.loads(stripped.decode("utf-8"))
                except (UnicodeDecodeError, json.JSONDecodeError):
                    continue
                summary["total_signals"] += 1
                if line_start >= last_consumed_offset:
                    summary["unconsumed_count"] += 1
                recent_signals.append(payload)
                if len(recent_signals) > recent_limit:
                    recent_signals.pop(0)
    except OSError:
        return summary

    summary["recent_signals"] = recent_signals
    return summary


def analyze_compaction_diagnostics(
    teams_dir: Path, team_filter: Optional[str], recent_limit: int = 5
) -> Dict[str, Dict[str, object]]:
    diagnostics: Dict[str, Dict[str, object]] = {}

    for team_dir in iter_team_dirs(teams_dir, team_filter):
        compaction_dir = team_dir / "state" / "compaction"
        extractor_payload = load_json_file(compaction_dir / EXTRACTOR_STATE_FILENAME) or {}
        watcher_payload = load_json_file(compaction_dir / WATCHER_STATE_FILENAME) or {}
        file_offsets = extractor_payload.get("file_offsets")
        error_map = extractor_payload.get("last_error_by_file")
        if not isinstance(file_offsets, dict):
            file_offsets = {}
        if not isinstance(error_map, dict):
            error_map = {}

        active_files = []
        for jsonl_path, checkpoint in sorted(file_offsets.items()):
            offset_value = checkpoint.get("offset") if isinstance(checkpoint, dict) else 0
            active_files.append(
                {
                    "jsonl_path": str(jsonl_path),
                    "offset": int(offset_value or 0),
                    "last_error": error_map.get(jsonl_path),
                }
            )

        last_consumed_offset = int(watcher_payload.get("last_consumed_offset") or 0)
        signal_summary = inspect_signal_log_file(
            compaction_dir / "signals" / COMPACTION_SIGNAL_FILENAME,
            last_consumed_offset,
            recent_limit,
        )

        diagnostics[team_dir.name] = {
            "extractor": {
                "heartbeat_at": extractor_payload.get("heartbeat_at"),
                "last_processed_signal": extractor_payload.get("last_processed_signal"),
                "active_files": active_files,
            },
            "watcher": {
                "last_consumed_offset": last_consumed_offset,
                "last_event_at": watcher_payload.get("last_event_at"),
                "last_reconciliation_at": watcher_payload.get("last_reconciliation_at"),
                "reconciliation_poll_count": int(watcher_payload.get("reconciliation_poll_count") or 0),
                "missed_event_recovery_count": int(
                    watcher_payload.get("missed_event_recovery_count") or 0
                ),
            },
            "signal_log": signal_summary,
        }

    return diagnostics


def analyze_protocol_telemetry(
    teams_dir: Path,
    team_filter: Optional[str],
    since: Optional[datetime],
) -> ProtocolTelemetryState:
    state = ProtocolTelemetryState()

    for team_dir in iter_team_dirs(teams_dir, team_filter):
        telemetry_path = team_dir / "state" / "protocol_telemetry.jsonl"
        if not telemetry_path.exists():
            continue
        state.files.append(telemetry_path)
        for line_no, _, payload in iter_telemetry_records(telemetry_path):
            state.total_lines += 1
            if payload.get("__invalid_json__"):
                state.invalid_lines += 1
                continue
            state.parsed_lines += 1
            if not telemetry_record_in_window(payload, since):
                continue
            metric = payload.get("metric")
            if metric == "wake_delivery":
                state.wake_events.append(payload)
            elif metric == "compaction_read_surfaced":
                state.surfaced_events.append(payload)

    return state


def analyze_runtime_session_health(
    teams_dir: Path, team_filter: Optional[str]
) -> Tuple[Counter, Dict[str, List[str]], List[Tuple[str, str, Optional[str]]]]:
    totals = Counter()
    missing_by_team: Dict[str, List[str]] = defaultdict(list)
    details: List[Tuple[str, str, Optional[str]]] = []

    if not teams_dir.exists():
        return totals, missing_by_team, details

    team_dirs = [teams_dir / team_filter] if team_filter else [p for p in teams_dir.iterdir() if p.is_dir()]
    tool_map = load_team_member_tools(teams_dir, team_filter)

    for team_dir in team_dirs:
        runtime_dir = team_dir / "runtime"
        if not runtime_dir.is_dir():
            continue
        team_name = team_dir.name
        for runtime_path in sorted(runtime_dir.glob("*.json")):
            try:
                payload = json.loads(runtime_path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError):
                totals["runtime_invalid"] += 1
                continue
            member_name = payload.get("member_name") or runtime_path.stem
            session_id = payload.get("session_id")
            tool = tool_map.get((team_name, str(member_name)))
            totals["runtime_members"] += 1
            if session_id:
                totals["runtime_with_session_id"] += 1
            else:
                totals["runtime_missing_session_id"] += 1
                missing_by_team[team_name].append(str(member_name))
            if tool:
                totals[f"tool::{tool}::members"] += 1
                if session_id:
                    totals[f"tool::{tool}::with_session_id"] += 1
            details.append((team_name, str(member_name), tool))

    for members in missing_by_team.values():
        members.sort()
    return totals, missing_by_team, details


def analyze_hook_installation(settings_path: Path, hooks_dir: Path) -> Dict[str, object]:
    status: Dict[str, object] = {
        "settings_exists": settings_path.exists(),
        "installed": False,
        "matcher_found": False,
        "command": None,
        "script_exists": False,
    }
    if not settings_path.exists():
        return status

    try:
        settings = json.loads(settings_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        status["parse_error"] = True
        return status

    session_start = (
        settings.get("hooks", {})
        .get("SessionStart", [])
        if isinstance(settings, dict)
        else []
    )
    if not isinstance(session_start, list):
        return status

    for entry in session_start:
        if not isinstance(entry, dict):
            continue
        if entry.get("matcher") != "compact":
            continue
        status["matcher_found"] = True
        hooks = entry.get("hooks")
        if not isinstance(hooks, list):
            continue
        for hook in hooks:
            if not isinstance(hook, dict):
                continue
            command = hook.get("command")
            if isinstance(command, str) and "taurhaus-session-start-compact" in command:
                status["installed"] = True
                status["command"] = command
                break
        if status["installed"]:
            break

    scripts = list(hooks_dir.glob("taurhaus-session-start-compact.*"))
    status["script_exists"] = any(script.is_file() for script in scripts)
    status["scripts"] = [str(script) for script in scripts]
    return status


def line_matches_hook_signal(raw_line: str, payload: dict) -> bool:
    if any(pattern in raw_line for pattern in HOOK_TEXT_PATTERNS):
        return True
    event = payload.get("event")
    message = payload.get("message")
    if isinstance(event, str) and "hook" in event.lower():
        return True
    if isinstance(message, str) and "compact hook" in message.lower():
        return True
    return False


def print_section(title: str) -> None:
    print(f"\n== {title} ==")


def print_kv(key: str, value: object) -> None:
    print(f"{key}: {value}")


def checkpoint_status(
    level: str,
    checkpoint_id: str,
    name: str,
    verification: str,
    working: str,
    broken: str,
) -> None:
    print(f"[{status_label(level)}] {checkpoint_id} {name}")
    print(f"  verify: {verification}")
    print(f"  working: {working}")
    print(f"  broken: {broken}")


def print_compaction_diagnostics(diagnostics_by_team: Dict[str, Dict[str, object]]) -> None:
    print_section("Compaction Diagnostics")
    if not diagnostics_by_team:
        print("No team compaction state directories found.")
        return

    for team_name, diagnostics in sorted(diagnostics_by_team.items()):
        extractor = diagnostics.get("extractor", {})
        watcher = diagnostics.get("watcher", {})
        signal_log = diagnostics.get("signal_log", {})
        active_files = extractor.get("active_files", []) if isinstance(extractor, dict) else []
        recent_signals = signal_log.get("recent_signals", []) if isinstance(signal_log, dict) else []

        print(f"{team_name}:")
        print(
            f"  extractor heartbeat={extractor.get('heartbeat_at') or 'n/a'} "
            f"active_files={len(active_files)}"
        )
        last_processed = extractor.get("last_processed_signal")
        if isinstance(last_processed, dict):
            print(
                f"  last processed: id={last_processed.get('signal_id') or 'n/a'} "
                f"offset={last_processed.get('jsonl_offset') or 'n/a'}"
            )
        for file_entry in active_files[:5]:
            if not isinstance(file_entry, dict):
                continue
            error_suffix = ""
            if file_entry.get("last_error"):
                error_suffix = f" error={file_entry['last_error']}"
            print(
                f"  file: {file_entry.get('jsonl_path')} "
                f"offset={file_entry.get('offset', 0)}{error_suffix}"
            )

        print(
            f"  watcher last_event={watcher.get('last_event_at') or 'n/a'} "
            f"last_reconcile={watcher.get('last_reconciliation_at') or 'n/a'} "
            f"polls={watcher.get('reconciliation_poll_count', 0)} "
            f"recovered={watcher.get('missed_event_recovery_count', 0)}"
        )
        print(
            f"  signal log path={signal_log.get('path')} total={signal_log.get('total_signals', 0)} "
            f"unconsumed={signal_log.get('unconsumed_count', 0)} "
            f"offset={watcher.get('last_consumed_offset', 0)}"
        )
        for signal in recent_signals:
            if not isinstance(signal, dict):
                continue
            signal_kind = signal.get("signal_kind")
            print(
                f"  recent signal: {signal.get('signal_id')} "
                f"{signal_kind or 'unknown'} "
                f"session={signal.get('session_id')} "
                f"at={signal.get('emitted_at')}"
            )


def print_protocol_telemetry(protocol_state: ProtocolTelemetryState) -> None:
    print_section("Mesh Protocol Telemetry")
    if not protocol_state.files:
        print("No protocol telemetry journals found.")
        return

    print_kv("Telemetry files", ", ".join(str(path) for path in protocol_state.files))
    print_kv("Parsed telemetry lines", f"{protocol_state.parsed_lines}/{protocol_state.total_lines}")
    if protocol_state.invalid_lines:
        print_kv("Invalid telemetry lines", protocol_state.invalid_lines)

    wake_by_stage = Counter(
        str(event.get("stage", "unknown")) for event in protocol_state.wake_events
    )
    print_kv("Wake events", len(protocol_state.wake_events))
    if wake_by_stage:
        print_kv(
            "Wake stages",
            ", ".join(f"{stage}={count}" for stage, count in sorted(wake_by_stage.items())),
        )

    print_kv("Compaction read surfaced", len(protocol_state.surfaced_events))
    if protocol_state.surfaced_events:
        recent = protocol_state.surfaced_events[-3:]
        for payload in recent:
            print(
                f"  surfaced: {payload.get('team_name')}/{payload.get('member_name')} "
                f"messages={payload.get('message_count')} ids={payload.get('message_ids')}"
            )


def main() -> int:
    args = parse_args()
    since, _, window_desc = resolve_window(args)
    resolved_log_path, log_candidates, log_warnings = resolve_log_selection(args.log)
    log_files = discover_log_files(resolved_log_path)
    existing_files = [path for path in log_files if path.exists()]

    if not existing_files:
        print(f"FAIL: no log files found for {resolved_log_path}", file=sys.stderr)
        return 1

    state = AnalyzerState()
    protocol_state = analyze_protocol_telemetry(args.teams_dir, args.team, since)
    detected_at: Dict[CompactionKey, datetime] = {}
    terminal_latencies_ms: List[float] = []
    injected_latencies_ms: List[float] = []
    compaction_by_tool = Counter()
    compaction_by_member = Counter()
    outcome_by_tool = Counter()
    outcome_by_member = Counter()
    delivery_reason_presence = Counter()
    scanner_counts_by_run: Dict[str, List[int]] = defaultdict(list)
    latest_scanner_run_id: Optional[str] = None
    latest_scanner_run_ts: Optional[datetime] = None

    for path, line_no, raw_line, payload in iter_jsonl_records(existing_files):
        state.total_lines += 1
        if payload.get("__invalid_json__"):
            state.invalid_lines += 1
            continue
        state.parsed_lines += 1

        if not record_in_window(payload, since):
            continue

        if args.team and payload.get("team_name") not in (None, args.team):
            event = payload.get("event")
            if event in COMPACTION_EVENTS:
                continue

        if line_matches_hook_signal(raw_line, payload):
            state.hook_log_hits += 1
            if len(state.hook_log_examples) < 3:
                state.hook_log_examples.append(f"{path.name}:{line_no}")

        event = payload.get("event")
        if event == "session_scanner.scan.completed":
            state.scanner_events.append(payload)
            run_id = payload.get("run_id")
            if isinstance(run_id, str):
                scanner_counts_by_run[run_id].append(int(payload.get("session_count", 0)))
                ts_value = payload.get("ts")
                if isinstance(ts_value, str):
                    try:
                        event_ts = parse_iso_timestamp(ts_value)
                    except ValueError:
                        event_ts = None
                    if event_ts and (latest_scanner_run_ts is None or event_ts >= latest_scanner_run_ts):
                        latest_scanner_run_ts = event_ts
                        latest_scanner_run_id = run_id
            continue

        if event not in COMPACTION_EVENTS:
            continue
        if args.team and payload.get("team_name") != args.team:
            continue

        state.compaction_events.append(payload)
        tool = str(payload.get("tool", "unknown"))
        member = f"{payload.get('team_name', '?')}/{payload.get('member_name', '?')}"
        compaction_by_tool[(tool, event)] += 1
        compaction_by_member[(member, event)] += 1

        key = compaction_key(payload)
        ts_value = payload.get("ts")
        event_ts = parse_iso_timestamp(ts_value) if isinstance(ts_value, str) else None

        if event == "compaction.detected" and key and event_ts:
            detected_at.setdefault(key, event_ts)
        if event in TERMINAL_COMPACTION_EVENTS:
            outcome_by_tool[(tool, event)] += 1
            outcome_by_member[(member, event)] += 1
            if key and event_ts:
                detect_ts = detected_at.get(key)
                if detect_ts is not None:
                    terminal_latencies_ms.append((event_ts - detect_ts).total_seconds() * 1000)
                if event == "compaction.injected":
                    if detect_ts is not None:
                        injected_latencies_ms.append((event_ts - detect_ts).total_seconds() * 1000)

        reason = payload.get("reason") or payload.get("error") or payload.get("delivery_reason")
        if reason:
            delivery_reason_presence["with_reason"] += 1
        elif event in TERMINAL_COMPACTION_EVENTS:
            delivery_reason_presence["without_reason"] += 1

    runtime_totals, missing_by_team, runtime_details = analyze_runtime_session_health(
        args.teams_dir, args.team
    )
    compaction_diagnostics = analyze_compaction_diagnostics(args.teams_dir, args.team)
    hook_status = analyze_hook_installation(args.claude_settings, args.hooks_dir)
    wake_stage_counts = Counter(str(event.get("stage", "unknown")) for event in protocol_state.wake_events)
    surfaced_count = len(protocol_state.surfaced_events)

    detected_count = sum(1 for event in state.compaction_events if event.get("event") == "compaction.detected")
    injected_count = sum(1 for event in state.compaction_events if event.get("event") == "compaction.injected")
    skipped_count = sum(1 for event in state.compaction_events if event.get("event") == "compaction.skipped")
    stale_count = sum(1 for event in state.compaction_events if event.get("event") == "compaction.stale")
    failed_count = sum(1 for event in state.compaction_events if event.get("event") == "compaction.failed")

    scanner_counts = [int(event.get("session_count", 0)) for event in state.scanner_events]
    scanner_zero = sum(1 for count in scanner_counts if count == 0)
    scanner_positive = sum(1 for count in scanner_counts if count > 0)
    latest_scanner_counts = (
        list(scanner_counts_by_run.get(latest_scanner_run_id, [])) if latest_scanner_run_id else []
    )
    latest_scanner_zero = sum(1 for count in latest_scanner_counts if count == 0)

    if not state.compaction_events:
        compaction_health = ("warn", "no compaction events found in selected window")
    elif detected_count > 0 and injected_count == 0:
        compaction_health = ("warn", "compactions detected but none reached transport delivery")
    elif failed_count > 0:
        compaction_health = ("warn", f"{failed_count} failed compaction deliveries observed")
    else:
        compaction_health = ("ok", "compaction events reached terminal transport outcomes in window")

    if injected_count == 0:
        consumption_health = ("unknown", "no transport-delivered compactions in selected window")
    elif surfaced_count > 0:
        consumption_health = (
            "ok",
            f"{surfaced_count} compaction card surfacing event(s) captured by mesh read telemetry",
        )
    else:
        consumption_health = (
            "warn",
            "transport delivery exists, but no mesh-read surfacing evidence is present in the selected window",
        )

    if not latest_scanner_counts:
        scanner_health = ("unknown", "no session scanner events in selected window")
    elif max(latest_scanner_counts) == 0:
        scanner_health = (
            "fail",
            f"latest run {latest_scanner_run_id} reported session_count=0 for all {len(latest_scanner_counts)} cycles",
        )
    elif latest_scanner_zero > 0:
        scanner_health = (
            "warn",
            f"latest run {latest_scanner_run_id} had {latest_scanner_zero}/{len(latest_scanner_counts)} zero-session cycles",
        )
    else:
        scanner_health = (
            "ok",
            f"latest run {latest_scanner_run_id} reported session_count>0 for all {len(latest_scanner_counts)} cycles",
        )

    runtime_members = runtime_totals["runtime_members"]
    runtime_with_session_id = runtime_totals["runtime_with_session_id"]
    if runtime_members == 0:
        runtime_health = ("unknown", "no runtime member files found")
    elif runtime_with_session_id == runtime_members:
        runtime_health = ("ok", f"all {runtime_members} runtime members have session_id")
    elif runtime_with_session_id == 0:
        runtime_health = ("fail", f"0/{runtime_members} runtime members have session_id")
    else:
        runtime_health = (
            "warn",
            f"{runtime_with_session_id}/{runtime_members} runtime members have session_id",
        )

    claude_injected = sum(
        1
        for event in state.compaction_events
        if event.get("event") == "compaction.injected" and event.get("tool") == "claude"
    )
    if hook_status.get("installed"):
        if state.hook_log_hits > 0:
            hook_health = ("ok", "Claude compact hook installed and hook-related log evidence found")
        elif claude_injected > 0:
            hook_health = ("ok", "Claude compact hook installed; hook fire inferred from Claude injected outcomes")
        else:
            hook_health = ("unknown", "Claude compact hook installed, but no hook fire evidence in selected window")
    else:
        hook_health = ("fail", "Claude compact hook is not installed in current settings")

    print("Compaction Reinjection Analysis")
    print("==============================")
    print_kv("Window", window_desc)
    print_kv("Team filter", args.team or "all teams")
    print_kv("Selected log", resolved_log_path)
    if log_candidates:
        print_kv(
            "Log candidates",
            ", ".join(
                f"{candidate.source}:{candidate.path}"
                for candidate in sorted(log_candidates, key=lambda candidate: (candidate.mtime, candidate.size), reverse=True)
                if candidate.mtime > 0
            ) or "none found",
        )
    for warning in log_warnings:
        print_kv("Log selection warning", warning)
    print_kv("Log files", ", ".join(path.name for path in existing_files))
    print_kv("Parsed lines", f"{state.parsed_lines}/{state.total_lines}")
    if state.invalid_lines:
        print_kv("Invalid JSON lines", state.invalid_lines)

    print_section("Health Signals")
    for name, (level, message) in (
        ("Compaction pipeline", compaction_health),
        ("Agent consumption evidence", consumption_health),
        ("Scanner health", scanner_health),
        ("Runtime session_id health", runtime_health),
        ("Claude hook status", hook_health),
    ):
        print(f"[{status_label(level)}] {name}: {message}")

    print_section("Checkpoint Matrix")
    cp1_level = "ok" if detected_count > 0 else "unknown"
    cp1_working = (
        "a downstream compaction.detected event exists in the selected window, so a transcript boundary definitely existed"
        if detected_count > 0
        else "manual transcript inspection finds compact/context_compacted records"
    )
    cp1_broken = "no compact/context_compacted record found in the target JSONL when inspected directly"
    checkpoint_status(
        cp1_level,
        "CP1",
        "Codex JSONL contains compaction boundary",
        "grep -R 'context_compacted\\|\"type\":\"compacted\"' ~/.codex/sessions | tail",
        cp1_working,
        cp1_broken,
    )

    cp2_level = scanner_health[0]
    checkpoint_status(
        cp2_level,
        "CP2",
        "Session scanner ran scan cycles",
        "grep 'session_scanner.scan.completed' <taurhaus.log.jsonl>",
        "session_scanner.scan.completed events exist and the latest run reports session_count > 0",
        "no scan.completed events, or the latest run reports session_count=0 for every cycle",
    )

    cp3_level = "ok" if detected_count > 0 else "warn"
    checkpoint_status(
        cp3_level,
        "CP3",
        "Compaction record was parsed and emitted",
        "grep 'compaction.detected' <taurhaus.log.jsonl>",
        "compaction.detected exists for the target team/member/session",
        "transcript boundary exists but no compaction.detected event is emitted",
    )

    cp4_level = "ok" if detected_count > 0 else "warn"
    checkpoint_status(
        cp4_level,
        "CP4",
        "Managed member resolution succeeded",
        "grep 'compaction.detected' <taurhaus.log.jsonl> and inspect team_name/member_name/session_id fields",
        "compaction.detected is present; current code only emits it after managed-member resolution succeeds",
        "scanner sees a boundary but no compaction.detected event appears for a managed member",
    )

    cp5_level = "ok" if injected_count or skipped_count or stale_count or failed_count else "warn"
    checkpoint_status(
        cp5_level,
        "CP5",
        "Delivery reached a terminal transport outcome",
        "grep 'compaction.injected\\|compaction.skipped\\|compaction.stale\\|compaction.failed' <taurhaus.log.jsonl>",
        "one of injected/skipped/stale/failed exists for the detected compaction",
        "compaction.detected exists but no terminal delivery event follows",
    )

    if injected_count == 0:
        cp6_level = "unknown"
    elif wake_stage_counts["tmux_injected"] > 0:
        cp6_level = "ok"
    elif wake_stage_counts["tmux_failed"] > 0:
        cp6_level = "fail"
    elif protocol_state.files:
        cp6_level = "warn"
    else:
        cp6_level = "unknown"
    checkpoint_status(
        cp6_level,
        "CP6",
        "Mesh wake prompt transport happened",
        "python3 scripts/analyze-compaction.py --team <team> --last 30m",
        "wake_delivery telemetry shows tmux_injected for the member after transport delivery",
        "transport delivery exists, but there is no tmux_injected evidence or there are tmux_failed events instead",
    )

    cp7_level = consumption_health[0]
    checkpoint_status(
        cp7_level,
        "CP7",
        "Compaction card was surfaced by mesh read",
        "python3 scripts/analyze-compaction.py --team <team> --last 30m",
        "compaction_read_surfaced telemetry exists for the target member",
        "transport delivery exists, but no surfaced telemetry has been recorded",
    )

    cp8_level = runtime_health[0]
    checkpoint_status(
        cp8_level,
        "CP8",
        "Runtime member records can support exact session resolution",
        "python3 scripts/analyze-compaction.py --team <team> --last 30m",
        "runtime session_id health shows managed members with populated session_id values",
        "runtime session_id health is partial or empty, causing ambiguous or skipped resolution",
    )

    cp9_level = hook_health[0]
    checkpoint_status(
        cp9_level,
        "CP9",
        "Claude compact hook bridge is installed",
        "python3 scripts/analyze-compaction.py --team <team> --last 30m",
        "Claude hook status is installed and optionally shows hook fire evidence",
        "hook not installed, matcher missing, script missing, or no evidence in a window where Claude compactions should have fired",
    )

    print_section("Compaction Outcomes")
    print_kv("Detected", detected_count)
    print_kv("Transport delivered (compaction.injected)", injected_count)
    print_kv("Skipped", skipped_count)
    print_kv("Stale", stale_count)
    print_kv("Failed", failed_count)
    print_kv("Wake prompt observed", wake_stage_counts["observed"])
    print_kv("Wake prompt suppressed", wake_stage_counts["suppressed"])
    print_kv("Wake prompt tmux injected", wake_stage_counts["tmux_injected"])
    print_kv("Wake prompt tmux failed", wake_stage_counts["tmux_failed"])
    print_kv("Compaction cards surfaced", surfaced_count)
    success_rate = (injected_count / detected_count * 100.0) if detected_count else None
    print_kv("Transport delivered / detected ratio", f"{success_rate:.1f}%" if success_rate is not None else "n/a")
    surfaced_rate = (surfaced_count / injected_count * 100.0) if injected_count else None
    print_kv(
        "Surfaced / transport-delivered ratio",
        f"{surfaced_rate:.1f}%" if surfaced_rate is not None else "n/a",
    )
    print_kv(
        "Consumption semantics",
        "compaction.injected proves transport delivery only; agent consumption is evidenced separately by compaction_read_surfaced",
    )
    if delivery_reason_presence["without_reason"]:
        print_kv(
            "Delivery reasons",
            "Structured compaction events do not currently include skip/fail reason fields",
        )

    print_section("Latency")
    if terminal_latencies_ms:
        print_kv("Detected -> terminal samples", len(terminal_latencies_ms))
        print_kv("Detected -> terminal min", format_duration_ms(min(terminal_latencies_ms)))
        print_kv("Detected -> terminal median", format_duration_ms(median(terminal_latencies_ms)))
        print_kv("Detected -> terminal max", format_duration_ms(max(terminal_latencies_ms)))
    else:
        print_kv("Detected -> terminal samples", 0)
        print_kv("Detected -> terminal latency", "n/a (no detected->terminal pairs in selected window)")

    if injected_latencies_ms:
        print_kv("Detected -> injected samples", len(injected_latencies_ms))
        print_kv("Detected -> injected min", format_duration_ms(min(injected_latencies_ms)))
        print_kv("Detected -> injected median", format_duration_ms(median(injected_latencies_ms)))
        print_kv("Detected -> injected max", format_duration_ms(max(injected_latencies_ms)))
    else:
        print_kv("Detected -> injected samples", 0)
        print_kv("Detected -> injected latency", "n/a (no detected->injected pairs in selected window)")

    print_section("Per Tool")
    tools = sorted({tool for tool, _ in compaction_by_tool.keys()} | {tool for tool, _ in outcome_by_tool.keys()})
    if not tools:
        print("No compaction events for any tool in selected window.")
    else:
        for tool in tools:
            print(
                f"{tool}: detected={compaction_by_tool[(tool, 'compaction.detected')]} "
                f"transport_delivered={outcome_by_tool[(tool, 'compaction.injected')]} "
                f"skipped={outcome_by_tool[(tool, 'compaction.skipped')]} "
                f"stale={outcome_by_tool[(tool, 'compaction.stale')]} "
                f"failed={outcome_by_tool[(tool, 'compaction.failed')]}"
            )

    print_section("Per Member")
    members = sorted({member for member, _ in compaction_by_member.keys()} | {member for member, _ in outcome_by_member.keys()})
    if not members:
        print("No member-level compaction events in selected window.")
    else:
        for member in members:
            print(
                f"{member}: detected={compaction_by_member[(member, 'compaction.detected')]} "
                f"transport_delivered={outcome_by_member[(member, 'compaction.injected')]} "
                f"skipped={outcome_by_member[(member, 'compaction.skipped')]} "
                f"stale={outcome_by_member[(member, 'compaction.stale')]} "
                f"failed={outcome_by_member[(member, 'compaction.failed')]}"
            )

    print_section("Scanner Health")
    print_kv("Scanner cycles", len(scanner_counts))
    print_kv("Scanner runs", len(scanner_counts_by_run))
    if latest_scanner_run_id:
        print_kv("Latest scanner run_id", latest_scanner_run_id)
        print_kv("Latest run cycles", len(latest_scanner_counts))
        if latest_scanner_counts:
            print_kv("Latest run zero-session cycles", latest_scanner_zero)
            print_kv("Latest run min session_count", min(latest_scanner_counts))
            print_kv("Latest run max session_count", max(latest_scanner_counts))
            print_kv("Latest run last session_count", latest_scanner_counts[-1])
    if scanner_counts:
        print_kv("Zero-session cycles", scanner_zero)
        print_kv("Positive-session cycles", scanner_positive)
        print_kv("Min session_count", min(scanner_counts))
        print_kv("Max session_count", max(scanner_counts))
        print_kv("Last session_count", scanner_counts[-1])

    print_section("Runtime Session IDs")
    print_kv("Runtime members", runtime_members)
    print_kv("Members with session_id", runtime_with_session_id)
    for team_name in sorted(missing_by_team):
        missing = ", ".join(missing_by_team[team_name])
        print(f"{team_name}: missing session_id -> {missing}")

    if runtime_details:
        tool_map = load_team_member_tools(args.teams_dir, args.team)
        tool_totals = Counter()
        for team_name, member_name, tool in runtime_details:
            if not tool:
                continue
            tool_totals[f"{tool}::members"] += 1
            runtime_path = args.teams_dir / team_name / "runtime" / f"{member_name}.json"
            try:
                payload = json.loads(runtime_path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError):
                continue
            if payload.get("session_id"):
                tool_totals[f"{tool}::with_session_id"] += 1
        for tool in sorted({key.split("::")[0] for key in tool_totals if key.endswith("::members")}):
            members_total = tool_totals[f"{tool}::members"]
            with_sid = tool_totals[f"{tool}::with_session_id"]
            print(f"{tool}: {with_sid}/{members_total} runtime members with session_id")

    print_compaction_diagnostics(compaction_diagnostics)
    print_protocol_telemetry(protocol_state)

    print_section("Claude Hook")
    print_kv("Settings file", args.claude_settings)
    print_kv("Hook installed", hook_status.get("installed"))
    print_kv("Compact matcher present", hook_status.get("matcher_found"))
    print_kv("Hook script exists", hook_status.get("script_exists"))
    if hook_status.get("command"):
        print_kv("Configured command", hook_status["command"])
    if state.hook_log_hits:
        print_kv("Hook-related log hits", state.hook_log_hits)
        print_kv("Hook log examples", ", ".join(state.hook_log_examples))
    elif claude_injected:
        print_kv("Hook fire evidence", f"inferred from {claude_injected} Claude injected outcomes")
    else:
        print_kv("Hook fire evidence", "none in selected window")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
