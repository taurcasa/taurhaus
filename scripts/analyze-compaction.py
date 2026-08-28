#!/usr/bin/env python3
"""Summarize native compaction-hook activity from taurhaus JSONL logs."""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from collections import Counter
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Iterable, Iterator, Optional


DEFAULT_TEAMS_DIR = Path.home() / ".claude" / "teams"
HOOK_EVENT_PREFIXES = (
    "compaction.claude_hook.",
    "compaction.codex_hook.",
    "compaction.grok_hook.",
    "compaction.compact_hook.",
)
TERMINAL_ACTIONS = {"delivered", "skipped", "failed"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Analyze native compaction-hook events from taurhaus JSONL logs."
    )
    parser.add_argument("--log", type=Path, help="Current taurhaus.log.jsonl path.")
    parser.add_argument("--team", help="Restrict events to one team.")
    parser.add_argument("--member", help="Restrict events to one member.")
    window = parser.add_mutually_exclusive_group()
    window.add_argument("--since", help="ISO timestamp at which the report starts.")
    window.add_argument("--last", help="Relative window such as 30m, 6h, or 1d.")
    parser.add_argument("--teams-dir", type=Path, default=DEFAULT_TEAMS_DIR)
    parser.add_argument(
        "--manual-run-id",
        help="Use one scripted run's team, member, and trigger timestamp as filters.",
    )
    parser.add_argument("--limit", type=int, default=20, help="Recent events to print.")
    return parser.parse_args()


def parse_timestamp(value: str) -> datetime:
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def parse_duration(value: str) -> timedelta:
    match = re.fullmatch(r"\s*(\d+)\s*([smhdw])\s*", value, re.IGNORECASE)
    if match is None:
        raise ValueError(f"invalid duration '{value}' (expected 30m, 6h, 1d, or 1w)")
    seconds = {"s": 1, "m": 60, "h": 3600, "d": 86400, "w": 604800}
    return timedelta(seconds=int(match.group(1)) * seconds[match.group(2).lower()])


def default_log_candidates() -> list[Path]:
    override = os.environ.get("TAURHAUS_DATA_DIR")
    if override:
        return [Path(override) / "taurhaus.log.jsonl"]
    candidates = [
        Path.home() / ".local/share/com.taurhaus.dev/taurhaus.log.jsonl",
        Path.home() / "Library/Application Support/com.taurhaus.dev/taurhaus.log.jsonl",
    ]
    candidates.extend(
        sorted(Path("/mnt/c/Users").glob("*/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl"))
    )
    return candidates


def resolve_log_path(explicit: Optional[Path]) -> Path:
    if explicit is not None:
        return explicit
    existing = []
    for path in default_log_candidates():
        try:
            stat = path.stat()
        except OSError:
            continue
        existing.append((stat.st_mtime, stat.st_size, str(path), path))
    if not existing:
        raise ValueError("no taurhaus.log.jsonl found; pass --log explicitly")
    return max(existing)[3]


def rotated_log_paths(log_path: Path) -> list[Path]:
    if log_path.name != "taurhaus.log.jsonl":
        return [log_path]
    paths = list(log_path.parent.glob("taurhaus.log*.jsonl"))
    return sorted(paths, key=lambda path: (path.name == log_path.name, path.name))


def iter_records(paths: Iterable[Path]) -> Iterator[dict[str, Any]]:
    for path in paths:
        try:
            handle = path.open("r", encoding="utf-8", errors="replace")
        except OSError:
            continue
        with handle:
            for raw in handle:
                try:
                    record = json.loads(raw)
                except json.JSONDecodeError:
                    continue
                if isinstance(record, dict):
                    yield record


def is_hook_event(record: dict[str, Any]) -> bool:
    event = record.get("event")
    return isinstance(event, str) and event.startswith(HOOK_EVENT_PREFIXES)


def event_timestamp(record: dict[str, Any]) -> Optional[datetime]:
    value = record.get("ts")
    if not isinstance(value, str):
        return None
    try:
        return parse_timestamp(value)
    except ValueError:
        return None


def event_action(record: dict[str, Any]) -> str:
    event = str(record.get("event") or "")
    return event.rsplit(".", 1)[-1]


def event_tool(record: dict[str, Any]) -> str:
    tool = record.get("tool")
    if isinstance(tool, str) and tool:
        return tool
    event = str(record.get("event") or "")
    for candidate in ("claude", "codex", "grok"):
        if event.startswith(f"compaction.{candidate}_hook."):
            return candidate
    return "unknown"


def load_manual_run(teams_dir: Path, team: Optional[str], run_id: str) -> dict[str, Any]:
    if team:
        candidates = [teams_dir / team / "state/compaction/manual-runs" / f"{run_id}.json"]
    else:
        candidates = list(teams_dir.glob(f"*/state/compaction/manual-runs/{run_id}.json"))
    if len(candidates) != 1:
        raise ValueError(f"manual run '{run_id}' resolved to {len(candidates)} metadata files")
    try:
        payload = json.loads(candidates[0].read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"could not read manual run '{run_id}': {error}") from error
    if not isinstance(payload, dict):
        raise ValueError(f"manual run '{run_id}' is not a JSON object")
    return payload


def print_counter(title: str, counter: Counter[str]) -> None:
    print(f"\n{title}")
    print("-" * len(title))
    if not counter:
        print("none")
        return
    for key in sorted(counter):
        print(f"{key}: {counter[key]}")


def main() -> int:
    args = parse_args()
    try:
        log_path = resolve_log_path(args.log)
        team = args.team
        member = args.member
        since = parse_timestamp(args.since) if args.since else None
        window_description = args.since or "all available logs"
        if args.last:
            since = datetime.now(timezone.utc) - parse_duration(args.last)
            window_description = f"last {args.last}"

        if args.manual_run_id:
            run = load_manual_run(args.teams_dir, team, args.manual_run_id)
            team = str(run.get("team_name") or team or "") or None
            member = str(run.get("member_name") or member or "") or None
            triggered_at = run.get("triggered_at")
            if isinstance(triggered_at, str):
                since = parse_timestamp(triggered_at)
                window_description = f"scripted run {args.manual_run_id}"

        records = []
        for record in iter_records(rotated_log_paths(log_path)):
            if not is_hook_event(record):
                continue
            timestamp = event_timestamp(record)
            if since is not None and (timestamp is None or timestamp < since):
                continue
            if team and record.get("team_name") != team:
                continue
            if member and record.get("member_name") != member:
                continue
            records.append(record)

        records.sort(key=lambda record: event_timestamp(record) or datetime.min.replace(tzinfo=timezone.utc))
        actions = Counter(event_action(record) for record in records)
        tools = Counter(event_tool(record) for record in records)
        terminal_count = sum(actions[action] for action in TERMINAL_ACTIONS)

        print("Native Compaction Hook Analysis")
        print("===============================")
        print(f"Log: {log_path}")
        print(f"Window: {window_description}")
        print(f"Team: {team or 'all'}")
        print(f"Member: {member or 'all'}")
        print(f"Hook events: {len(records)}")
        print(f"Terminal outcomes: {terminal_count}")
        print_counter("By action", actions)
        print_counter("By tool", tools)

        print("\nRecent hook events")
        print("------------------")
        if not records:
            print("none")
        else:
            recent = records[-args.limit :] if args.limit > 0 else []
            for record in recent:
                target = "/".join(
                    str(value)
                    for value in (record.get("team_name"), record.get("member_name"))
                    if value
                )
                detail = (
                    record.get("skip_reason")
                    or record.get("failure_stage")
                    or record.get("additional_context_bytes")
                    or ""
                )
                print(
                    f"{record.get('ts', 'n/a')} {record.get('event')} "
                    f"target={target or 'unresolved'} detail={detail}"
                )
        return 0
    except ValueError as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
