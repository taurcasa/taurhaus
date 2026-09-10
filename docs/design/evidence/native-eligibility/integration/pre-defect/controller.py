"""Isolated integration trial; no operator config or process is adopted.

Run from checkout root: python3 docs/design/evidence/native-eligibility/integration/controller.py LABEL
CODEX_TRIAL_BINARY optionally names the installed native 0.153.4 executable.
Only auth.json is copied from CODEX_HOME (or the default home); never logged.
"""
import hashlib
import json
import os
from pathlib import Path
import secrets
import shlex
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time

CHECKOUT = Path.cwd()
OUT = Path(__file__).resolve().parent / sys.argv[1]
OUT.mkdir()
os.umask(0o077)
ROOT = Path(tempfile.mkdtemp(prefix="th-int-"))
for directory in ["home/.local/bin", "codex", "project", "tmp", "tmux", "claude", "grok", "gemini", "data"]:
    (ROOT / directory).mkdir(parents=True, exist_ok=True, mode=0o700)
BIN = ROOT / "home/.local/bin"
ENV = {"PATH": f"{BIN}:/usr/bin:/bin", "HOME": str(ROOT / "home"),
       "CODEX_HOME": str(ROOT / "codex"), "TMPDIR": str(ROOT / "tmp"),
       "TMUX_TMPDIR": str(ROOT / "tmux"), "TAURHAUS_DATA_DIR": str(ROOT / "data"),
       "TAURHAUS_CLAUDE_DIR": str(ROOT / "claude"), "CLAUDE_CONFIG_DIR": str(ROOT / "claude"),
       "CLAUDE_DIR": str(ROOT / "claude"), "GROK_HOME": str(ROOT / "grok"),
       "TAURHAUS_AGY_DIR": str(ROOT / "gemini"), "GEMINI_CLI_HOME": str(ROOT / "gemini"),
       "LANG": "C.UTF-8", "TERM": "xterm-256color", "SHELL": "/bin/bash",
       "RUST_LOG": "info", "TAURHAUS_TRIAL_ID": ROOT.name}
TEAM = "integration"
MEMBER = "seat"
EVENTS = (OUT / "events.jsonl").open("w", buffering=1)
children = []
step = 1
turns = []


def log(kind, **fields):
    row = {"at": time.time(), "kind": kind, **fields}
    EVENTS.write(json.dumps(row) + "\n")
    print(json.dumps(row), flush=True)


def run(argv, timeout=20):
    log("command", argv=argv)
    proc = subprocess.Popen(argv, env=ENV, cwd=ROOT / "project", stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, start_new_session=True)
    children.append(proc)
    try:
        output = proc.communicate(timeout=timeout)[0].decode(errors="replace")
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGKILL)
        proc.communicate()
        raise
    log("command_result", exit=proc.returncode, output=output)
    if proc.returncode:
        raise RuntimeError(f"command exit {proc.returncode}: {output}")
    return output


def rpc(method, params):
    request = {"id": secrets.token_hex(8), "method": method, "params": params}
    log("daemon_request", request=request)
    with socket.create_connection(("127.0.0.1", PORT), timeout=10) as conn:
        request["auth"] = (ROOT / "data/daemon.token").read_text().strip()
        conn.sendall(json.dumps(request).encode() + b"\n")
        stream = conn.makefile("rb")
        while True:
            response = json.loads(stream.readline())
            if response.get("id") == request["id"]:
                break
    log("daemon_response", response=response)
    if "error" in response:
        raise RuntimeError(str(response["error"]))
    return response["result"]


def operation(method, params):
    accepted = rpc(method, params)
    deadline = time.monotonic() + 100
    while time.monotonic() < deadline:
        status = rpc(method + "_status", {"run_id": accepted["run_id"]})
        if status["outcome"]["status"] != "running":
            return status
        time.sleep(.3)
    raise TimeoutError(method)


def capture(name):
    p = subprocess.run(["tmux", "list-panes", "-a", "-F", "#{pane_id}"],
                       env=ENV, capture_output=True, text=True)
    for pane in p.stdout.splitlines():
        text = run(["tmux", "capture-pane", "-p", "-S", "-", "-t", pane])
        (OUT / f"{name}-pane-{pane[1:]}.txt").write_text(text)


def identities():
    found = []
    for proc in Path("/proc").iterdir():
        if not proc.name.isdigit():
            continue
        try:
            if (b"TAURHAUS_TRIAL_ID=" + ROOT.name.encode() + b"\0") not in (proc / "environ").read_bytes():
                continue
            stat = (proc / "stat").read_text().rsplit(")", 1)[1].split()
            found.append({"pid": int(proc.name), "start_ticks": stat[19],
                          "argv": (proc / "cmdline").read_bytes().decode(errors="replace").split("\0")})
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            pass
    return found


def snapshot():
    team = ROOT / "claude/teams" / TEAM
    for glob in ["config.json", "runtime/*.json", "state/delivery/*", "journal/*.jsonl", "state/app-server/*holder*"]:
        for path in team.glob(glob):
            if path.is_file():
                target = OUT / "team" / path.relative_to(team)
                target.parent.mkdir(parents=True, exist_ok=True)
                if path.name == "config.json":
                    config = json.loads(path.read_text())
                    for member in config.get("members", []):
                        if "controlAuthTokenHash" in member:
                            member["controlAuthTokenHash"] = "<redacted credential hash>"
                    target.write_text(json.dumps(config, indent=2) + "\n")
                else:
                    shutil.copyfile(path, target)
    for path in (ROOT / "data").glob("*.jsonl"):
        shutil.copyfile(path, OUT / path.name)


def stop_on_signal(signum, frame):
    raise RuntimeError(f"controller interrupted: {signum}")


for sig in [signal.SIGTERM, signal.SIGINT]:
    signal.signal(sig, stop_on_signal)

exit_code = 1
try:
    source = Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex"))) / "auth.json"
    assert source.is_file() and not source.is_symlink()
    shutil.copyfile(source, ROOT / "codex/auth.json")
    (ROOT / "codex/auth.json").chmod(0o600)
    native = os.environ.get("CODEX_TRIAL_BINARY")
    if not native:
        package = Path(shutil.which("codex")).resolve().parents[1]
        native = next(package.glob("node_modules/@openai/codex-linux-x64/vendor/*/bin/codex"))
    for name, source in [("codex", native), ("mesh", "/home/mstie/projects/mesh-push/target/debug/mesh"),
                         ("taurhaus-daemon", CHECKOUT / "src-tauri/target/debug/taurhaus-daemon")]:
        shutil.copyfile(source, BIN / name)
        (BIN / name).chmod(0o700)
        log("binary", name=name, sha256=hashlib.sha256((BIN / name).read_bytes()).hexdigest())
    for tool in ["claude", "agy", "grok"]:
        (BIN / tool).write_text("#!/bin/sh\nexit 77\n")
        (BIN / tool).chmod(0o700)
    for rc in [".bashrc", ".profile", ".zshrc"]:
        (ROOT / "home" / rc).write_text('export PATH="$HOME/.local/bin:/usr/bin:/bin"\n')
    (ROOT / "project/AGENTS.md").write_text("This is an isolated transport trial. Reply briefly. Never execute tools or commands.\n")
    run(["git", "init", "-q"])
    run(["git", "add", "AGENTS.md"])
    run(["git", "-c", "user.name=Trial", "-c", "user.email=trial@example.invalid", "commit", "-qm", "trial instructions"])
    for PORT in secrets.SystemRandom().sample(range(20000, 32000), 12000):
        with socket.socket() as probe:
            try:
                probe.bind(("127.0.0.1", PORT))
                break
            except OSError:
                continue
    ENV["TAURHAUS_DAEMON_PORT"] = str(PORT)
    log("isolation", root=str(ROOT), environment=ENV, initial_codex_entries=["auth.json"],
        max_model_turns=8, max_usd=3, marker=secrets.token_hex(8))
    (ROOT / "codex/config.toml").write_text('model="gpt-5.6-luna"\nmodel_reasoning_effort="low"\napproval_policy="never"\nsandbox_mode="read-only"\nweb_search="disabled"\n[projects.' + json.dumps(str(ROOT / "project")) + ']\ntrust_level="trusted"\n')
    # All descendant processes share this disposable PID namespace. Operator
    # homes and services are absent; only the scratch root is writable.
    bw = ["bwrap", "--die-with-parent", "--unshare-pid", "--ro-bind", "/", "/",
          "--tmpfs", "/home", "--tmpfs", "/tmp", "--tmpfs", "/run", "--proc", "/proc", "--dev", "/dev",
          "--bind", str(ROOT), str(ROOT), "--chdir", str(ROOT / "project")]
    run(bw + [str(BIN / "codex"), "--version"])
    daemon_argv = [str(BIN / "taurhaus-daemon"), "--port", str(PORT), "--data-dir", str(ROOT / "data")]
    boot = ROOT / "boot.sh"
    boot.write_text('#!/bin/bash\nset -eu\ntmux -D -f /dev/null &\n'
                    'for i in {1..100}; do test -S "$TMUX_TMPDIR/tmux-$(id -u)/default" && break; sleep .1; done\n'
                    'tmux set-option -g default-shell /bin/bash\n'
                    'tmux new-session -d -s taurhaus -x 140 -y 48 /bin/bash\n'
                    'exec ' + shlex.join(daemon_argv) + '\n')
    log("bootstrap_script", text=boot.read_text())
    command = bw + ["/bin/bash", str(boot)]
    log("daemon_spawn", argv=command)
    daemon_log = (OUT / "daemon.log").open("w")
    daemon = subprocess.Popen(command, env=ENV, stdout=daemon_log, stderr=subprocess.STDOUT, start_new_session=True)
    children.append(daemon)
    for _ in range(100):
        if (ROOT / "data/daemon.token").exists():
            time.sleep(.2)
            break
        if daemon.poll() is not None:
            raise RuntimeError(f"daemon exited {daemon.returncode}")
        time.sleep(.1)
    rpc("ping", {})
    policy = json.loads((CHECKOUT.parent / "taurhaus/.check-logs/mesh-next-phase0/verify-s3/policy.json").read_text())
    (ROOT / "policy.json").write_text(json.dumps(policy))
    mesh = [str(BIN / "mesh"), "--claude-dir", str(ROOT / "claude"), "--team", TEAM, "--name", "lead"]
    setup_commands = [mesh + ["team", "create", "--messaging-canonical", "--isolated", "--retention-policy", str(ROOT / "policy.json")],
                      mesh + ["team", "delivery", "--owner", "team"]]
    setup = ROOT / "setup.sh"
    setup.write_text("#!/bin/bash\nset -eu\n" + "\n".join(shlex.join(cmd) for cmd in setup_commands) + "\ntouch " + shlex.quote(str(ROOT / "setup-done")) + "\n")
    setup.chmod(0o700)
    rpc("launch_session", {"project_path": str(ROOT / "project"), "mode": "fresh", "cli_tool": "codex",
                           "command_override": str(setup), "tmux_layout": "new_window"})
    for _ in range(100):
        if (ROOT / "setup-done").exists():
            break
        time.sleep(.1)
    capture("setup")
    assert (ROOT / "setup-done").exists(), "canonical team setup did not finish"
    operation("coordination.add_member", {"request": {"team_name": TEAM, "member_name": MEMBER,
               "backend_kind": "codex", "project_path": str(ROOT / "project")}})
    config_path = ROOT / "claude/teams" / TEAM / "config.json"
    config = json.loads(config_path.read_text())
    for member in config["members"]:
        if member["name"] == MEMBER:
            member.update(adapter_mode="app_server", model="gpt-5.6-luna", reasoningEffort="low")
    config_path.write_text(json.dumps(config))
    result = operation("coordination.resume_member", {"request": {"team_name": TEAM, "member_name": MEMBER},
                       "cli_commands": {"codex": {"fresh": "codex --sandbox read-only --ask-for-approval never",
                       "continue_cmd": "codex --sandbox read-only --ask-for-approval never",
                       "resume": "codex --sandbox read-only --ask-for-approval never resume"}}, "tmux_layout": "new_window"})
    capture("step-1")
    log("step_outcome", step=1, result=result)
    assert result["outcome"]["status"] == "completed", "member launch failed"
    report = result["outcome"]["report"]
    assert report.get("success", True), "member launch report unsuccessful"
    # A passing launch is inspected before any paid delivery is authorized.
    raise RuntimeError("step 1 requires runtime inspection before continuing")
except BaseException as error:
    log("stopped", step=step, error=str(error), type=type(error).__name__)
finally:
    snapshot()
    before = identities()
    (OUT / "identities.json").write_text(json.dumps(before, indent=2))
    for child in reversed(children):
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=5)
    time.sleep(.3)
    survivors = identities()
    with socket.socket() as probe:
        probe.settimeout(.2)
        port_closed = probe.connect_ex(("127.0.0.1", PORT)) != 0 if "PORT" in globals() else True
    cleanup = {"survivors": survivors, "port_closed": port_closed, "auth_removed": True, "root": str(ROOT)}
    shutil.rmtree(ROOT)
    cleanup["root_removed"] = not ROOT.exists()
    (OUT / "cleanup.json").write_text(json.dumps(cleanup, indent=2))
    (OUT / "cost-ledger.json").write_text(json.dumps({"turns": turns, "paid_inputs": len(turns), "usd": 0,
        "basis": "No turn/start, turn/steer, TUI input or compaction requested by controller; inspect recovery launch evidence."}, indent=2))
    log("cleanup", **cleanup)
    EVENTS.close()
    if survivors or not port_closed:
        exit_code = 2
sys.exit(exit_code)
