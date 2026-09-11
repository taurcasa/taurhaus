"""Offline evidence predicates; legacy format is selected by each run's ruling."""
from datetime import datetime


def stamp(value):
    try:return datetime.fromisoformat(value.replace('Z','+00:00')).timestamp()
    except (AttributeError,ValueError):return float('-inf')


def pending(rows,message_id,health,*,activity=None,now=None,obligations=()):
    accepted=next((r for r in rows if r.get('event_type')=='message_accepted' and r.get('payload',{}).get('message_id')==message_id),None)
    if not accepted or not activity or now is None:return None
    if activity.get('activity_confidence') not in ('active','likely_working'):return None
    if not 0<=now-stamp(activity.get('observed_at'))<=120:return None
    receipts=[r for r in rows if r.get('payload',{}).get('message_id')==message_id and r.get('event_type') in ('receipt','delivery_receipt')]
    begun=any(r.get('payload',{}).get('message_id')==message_id and r.get('payload',{}).get('stage') in ('attempt_started','outcome_unknown','submitted','native_enqueued','consumed') for r in rows)
    if receipts or begun:return None
    accepted_at=stamp(accepted.get('committed_at'))
    if now<accepted_at:return None
    obligation=next((r for r in obligations if r.get('obligation',{}).get('message_id')==message_id and r['obligation'].get('recipient')=='alpha'),None)
    heartbeat=stamp(health.get('heartbeat'))
    if obligation:opportunity={'obligation':obligation}
    elif heartbeat>=accepted_at:opportunity={'health':health}
    else:return None
    return {'source':'accepted without receipt while working; scheduler opportunity verified','message_id':message_id,'accepted':accepted,'receipt_count':0,'scheduler_opportunity':opportunity}


def format_boundary(config,marker,before,after,verified,*,expected_legacy_format=1):
    # Run2 required 1; run3 uses the RC's documented legacy format 0.
    assert expected_legacy_format in (0,1),'expected format must be legacy'
    assert config.get('messaging_format',1)==expected_legacy_format,f'required resulting format {expected_legacy_format} not observed'
    assert marker.get('transition')=='complete','format boundary not committed'
    assert marker.get('legacy_cut')==config.get('messaging_downgrade_sha256') and verified,'format report digest not verified'
    assert before and after==before,'canonical history not retained unchanged'


def assistant_reply(rows,marker,after):
    for row in rows:
        payload=row.get('payload',{})
        if stamp(row.get('timestamp'))<stamp(after):continue
        if row.get('type')=='response_item' and payload.get('type')=='message' and payload.get('role')=='assistant':
            if any(marker in c.get('text','') for c in payload.get('content',[])):return row
    return None


if __name__ == '__main__':
    # Offline checks stay in this named file to keep the review fix scoped.
    import ast
    import json
    from pathlib import Path
    import unittest

    class RollbackReviewRegression(unittest.TestCase):
        def test_run3_accepts_legacy_zero_and_checks_boundary_evidence(self):
            # // Regression: 22643c6f reused 011c2738's run2-only format-1 predicate in run3.
            config = {'messaging_format': 0, 'messaging_downgrade_sha256': 'digest'}
            marker = {'transition': 'complete', 'legacy_cut': 'digest'}
            history = [{'event_type': 'message_accepted', 'payload': {'message_id': 'B'}}]
            format_boundary(config, marker, history, history, True, expected_legacy_format=0)
            for changes in ({'messaging_format': 2}, {'messaging_downgrade_sha256': 'wrong'}):
                with self.subTest(changes=changes), self.assertRaises(AssertionError):
                    format_boundary(config | changes, marker, history, history, True, expected_legacy_format=0)
            for boundary, after, verified in ((marker | {'transition': 'pending'}, history, True),
                                               (marker, history, False), (marker, [], True)):
                with self.subTest(boundary=boundary, after=after, verified=verified), self.assertRaises(AssertionError):
                    format_boundary(config, boundary, history, after, verified, expected_legacy_format=0)

        def test_run2_keeps_its_original_format_requirement(self):
            config = {'messaging_format': 1, 'messaging_downgrade_sha256': 'digest'}
            marker = {'transition': 'complete', 'legacy_cut': 'digest'}
            format_boundary(config, marker, ['B'], ['B'], True)
            with self.assertRaises(AssertionError):
                format_boundary(config | {'messaging_format': 0}, marker, ['B'], ['B'], True)

        def test_run3_selects_its_own_legacy_format(self):
            # // Regression: 22643c6f imported the superseded run2 expectation unchanged.
            tree = ast.parse((Path(__file__).parents[1] / 'run3/controller.py').read_text())
            calls = [n for n in ast.walk(tree) if isinstance(n, ast.Call)
                     and isinstance(n.func, ast.Name) and n.func.id == 'format_boundary']
            self.assertEqual(len(calls), 1)
            self.assertEqual({k.arg: ast.literal_eval(k.value) for k in calls[0].keywords},
                             {'expected_legacy_format': 0})

        def test_credential_copy_has_no_fingerprint(self):
            # // Regression: 22643c6f logged a digest of live credential bytes into evidence.
            run3 = Path(__file__).parents[1] / 'run3'
            tree = ast.parse((run3 / 'controller.py').read_text())
            calls = [n for n in ast.walk(tree) if isinstance(n, ast.Call) and n.args
                     and isinstance(n.args[0], ast.Constant) and n.args[0].value == 'auth_copy']
            self.assertEqual(len(calls), 1)
            allowed = {'copied_files', 'mode', 'initial_codex_entries', 'source_label'}
            self.assertEqual({k.arg for k in calls[0].keywords}, allowed)
            for line in (run3 / 'run/events.jsonl').read_text().splitlines():
                row = json.loads(line)
                if row['kind'] == 'auth_copy':
                    self.assertEqual(set(row) - {'at', 'kind'}, allowed)

        def test_recorded_stop_is_harness_and_later_steps_remain_unmeasured(self):
            # // Regression: b840d330 attributed the inherited predicate failure to Mesh.
            run3 = Path(__file__).parents[1] / 'run3'
            outcome = json.loads((run3 / 'run/step3-outcome.json').read_text())
            analysis = json.loads((run3 / 'analysis.json').read_text())
            self.assertEqual(outcome['classification'], 'harness')
            self.assertEqual(analysis['classification'], 'harness')
            self.assertEqual(outcome['outcome'], 'FAIL')
            self.assertFalse(analysis['handoff_request_observed'])
            self.assertEqual(analysis['ownership_boundary_events'], [])
            for step in (4, 5, 6):
                self.assertEqual(json.loads((run3 / f'run/step{step}-outcome.json').read_text())['outcome'], 'NOT RUN')

    unittest.main()
