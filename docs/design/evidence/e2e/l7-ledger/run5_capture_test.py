"""Offline privacy contract for the fifth trial's additional native evidence."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import gates
import run5_capture
from run5_capture import public_row


class CaptureRules(unittest.TestCase):
    def test_native_evidence_keeps_identity_without_private_text(self):
        row = {'timestamp': 'fixture-time', 'type': 'event_msg', 'payload': {
            'type': 'task_complete', 'turn_id': 'fixture-turn',
            'last_agent_message': 'private fixture body'}}
        result = public_row(row)
        self.assertEqual(result['payload'], {'type': 'task_complete', 'turn_id': 'fixture-turn'})
        self.assertEqual(result['timestamp'], 'fixture-time')
        self.assertNotIn('private fixture body', str(result))
        self.assertEqual(len(result['source_sha256']), 64)

    def test_notify_hyphenated_identity_is_retained(self):
        result = public_row({'type': 'agent-turn-complete', 'turn-id': 'turn',
                             'thread-id': 'thread', 'input-messages': ['private'],
                             'last-assistant-message': 'private'})
        self.assertEqual(result['turn-id'], 'turn')
        self.assertEqual(result['thread-id'], 'thread')
        self.assertNotIn('private', str(result))


    # // Regression: b46b0271 omitted source identity and wrote live captures in place.
    def test_capture_binds_tail_to_one_source_read(self):
        with tempfile.TemporaryDirectory(prefix='th-l7-') as temporary:
            root = Path(temporary)
            out = root / 'evidence/run5'
            out.mkdir(parents=True)
            (out / 'events.jsonl').write_text(json.dumps({'kind': 'isolation', 'root': str(root)}) + '\n')
            sessions = root / 'codex/sessions'
            sessions.mkdir(parents=True)
            source = sessions / 'rollout-fixture.jsonl'
            raw = b'{"type":"event_msg","payload":{"type":"task_complete","turn_id":"fixture"}}\n'
            source.write_bytes(raw)
            with patch.object(run5_capture, 'BASE', out.parent):
                run5_capture.capture()
            result = json.loads((out / 'rollout-tail.json').read_text())[0]
            self.assertEqual(result['sha256'], hashlib.sha256(raw).hexdigest())
            self.assertEqual(result['bytes'], len(raw))
            self.assertRegex(result['captured_at'], r'^\d{4}-\d{2}-\d{2}T.*\+00:00$')
            self.assertEqual(result['complete_rows'], 1)
            self.assertEqual(result['tail'][0]['payload']['turn_id'], 'fixture')

    def test_interrupted_capture_preserves_published_files(self):
        with tempfile.TemporaryDirectory(prefix='th-l7-') as temporary:
            root = Path(temporary)
            out = root / 'evidence/run5'
            out.mkdir(parents=True)
            (out / 'events.jsonl').write_text(json.dumps({'kind': 'isolation', 'root': str(root)}) + '\n')
            (root / 'data').mkdir()
            (root / 'data/codex-notify.jsonl').write_text('{"type":"agent-turn-complete"}\n')
            original = Path.write_text
            for name in ('notify-records.jsonl', 'rollout-tail.json'):
                with self.subTest(name=name):
                    destination = out / name
                    destination.write_text('previous complete evidence')
                    def interrupted(path, text, *args, **kwargs):
                        if path.name.startswith(name):
                            original(path, text[:4])
                            raise OSError('fixture interrupted write')
                        return original(path, text, *args, **kwargs)
                    with patch.object(run5_capture, 'BASE', out.parent), patch.object(Path, 'write_text', interrupted):
                        with self.assertRaises(OSError):
                            run5_capture.capture()
                    self.assertEqual(destination.read_text(), 'previous complete evidence')

    # // Regression: a63562e6 claimed a fixed base using ancestry only; 26c06132 reverted #176.
    def test_preflight_rejects_reverted_or_dirty_fixed_content(self):
        for head, working in [('old', 'old'), ('fixed', 'old')]:
            with self.subTest(head=head, working=working), patch.object(
                    gates.subprocess, 'check_output', side_effect=['fixed', head, working]):
                result = gates.base_content_check(Path('/fixture'))
                self.assertFalse(result['passed'])

    def test_preflight_accepts_fixed_head_and_working_content(self):
        with patch.object(gates.subprocess, 'check_output', side_effect=['fixed'] * 3):
            self.assertTrue(gates.base_content_check(Path('/fixture'))['passed'])

    # // Regression: a63562e6 used an unretained wrapper for run5 gate output/cleanup.
    def test_gate_driver_reproduces_named_run_and_output_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            base = root / 'evidence'
            run = base / 'run5'
            run.mkdir(parents=True)
            (run / 'cleanup.json').write_text(json.dumps(dict(auth_removed=True,
                root_removed=True, survivors=[], port_closed=True)))
            output = root / 'review'
            logs = root / 'logs'
            seen = []
            def command(argv, cwd, label):
                seen.append((argv, gates.OUT, gates.LOGS))
                return {'command': ' '.join(argv), 'exit': 0}
            with patch.object(gates, 'BASE', base, create=True), patch.object(gates, 'ROOT', root), patch.object(gates, 'OUT', base), patch.object(gates, 'LOGS', logs), patch.object(Path, 'cwd', return_value=root), patch.object(gates, 'command', side_effect=command):
                self.assertEqual(gates.main(['--run-name', 'run5', '--log-dir', str(logs), '--output-dir', str(output)]), 0)
            self.assertEqual([row[0] for row in seen], [['just', 'check-quick'], ['just', 'lint'], ['just', 'test-contracts']])
            self.assertTrue(all(row[1:] == (output, logs) for row in seen))
            self.assertEqual(len(json.loads((output / 'checks-result.json').read_text())['commands']), 3)
            (run / 'cleanup.json').write_text(json.dumps(dict(auth_removed=False,
                root_removed=True, survivors=[], port_closed=True)))
            with patch.object(gates, 'BASE', base, create=True), patch.object(gates, 'ROOT', root), patch.object(gates, 'OUT', base), patch.object(gates, 'LOGS', logs), patch.object(Path, 'cwd', return_value=root), patch.object(gates, 'command') as called:
                with self.assertRaises(AssertionError):
                    gates.main(['--run-name', 'run5'])
                called.assert_not_called()


if __name__ == '__main__':
    unittest.main()
