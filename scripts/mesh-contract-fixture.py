"""Generate contract inputs with the locked Mesh executable, in an empty root.

No task/config/workflow JSON is authored here. The clock shim advances only
Mesh's realtime clock, allowing bounded monitor cycles without a ten-minute
cooldown. It neither changes the host clock nor accelerates polling.
"""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import time


def wait_for_cycle(process, ready, description):
    deadline = time.monotonic() + 30
    while True:
        status = process.poll()
        assert status is None, f"scratch monitor exited ({status}): {description}"
        try:
            if ready():
                return
        except (FileNotFoundError, json.JSONDecodeError):
            # The producer may be creating/replacing the file while we poll.
            pass
        if time.monotonic() >= deadline:
            raise TimeoutError(f"scratch monitor timed out: {description}")
        time.sleep(0.05)


def self_test():
    import unittest
    from unittest.mock import Mock, patch

    # // Regression: 22d0fc03 terminated the monitor after 250 ms, before
    # slow cycles persisted launch_health. Exercise waits without any CLI.
    class MonitorWaitTests(unittest.TestCase):
        def test_waits_for_delayed_cycle_completion(self):
            process = Mock()
            process.poll.return_value = None
            clock = [0.0]
            def sleep(seconds):
                clock[0] += seconds
            with patch.object(time, "monotonic", side_effect=lambda: clock[0]), \
                    patch.object(time, "sleep", side_effect=sleep):
                wait_for_cycle(process, lambda: clock[0] >= 1.0, "delayed health")
            self.assertGreaterEqual(clock[0], 1.0)
            self.assertLess(clock[0], 2.0)

        def test_times_out_with_cycle_context(self):
            process = Mock()
            process.poll.return_value = None
            with patch.object(time, "monotonic", side_effect=[0.0, 31.0]), \
                    self.assertRaisesRegex(TimeoutError, "missing health"):
                wait_for_cycle(process, lambda: False, "missing health")

        def test_reports_early_exit(self):
            process = Mock()
            process.poll.return_value = 7
            with self.assertRaisesRegex(AssertionError, "exited.*7"):
                wait_for_cycle(process, lambda: False, "health")

        def test_retries_partial_file_writes(self):
            process = Mock()
            process.poll.return_value = None
            ready = Mock(side_effect=[FileNotFoundError(),
                                      json.JSONDecodeError("partial", "", 0), True])
            with patch.object(time, "sleep"):
                wait_for_cycle(process, ready, "health")
            self.assertEqual(ready.call_count, 3)

    result = unittest.TextTestRunner().run(
        unittest.defaultTestLoader.loadTestsFromTestCase(MonitorWaitTests))
    return 0 if result.wasSuccessful() else 1


if sys.argv[1:] == ["--self-test"]:
    sys.exit(self_test())

binary, directory, scenario = sys.argv[1:]
root = Path(directory).resolve()
root.mkdir(parents=True, exist_ok=True)
env = {"HOME": str(root), "PATH": "/usr/bin:/bin", "MESH_TEAM": "deadline-team",
       "MESH_NAME": "team-lead", "TMUX_TMPDIR": str(root / "tmux")}


def command(*args):
    return [binary, "--claude-dir", str(root), *args]


def run(*args):
    result = subprocess.run(command(*args), cwd=root, env=env,
                            capture_output=True, text=True, timeout=30, check=True)
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
                   env=env, cwd=root, check=True, capture_output=True, timeout=30)
    env["LD_PRELOAD"] = str(library)
    # First cycle establishes uncertainty; later cycles cross heartbeat,
    # uncertainty and reminder windows. Each restart polls immediately once.
    offsets = [0, 240, 900, 900, 960, 960]
    for cycle, offset in enumerate(offsets):
        env["MESH_FIXTURE_CLOCK_OFFSET"] = str(offset)
        log_path = root / f"monitor-{cycle}.log"
        with log_path.open("wb") as log:
            process = subprocess.Popen(command("team-daemon", "start"), env=env,
                                       cwd=root, stdout=log, stderr=log)
            try:
                def cycle_completed():
                    records = json.loads(path.read_text())["metadata"].get("idle_monitor_records", [])
                    if any(record["kind"] == "launch_health" for record in records):
                        return True
                    # The locked producer logs this only after run_cycle returns.
                    # Preparatory cycles can defer without producing a record;
                    # the final cycle must persist health, not merely return.
                    return (cycle < len(offsets) - 1 and
                            "[mesh team-daemon] idle-monitor:" in log_path.read_text())

                wait_for_cycle(process, cycle_completed,
                               f"cycle {cycle}, offset {offset}, expected launch_health by final cycle")
            finally:
                if process.poll() is None:
                    process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        if any(record["kind"] == "launch_health" for record in
               json.loads(path.read_text())["metadata"].get("idle_monitor_records", [])):
            break
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
