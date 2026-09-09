"""Terminal contract checks use scratch roots and a fake subprocess only."""
import fcntl
import json
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import compaction_test_lib as lib


class TerminalExclusion(unittest.TestCase):
    def test_compaction_defers_then_retries_with_inherited_flock(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            terminal = root / "team/state/terminal"
            terminal.mkdir(parents=True)
            target = SimpleNamespace(teams_dir=root, team_name="team", member_name="seat", pane_id="%1", tmux_socket=root / "socket", attachment_generation=4)
            calls = []
            def fake(argv, **kwargs):
                self.assertEqual(argv[2], "--terminal-child")
                argv = argv[4:]
                calls.append(argv)
                self.assertEqual(argv[:3], ["tmux", "-S", str(target.tmux_socket)])
                self.assertNotIn("TMUX", kwargs["env"])
                self.assertIsNotNone(kwargs["stdin"])
                self.assertEqual(json.loads((terminal / "seat.holder.json").read_text())["op"], "compaction-test")
                with (terminal / "seat.lock").open("a") as other:
                    with self.assertRaises(BlockingIOError):
                        fcntl.flock(other, fcntl.LOCK_EX | fcntl.LOCK_NB)
                return subprocess.CompletedProcess(argv, 0, "", "")
            with (terminal / "seat.lock").open("a") as holder, patch.object(lib.subprocess, "run", side_effect=fake):
                fcntl.flock(holder, fcntl.LOCK_EX)
                with self.assertRaises(lib.CompactionTestError):
                    lib.tmux_send_literal(target, "text")
                self.assertEqual(calls, [])
                fcntl.flock(holder, fcntl.LOCK_UN)
                lib.tmux_send_literal(target, "text")
            self.assertEqual(len(calls), 2)
            self.assertFalse((terminal / "seat.holder.json").exists())
            self.assertTrue((terminal / "seat.lock").exists())


if __name__ == "__main__":
    unittest.main()
