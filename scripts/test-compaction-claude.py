#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

from compaction_test_lib import (
    DEFAULT_TEAMS_DIR,
    CompactionTestError,
    ManualRun,
    build_run_id,
    ensure_resumable_task,
    find_any_log_event,
    find_debug_line,
    latest_claude_debug_log,
    now_utc,
    resolve_active_log_path,
    resolve_member_target,
    tmux_send_literal,
    to_iso,
    wait_for,
    write_manual_run,
)


TERMINAL_EVENTS = (
    "compaction.claude_hook.delivered",
    "compaction.claude_hook.skipped",
    "compaction.claude_hook.failed",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Trigger and verify Claude compaction reinjection in a managed session.")
    parser.add_argument("--team", required=True)
    parser.add_argument("--member", required=True)
    parser.add_argument("--teams-dir", type=Path, default=DEFAULT_TEAMS_DIR)
    parser.add_argument("--fill-prompt", default=None, help="Optional prompt to send before /compact. Default sends a short deterministic filler prompt.")
    parser.add_argument("--fill-wait", type=float, default=3.0, help="Seconds to wait after filler prompt before sending /compact.")
    parser.add_argument("--timeout", type=int, default=90, help="Max seconds to wait for hook/delivery evidence.")
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        target = resolve_member_target(args.teams_dir, args.team, args.member)
        if target.cli_tool != "claude":
            raise CompactionTestError(f"member '{args.member}' is '{target.cli_tool}', not Claude")
        if target.runtime_health != "healthy":
            raise CompactionTestError(
                f"member '{args.member}' runtime health is {target.runtime_health!r}; need a live Claude session"
            )
        if not target.runtime_session_id:
            raise CompactionTestError(f"member '{args.member}' has no runtime session_id")
        ensure_resumable_task(target.operational_snapshot_path)

        run_id = args.run_id or build_run_id("claude", target.team_name, target.member_name)
        trigger_at = now_utc()
        debug_log = latest_claude_debug_log()
        manual_run = ManualRun(
            run_id=run_id,
            tool="claude",
            team_name=target.team_name,
            member_name=target.member_name,
            pane_id=target.pane_id,
            project_path=str(target.project_path),
            session_id=target.runtime_session_id,
            jsonl_path=str(target.runtime_jsonl_path) if target.runtime_jsonl_path else None,
            debug_log_path=str(debug_log),
            trigger_command="/compact",
            trigger_mode="tmux_operator",
            triggered_at=to_iso(trigger_at),
        )
        metadata_path = write_manual_run(args.teams_dir, manual_run)
        log_path = resolve_active_log_path()
        fill_prompt = args.fill_prompt or f"Reply with exactly CLAUDE_COMPACTION_TEST_FILL_{run_id}."

        if args.dry_run:
            print(json.dumps({
                "run_id": run_id,
                "team": target.team_name,
                "member": target.member_name,
                "pane_id": target.pane_id,
                "session_id": target.runtime_session_id,
                "debug_log": str(debug_log),
                "log_path": str(log_path),
                "metadata_path": str(metadata_path),
                "fill_prompt": fill_prompt,
                "compact_command": "/compact",
            }, indent=2))
            return 0

        tmux_send_literal(target.pane_id, fill_prompt)
        time.sleep(args.fill_wait)
        trigger_at = now_utc()
        manual_run.triggered_at = to_iso(trigger_at)
        metadata_path.write_text(json.dumps(manual_run.to_dict(), indent=2) + "\n", encoding="utf-8")
        tmux_send_literal(target.pane_id, "/compact")

        precompact = wait_for(
            lambda: ({"line": line} if (line := find_debug_line(debug_log, trigger_at, "Getting matching hook commands for PreCompact with query: manual")) else None),
            args.timeout,
        )
        sessionstart = wait_for(
            lambda: ({"line": line} if (line := find_debug_line(debug_log, trigger_at, "Getting matching hook commands for SessionStart with query: compact")) else None),
            args.timeout,
        )
        hook_success = wait_for(
            lambda: ({"line": line} if (line := find_debug_line(debug_log, trigger_at, "Hook SessionStart:compact (SessionStart) success")) else None),
            args.timeout,
        )
        received = wait_for(
            lambda: find_any_log_event(
                log_path,
                events=["compaction.claude_hook.received"],
                team_name=target.team_name,
                member_name=target.member_name,
                session_id=target.runtime_session_id,
                since=trigger_at,
            ),
            args.timeout,
        )
        terminal = wait_for(
            lambda: find_any_log_event(
                log_path,
                events=TERMINAL_EVENTS,
                team_name=target.team_name,
                member_name=target.member_name,
                session_id=target.runtime_session_id,
                since=trigger_at,
            ),
            args.timeout,
        )
        if terminal.get("event") != "compaction.claude_hook.delivered":
            raise CompactionTestError(
                f"Claude hook reached terminal state {terminal.get('event')} instead of compaction.claude_hook.delivered"
            )

        print(json.dumps({
            "run_id": run_id,
            "team": target.team_name,
            "member": target.member_name,
            "session_id": target.runtime_session_id,
            "metadata_path": str(metadata_path),
            "debug_log": str(debug_log),
            "log_path": str(log_path),
            "precompact": precompact["line"],
            "sessionstart": sessionstart["line"],
            "hook_success": hook_success["line"],
            "received_event": received.get("event"),
            "terminal_event": terminal.get("event"),
            "next_step": f"python3 scripts/analyze-compaction.py --team {target.team_name} --manual-run-id {run_id}",
        }, indent=2))
        return 0
    except CompactionTestError as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
