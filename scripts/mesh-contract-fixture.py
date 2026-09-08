"""Generate contract inputs with the locked Mesh executable, in an empty root.

No task/config/workflow JSON is authored here. The clock shim advances only
Mesh's realtime clock, allowing bounded monitor cycles without a ten-minute
cooldown. It neither changes the host clock nor accelerates polling.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

binary, directory, scenario = sys.argv[1:]
root = Path(directory).resolve()
root.mkdir(parents=True, exist_ok=True)
env = {"HOME": str(root), "PATH": "/usr/bin:/bin", "MESH_TEAM": "deadline-team",
       "MESH_NAME": "team-lead", "TMUX_TMPDIR": str(root / "tmux")}


def command(*args):
    return [binary, "--claude-dir", str(root), *args]


def run(*args):
    result = subprocess.run(command(*args), cwd=root, env=env,
                            capture_output=True, text=True, timeout=10, check=True)
    return result.stdout


lock = json.loads((Path(__file__).resolve().parents[1] /
                   "src-tauri/resources/mesh.lock.json").read_text())
version = json.loads(run("version", "--json"))
assert all(version[key] == value for key, value in lock.items()), "Mesh lock mismatch"
run("join", "--team", "deadline-team", "--name", "team-lead", "--type", "lead")
run("join", "--team", "deadline-team", "--name", "builder")
task = json.loads(run("task", "create", "--subject", "Contract fixture", "--json",
                      "--first-step", "Inspect scratch inputs", "--deliverable", "Scratch result",
                      "--completion-signal", "Report result", "--deadline", "20"))
task_id = task["id"]
(root / "task-id").write_text(task_id)
path = root / "tasks/deadline-team" / f"{task_id}.json"
run("task", "assign", task_id, "--owner", "builder", "--status", "in_progress", "--awaiting-go")
shutil.copyfile(path, root / "awaiting.json")
run("task", "update", task_id, "--go")
shutil.copyfile(path, root / "go.json")
shutil.copyfile(root / "teams/deadline-team/config.json", root / "go-config.json")

if scenario == "monitor":
    source = root / "clock.c"
    source.write_text('''#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdlib.h>
#include <time.h>
int clock_gettime(clockid_t id, struct timespec *ts) {
    int (*real_clock)(clockid_t, struct timespec *) = dlsym(RTLD_NEXT, "clock_gettime");
    int rc = real_clock(id, ts);
    const char *offset = getenv("MESH_FIXTURE_CLOCK_OFFSET");
    if (rc == 0 && id == CLOCK_REALTIME && offset) ts->tv_sec += atol(offset);
    return rc;
}
''')
    library = root / "clock.so"
    subprocess.run(["/usr/bin/cc", "-shared", "-fPIC", str(source), "-ldl", "-o", str(library)],
                   env=env, cwd=root, check=True, capture_output=True, timeout=10)
    env["LD_PRELOAD"] = str(library)
    # First cycle establishes uncertainty; later cycles cross heartbeat,
    # uncertainty and reminder windows. Each restart polls immediately once.
    for offset in [0, 240, 900, 900, 960, 960]:
        env["MESH_FIXTURE_CLOCK_OFFSET"] = str(offset)
        with (root / "monitor.log").open("ab") as log:
            process = subprocess.Popen(command("team-daemon", "start"), env=env,
                                       cwd=root, stdout=log, stderr=log)
            try:
                time.sleep(0.25)
                assert process.poll() is None, "scratch monitor exited early"
            finally:
                if process.poll() is None:
                    process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
    records = json.loads(path.read_text())["metadata"].get("idle_monitor_records", [])
    assert [record["kind"] for record in records] == ["nudge", "nudge", "launch_health"], records

elif scenario == "wait":
    run("task", "assign", task_id, "--owner", "builder", "--status", "in_progress",
        "--awaiting-go", "--admin-reason", "Exercise a new wait")
    shutil.copyfile(path, root / "new-wait.json")
    run("task", "assign", task_id, "--owner", "builder", "--status", "in_progress",
        "--admin-reason", "Exercise reassignment")
    shutil.copyfile(path, root / "reassigned.json")
    run("status", "set", "--name", "builder", "--state", "blocked", "--reason", "Awaiting candidate")
    shutil.copyfile(root / "teams/deadline-team/config.json", root / "blocked-config.json")

elif scenario == "ruling":
    run("task", "ruling", task_id, "--kind", "ruling", "--field", "oversize_diff", "--value", "failed")
    run("task", "ruling", task_id, "--kind", "ruling", "--field", "budget_raised", "--value", "approved")

else:
    raise ValueError(scenario)
