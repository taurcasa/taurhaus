"""Pinned checkout-local builds with bounded Cargo exclusion; no installation."""
import json, os, signal, subprocess, time
from pathlib import Path
ROOT = Path.cwd()
OUT = Path(__file__).parent / os.environ.get("TRIAL_EVIDENCE_LABEL", "attempt11")
OUT.mkdir(exist_ok=True)
MESH = Path("/home/mstie/projects/mesh-trial")
CAP = MESH / "src/delivery/app_server/capabilities.rs"
original = CAP.read_text()
assert subprocess.check_output(["git", "-C", str(MESH), "status", "--porcelain", "--", str(CAP)], text=True) == ""
assert subprocess.check_output(["git", "branch", "--show-current"], text=True).strip() == "feat/integration-trial"
child = None
def interrupted(signum, frame):
    raise RuntimeError(f"interrupted {signum}")
for sig in [signal.SIGTERM, signal.SIGINT]: signal.signal(sig, interrupted)
def build(name, cwd, command):
    global child
    deadline = time.monotonic() + 1800
    probes = []
    while True:
        p = subprocess.run(["pgrep", "-af", "(^|/)cargo( |$)"], capture_output=True, text=True)
        probes.append({"at":time.time(), "exit":p.returncode, "output":p.stdout})
        if p.returncode == 1: break
        if time.monotonic() >= deadline: raise TimeoutError("Cargo exclusion exceeded 30 minutes")
        time.sleep(30)
    env = dict(os.environ, CARGO_TARGET_DIR=str(cwd / "target"))
    with (OUT / f"{name}.log").open("w") as stream:
        child = subprocess.Popen(command, cwd=cwd, env=env, stdout=stream, stderr=subprocess.STDOUT, start_new_session=True)
        code = child.wait(timeout=1800)
    data = {"command":command, "cwd":str(cwd), "target":env["CARGO_TARGET_DIR"], "exit":code, "cargo_preflight":probes}
    (OUT / f"{name}.json").write_text(json.dumps(data, indent=2))
    print(json.dumps(data), flush=True)
    if code: raise RuntimeError(f"{name} exit {code}")
try:
    build("daemon-build", ROOT / "src-tauri", ["cargo", "build", "--bin", "taurhaus-daemon"])
    old = 'disposition: "disabled", enabled: false, strongest_receipt: "native_enqueued",'
    new = 'disposition: if build == "0.153.4" { "trial" } else { "disabled" }, enabled: build == "0.153.4", strongest_receipt: "native_enqueued",'
    assert original.count(old) == 1
    trial = original.replace(old, new)
    trial = trial.replace('host: "taurhaus-owned persistent thread; UNVERIFIED",', 'host: if build == "0.153.4" { "taurhaus-daemon-owned-thread/1" } else { "taurhaus-owned persistent thread; UNVERIFIED" },')
    trial = trial.replace('transport: TRANSPORT, configuration: "UNVERIFIED", trust: "UNVERIFIED",', 'transport: TRANSPORT, configuration: if build == "0.153.4" { "strict-config/1" } else { "UNVERIFIED" }, trust: if build == "0.153.4" { "daemon-owned/1" } else { "UNVERIFIED" },')
    hosted = (ROOT / "src-tauri/src/coordination/hosted.rs").read_text()
    for identity in ["taurhaus-daemon-owned-thread/1", "strict-config/1", "daemon-owned/1"]:
        assert identity in hosted
    CAP.write_text(trial)
    (OUT / "mesh-trial-descriptor.diff").write_text(subprocess.check_output(["git", "-C", str(MESH), "diff", "--", str(CAP)], text=True))
    build("mesh-build", MESH, ["cargo", "build", "--bin", "mesh"])
except BaseException:
    CAP.write_text(original)
    raise
finally:
    if child is not None and child.poll() is None:
        os.killpg(child.pid, signal.SIGTERM)
        try: child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL); child.wait()
