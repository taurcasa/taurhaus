"""Run exact unpaid gates in a credential-free home and private PID namespace."""
import json
import os
from pathlib import Path
from support import clean
import shutil
import signal
import subprocess
import tempfile
import time

checkout = Path.cwd()
out = Path(__file__).resolve().parent / "gates"
assert (out.parent / "run/cleanup.json").exists(), "teardown required first"
out.mkdir(parents=True, exist_ok=True)
root = Path(tempfile.mkdtemp(prefix="th-int-gates-"))
operator = Path.home()
children = []
for name in ["home", "bin", "tmp", "tmux", "claude", "codex", "grok", "gemini", "data"]:
    (root / name).mkdir(mode=0o700)
shutil.copyfile(shutil.which("node"), root / "bin/node")
(root / "bin/node").chmod(0o700)
for tool in ["claude", "codex", "agy", "grok", "tmux", "mesh"]:
    path = root / "bin" / tool
    path.write_text("#!/bin/sh\necho 'credential-free gate: external runtime blocked' >&2\nexit 77\n")
    path.chmod(0o700)
tool_dirs = sorted({str(Path(shutil.which(tool)).parent) for tool in ["cargo", "bun", "just"]})
env = {"HOME": str(root / "home"), "PATH": ":".join([str(root / "bin"), *tool_dirs, "/usr/bin", "/bin"]),
       "TMPDIR": str(root / "tmp"), "TMUX_TMPDIR": str(root / "tmux"), "CODEX_HOME": str(root / "codex"),
       "CLAUDE_CONFIG_DIR": str(root / "claude"), "TAURHAUS_CLAUDE_DIR": str(root / "claude"),
       "GROK_HOME": str(root / "grok"), "TAURHAUS_AGY_DIR": str(root / "gemini"),
       "TAURHAUS_DATA_DIR": str(root / "data"), "CARGO_BUILD_JOBS": "1", "CLAUDE_DIR": str(root / "claude"), "GEMINI_CLI_HOME": str(root / "gemini"), "CARGO_HOME": str(operator / ".cargo"),
       "RUSTUP_HOME": str(operator / ".rustup"), "CARGO_TARGET_DIR": str(checkout / "src-tauri/target"),
       "SHELL": "/bin/bash", "LANG": "C.UTF-8", "TAURHAUS_TRIAL_ID": root.name}
wrapper = ["bwrap", "--die-with-parent", "--unshare-pid", "--ro-bind", "/", "/", "--tmpfs", "/home",
           "--tmpfs", "/tmp", "--tmpfs", "/run", "--proc", "/proc", "--dev", "/dev"]
for path in [operator / ".cargo", operator / ".rustup", operator / ".bun"]:
    wrapper += ["--ro-bind", str(path), str(path)]
# Cargo uses lock files in its own cache; this is a tool cache, not a harness home.
wrapper += ["--bind", str(operator / ".cargo"), str(operator / ".cargo")]
common_git = Path(subprocess.check_output(["git", "rev-parse", "--git-common-dir"], text=True).strip()).resolve()
wrapper += ["--ro-bind", str(common_git), str(common_git), "--bind", str(checkout), str(checkout),
            "--bind", str(root), str(root), "--chdir", str(checkout)]


def redact(text):
    for name in [".cargo", ".rustup", ".bun"]:
        text = text.replace(str(operator / name), f"<tool-cache>/{name}")
    return clean(text.replace(str(common_git), "<taurhaus-git-metadata>"))


def interrupted(signum, frame):
    raise RuntimeError(f"gate controller interrupted {signum}")


for sig in [signal.SIGTERM, signal.SIGINT]:
    signal.signal(sig, interrupted)
try:
    (out / "gate-isolation.json").write_text(redact(json.dumps({"environment": env, "wrapper": wrapper}, indent=2)))
    for name, command in [("check-quick", ["just", "check-quick"]), ("lint", ["just", "lint"]),
                          ("test-contracts", ["just", "test-contracts"])]:
        if name not in os.environ.get("GATE_ONLY", "check-quick,lint,test-contracts").split(","): continue
        deadline = time.monotonic() + 1800
        preflights = []
        while True:
            probe = subprocess.run(["pgrep", "-af", "(^|/)cargo( |$)"], capture_output=True, text=True)
            preflights.append({"exit": probe.returncode, "output": redact(probe.stdout)})
            if len(probe.stdout.splitlines()) < 3:
                break
            if time.monotonic() >= deadline:
                raise TimeoutError("cargo preflight")
            time.sleep(30)
        start = time.monotonic()
        with (out / f"gate-{name}.txt").open("w") as stream:
            proc = subprocess.Popen(wrapper + command, env=env, stdout=stream, stderr=subprocess.STDOUT,
                                    start_new_session=True)
            children.append(proc)
            proc.wait(timeout=1800)
        path = out / f"gate-{name}.txt"
        path.write_text(redact(path.read_text()))
        result = {"command": command, "exit": proc.returncode, "seconds": time.monotonic() - start,
                  "cargo_preflight": preflights}
        (out / f"gate-{name}.json").write_text(json.dumps(result, indent=2))
        print(json.dumps(result), flush=True)
finally:
    for proc in children:
        if proc.poll() is None:
            os.killpg(proc.pid, signal.SIGTERM)
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(proc.pid, signal.SIGKILL)
                proc.wait(timeout=5)
    shutil.rmtree(root)
    (out / "gate-cleanup.json").write_text(json.dumps({"children_waited": True, "root_removed": not root.exists()}))
