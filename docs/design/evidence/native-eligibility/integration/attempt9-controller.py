"""Isolated integration trial; no operator config or process is adopted.

Run after attempt3-build.py with TRIAL_EVIDENCE_LABEL=attempt9: python3 docs/design/evidence/native-eligibility/integration/attempt9-controller.py LABEL
CODEX_TRIAL_BINARY optionally names the installed native 0.153.4 executable.
Only auth.json is copied from CODEX_HOME (or the default home); never logged.
"""
import hashlib
import json
import os
from pathlib import Path
import secrets
import re
import shlex
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import traceback
import threading
from attempt8_passive import observe, complete_rows
from attempt9_support import enforce_budget, PRIOR_TURNS, PRIOR_CONSERVATIVE_USD
from attempt6_support import clean, ledger, new_events
from continuation_retention import retained_view, retain_host_event
from attempt9_support import retained_log

CHECKOUT = Path.cwd()
OUT = Path(__file__).resolve().parent / sys.argv[1]
OUT.mkdir(parents=True)
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
host_events = []
previous_host_events = []
passive_stop = threading.Event()
passive_thread = None
last_host_poll = 0
host_poll_enabled = True
marker = "cobalt" + secrets.token_hex(5)


def log(kind, **fields):
    row = clean({"at": time.time(), "kind": kind, **fields})
    EVENTS.write(json.dumps(row) + "\n")
    print(json.dumps({"kind": kind, **({"error": row["error"]} if "error" in row else {})}), flush=True)


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


def rpc(method, params, allow_error=False):
    request = {"id": secrets.token_hex(8), "method": method, "params": params}
    if method != "coordination.hosted_transcript":
        log("daemon_request", request=request)
    with socket.create_connection(("127.0.0.1", PORT), timeout=10) as conn:
        request["auth"] = (ROOT / "data/daemon.token").read_text().strip()
        conn.sendall(json.dumps(request).encode() + b"\n")
        stream = conn.makefile("rb")
        while True:
            response = json.loads(stream.readline())
            if response.get("id") == request["id"]:
                break
    response = clean(response)
    if method != "coordination.hosted_transcript":
        log("daemon_response", response=response)
    if allow_error: return response
    if "error" in response:
        raise RuntimeError(str(response["error"]))
    return response["result"]


def operation(method, params):
    accepted = rpc(method, params)
    deadline = time.monotonic() + 100
    while time.monotonic() < deadline:
        status = rpc("coordination.initialize_status" if method == "coordination.initialize_team" else method + "_status", {"run_id": accepted["run_id"]})
        if status["outcome"]["status"] != "running":
            return status
        budget_check()
        snapshot()
        time.sleep(1)
    raise TimeoutError(method)


def capture(name):
    p = subprocess.run(["tmux", "list-panes", "-a", "-F", "#{pane_id}"],
                       env=ENV, capture_output=True, text=True)
    for pane in p.stdout.splitlines():
        text = run(["tmux", "capture-pane", "-p", "-S", "-12", "-t", pane])
        (OUT / f"{name}-pane-{pane[1:]}.txt").write_text("\n".join(text.splitlines()[-60:]) + "\n")


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
    for glob in ["config.json", "runtime/*.json", "state/delivery/*.json", "state/messaging-v2/segments/*.jsonl", "state/workflow_events.jsonl"]:
        for path in team.glob(glob):
            try:
                data=path.read_text()
                if path.suffix == '.json': data=json.dumps(clean(json.loads(data)),indent=2)+'\n'
                else: data=''.join(json.dumps(clean(r))+'\n' for r in complete_rows(data))
            except (FileNotFoundError, ValueError): continue
            target=OUT/'team'/path.relative_to(team)
            target.parent.mkdir(parents=True,exist_ok=True)
            temporary=target.with_suffix(target.suffix+'.tmp')
            temporary.write_text(data); temporary.replace(target)
    for path in (ROOT/'data').glob('*.jsonl'):
        rows=[clean(row) for row in complete_rows(path.read_text())]
        rows,_=retained_log(rows)
        (OUT/path.name).write_text('\n'.join(dict.fromkeys(json.dumps(r,sort_keys=True) for r in rows))+'\n')


def budget_check():
    # Include production startup recovery and any automatic onboarding turns.
    # Counts are from the real rollout; subscription billing is not exposed.
    starts = set(); total = 0
    for path in ROOT.rglob("rollout-*.jsonl"):
        for line in path.read_text().splitlines():
            try: row = json.loads(line)
            except ValueError: continue
            payload = row.get("payload", {})
            if row.get("type") != "event_msg": continue
            if payload.get("type") == "task_started":
                starts.add(payload.get("turn_id", row.get("timestamp")))
            if payload.get("type") == "token_count" and payload.get("info"):
                usage = payload["info"].get("total_token_usage", {})
                total = max(total, usage.get("input_tokens", 0) + usage.get("output_tokens", 0))
    # Conservative $1.20/M for every token (packet's highest Luna token rate).
    # No evidence-size or time reserve abort; only spend and turn caps.
    accounted = ledger(host_events, starts)
    (OUT / "cost-ledger.json").write_text(json.dumps(accounted, indent=2))
    enforce_budget(accounted["paid_inputs"], accounted["conservative_usd"])
    assert total * 1.2 / 1000000 <= 3, "cost budget reached"


def poll_host(force=False):
    global last_host_poll, previous_host_events
    if not host_poll_enabled: return
    if not force and time.monotonic() - last_host_poll < 1:
        return
    last_host_poll = time.monotonic()
    value = rpc("coordination.hosted_transcript", {"team_name":TEAM, "member_name":MEMBER})
    current = value.get("events", [])
    fresh = new_events(previous_host_events, current)
    previous_host_events = current
    fresh = [e for e in fresh if retain_host_event(e)]
    host_events.extend(fresh)
    (OUT / "hosted-transcript.json").write_text(json.dumps(retained_view(value), indent=2))
    with (OUT / "host-events.jsonl").open("a") as stream:
        for event in fresh:
            stream.write(json.dumps(event) + "\n")
    budget_check()


def stop_on_signal(signum, frame):
    raise RuntimeError(f"controller interrupted: {signum}")


for sig in [signal.SIGTERM, signal.SIGINT]:
    signal.signal(sig, stop_on_signal)

exit_code = 1
try:
    source = Path("/home") / "mstie" / ".codex" / "auth.json"
    assert source.is_file() and not source.is_symlink()
    shutil.copyfile(source, ROOT / "codex/auth.json")
    (ROOT / "codex/auth.json").chmod(0o600)
    native = os.environ.get("CODEX_TRIAL_BINARY")
    if not native:
        package = Path(shutil.which("codex")).resolve().parents[1]
        native = next(package.glob("node_modules/@openai/codex-linux-x64/vendor/*/bin/codex"))
    for name, source in [("codex", native), ("mesh", "/home/mstie/projects/mesh-trial/target/debug/mesh"),
                         ("claude", Path(shutil.which("claude")).resolve()),
                         ("taurhaus-daemon", CHECKOUT / "src-tauri/target/debug/taurhaus-daemon")]:
        shutil.copyfile(source, BIN / name)
        (BIN / name).chmod(0o700)
        log("binary", name=name, sha256=hashlib.sha256((BIN / name).read_bytes()).hexdigest())
    for tool in ["agy", "grok"]:
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
        max_model_turns=16, max_usd=3, prior_turns=PRIOR_TURNS, prior_conservative_usd=PRIOR_CONSERVATIVE_USD, marker=marker)
    (ROOT / "codex/config.toml").write_text('model="gpt-5.6-luna"\nmodel_reasoning_effort="low"\napproval_policy="never"\nsandbox_mode="read-only"\nweb_search="disabled"\nmodel_context_window=32768\n[projects.' + json.dumps(str(ROOT / "project")) + ']\ntrust_level="trusted"\n')
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
                    + shlex.join(daemon_argv) + ' &\nwait $!\nwhile true; do sleep 1; done\n')
    log("bootstrap_script", text=boot.read_text())
    command = bw + ["/bin/bash", str(boot)]
    log("daemon_spawn", argv=command)
    daemon_log = (ROOT / "daemon.log").open("w")
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
    policy_source = (CHECKOUT / "src/lib/components/meshTabUtils.js").read_text()
    policy = json.loads(re.search(r"DEFAULT_CANONICAL_POLICY = Object.freeze\((\{.*?\})\)", policy_source, re.S)[1])
    (OUT / "policy.json").write_text(json.dumps(policy, indent=2))
    commands = {"codex": {"fresh": "codex --sandbox read-only --ask-for-approval never",
        "continue_cmd": "codex --sandbox read-only --ask-for-approval never",
        "resume": "codex --sandbox read-only --ask-for-approval never resume"}}
    request = {"team_name": TEAM, "team_description": "Disposable integration trial attempt 9",
        "lead_mode": "launch_new",
        "lead": {"name":"lead", "cli_tool":"claude", "model":"claude-haiku-4-5",
            "project_id":str(ROOT / "project")},
        "agents": [{"name": MEMBER, "cli_tool":"codex", "model":"gpt-5.6-luna",
            "reasoning_effort":"low", "delivery":"app_server", "project_id":str(ROOT / "project"),
            "instructions":"Isolated transport trial. Reply briefly. Never execute tools or commands."}],
        "messaging": {"mode":"canonical", "retentionPolicy":policy}}
    result = operation("coordination.initialize_team", {"request":request,
        "cli_commands":commands, "tmux_layout":"new_window"})
    (OUT / "initialize-result.json").write_text(json.dumps(result, indent=2))
    capture("initialize")
    snapshot()
    assert result["outcome"]["status"] == "completed", "production initialize RPC failed"
    report = result["outcome"]["report"]
    assert report["failed_step"] is None, "production initialize failed at " + str(report)
    record_path = ROOT / "claude/teams" / TEAM / "runtime" / (MEMBER + ".json")
    record = json.loads(record_path.read_text())
    assert record.get("appServer"), "step 1: no appServer attachment"
    app = record["appServer"]
    for key, value in {"host":"taurhaus-daemon-owned-thread/1", "configuration":"strict-config/1",
                       "trust":"daemon-owned/1", "transport":"unix-websocket", "build":"0.153.4"}.items():
        assert app[key] == value, (key, app[key])
    assert record["terminalContract"] == 1
    assert app["threadId"] and app["instructionSources"], "thread/instruction sources missing"
    assert Path(app["socketPath"]).is_socket()
    time.sleep(3)
    capture("step-1")
    snapshot()
    (OUT / "step1-runtime.json").write_text(json.dumps(clean(record), indent=2))
    for path in [Path(app["socketPath"]).parent / "tui/config.toml"]:
        (OUT / ("generated-config-" + str(len(list(OUT.glob('generated-config-*')))) + ".toml")).write_text(path.read_text())
    poll_host(force=True)
    (OUT / "step1-identities.json").write_text(json.dumps(clean(identities()), indent=2))
    log("inspection_ready", record=record)
    # Parent-controlled, bounded actions retain exact input/command evidence.
    # No automatic continuation or paid retry after any assertion/refusal.
    while True:
        poll_host()
        budget_check()
        actionfile = OUT / "action.json"
        if not actionfile.exists():
            time.sleep(.2)
            continue
        action = json.loads(actionfile.read_text()); actionfile.unlink()
        log("action", action=action)
        paid = (action["op"] == "mesh" and action.get("argv", [""])[0] == "send") or (action["op"] == "rpc" and action.get("method") == "coordination.hosted_input") or (action["op"] == "tmux" and action.get("argv", [""])[0] == "send-keys")
        if paid:
            assert ledger(host_events)["paid_inputs"] + PRIOR_TURNS < 16, "turn budget reached before submission"

        if action["op"] == "fail":
            raise RuntimeError(action["reason"])
        if action["op"] == "finish":
            exit_code = 0
            break
        if action["op"] == "step":
            step = action["step"]
        elif action["op"] == "passive_start":
            passive_thread = threading.Thread(target=observe, args=(ROOT, OUT / "step4-locks.jsonl", passive_stop))
            passive_thread.start()
        elif action["op"] == "passive_end":
            passive_stop.set(); passive_thread.join()
        elif action["op"] == "snapshot":
            snapshot()
        elif action["op"] == "host_poll":
            # Controller observation can stop around the sanctioned lifecycle RPCs.
            # This never pauses or changes a product process.
            host_poll_enabled = action["enabled"]
        elif action["op"] == "rpc_expected_error":
            value = rpc(action["method"], action["params"], allow_error=True)
            (OUT / action["save"]).write_text(json.dumps(value, indent=2))
            assert "error" in value and action["contains"] in str(value["error"]), value
        elif action["op"] == "restart_daemon":
            assert step == 6, "daemon restart only permitted in step 6"
            poll_host(force=True)
            old = [i for i in identities() if i["argv"][0] == str(BIN / "taurhaus-daemon")]
            assert len(old) == 1, "expected exactly one owned daemon"
            identity = old[0]
            log("normal_daemon_stop", identity=identity, signal="SIGINT (installed ctrlc shutdown handler)")
            os.kill(identity["pid"], signal.SIGINT)
            for _ in range(200):
                if not any(i["pid"] == identity["pid"] and i["start_ticks"] == identity["start_ticks"] for i in identities()): break
                time.sleep(.1)
            else: raise AssertionError("normal daemon shutdown did not complete")
            restart = shlex.join(daemon_argv) + " >>" + shlex.quote(str(ROOT / "daemon-restart.log")) + " 2>&1"
            run(["tmux", "new-window", "-d", "-t", "taurhaus", "/bin/bash -c " + shlex.quote(restart)])
            for _ in range(100):
                try:
                    rpc("ping", {}); break
                except (ConnectionRefusedError, FileNotFoundError): time.sleep(.1)
            else: raise AssertionError("restarted daemon did not become ready")
            value = operation("coordination.resume_member", {"request":{"team_name":TEAM,"member_name":MEMBER}, "cli_commands":commands, "tmux_layout":"new_window"})
            (OUT / action["save"]).write_text(json.dumps(value, indent=2))
            assert value["outcome"]["status"] == "completed" and not value["outcome"]["report"].get("failed_step"), value
            previous_host_events = []
            poll_host(force=True)
            (OUT / "step6-identities.json").write_text(json.dumps(clean(identities()), indent=2))
        elif action["op"] == "capture":
            capture(action["name"]); snapshot()
        elif action["op"] == "rpc":
            value = rpc(action["method"], action["params"])
            (OUT / action["save"]).write_text(json.dumps(value, indent=2))
        elif action["op"] == "operation":
            value = operation(action["method"], action["params"])
            (OUT / action["save"]).write_text(json.dumps(value, indent=2))
        elif action["op"] == "tmux":
            run(["tmux"] + action["argv"])
        elif action["op"] == "mesh":
            # Execute in the existing private tmux/PID namespace so runtime PIDs
            # match the daemon's publication; never use the operator server.
            seq = secrets.token_hex(4)
            output = ROOT / (seq + ".out"); status = ROOT / (seq + ".exit")
            command = shlex.join([str(BIN / "mesh")] + action["argv"])
            command += " >" + shlex.quote(str(output)) + " 2>&1; echo $? >" + shlex.quote(str(status))
            run(["tmux", "new-window", "-d", "-t", "taurhaus", "/bin/bash -c " + shlex.quote(command)])
            for _ in range(150):
                if status.exists(): break
                time.sleep(.1)
            assert status.exists(), "mesh command timeout"
            log("mesh_result", exit=int(status.read_text()), output=output.read_text())
            (OUT / action["save"]).write_text(output.read_text())
        else:
            raise ValueError(action)
        snapshot()
        log("action_done", op=action["op"])

except BaseException as error:
    log("stopped", step=step, error=str(error), type=type(error).__name__)
    traceback.print_exc()
finally:
    passive_stop.set()
    if passive_thread: passive_thread.join()
    try:
        snapshot()
    except Exception as error:
        log("snapshot_error", error=str(error))
    # Drain already-submitted startup usage through the daemon before teardown.
    # Read-only; never a model retry. Skip if launch did not publish a host.
    try:
        saved = ROOT / "claude/teams" / TEAM / "runtime" / (MEMBER + ".json")
        if saved.exists() and json.loads(saved.read_text()).get("appServer"):
            for _ in range(30):
                poll_host(force=True)
                view = json.loads((OUT / "hosted-transcript.json").read_text())
                if view.get("thread", {}).get("status", {}).get("type") == "idle": break
                time.sleep(.5)
    except Exception as error:
        log("final_host_read_error", error=str(error))
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
    snapshot()
    for raw in ROOT.glob("daemon*.log"):
        (OUT / raw.name.replace(".log", "-stderr.txt")).write_text(clean(raw.read_text()))
    # Keep only non-secret rollout events relevant to identity and spend.
    usage_events = []
    for path in ROOT.rglob("rollout-*.jsonl"):
        for line in path.read_text().splitlines():
            try: row = json.loads(line)
            except ValueError: continue
            payload = row.get("payload", {})
            if row.get("type") == "event_msg" and payload.get("type") in ["task_started", "task_complete", "token_count"]:
                usage_events.append(clean(row))
                if payload.get("type") == "task_started": turns.append(payload)
    (OUT / "usage-events.json").write_text(json.dumps(usage_events, indent=2))
    with socket.socket() as probe:
        probe.settimeout(.2)
        port_closed = probe.connect_ex(("127.0.0.1", PORT)) != 0 if "PORT" in globals() else True
    cleanup = {"survivors": survivors, "port_closed": port_closed, "auth_removed": True, "root": str(ROOT)}
    shutil.rmtree(ROOT)
    cleanup["root_removed"] = not ROOT.exists()
    (OUT / "cleanup.json").write_text(json.dumps(cleanup, indent=2))
    (OUT / "cost-ledger.json").write_text(json.dumps(ledger(host_events, [t["turn_id"] for t in turns]), indent=2))
    log("cleanup", **cleanup)
    restored = subprocess.run(["git", "-C", "/home/mstie/projects/mesh-trial", "checkout", "--",
        "src/delivery/app_server/capabilities.rs"], capture_output=True, text=True)
    log("descriptor_restored", exit=restored.returncode)
    EVENTS.close()
    # Sanitize textual logs before retaining them; auth contents were never logged.
    for path in OUT.rglob("*"):
        if path.is_file():
            text = path.read_text()
            if path.suffix == ".json":
                text = json.dumps(clean(json.loads(text)), indent=2) + "\n"
            elif path.suffix == ".jsonl":
                text = "\n".join(json.dumps(clean(json.loads(line))) for line in text.splitlines() if line) + "\n"
            else:
                text = clean(text)
            path.write_text(text)

    if survivors or not port_closed:
        exit_code = 2
sys.exit(exit_code)
