"""Read-only audit of retained attempt 2 evidence and owned-process cleanup."""
import json, re, socket, subprocess
from pathlib import Path
OUT = Path(__file__).parent / "attempt2"
RUN = OUT / "run"
identities = json.loads((RUN / "identities.json").read_text())
live = []
for identity in identities:
    try:
        stat = Path(f"/proc/{identity['pid']}/stat").read_text().rsplit(")", 1)[1].split()
        if stat[19] == identity["start_ticks"]: live.append(identity)
    except (FileNotFoundError, ProcessLookupError): pass
cleanup = json.loads((RUN / "cleanup.json").read_text())
root = Path(cleanup["root"])
rows = [json.loads(line) for line in (RUN / "events.jsonl").read_text().splitlines()]
isolation = next(row for row in rows if row["kind"] == "isolation")
port = int(isolation["environment"]["TAURHAUS_DAEMON_PORT"])
with socket.socket() as probe:
    probe.settimeout(.2)
    port_closed = probe.connect_ex(("127.0.0.1", port)) != 0
with socket.socket(socket.AF_UNIX) as probe:
    probe.settimeout(.2)
    socket_closed = probe.connect_ex(str(root / "tmux/tmux-1000/default")) != 0
hygiene = []
for path in OUT.rglob("*"):
    if not path.is_file(): continue
    text = path.read_text()
    for value in re.findall(r'/home/mstie[^\s"\\]*', text):
        if not value.startswith(("/home/mstie/projects/taurhaus-trial", "/home/mstie/projects/mesh-push")):
            hygiene.append(str(path))
    if re.search(r'(?:sk-[A-Za-z0-9_-]{16,}|Bearer [A-Za-z0-9._-]{16,}|"(?:access_token|refresh_token|id_token)"\s*:)', text):
        hygiene.append(str(path))
mesh_status = subprocess.check_output(["git", "-C", "/home/mstie/projects/mesh-push", "status", "--porcelain"], text=True)
report = json.loads((RUN / "initialize-result.json").read_text())["outcome"]["report"]
steps = report["succeeded_steps"]
resume = json.loads((RUN / "resume-result.json").read_text())["outcome"]["report"]
gates = {name:json.loads((OUT / "gates" / f"gate-{name}.json").read_text())["exit"]
    for name in ["check-quick", "lint", "test-contracts"]}
gate_isolation = json.loads((OUT / "gates/gate-isolation.json").read_text())
gate_root = Path(gate_isolation["environment"]["HOME"]).parent
assert not live and port_closed and socket_closed and not root.exists()
assert not hygiene and not mesh_status and not gate_root.exists()
assert report["failed_step"] is None and steps.index("launch_sessions") < steps.index("opt_in_delivery")
assert resume["failed_step"] == "load_member" and not resume["resumed"]
assert not any(gates.values())
result = {"owned_identities_checked":len(identities), "surviving_owned_identities":live,
    "root_removed":not root.exists(), "auth_removed":not (root / "codex/auth.json").exists(),
    "private_port":port, "port_closed":port_closed, "tmux_socket_closed":socket_closed,
    "mesh_worktree_clean":not mesh_status, "gate_root_removed":not gate_root.exists(),
    "evidence_hygiene_issues":hygiene, "initialize_passed":True,
    "launch_before_opt_in":True, "step_1":"inconclusive: online-seat resume refused before hosted launch",
    "steps_2_through_7":"not run", "gates":gates, "product_files_changed":[],
    "model_turns_observed":0, "model_spend_observed_usd":0}
(OUT / "final-verification.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result))
