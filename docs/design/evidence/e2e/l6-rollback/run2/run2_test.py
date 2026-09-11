"""Synthetic evidence only; no subprocesses, CLIs, or credential access."""
import sys
import json
import hashlib
import tempfile
import threading
from unittest.mock import Mock, patch
import unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from run2_rules import pending as pending_observation

class Run2Tests(unittest.TestCase):
    def setUp(self):
        self.accepted={'event_type':'message_accepted','committed_at':'2026-09-10T00:00:02Z','payload':{'message_id':'B'}}
        self.activity={'activity_confidence':'likely_working','observed_at':'2026-09-10T00:00:02Z'}

    def test_pending_row_is_never_scheduler_opportunity(self):
        # // Regression: 82e2655b accepted an invented stage:pending row as evidence.
        fake={'event_type':'receipt','payload':{'message_id':'B','stage':'pending'}}
        self.assertIsNone(pending_observation([self.accepted,fake],'B',{},activity=self.activity,now=1788998403))

    def test_pending_obligation_can_prove_scheduler_opportunity(self):
        from run2_rules import pending
        obligation={'obligation':{'message_id':'B','recipient':'alpha'},'since':'2026-09-10T00:00:02Z'}
        result=pending([self.accepted],'B',{},activity=self.activity,now=1788998403,obligations=[obligation])
        self.assertEqual(result['scheduler_opportunity']['obligation'],obligation)
        for wrong in ('other',None):
            obligation['obligation']['message_id']=wrong
            self.assertIsNone(pending([self.accepted],'B',{},activity=self.activity,now=1788998403,obligations=[obligation]))

    def test_heartbeat_compared_as_time_and_working_required(self):
        from run2_rules import pending
        health={'heartbeat':'2026-09-10T00:00:03+00:00'}
        self.assertIsNotNone(pending([self.accepted],'B',health,activity=self.activity,now=1788998403))
        self.assertIsNone(pending([self.accepted],'B',health,activity={**self.activity,'activity_confidence':'idle'},now=1788998403))
        # // Regression: 955df28f tested a receipt shape the journal never emits.
        begun={'event_type':'delivery_attempt','payload':{'message_id':'B','stage':'attempt_started'}}
        self.assertIsNone(pending([self.accepted,begun],'B',health,activity=self.activity,now=1788998403))

    def test_begun_stages_excluded_independent_of_event_type(self):
        # // Regression: 955df28f ignored delivery_attempt rows after durable claim.
        health={'heartbeat':'2026-09-10T00:00:03Z'}
        for stage in ('attempt_started','outcome_unknown','submitted','native_enqueued','consumed'):
            for event_type in ('delivery_attempt','receipt','other'):
                row={'event_type':event_type,'payload':{'message_id':'B','stage':stage}}
                with self.subTest(stage=stage,event_type=event_type):
                    self.assertIsNone(pending_observation([self.accepted,row],'B',health,activity=self.activity,now=1788998403))
                    row['payload']['message_id']='other'
                    self.assertIsNotNone(pending_observation([self.accepted,row],'B',health,activity=self.activity,now=1788998403))

    def test_owner_lock_refusal_stops_before_step2_pass(self):
        # // Regression: 955df28f called a lifetime owner-lock refusal temporary.
        from run2controller import Trial
        trial=Mock(spec=Trial)
        trial.send_marker.side_effect=[('A','A'),('B','B')]
        trial.record.return_value={'attachmentGeneration':1}
        trial.delivery_rows.return_value=[self.accepted]
        trial.config.return_value={'messaging_format':2}
        trial.mesh_raw.return_value=(1,'error: IO error: delivery: team owner already holds lifetime lock')
        trial.wait.side_effect=RuntimeError('step 3 reached')
        with self.assertRaisesRegex(AssertionError,'permanent format refusal'):
            Trial.remaining_steps(trial)
        trial.pass_step.assert_called_once()
        trial.wait.assert_not_called()

    def test_spec_disposable_source_and_explicit_home_authorization(self):
        # // Regression: 82e2655b pinned the primary home instead of the binding spec source.
        from preflight import AUTHORIZED_AUTH_SOURCE, credential_source, PreflightUnavailable
        self.assertEqual(Path(AUTHORIZED_AUTH_SOURCE).parent.name,'.codex-account-b')
        named='/home/example/.codex-account-b/auth.json'
        with patch.object(Path,'is_symlink',return_value=False), patch.object(Path,'is_file',return_value=True), patch.object(Path,'read_bytes',side_effect=AssertionError('credential read')):
            self.assertEqual(credential_source(named,authorized_sources=(named,)),Path(named))
            with self.assertRaises(PreflightUnavailable):
                credential_source('/home/example/.codex/auth.json',authorized_sources=(named,))

    def test_auth_copy_records_label_and_digest_without_contents(self):
        # // Regression: 955df28f exported no credential source identity.
        from run2controller import Trial
        with tempfile.TemporaryDirectory() as root:
            trial=Trial.__new__(Trial)
            trial.root=Path(root)
            trial.candidate_preflight=Mock()
            trial.log=Mock()
            source=Path(root)/'disposable-account/auth.json'
            source.parent.mkdir()
            source.write_bytes(b'synthetic-test-only')
            with patch('run2controller.shutil.which',side_effect=RuntimeError('stop before CLI lookup')):
                with self.assertRaisesRegex(RuntimeError,'stop before CLI lookup'):
                    trial.boot(str(source))
            fields=trial.log.call_args.kwargs
            self.assertEqual(fields['source_label'],'disposable-account/auth.json')
            self.assertEqual(fields['source_sha256'],hashlib.sha256(b'synthetic-test-only').hexdigest())
            self.assertNotIn('synthetic-test-only',json.dumps(fields))
            self.assertEqual((trial.root/'codex/auth.json').stat().st_mode & 0o777,0o600)

    def test_event_write_holds_lock(self):
        # // Regression: 955df28f shared an unsynchronized event stream with the observer.
        from run2controller import Trial
        trial=Trial.__new__(Trial)
        trial.evidence_lock=threading.RLock()
        trial.events=Mock()
        def write(row):
            self.assertTrue(trial.evidence_lock._is_owned())
            self.assertEqual(json.loads(row)['kind'],'probe')
        trial.events.write.side_effect=write
        trial.log('probe')

    def test_format_boundary_requires_exact_format_and_retained_history(self):
        from run2_rules import format_boundary
        config={'messaging_format':1,'messaging_downgrade_sha256':'digest'}
        marker={'transition':'complete','legacy_cut':'digest'}
        format_boundary(config,marker,[self.accepted],[self.accepted],True)
        for fmt,after,verified in ((2,[self.accepted],True),(0,[self.accepted],True),(1,[],True),(1,[self.accepted],False)):
            with self.assertRaises(AssertionError):format_boundary({**config,'messaging_format':fmt},marker,[self.accepted],after,verified)

    def test_reply_requires_assistant_message_not_tool_echo(self):
        from run2_rules import assistant_reply
        rows=[{'timestamp':'2026-09-10T00:00:03Z','type':'response_item','payload':{'type':'function_call_output','output':'C-marker'}}]
        self.assertIsNone(assistant_reply(rows,'C-marker','2026-09-10T00:00:02Z'))
        rows.append({'timestamp':'2026-09-10T00:00:04Z','type':'response_item','payload':{'type':'message','role':'assistant','content':[{'type':'output_text','text':'C-marker'}]}})
        self.assertEqual(assistant_reply(rows,'C-marker','2026-09-10T00:00:02Z'),rows[-1])

if __name__=='__main__':unittest.main()
