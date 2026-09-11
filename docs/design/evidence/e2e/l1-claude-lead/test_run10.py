"""Offline run10 regressions: generated transcript data, no native CLI or credentials."""
import json
import os
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest.mock import Mock, patch
import controller
import steps


def attachment():
    # Same native shape as run9/step5-native-hook-attachments.json, generated afresh.
    return {'type': 'attachment', 'attachment': {
        'type': 'hook_success', 'hookName': 'SessionStart:compact',
        'stdout': json.dumps({'hookSpecificOutput': {'additionalContext':
            '[taurhaus] recovery_card\nIdentity: lead on l1-claude-lead\n'
            'Current task: #1 — Run10 native mailbox recovery'}})}}


class Run10Tests(unittest.TestCase):
    def test_snapshot_retains_native_hook_attachment(self):
        # // Regression: 5ddff9fe retained only user/system/assistant and hid native recovery.
        with tempfile.TemporaryDirectory() as directory:
            trial = controller.Trial.__new__(controller.Trial)
            trial.root = Path(directory)
            trial.rows = Mock(side_effect=lambda pattern: [attachment()] if pattern.startswith('claude/') else [])
            saved = {}
            trial.save = lambda name, value: saved.update({name: value})
            trial.retain_logs = Mock(return_value=[])
            trial.identities = Mock(return_value=[])
            trial.ledger = Mock()
            trial.snapshot()
        self.assertEqual(saved['claude-transcript.json'], [attachment()])

    def check_recovery(self, row, expected=True, missing_event=False, unchanged=False):
        before = {'contextGeneration': '0', 'recovery': {'last_delivered': {
            'content_revision': 'old', 'card_key': {'contract': {'effective_role_revision': 'role'}}}}}
        after = {'contextGeneration': '0' if unchanged else '1', 'recovery': {'last_delivered': {
            'content_revision': 'new', 'card_key': {'contract': {'effective_role_revision': 'role'}}}}}
        events = [{'event': 'compaction.claude_hook.' + kind} for kind in
                  (('received', 'resolved') if missing_event else ('received', 'resolved', 'delivered'))]
        compacted = False
        def submit(*args, **kwargs):
            nonlocal compacted
            compacted = True
        def read(name):
            if name == 'latest/team/runtime/lead.json': return after if compacted else before
            if name == 'daemon-events.json': return events if compacted else []
            return None
        def rows():
            return [{'type': 'system', 'subtype': 'compact_boundary'}, row] if compacted else []
        with patch.object(steps, 'prepare_resumable_assignment', return_value={'task': {'subject': 'Run10 native mailbox recovery'}}), \
                patch.object(steps, 'read', side_effect=read), patch.object(steps, 'claude_rows', side_effect=rows), \
                patch.object(steps, 'send_input', side_effect=submit), patch.object(steps, 'snapshot'), \
                patch.object(steps, 'save'), patch.object(steps, 'action'), \
                patch.object(steps, 'wait', side_effect=lambda predicate, *a, **k: self.assertEqual(predicate(), expected)):
            steps.main(5)

    def test_step5_accepts_hook_success_as_native_context(self):
        # // Regression: 52de2ac4's user/system predicate rejected the real SessionStart attachment.
        self.check_recovery(attachment())

    def test_step5_still_requires_hook_generation_and_assignment(self):
        self.check_recovery(attachment(), False, missing_event=True)
        self.check_recovery(attachment(), False, unchanged=True)
        row = attachment()
        row['attachment']['stdout'] = 'unrelated attachment'
        self.check_recovery(row, False)

    def test_run10_driver_uses_fresh_label(self):
        with patch.dict(os.environ, {'TRIAL_EVIDENCE_LABEL': 'run10'}):
            driver = runpy.run_path(str(controller.BASE/'run3_driver.py'))
        self.assertEqual(driver['OUT'], controller.BASE/'run10')

    def test_step6_marks_fresh_mail_before_teardown(self):
        # // Regression: 5882ee3c's unexecuted step6 stopped without the required final read/mark.
        inputs = []
        with patch.object(steps, 'send_input', side_effect=lambda tool, pane, text, **k: inputs.append((tool, text))), \
                patch.object(steps, 'wait'), patch.object(steps, 'snapshot'), \
                patch.object(steps, 'journal_pages'), patch.object(steps, 'action'):
            steps.main(6)
        self.assertTrue(any(tool == 'claude' and 'mesh read --unread' in text and '--mark-read' in text
                            for tool, text in inputs))


if __name__ == '__main__': unittest.main()
