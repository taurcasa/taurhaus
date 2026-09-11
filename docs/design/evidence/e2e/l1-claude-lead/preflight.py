"""Fail closed before initialize (which itself submits paid onboarding).

This is the executed preflight, not an unexecuted six-step runtime controller.
No credentials are needed to determine whether the required budget is verified.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile


def budget_prerequisite(help_text):
    excerpt = []
    collecting = False
    for line in help_text.splitlines():
        if line.lstrip().startswith("--max-budget-usd"):
            collecting = True
            excerpt.append(line)
        elif collecting:
            if not line.startswith("    ") or line.lstrip().startswith("-"):
                break
            excerpt.append(line)
    print_only = any("only works with --print" in line for line in excerpt)
    reason = (
        "Claude advertises --max-budget-usd only for --print; independent managed "
        "TUI and /compact dollar enforcement is unverified."
        if print_only else
        "Independent managed TUI and /compact dollar enforcement is unverified."
    )
    return {"launch_allowed": False, "classification": "harness",
            "reason": reason, "help_excerpt": excerpt}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--claude-binary", required=True, type=Path)
    parser.add_argument("--codex-binary", required=True, type=Path)
    args = parser.parse_args()
    out = Path(__file__).resolve().parent
    children = []
    report = {"status": "unavailable", "stopped_before_step": 1,
              "classification": "harness", "commands": [], "binaries": [],
              "credentials_copied": [], "daemon_launched": False,
              "tmux_server_launched": False, "initialize_calls": 0,
              "cost_ledger": {"claude_inputs": 0, "codex_inputs": 0,
                              "generations": 0, "compact_inputs": 0,
                              "turns": [], "seat_spend_usd": 0}}
    old_handlers = {}

    def interrupted(signum, _frame):
        raise RuntimeError(f"preflight interrupted: {signum}")

    for sig in (signal.SIGINT, signal.SIGTERM):
        old_handlers[sig] = signal.signal(sig, interrupted)
    root = Path(tempfile.mkdtemp(prefix="th-l1-preflight-"))
    try:
        os.chmod(root, 0o700)
        for directory in ("home/.local/bin", "claude", "codex", "grok", "gemini",
                          "data", "tmp", "tmux", "project", "cache", "config"):
            (root / directory).mkdir(parents=True, exist_ok=True, mode=0o700)
        env = {"PATH": f"{root}/home/.local/bin:/usr/bin:/bin", "HOME": str(root / "home"),
               "CLAUDE_CONFIG_DIR": str(root / "claude"), "CLAUDE_DIR": str(root / "claude"),
               "TAURHAUS_CLAUDE_DIR": str(root / "claude"), "CODEX_HOME": str(root / "codex"),
               "GROK_HOME": str(root / "grok"), "TAURHAUS_AGY_DIR": str(root / "gemini"),
               "GEMINI_CLI_HOME": str(root / "gemini"), "TAURHAUS_DATA_DIR": str(root / "data"),
               "TMPDIR": str(root / "tmp"), "TMUX_TMPDIR": str(root / "tmux"),
               "XDG_CONFIG_HOME": str(root / "config"), "XDG_CACHE_HOME": str(root / "cache"),
               "XDG_DATA_HOME": str(root / "data"), "LANG": "C.UTF-8", "TERM": "xterm-256color",
               "DISABLE_AUTOUPDATER": "1", "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1"}
        report["environment"] = env
        for name, source in (("claude", args.claude_binary), ("codex", args.codex_binary)):
            target = root / "home/.local/bin" / name
            shutil.copyfile(source, target)
            target.chmod(0o700)
            report["binaries"].append({"name": name, "sha256": hashlib.sha256(target.read_bytes()).hexdigest()})
        bw = ["bwrap", "--die-with-parent", "--unshare-pid", "--unshare-net",
              "--ro-bind", "/", "/", "--tmpfs", "/home", "--tmpfs", "/tmp",
              "--tmpfs", "/run", "--proc", "/proc", "--dev", "/dev",
              "--bind", str(root), str(root), "--chdir", str(root / "project")]
        help_text = ""
        for name, option in (("claude", "--version"), ("claude", "--help"), ("codex", "--version")):
            argv = bw + [str(root / "home/.local/bin" / name), option]
            child = subprocess.Popen(argv, env=env, cwd=root / "project", start_new_session=True,
                                     stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            children.append(child)
            output = child.communicate(timeout=60)[0].decode(errors="replace")
            report["commands"].append({"argv": argv, "pid": child.pid, "exit": child.returncode,
                                       "output": output if option == "--version" else
                                       "\n".join(budget_prerequisite(output)["help_excerpt"])})
            if child.returncode:
                raise RuntimeError(f"{name} {option} exited {child.returncode}")
            if option == "--help":
                help_text = output
        report.update(budget_prerequisite(help_text))
    except Exception as error:
        report["reason"] = str(error)
    finally:
        for child in reversed(children):
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait(timeout=5)
        # Namespace init lifetime owns every descendant; no daemon/tmux was started.
        shutil.rmtree(root)
        report["cleanup"] = {"child_exit_codes": [child.returncode for child in children],
                             "all_owned_children_reaped": all(child.poll() is not None for child in children),
                             "scratch_root_removed": not root.exists(), "credential_copies_remaining": 0,
                             "listeners_started": 0, "daemon_or_tmux_survivors": 0}
        for sig, handler in old_handlers.items():
            signal.signal(sig, handler)
        report["controller_exit"] = 2
        (out / "preflight.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps({"status": report["status"], "reason": report["reason"],
                          "exit": 2, "seat_spend_usd": 0}))
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
