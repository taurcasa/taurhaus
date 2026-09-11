"""Offline fixtures only: no CLI, real credentials, or runtime homes."""
import unittest
from rollback import retry_transient, ownership_boundary, preserved_message

class RollbackTests(unittest.TestCase):
    def test_meter_counts_legacy_executor_input(self):
        from controller import Trial
        from unittest.mock import Mock
        trial=Trial.__new__(Trial)
        trial.sessions=lambda:[];trial.notify=lambda:[];trial.save=Mock()
        trial.reservations=[{'reason':'B-work','confirmed':True}]
        trial.journals=lambda:[{'payload':{'recipient':'alpha','stage':'submitted','attempt_id':'canonical'}}]
        trial.workflow=lambda:[{'eventType':'message_delivery_recorded','channel':'tmux','outcome':'tmux_injected','message_id':'legacy','target':'%2'}]
        trial.record=lambda:{'paneId':'%2'}
        self.assertEqual(trial.budget(observe_only=True)['paid_inputs'],3)

    def test_legacy_read_array_after_banner(self):
        # // Regression: 57c8ff36's object-only parser cannot read legacy Mesh arrays.
        from controller import mesh_json
        self.assertEqual(mesh_json('[mesh] pending info: 0\n[\n {"id":"B","read":true}\n]\n'),[{'id':'B','read':True}])

    def test_retry_busy_inside_sixty_second_window(self):
        clock=[0];calls=[]
        def command():
            calls.append(clock[0]);return (1,'handoff controller busy') if clock[0]<62 else (0,'ok')
        result=retry_transient(command,lambda:clock[0],lambda n:clock.__setitem__(0,clock[0]+n),90)
        self.assertEqual(result,(0,'ok'));self.assertGreaterEqual(calls[-1],60)

    def test_permanent_refusal_is_never_retried(self):
        calls=[]
        def command():calls.append(1);return 1,'rollback_compatibility_missing_or_changed'
        self.assertEqual(retry_transient(command,lambda:0,lambda n:self.fail('slept'),90)[0],1)
        self.assertEqual(len(calls),1)

    def test_exactly_one_boundary_and_verified_compatibility(self):
        events=[{'eventType':'delivery_owner_changed','previous_owner':'team','new_owner':'members'}]
        ownership_boundary(events,{'delivery_owner':'members','delivery_rollback_sha256':'hash'},True)
        for rows,verified in [(events*2,True),(events,False),([],True)]:
            with self.assertRaises(AssertionError):ownership_boundary(rows,{'delivery_owner':'members','delivery_rollback_sha256':'hash'},verified)

    def test_preserve_logical_id_and_independent_read_transport(self):
        original={'id':'B','read':False,'delivered':True}
        preserved_message(original,{'id':'B','read':False,'delivered':True})
        for changed in [{'id':'other','read':False,'delivered':True},{'id':'B','read':True,'delivered':True},{'id':'B','read':False,'delivered':False}]:
            with self.assertRaises(AssertionError):preserved_message(original,changed)

if __name__=='__main__':unittest.main()
