#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from compaction_test_lib import (
    DEFAULT_TEAMS_DIR,
    CompactionTestError,
    ManualRun,
    build_run_id,
    ensure_resumable_task,
    find_any_log_event,
    now_utc,
    resolve_active_log_path,
    resolve_member_target,
    tmux_send_literal,
    to_iso,
    wait_for,
    write_manual_run,
)


TERMINAL_EVENTS = (
    "compaction.codex_hook.delivered",
    "compaction.codex_hook.skipped",
    "compaction.codex_hook.failed",
)
DEFAULT_MAX_TURNS = 6
DEFAULT_FILLER_LINES = 900


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Trigger and verify automatic Codex compaction through the native hook."
    )
    parser.add_argument("--team", required=True)
    parser.add_argument("--member", required=True)
    parser.add_argument("--teams-dir", type=Path, default=DEFAULT_TEAMS_DIR)
    parser.add_argument(
        "--fill-prompt",
        default=None,
        help="Optional prompt sent on each turn instead of reading the generated filler file.",
    )
    parser.add_argument("--max-turns", type=int, default=DEFAULT_MAX_TURNS)
    parser.add_argument(
        "--turn-timeout",
        type=int,
        default=90,
        help="Seconds to wait for a hook outcome after each filler turn.",
    )
    parser.add_argument("--filler-lines", type=int, default=DEFAULT_FILLER_LINES)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args()


def filler_body(run_id: str, line_count: int) -> str:
    lines = (
        f"- filler {run_id}.{index}: read this line only to grow the active Codex thread toward automatic compaction."
        for index in range(line_count)
    )
    return "# Automatic compaction filler\n\n" + "\n".join(lines) + "\n"


def main() -> int:
    filler_path: Path | None = None
    created_filler = False
    try:
        args = parse_args()
        if args.max_turns < 1:
            raise CompactionTestError("--max-turns must be at least 1")
        if args.filler_lines < 1:
            raise CompactionTestError("--filler-lines must be at least 1")

        target = resolve_member_target(args.teams_dir, args.team, args.member)
        if target.cli_tool != "codex":
            raise CompactionTestError(f"member '{args.member}' is '{target.cli_tool}', not Codex")
        if target.runtime_health != "healthy":
            raise CompactionTestError(
                f"member '{args.member}' runtime health is {target.runtime_health!r}; need a live Codex session"
            )
        ensure_resumable_task(target.operational_snapshot_path)

        run_id = args.run_id or build_run_id("codex", target.team_name, target.member_name)
        trigger_at = now_utc()
        manual_run = ManualRun(
            run_id=run_id,
            tool="codex",
            team_name=target.team_name,
            member_name=target.member_name,
            pane_id=target.pane_id,
            project_path=str(target.project_path),
            session_id=target.runtime_session_id,
            jsonl_path=str(target.runtime_jsonl_path) if target.runtime_jsonl_path else None,
            debug_log_path=None,
            trigger_command="automatic compaction via filler turns",
            trigger_mode="automatic",
            triggered_at=to_iso(trigger_at),
        )
        metadata_path = write_manual_run(args.teams_dir, manual_run)
        log_path = resolve_active_log_path()
        filler_path = target.project_path / f".taurhaus-compaction-filler-{run_id}.md"
        prompt = args.fill_prompt or (
            f"Read {filler_path} and reply with only the number of list items it contains."
        )

        if args.dry_run:
            print(
                json.dumps(
                    {
                        "run_id": run_id,
                        "team": target.team_name,
                        "member": target.member_name,
                        "pane_id": target.pane_id,
                        "session_id": target.runtime_session_id,
                        "log_path": str(log_path),
                        "metadata_path": str(metadata_path),
                        "trigger": "automatic",
                        "max_turns": args.max_turns,
                        "filler_path": str(filler_path),
                        "fill_prompt": prompt,
                    },
                    indent=2,
                )
            )
            return 0

        if args.fill_prompt is None:
            filler_path.write_text(filler_body(run_id, args.filler_lines), encoding="utf-8")
            created_filler = True

        terminal = None
        turns = 0
        for turns in range(1, args.max_turns + 1):
            tmux_send_literal(target.pane_id, prompt)
            try:
                terminal = wait_for(
                    lambda: find_any_log_event(
                        log_path,
                        events=TERMINAL_EVENTS,
                        team_name=target.team_name,
                        member_name=target.member_name,
                        session_id=target.runtime_session_id,
                        since=trigger_at,
                    ),
                    args.turn_timeout,
                )
                break
            except CompactionTestError:
                continue

        if terminal is None:
            raise CompactionTestError(
                f"Codex produced no native hook outcome within {args.max_turns} filler turns; "
                "confirm model_auto_compact_token_limit was set before this member launched"
            )
        if terminal.get("event") != "compaction.codex_hook.delivered":
            reason = terminal.get("skip_reason") or terminal.get("failure_stage") or "unknown"
            raise CompactionTestError(
                f"Codex hook reached {terminal.get('event')} ({reason}) instead of delivery"
            )

        received = wait_for(
            lambda: find_any_log_event(
                log_path,
                events=["compaction.codex_hook.received"],
                team_name=target.team_name,
                member_name=target.member_name,
                session_id=target.runtime_session_id,
                since=trigger_at,
            ),
            args.turn_timeout,
        )

        print(
            json.dumps(
                {
                    "run_id": run_id,
                    "team": target.team_name,
                    "member": target.member_name,
                    "session_id": target.runtime_session_id,
                    "metadata_path": str(metadata_path),
                    "log_path": str(log_path),
                    "trigger": "automatic",
                    "turns": turns,
                    "received_event": received.get("event"),
                    "terminal_event": terminal.get("event"),
                    "additional_context_bytes": terminal.get("additional_context_bytes"),
                    "next_step": (
                        f"python3 scripts/analyze-compaction.py --team {target.team_name} "
                        f"--member {target.member_name} --manual-run-id {run_id}"
                    ),
                },
                indent=2,
            )
        )
        return 0
    except CompactionTestError as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 1
    finally:
        if filler_path is not None and created_filler:
            filler_path.unlink(missing_ok=True)


if __name__ == "__main__":
    raise SystemExit(main())
