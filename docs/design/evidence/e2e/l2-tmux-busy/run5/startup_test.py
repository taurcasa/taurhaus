"""Run-4f synthetic startup regressions; no real home reads or CLI calls."""
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch
from controller import Trial
import support


class StartupTests(unittest.TestCase):
    def trial(self, directory, pane):
        trial=Trial.__new__(Trial)
        trial.out=Path(directory);trial.classification='harness'
        trial.record=Mock(return_value={'tmuxSocket':'/scratch/socket','paneId':'%2'})
        trial.run=Mock(side_effect=lambda *a, **kw:pane())
        trial.snapshot=Mock();trial.budget=Mock();trial.save=Mock()
        return trial

    def test_blank_startup_waits_120_seconds_and_is_environment(self):
        # // Regression: 7a5bf570 entered onboarding before Codex rendered in run 4e.
        with tempfile.TemporaryDirectory() as directory:
            trial=self.trial(directory,lambda:'')
            tick=[0]
            with patch('controller.time.monotonic',side_effect=lambda:tick[0]), patch('controller.time.sleep',side_effect=lambda n:tick.__setitem__(0,tick[0]+n)):
                with self.assertRaisesRegex(RuntimeError,'codex_startup_stall'):
                    trial.startup_preflight()
            self.assertEqual(tick[0],120)
            self.assertEqual(trial.classification,'environment')
            self.assertEqual(len(list(Path(directory).glob('startup-*-pane-2.txt'))),5)

    def test_delayed_composer_gets_separate_onboarding_window(self):
        # // Regression: 7a5bf570 charged startup stall to Taurhaus onboarding.
        with tempfile.TemporaryDirectory() as directory:
            tick=[0]
            trial=self.trial(directory,lambda:'› Ask Codex to do anything\n\ngpt-5.6-luna low' if tick[0]>=95 else '')
            with patch('controller.time.monotonic',side_effect=lambda:tick[0]), patch('controller.time.sleep',side_effect=lambda n:tick.__setitem__(0,tick[0]+n)):
                trial.startup_preflight()
            self.assertEqual(tick[0],95)
            self.assertEqual(trial.classification,'harness')

    def test_composer_requires_prompt_and_model_footer(self):
        for pane in ('','gpt-5.6-luna low','› Ask Codex to do anything'):
            self.assertFalse(support.startup_composer(pane))
        self.assertTrue(support.startup_composer('› \n\ngpt-5.6-luna low · 100% left'))

    def test_failure_logs_retained_with_secrets_and_root_removed(self):
        # // Regression: 7a5bf570 discarded scratch Codex log/ during failure cleanup.
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)/'th-l2-synthetic';log=root/'codex/log/nested';log.mkdir(parents=True)
            text='startup stalled '+str(root)+' th-l2-synthetic\nAuthorization: Bearer synthetic-secret\naccess_token="synthetic-access"\n'
            (log/'codex-tui.log').write_text(text)
            out=Path(directory)/'export'
            support.retain_codex_logs(root,out)
            kept=(out/'codex-log/nested/codex-tui.log').read_text()
            self.assertIn('startup stalled',kept)
            for forbidden in ('th-l2-synthetic','synthetic-secret','synthetic-access'):
                self.assertNotIn(forbidden,kept)


if __name__=='__main__':unittest.main()
