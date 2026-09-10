"""Synthetic TUI regression tests: no processes, credentials or real CLI."""
import unittest
import support


class SubmissionTests(unittest.TestCase):
    # // Regression: 31aac7d7 inherited immediate Enter without confirming the composer cleared.
    def exercise(self, required_enters=1, clears=True, starts=True, visible=True):
        clock=[0.0]; keys=[]; captures=[]
        def read():
            entered=keys.count('Enter')>=required_enters
            pane='› Print sixty lines' if visible else '› Ask Codex to do anything'
            if entered and clears:pane='• Working\n› Ask Codex to do anything'
            rows=[{'type':'event_msg','payload':{'type':'task_started','turn_id':'new'}}] if entered and starts else []
            captures.append((clock[0],pane))
            return pane,rows
        def sleep(seconds):clock[0]+=seconds
        result=support.confirm_submission('Print sixty lines',keys.append,read,lambda:clock[0],sleep,set())
        return result,keys,captures

    def test_confirms_only_after_visible_paste_and_new_turn(self):
        result,keys,captures=self.exercise()
        self.assertEqual(keys,['Print sixty lines','Enter'])
        self.assertIn('Print sixty lines',captures[0][1])
        self.assertTrue(result['composer_empty'])
        self.assertEqual(result['new_turn_ids'],['new'])

    def test_one_extra_enter_for_paste_that_remains_in_composer(self):
        result,keys,_=self.exercise(required_enters=2)
        self.assertEqual(keys,['Print sixty lines','Enter','Enter'])
        self.assertEqual(result['enter_count'],2)
        self.assertGreaterEqual(result['confirmed_at'],10)

    def test_never_confirms_a_turn_with_text_still_in_composer(self):
        with self.assertRaisesRegex(RuntimeError,'not confirmed'):
            self.exercise(clears=False)

    def test_empty_composer_without_new_turn_is_not_confirmed(self):
        with self.assertRaisesRegex(RuntimeError,'not confirmed'):
            self.exercise(starts=False)

    def test_no_enter_until_paste_is_visible(self):
        with self.assertRaisesRegex(RuntimeError,'not visible'):
            self.exercise(visible=False)

    def test_composer_parser_ignores_old_transcript_input(self):
        self.assertEqual(support.composer_text('› old input\nanswer\n› Ask Codex to do anything\nfooter'),'Ask Codex to do anything')

if __name__=='__main__':unittest.main()
