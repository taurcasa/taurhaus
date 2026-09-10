"""Offline passive-sampler regression; synthetic proc tree, no live CLI or homes."""
from pathlib import Path
import tempfile
import unittest
from attempt8_passive import sample, complete_rows

class PassiveTests(unittest.TestCase):
    def test_inflight_log_row_does_not_abort_trial(self):
        # // Regression: 47b7b8c2 snapshot parsed an in-progress JSONL write as fatal.
        self.assertEqual(complete_rows('{"event":"ready"}\n{"event":'), [{'event':'ready'}])
        self.assertEqual(complete_rows('{"event":"ready"}\n'), [{'event':'ready'}])

    def test_reads_holder_without_intervention(self):
        # // Regression: 47b7b8c2 retained a SIGSTOP/SIGCONT probe that broke bounded host reads.
        with tempfile.TemporaryDirectory() as root:
            proc = Path(root)
            (proc/'locks').write_text('1: FLOCK ADVISORY WRITE 42 08:30:123 0 EOF\n')
            (proc/'42/fdinfo').mkdir(parents=True)
            (proc/'42/environ').write_bytes(b'TAURHAUS_TRIAL_ID=trial\0')
            (proc/'42/cmdline').write_bytes(b'/tmp/trial/mesh\0')
            (proc/'42/stat').write_text('42 (mesh) ' + ' '.join(['0']*19+['777']))
            (proc/'42/fdinfo/3').write_text('ino:\t123\nlock:\t1: FLOCK ADVISORY WRITE 42 08:30:123 0 EOF\n')
            result = sample(proc, 123, 'trial')
            self.assertEqual(result[0]['pid'], 42)
            self.assertEqual(result[0]['start_ticks'], '777')
            self.assertIn('lock:', result[0]['fdinfo'][0])
            self.assertEqual(sample(proc, 124, 'trial'), [])
            self.assertEqual(sample(proc, 123, 'foreign'), [])

if __name__ == '__main__': unittest.main()
