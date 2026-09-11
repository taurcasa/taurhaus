"""Isolated integration trial; no operator config or process is adopted.

Run build.py, then finish.py --controller; execute steps.py 1 through 6 in order.
The installed native Codex and its code-mode sibling are copied and version checked.
Only the explicitly authorized auth.json is copied; never logged.
"""
import hashlib
import itertools
import json
import os
from pathlib import Path
import secrets
import re
import shlex
import shutil
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time
import traceback
import threading
from support import clean, ledger, new_events, complete_rows, rollout_events, enforce_budget, pending, retry_busy, host_needs_resume
from retention import retained_view, retain_host_event

CHECKOUT = Path.cwd()
OUT = Path(__file__).resolve().parent / sys.argv[1]
OUT.mkdir(parents=True, exist_ok=True)
os.umask(0o077)
ROOT = Path(tempfile.mkdtemp(prefix="th-l5-"))
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
TEAM = "l5-restarts"
MEMBER = "beta"
EVENTS = (OUT / "events.jsonl").open("w", buffering=1)
children = []
step = 1
setup_complete = False
turns = []
warmup_inputs = 0
host_events = []
previous_host_events = []
last_host_poll = 0
owner_stop = threading.Event()
owner_thread = None
marker = "cobalt" + secrets.token_hex(5)


def log(kind, **fields):
    row = clean({"at": time.time(), "kind": kind, **fields})
    EVENTS.write(json.dumps(row) + "\n")
    print(json.dumps({"kind": kind, **({"error": row["error"]} if "error" in row else {})}), flush=True)


def run(argv, timeout=120):
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
    log("command_result", exit=proc.returncode, output="\n".join(output.splitlines()[-60:]))
    if proc.returncode:
        raise RuntimeError(f"command exit {proc.returncode}: {output}")
    return output


def rpc_once(method, params, allow_error=False):
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
    if isinstance(response.get("result"),dict):
        response["result"].pop("account_observations",None)
    response = clean(response)
    if method != "coordination.hosted_transcript":
        log("daemon_response", response=response)
    if allow_error: return response
    if "error" in response:
        raise RuntimeError(str(response["error"]))
    return response["result"]


def rpc(method, params, allow_error=False):
    deadline=time.monotonic()+65
    attempt=0
    while True:
        response=rpc_once(method,params,allow_error=True)
        if 'error' not in response:
            return response if allow_error else response['result']
        busy=any(s in str(response['error']).lower() for s in ['host member busy','lock busy'])
        if not busy or time.monotonic()>=deadline:
            if allow_error:return response
            raise RuntimeError(str(response['error']))
        attempt+=1
        log('transient_refusal',method=method,attempt=attempt,error=response['error'])
        time.sleep(.5)


def operation(method, params):
    accepted = rpc(method, params)
    deadline = time.monotonic() + 180
    while time.monotonic() < deadline:
        status = rpc("coordination.initialize_status" if method == "coordination.initialize_team" else method + "_status", {"run_id": accepted["run_id"]})
        assert time.monotonic() < paid_deadline, "15 minute runtime deadline"
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
            tagged = (b"TAURHAUS_TRIAL_ID=" + ROOT.name.encode() + b"\0") in (proc / "environ").read_bytes()
            owned_binary = str(BIN).encode() in (proc / "cmdline").read_bytes()
            if not tagged and not owned_binary:
                continue
            stat = (proc / "stat").read_text().rsplit(")", 1)[1].split()
            found.append({"pid": int(proc.name), "start_ticks": stat[19],
                          "argv": (proc / "cmdline").read_bytes().decode(errors="replace").split("\0")})
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            pass
    return found


def observe_owners():
    with (OUT / 'owner-observations.jsonl').open('w',buffering=1) as stream:
        while not owner_stop.is_set():
            deadline=time.monotonic()+.5
            try:
                owners=[i for i in identities() if 'team-daemon' in i['argv'] and 'start' in i['argv']]
                path=ROOT / 'claude/teams' / TEAM / 'state/delivery/epoch.json'
                epoch=json.loads(path.read_text()) if path.exists() else None
                value={'owners':owners,'epoch':epoch}
                stream.write(json.dumps(clean({'at':time.time(),**value}))+'\n')
            except (OSError,ValueError) as error:
                stream.write(json.dumps({'at':time.time(),'error':type(error).__name__})+'\n')
            owner_stop.wait(max(0,deadline-time.monotonic()))


def snapshot():
    tasks=ROOT/'claude/tasks'/TEAM
    for path in tasks.glob('*.json'):
        target=OUT/'tasks'/path.name; target.parent.mkdir(exist_ok=True)
        try: target.write_text(json.dumps(clean(json.loads(path.read_text())),indent=2))
        except (FileNotFoundError, ValueError): pass
    team = ROOT / "claude/teams" / TEAM
    for glob in ["state/activity/*.json", "config.json", "runtime/*.json", "state/delivery/*.json", "state/messaging-v2/segments/*.jsonl", "state/workflow_events.jsonl"]:
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
        (OUT/path.name).write_text('\n'.join(json.dumps(r,sort_keys=True) for r in rows if r is not None)+'\n')


def budget_check():
    extra=[]; usage=[]; replies=[]
    beta_threads={e.get('params',{}).get('threadId') for e in host_events}
    for path in ROOT.rglob('rollout-*.jsonl'):
        rows=complete_rows(path.read_text())
        meta=next((r['payload'] for r in rows if r.get('type')=='session_meta'),{})
        thread=meta.get('id',path.stem)
        if thread not in beta_threads:
            extra.extend(rollout_events(rows,thread))
        for row in rows:
            payload=row.get('payload',{})
            if row.get('type')=='event_msg' and payload.get('type') in ['task_started','task_complete','token_count']:
                usage.append(clean({'thread_id':thread,**row}))
            if row.get('type')=='response_item' and True:
                replies.append(clean({'thread_id':thread,**row}))
    (OUT/'usage-events.json').write_text(json.dumps(usage,indent=2))
    (OUT/'rollout-items.json').write_text(json.dumps(replies,indent=2))
    notify=ROOT/'data/codex-notify.jsonl'
    if notify.exists(): (OUT/'codex-notify.jsonl').write_text(notify.read_text())
    accounted=ledger(host_events+extra,notify_records=complete_rows(notify.read_text()) if notify.exists() else [])
    accounted['paid_inputs'] += warmup_inputs
    accounted['warmup_inputs'] = warmup_inputs
    (OUT/'cost-ledger.json').write_text(json.dumps(accounted,indent=2))
    # Metering is observational; never gates lifecycle or observer actions.
    accounted['caps'] = {'inputs':20,'usd':.30}
    return accounted


def poll_host(force=False):
    global last_host_poll, previous_host_events
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


def warm_scratch_home():
    """One unprompted private TUI start, clean quit, and persisted migration barrier."""
    global warmup_inputs
    warmup_inputs = 1
    started = time.time()
    log('warmup_started', counted_inputs=1)
    status = ROOT / 'warmup.exit'
    command = shlex.join([str(BIN / 'codex'), '--yolo']) + '; echo $? >' + shlex.quote(str(status))
    pane = run(['tmux', 'new-window', '-d', '-P', '-F', '#{pane_id}', '-t', 'taurhaus',
                '/bin/bash -c ' + shlex.quote(command)]).strip()
    deadline = time.monotonic() + 120
    while True:
        screen = run(['tmux', 'capture-pane', '-p', '-t', pane])
        if 'context left' in screen.lower() or '? for shortcuts' in screen.lower(): break
        assert not status.exists(), 'warmup TUI exited before composer'
        assert time.monotonic() < deadline, 'warmup composer deadline'
        time.sleep(.5)
    (OUT / 'warmup-pane.txt').write_text('\n'.join(screen.splitlines()[-60:]) + '\n')
    run(['tmux', 'send-keys', '-t', pane, '-l', '/quit'])
    run(['tmux', 'send-keys', '-t', pane, 'Enter'])
    while not status.exists():
        assert time.monotonic() < deadline, 'warmup clean quit deadline'
        time.sleep(.2)
    code = int(status.read_text())
    databases = sorted(p.name for p in (ROOT / 'codex').glob('*.sqlite'))
    assert code == 0 and databases, 'warmup requires clean exit and SQLite present'
    result = {'started_at': started, 'completed_at': time.time(), 'exit': code,
              'composer_seen': True, 'sqlite_files': databases, 'counted_inputs': 1,
              'model_prompts': 0, 'metered_usd': 0, 'basis': 'TUI start and quit; no model prompt submitted'}
    (OUT / 'warmup.json').write_text(json.dumps(result, indent=2) + '\n')
    log('warmup_completed', **result)


def setup_postmortem():
    """Best-effort metadata only; never open auth, databases or account records."""
    data = {"full_child_stderr": "unavailable: daemon owns the child pipe; only its sanitized 512-character tail is exposed"}

    def probe(name, action):
        try:
            data[name] = action()
        except Exception as error:
            data[name] = {"error": type(error).__name__}

    def entries():
        paths = list(itertools.islice((ROOT / "codex").iterdir(), 101))
        data["codex_entries_truncated"] = len(paths) > 100
        result = []
        for path in paths[:100]:
            info = path.lstat()
            result.append({"name": path.name, "mode": stat.filemode(info.st_mode), "bytes": info.st_size})
        return result

    def diagnostics():
        # Supplement the complete JSONL snapshot, never replace/filter it.
        with (ROOT / "data/taurhaus.log.jsonl").open("rb") as stream:
            size = stream.seek(0, 2)
            offset = max(0, size - 65536)
            stream.seek(offset)
            tail = stream.read(65536)
            if offset: tail = tail.partition(b"\n")[2]  # skip the potentially partial row
            rows = complete_rows(tail.decode(errors="replace"))
        data["diagnostic_window_bytes"] = 65536
        return [{k: row[k] for k in ["event", "member", "ts", "exit_status", "stderr_tail"] if k in row}
                for row in rows if row.get("event") in ["hosted.launch.failed", "hosted.launch.timed_out"]][-10:]

    def panes():
        result = subprocess.run(["tmux", "list-panes", "-a", "-F", "#{pane_id} #{pane_pid} #{pane_dead}"],
                                env=ENV, capture_output=True, text=True, timeout=5)
        return {"exit": result.returncode, "stdout": "".join(result.stdout.splitlines(keepends=True)[:60]),
                "stderr": result.stderr[-4096:]}

    probe("codex_entries", entries)
    probe("team_directory_present", lambda: (ROOT / "claude/teams" / TEAM).is_dir())
    probe("hosted_child_diagnostics", diagnostics)
    probe("tmux_panes", panes)
    (OUT / "setup-postmortem.json").write_text(json.dumps(clean(data), indent=2) + "\n")


def stop_on_signal(signum, frame):
    raise RuntimeError(f"controller interrupted: {signum}")


for sig in [signal.SIGTERM, signal.SIGINT]:
    signal.signal(sig, stop_on_signal)

exit_code = 1
try:
    assert CHECKOUT.name == "taurhaus-l5-restarts"
    source = Path(os.environ["L5_CREDENTIAL_SOURCE"])  # <authorized-source>, supplied explicitly; no fallback
    assert source.is_file() and not source.is_symlink()
    shutil.copyfile(source, ROOT / "codex/auth.json")
    (ROOT / "codex/auth.json").chmod(0o600)
    native = None
    if not native:
        package = Path(shutil.which("codex")).resolve().parents[1]
        native = next(package.glob("node_modules/@openai/codex-linux-x64/vendor/*/bin/codex"))
    for name, source in [("codex", native), ("codex-code-mode-host", Path(native).with_name("codex-code-mode-host")), ("mesh", "/home/mstie/projects/mesh-l5/target/debug/mesh"),
                         ("claude", Path(shutil.which("claude")).resolve()),
                         ("taurhaus-daemon", CHECKOUT / "src-tauri/target/release/taurhaus-daemon")]:
        shutil.copyfile(source, BIN / name)
        (BIN / name).chmod(0o700)
        log("binary", name=name, sha256=hashlib.sha256((BIN / name).read_bytes()).hexdigest())
    for tool in ["agy", "grok", "gemini"]:
        (BIN / tool).write_text("#!/bin/sh\nexit 77\n")
        (BIN / tool).chmod(0o700)
    for rc in [".bashrc", ".profile", ".zshrc"]:
        (ROOT / "home" / rc).write_text('export PATH="$HOME/.local/bin:/usr/bin:/bin"\n')
    (ROOT / "project/AGENTS.md").write_text("Reply briefly. When a message assigns you a task, run exactly the mesh lifecycle commands the message names, in order, then reply with the task id and the word done. When mesh read returns done: false, repeat the same read command and filters with --since followed by the returned cursor until done: true, even when a page is empty. Never modify files.")
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
        max_model_turns=20, max_usd=.30, marker=marker)
    (ROOT / "codex/config.toml").write_text('model="gpt-5.6-luna"\nmodel_reasoning_effort="low"\napproval_policy="never"\nsandbox_mode="danger-full-access"\nweb_search="disabled"\nmodel_context_window=32768\n[projects.' + json.dumps(str(ROOT / "project")) + ']\ntrust_level="trusted"\n')
    # All descendant processes share this disposable PID namespace. Operator
    # homes and services are absent; only the scratch root is writable.
    bw = ["bwrap", "--die-with-parent", "--unshare-pid", "--ro-bind", "/", "/",
          "--tmpfs", "/home", "--tmpfs", "/tmp", "--tmpfs", "/run", "--proc", "/proc", "--dev", "/dev",
          "--bind", str(ROOT), str(ROOT), "--chdir", str(ROOT / "project")]
    assert "0.153.4" in run(bw + [str(BIN / "codex"), "--version"]), "wrong Codex version"
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
    owner_thread=threading.Thread(target=observe_owners,daemon=True)
    owner_thread.start()
    for _ in range(650):
        if (ROOT / "data/daemon.token").exists():
            time.sleep(.2)
            break
        if daemon.poll() is not None:
            raise RuntimeError(f"daemon exited {daemon.returncode}")
        time.sleep(.1)
    ping = rpc("ping", {})
    assert ping["protocol_version"] == 27, "daemon protocol mismatch"
    log("startup_ping", result=ping)
    paid_deadline = time.monotonic() + 900
    warm_scratch_home()
    policy_source = (CHECKOUT / "src/lib/components/meshTabUtils.js").read_text()
    policy = json.loads(re.search(r"DEFAULT_CANONICAL_POLICY = Object.freeze\((\{.*?\})\)", policy_source, re.S)[1])
    (OUT / "policy.json").write_text(json.dumps(policy, indent=2))
    commands = {"codex": {"fresh": "codex --yolo",
        "continue_cmd": "codex --yolo",
        "resume": "codex --yolo resume"}}
    request = {"team_name": TEAM, "team_description": "Disposable messaging trial",
        "lead_mode": "launch_new",
        "lead": {"name":"lead", "cli_tool":"claude", "model":"claude-haiku-4-5",
            "project_id":str(ROOT / "project")},
        "agents": [{"name": name, "cli_tool":"codex", "model":"gpt-5.6-luna",
            "reasoning_effort":"low", "delivery":delivery, "project_id":str(ROOT / "project"),
            "instructions":"Reply briefly. Follow the project AGENTS.md. Never modify files."}
            for name,delivery in [("alpha","tmux"),("beta","app_server")]],
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
    capture("step-1")
    snapshot()
    (OUT / "step1-runtime.json").write_text(json.dumps(clean(record), indent=2))
    for path in [Path(app["socketPath"]).parent / "tui/config.toml"]:
        (OUT / ("generated-config-" + str(len(list(OUT.glob('generated-config-*')))) + ".toml")).write_text(path.read_text())
    poll_host(force=True)
    (OUT / "step1-identities.json").write_text(json.dumps(clean(identities()), indent=2))
    log("inspection_ready", record=record)
    setup_complete = True
    # Parent-controlled, bounded actions retain exact input/command evidence.
    # No automatic continuation or paid retry after any assertion/refusal.
    while True:
        assert time.monotonic() < paid_deadline, "15 minute runtime deadline"
        actionfile = OUT / "action.json"
        if not actionfile.exists():
            poll_host()
            budget_check()
        if not actionfile.exists():
            time.sleep(.2)
            continue
        action = json.loads(actionfile.read_text()); actionfile.unlink()
        log("action", action=action)
        paid = (action["op"] == "mesh" and action.get("argv", [""])[0] == "send") or (action["op"] == "rpc" and action.get("method") == "coordination.hosted_input") or (action["op"] == "tmux" and action.get("argv", [""])[0] == "send-keys")
        if paid:
            accounting = budget_check()
            log("input_accounting", ledger=accounting)
            enforce_budget(accounting['paid_inputs'], accounting['api_equivalent_usd'])

        if action["op"] == "pending_restart":
            label=action['label']; boundary=action['boundary']
            deadline=time.monotonic()+65
            while True:
                view=rpc('get_runtime_session_snapshot',{})
                team=ROOT/'claude/teams'/TEAM
                journal=[r for path in (team/'state/messaging-v2/segments').glob('*.jsonl')
                         for r in complete_rows(path.read_text())]
                samples={}
                for seat,mid in action['messages'].items():
                    record=json.loads((team/'runtime'/f'{seat}.json').read_text())
                    logical=record.get('appServer',{}).get('threadId') or record.get('session_id')
                    matches=[r for r in view['runtime_sessions'] if r.get('session_id')==logical and r.get('tmux_pane')==record.get('paneId')]
                    activity=matches[0] if len(matches)==1 and not view.get('degraded') else {}
                    samples[seat]={'message_id':mid,'activity':activity,
                        'journal':[r for r in journal if r.get('payload',{}).get('message_id')==mid],
                        'pending':pending(journal,mid,seat,activity),'at':time.time()}
                admitted=[seat for seat,sample in samples.items() if sample['pending']]
                if admitted:
                    # If a seat already settled, retain its unproved timing and
                    # cross for the remaining backlog rather than wait for a receipt.
                    break
                if time.monotonic()>=deadline:
                    (OUT/(label+'-pending-samples.json')).write_text(json.dumps(clean(samples),indent=2))
                    raise AssertionError('backlog unproved: neither seat accepted, unexposed and working')
                time.sleep(.5)
            for seat,sample in samples.items():
                (OUT/(label+'-'+seat+'-pending.json')).write_text(json.dumps(clean(sample),indent=2))
            coverage={'pending_seats':admitted,'unproved':[{'seat':seat,'classification':'harness timing',
                'reason':'not pending at boundary sample'} for seat in samples if seat not in admitted]}
            (OUT/(label+'-coverage.json')).write_text(json.dumps(coverage,indent=2))
            log('pending_boundary_sample',label=label,samples=samples,coverage=coverage)
            if boundary=='restart_daemon':
                (OUT/'step2-outcome.json').write_text(json.dumps({'step':2,'outcome':'PASS','classification':'runtime',
                    'at':time.time(),'coverage':coverage}))
                step=3
                action={'op':'restart_daemon','save':action['save']}
            else:
                assert step==5
                action={'op':'mesh','argv':['team-daemon','restart-self','--claude-dir',str(ROOT/'claude'),
                    '--team',TEAM,'--name','lead'],'save':action['save']}
                log('normal_mesh_restart_initiated',at_boundary=time.time())

        if action["op"] == "fail":
            raise RuntimeError(action["reason"])
        if action["op"] == "finish":
            exit_code = 0
            break
        if action["op"] == "step":
            step = action["step"]
        elif action["op"] == "snapshot":
            snapshot()
            (OUT / "identities.json").write_text(json.dumps(clean(identities()),indent=2))
        elif action["op"] == "rpc_expected_error":
            value = rpc(action["method"], action["params"], allow_error=True)
            (OUT / action["save"]).write_text(json.dumps(value, indent=2))
            assert "error" in value and action["contains"] in str(value["error"]), value
        elif action["op"] == "restart_daemon":
            assert step == 3, "daemon restart only permitted in step 3"
            old = [i for i in identities() if i["argv"][0] == str(BIN / "taurhaus-daemon")]
            assert len(old) == 1, "expected exactly one owned daemon"
            identity = old[0]
            log("normal_daemon_stop", identity=identity, signal="SIGINT (installed ctrlc shutdown handler)")
            os.kill(identity["pid"], signal.SIGINT)
            for _ in range(650):
                if not any(i["pid"] == identity["pid"] and i["start_ticks"] == identity["start_ticks"] for i in identities()): break
                time.sleep(.1)
            else: raise AssertionError("normal daemon shutdown did not complete")
            log("post_daemon_stop", identities=identities())
            restart = shlex.join(daemon_argv) + " >>" + shlex.quote(str(ROOT / "daemon-restart.log")) + " 2>&1"
            run(["tmux", "new-window", "-d", "-t", "taurhaus", "/bin/bash -c " + shlex.quote(restart)])
            for _ in range(650):
                try:
                    rpc("ping", {}); break
                except (ConnectionRefusedError, FileNotFoundError): time.sleep(.1)
            else: raise AssertionError("restarted daemon did not become ready")
            log("post_daemon_restart", identities=identities(), ping=rpc("ping", {}))
            probe = rpc("coordination.hosted_transcript", {"team_name":TEAM,"member_name":MEMBER}, allow_error=True)
            if host_needs_resume(probe):
                value = operation("coordination.resume_member", {"request":{"team_name":TEAM,"member_name":MEMBER}, "cli_commands":commands, "tmux_layout":"new_window"})
                assert value["outcome"]["status"] == "completed" and not value["outcome"]["report"].get("failed_step"), value
            else:
                value = {"recovery": "host transcript readable without resume", "probe": probe}
            (OUT / action["save"]).write_text(json.dumps(clean(value), indent=2))
            previous_host_events = []
            poll_host(force=True)
            (OUT / "step3-identities.json").write_text(json.dumps(clean(identities()), indent=2))
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
            retry_deadline=time.monotonic()+65
            attempt=0
            while True:
                seq = secrets.token_hex(4)
                output = ROOT / (seq + ".out"); status = ROOT / (seq + ".exit")
                command = shlex.join([str(BIN / "mesh")] + action["argv"])
                command += " >" + shlex.quote(str(output)) + " 2>&1; echo $? >" + shlex.quote(str(status))
                run(["tmux", "new-window", "-d", "-t", "taurhaus", "/bin/bash -c " + shlex.quote(command)])
                for _ in range(1200):
                    if status.exists(): break
                    time.sleep(.1)
                assert status.exists(), "mesh command timeout"
                if not retry_busy(int(status.read_text()),output.read_text(),time.monotonic(),retry_deadline):break
                attempt+=1
                log('transient_refusal',method='mesh',attempt=attempt,error=output.read_text())
                time.sleep(.5)
            log("mesh_result", exit=int(status.read_text()), output=output.read_text())
            (OUT / action["save"]).write_text(clean(output.read_text()))
            (OUT / (action["save"] + ".exit.json")).write_text(json.dumps({"exit": int(status.read_text())}))
        else:
            raise ValueError(action)
        snapshot()
        log("action_done", op=action["op"])

except BaseException as error:
    log("stopped", step=step, error=str(error), type=type(error).__name__)
    if not setup_complete:
        try:
            setup_postmortem()
        except Exception as diagnostic_error:
            log("postmortem_error", error=type(diagnostic_error).__name__)
    traceback.print_exc()
finally:
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
    deadline=time.monotonic()+60
    while identities() and time.monotonic()<deadline: time.sleep(.2)
    survivors = identities()
    owner_stop.set()
    if owner_thread: owner_thread.join(timeout=5)
    snapshot()
    # Final accounting includes both seats before removal.
    try: budget_check()
    except Exception as error: log("final_budget_error",error=str(error))
    # Keep only non-secret rollout events relevant to identity and spend.
    with socket.socket() as probe:
        probe.settimeout(.2)
        port_closed = probe.connect_ex(("127.0.0.1", PORT)) != 0 if "PORT" in globals() else True
    cleanup = {"survivors": survivors, "port_closed": port_closed, "root": str(ROOT)}
    (ROOT / "codex/auth.json").unlink(missing_ok=True)
    cleanup["auth_copy_removed_before_root"] = not (ROOT / "codex/auth.json").exists()
    shutil.rmtree(ROOT)
    cleanup["root_removed"] = not ROOT.exists()
    cleanup["auth_removed"] = not (ROOT / "codex/auth.json").exists()
    (OUT / "cleanup.json").write_text(json.dumps(cleanup, indent=2))
    log("cleanup", **cleanup)
    EVENTS.close()
    # Sanitize textual logs before retaining them; auth contents were never logged.
    for path in OUT.rglob("*"):
        if path.is_file():
            text = path.read_text()
            if path.suffix == ".json":
                try: value = json.loads(text)
                except ValueError: value = {"raw_stdout": text}
                text = json.dumps(clean(value), indent=2) + "\n"
            elif path.suffix == ".jsonl":
                text = "\n".join(json.dumps(clean(json.loads(line))) for line in text.splitlines() if line) + "\n"
            else:
                text = clean(text)
            path.write_text(text)

    if survivors or not port_closed:
        exit_code = 2
sys.exit(exit_code)
