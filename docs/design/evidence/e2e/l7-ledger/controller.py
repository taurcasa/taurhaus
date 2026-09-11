"""Six ordered live steps; adapted from the versioned lane-2 run-3 controller.

Run from this checkout with --auth-source naming the operator-authorized file.
No unit test runs this module's main or invokes any real CLI.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import time
import uuid

from runtime import Trial as Runtime, BASE, CHECKOUT, TEAM, mesh_json
from support import complete_rows, delivered, receipt_retry


def output_text(value):
    if isinstance(value, list):
        return '\n'.join(output_text(v) for v in value)
    if isinstance(value, dict):
        return output_text(value.get('text', value.get('output', '')))
    return str(value)


def objects(text):
    decoder = json.JSONDecoder()
    result = []
    for match in re.finditer(r'\{', text):
        try:
            value, _ = decoder.raw_decode(text[match.start():])
            result.append(value)
        except ValueError:
            pass
    return result


class Trial(Runtime):
    def teardown(self):
        calls = {r.get('payload', {}).get('call_id'): r['payload']
                 for rows in self.sessions() for r in rows
                 if r.get('payload', {}).get('type') in ('custom_tool_call', 'function_call')}
        proofs = []
        for row in self.tool_results():
            call = calls.get(row['payload'].get('call_id'), {})
            code = call.get('input', call.get('arguments', ''))
            if 'mesh ledger entry' not in code and 'mesh task complete' not in code:
                continue
            text = output_text(row['payload'].get('output', ''))
            proofs.append({'call_id': row['payload'].get('call_id'), 'timestamp': row.get('timestamp'),
                           'call_code': code, 'exit_codes': re.findall(r'(?:exit_code["\s:]+|exited with code\s+)(\d+)', text),
                           'receipts_and_errors': [o for o in objects(text) if 'receipts' in o or 'ledger' in o or 'error' in o],
                           'output_sha256': hashlib.sha256(text.encode()).hexdigest()})
        self.save('seat-tool-proofs.json', proofs)
        super().teardown()

    def raw_save(self, name, value):
        # Only safe declared artifact/ledger data, never message journals or runtime config.
        path = self.out / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value, indent=2) + '\n')

    def workflow(self):
        path = self.team / 'state/workflow_events.jsonl'
        return complete_rows(path.read_text()) if path.exists() else []

    def task(self):
        return json.loads((self.root / 'claude/tasks' / TEAM / '1.json').read_text())

    def ledger(self):
        return [row for p in (self.team / 'state/ledger/v1/segments').glob('*.jsonl') for row in complete_rows(p.read_text())]

    def settled(self):
        return self.fresh_idle() and delivered(self.journals(), self.last_delivery, [r for rows in self.sessions() for r in rows])

    def send(self, instruction):
        self.wait(self.settled, 'previous delivery has not settled; no send allowed', 120, owner='mesh')
        self.reserve('step ' + str(self.step) + ' instruction')
        value = mesh_json(self.mesh(['send', 'alpha', instruction, '--summary', 'L7 step ' + str(self.step)]))
        self.last_delivery = value['message_id']
        self.save(f'step{self.step}-accepted.json', value)

    def tool_results(self):
        return [r for rows in self.sessions() for r in rows if r.get('payload', {}).get('type') in ('function_call_output', 'custom_tool_call_output')]

    def seat_receipt(self, event_id, source=False, after=0):
        for row in self.tool_results()[after:]:
            for obj in objects(output_text(row.get('payload', {}).get('output', ''))):
                value = obj.get('ledger', {}) if source else obj
                if any(r.get('event_id') == event_id for r in value.get('receipts', [])):
                    return {'receipt': obj, 'tool_call_id': row['payload'].get('call_id'), 'timestamp': row.get('timestamp'), 'row_sha256': hashlib.sha256(json.dumps(row).encode()).hexdigest()}
        return None

    def require_intake(self, event_id, source=False, after=0):
        def observed():
            # An explicit intake rejection is terminal even if the lifecycle committed.
            for row in self.tool_results()[after:]:
                text = output_text(row.get('payload', {}).get('output', ''))
                if 'source committed; ledger rejected' in text or 'source committed; ledger durability unknown' in text:
                    self.save('source-intake-failure.json', {'task': self.task(), 'source_committed': True, 'ledger_intake': 'rejected or durability unknown', 'errors': [o for o in objects(text) if 'error' in o]})
                    raise AssertionError('source committed; ledger intake failed; no lifecycle replay')
            receipt = self.seat_receipt(event_id, source, after)
            return receipt if receipt and self.settled() else None
        return self.wait(observed, 'seat artifact intake receipt or settled delivery missing', 150, owner='mesh')

    def artifact(self, name, event_id):
        path = self.root / 'project' / name
        data = path.read_bytes()
        assert 0 < len(data) <= 2048, 'artifact exceeds 2 KiB'
        event = next(e for e in self.ledger() if e['event_id'] == event_id)
        digest = hashlib.sha256(data).hexdigest()
        assert digest in json.dumps(event), 'artifact raw digest not bound in ledger'
        assert 'alpha' in json.dumps(event['author']), 'event not attributed to real seat'
        (self.out / name).write_bytes(data)
        self.raw_save(f'step{self.step}-event.json', event)
        self.raw_save(f'step{self.step}-artifact.json', {'file': name, 'bytes': len(data), 'sha256': digest, 'event_id': event_id, 'root_id': event['entry_id'], 'author': event['author']})
        return data

    def steps(self):
        self.classification = 'mesh'
        # 1. The only task, then its immutable assignment and lead-approved packet.
        self.mesh(['task', 'create', '--subject', 'L7 tiny artifact round trip',
                   '--description', 'Demonstrate real seat artifact intake and completion.',
                   '--deliverable', 'OBSERVATION.md and RESULT.md, each at most 2048 bytes.',
                   '--first-step', 'Read this assignment; accept and start it with its full assignment ID; reply TASK_READY and await explicit artifact instructions.',
                   '--completion-signal', 'Complete only when instructed with RESULT.md through --summary-file.',
                   '--review-route', 'Lead checks ledger receipts and offline read-back; stop after the requested completion.'])
        self.wait(self.settled, 'onboarding not settled before assignment', 120)
        self.reserve('task assignment')
        assignment = mesh_json(self.mesh(['task', 'assign', '1', '--owner', 'alpha', '--json']))
        self.save('step1-assignment-receipt.json', assignment)
        self.assignment = self.task()['metadata']['assignment_id']
        self.last_delivery = assignment.get('message_id', assignment.get('id'))
        if not self.last_delivery:
            accepted = [r for r in self.journals() if r.get('event_type') == 'message_accepted' and self.assignment in json.dumps(r)]
            self.last_delivery = accepted[-1]['payload']['message_id']
        packet = {'project': 'l7-project', 'repo_id': 'l7-repo', 'wave': 'l7', 'packet_revision': 'frozen-1',
                  'scopes': [{'scope': 'S7', 'owner': 'alpha', 'task_id': '1', 'assignment_id': self.assignment}]}
        (self.root / 'project/packet.json').write_text(json.dumps(packet))
        self.raw_save('step1-packet.json', packet)
        initialized = mesh_json(self.mesh(['ledger', 'init', '--wave', 'l7', '--authority-file', 'packet.json', '--repo-root', str(self.root / 'project')]))
        self.raw_save('step1-init-receipt.json', initialized)
        manifest = json.loads((self.team / 'state/ledger/v1/manifest.json').read_text())
        self.raw_save('step1-manifest.json', manifest)
        self.incarnation = manifest['team_incarnation_id']
        self.wait(lambda: self.settled() and self.task()['metadata'].get('started_at'), 'seat did not accept/start frozen assignment', 150)
        self.save('step1-immutable-assignment.json', [e for e in self.workflow() if e.get('eventType') == 'task_assigned'])
        self.pass_step()

        # 2. Header identity is frozen; the actual file and observation prose are seat-authored.
        self.step = 2
        self.note_id = str(uuid.uuid4())
        event = {'event_id': self.note_id, 'entry_key': {'wave': 'l7', 'scope': 'S7', 'kind': 'note', 'slot': 'alpha.observation'},
                 'references': [{'authority': 'scope', 'project': 'l7-project', 'wave': 'l7', 'scope': 'S7', 'packet_revision': 'frozen-1'},
                                {'authority': 'task', 'team_incarnation_id': self.incarnation, 'task_id': '1', 'assignment_id': self.assignment},
                                {'authority': 'artifact', 'path': 'self', 'role': 'result_artifact', 'assessment': 'source_checked'}]}
        header = json.dumps({'ledger': {'adapter_version': 1, 'events': [event]}})
        self.send('ACTION REQUIRED: Explicitly read/mark this instruction. Write OBSERVATION.md (<=2048 bytes) with leading --- newline, this JSON object as valid YAML front matter, newline --- newline, then a short standalone observation about the scratch-only test boundary, not completion prose: ' + header + '. Execute mesh ledger entry --file OBSERVATION.md --claude-dir "$CLAUDE_DIR" --team l7-ledger --name alpha. Print command exit_code and stdout receipt; reply NOTE_DONE. Do not complete the task.')
        note_receipt = self.require_intake(self.note_id)
        self.raw_save('step2-seat-receipt.json', note_receipt)
        self.artifact('OBSERVATION.md', self.note_id)
        live = mesh_json(self.mesh(['ledger', 'render', '--view', 'current', '--format', 'json']))
        assert self.note_id in json.dumps(live)
        self.raw_save('step2-live-render.json', live)
        self.capture('step2')
        self.pass_step()

        # 3. Submission form omits authored wave/scope; Mesh derives them from assignment.
        self.step = 3
        self.result_id = str(uuid.uuid4())
        event = {'event_id': self.result_id, 'entry_key': {'kind': 'note', 'slot': 'alpha.result'},
                 'references': [{'authority': 'artifact', 'path': 'self', 'role': 'result_artifact', 'assessment': 'source_checked'}]}
        header = json.dumps({'ledger': {'adapter_version': 1, 'events': [event]}})
        self.send('ACTION REQUIRED: Explicitly read/mark. Write a different RESULT.md (<=2048 bytes), leading --- newline, this JSON object as YAML, newline --- newline, then a brief RESULT paragraph stating artifact intake completed; do not repeat the separate observation: ' + header + '. Execute exactly once: mesh task complete 1 --assignment ' + self.assignment + ' --summary-file RESULT.md --claude-dir "$CLAUDE_DIR" --team l7-ledger --name alpha. Print command exit_code and stdout source and ledger receipts; reply RESULT_DONE. Never repeat completion even on error.')
        result_receipt = self.require_intake(self.result_id, source=True)
        self.raw_save('step3-seat-source-ledger-receipts.json', result_receipt)
        data = self.artifact('RESULT.md', self.result_id)
        assert self.task()['status'] == 'completed'
        self.save('step3-completed-task.json', self.task())
        completed = [r for r in self.workflow() if r.get('eventType') == 'task_completed']
        assert len(completed) == 1
        self.source = completed[0]
        assert 'adapter_version' not in str(self.source.get('summary', '')), 'header leaked into human summary'
        self.wait(lambda: self.completion_delivery(), 'lead completion delivery missing', 120)
        self.save('step3-completion-delivery.json', self.completion_delivery())
        self.save('step3-source.json', self.source)
        self.raw_save('step3-receipt-table.json', {'source': result_receipt['receipt']['source'], 'ledger': result_receipt['receipt']['ledger'], 'source_event_id': self.source['eventId'], 'header_stripped': True, 'artifact_sha256': hashlib.sha256(data).hexdigest()})
        self.pass_step()

        # 4. Retry the original event with its exact source locator, never lifecycle.
        self.step = 4
        before_results = len(self.tool_results())
        before = len(self.ledger())
        before_completed = len([r for r in self.workflow() if r.get('eventType') == 'task_completed'])
        before_notices = self.completion_notice_ids()
        self.send('ACTION REQUIRED: Explicitly read/mark. Retry ONLY this ledger identity once: mesh ledger entry --file RESULT.md --task 1 --assignment ' + self.assignment + ' --submission-event ' + self.source['eventId'] + ' --claude-dir "$CLAUDE_DIR" --team l7-ledger --name alpha. Do not edit files or call task complete. Print command exit_code and stdout receipt and reply RETRY_DONE.')
        retried = self.require_intake(self.result_id, after=before_results)
        original = result_receipt['receipt']['ledger']
        assert receipt_retry(original, retried['receipt'], before, len(self.ledger()))
        assert before_completed == len([r for r in self.workflow() if r.get('eventType') == 'task_completed'])
        assert before_notices == self.completion_notice_ids()
        self.raw_save('step4-retry.json', {'original': original, 'retry': retried, 'ledger_count_before': before, 'ledger_count_after': len(self.ledger()), 'completion_count': before_completed, 'completion_notice_ids': before_notices})
        self.pass_step()

        # 5. Ordinary managed stop freezes the only writer, then a single boundary.
        self.step = 5
        self.save('step5-stop.json', self.rpc('stop_session', {'tmux_pane': self.record()['paneId'], 'cli_tool': 'codex'}))
        self.wait(lambda: self.record().get('health') == 'session_dead', 'writer not stopped', 120, owner='taurhaus')
        (self.root / 'exports').mkdir()
        snapshot = mesh_json(self.mesh(['ledger', 'snapshot', '--wave', 'l7', '--boundary', 'close-1', '--output-dir', str(self.root / 'exports/boundary')]))
        self.raw_save('step5-snapshot-receipt.json', snapshot)
        bundle = self.out / 'bundle'
        shutil.copytree(self.root / 'exports/boundary', bundle, symlinks=False)
        # Offline process sees only a copied binary and bundle, with live /tmp and /home hidden.
        offline = self.root / 'offline'
        offline.mkdir()
        shutil.copytree(bundle, offline / 'bundle')
        shutil.copyfile(self.root / 'home/.local/bin/mesh', offline / 'mesh')
        (offline / 'mesh').chmod(0o700)
        bw = ['bwrap', '--die-with-parent', '--unshare-pid', '--ro-bind', '/', '/', '--tmpfs', '/home', '--tmpfs', '/tmp', '--tmpfs', '/run', '--proc', '/proc', '--dev', '/dev', '--ro-bind', str(offline), '/offline', '--chdir', '/offline', '--clearenv', '--setenv', 'PATH', '/usr/bin:/bin', '/offline/mesh']
        for view in ('current', 'narrative'):
            rendered = self.run(bw + ['ledger', 'render', '--input-bundle', '/offline/bundle', '--view', view, '--format', 'markdown'])
            (self.out / f'step5-offline-{view}.md').write_text(rendered)
            assert rendered == (bundle / ('ledger.md' if view == 'current' else 'narrative.md')).read_text()
        structured = mesh_json(self.run(bw + ['ledger', 'render', '--input-bundle', '/offline/bundle', '--format', 'json']))
        self.raw_save('step5-offline-render.json', structured)
        for name in ('OBSERVATION.md', 'RESULT.md'):
            data = (self.out / name).read_bytes()
            assert any(p.read_bytes() == data for p in (bundle / 'artifacts').rglob('*') if p.is_file())
        assert not list(bundle.rglob('*messaging*')) and not list(bundle.rglob('*inbox*'))
        self.pass_step()

        self.step = 6
        self.raw_save('step6-receipt-table.json', {'standalone': note_receipt, 'completion': result_receipt, 'retry': retried, 'source_event': self.source['eventId'], 'declarations': len(self.ledger()), 'offline_views': ['current', 'narrative'], 'review': 'independent Opus lens required from orchestrator; not available in this executor'})

    def completion_notice_ids(self):
        return [r['payload']['message_id'] for r in self.journals() if r.get('event_type') == 'message_accepted' and r.get('payload', {}).get('author', {}).get('claimed_sender') == 'alpha' and 'complet' in json.dumps(r).lower()]

    def completion_delivery(self):
        ids = self.completion_notice_ids()
        rows = [r for r in self.journals() if r.get('payload', {}).get('message_id') in ids]
        return rows if any(r.get('payload', {}).get('recipient') == 'lead' and r.get('payload', {}).get('stage') == 'native_enqueued' for r in rows) else None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--auth-source', required=True)
    args = parser.parse_args()
    assert Path.cwd() == CHECKOUT
    os.umask(0o077)
    trial = Trial()
    def interrupted(sig, frame):
        raise RuntimeError(f'controller interrupted {sig}')
    signal.signal(signal.SIGINT, interrupted)
    signal.signal(signal.SIGTERM, interrupted)
    try:
        trial.boot(args.auth_source)
        trial.steps()
        trial.code = 0
    except BaseException as error:
        trial.save(f'step{trial.step}-outcome.json', {'step': trial.step, 'outcome': 'FAIL', 'classification': trial.classification, 'reason': str(error)})
        print(json.dumps({'step': trial.step, 'classification': trial.classification, 'error': str(error)}), flush=True)
    finally:
        trial.teardown()
        for step in range(trial.step + 1, 7):
            trial.save(f'step{step}-outcome.json', {'step': step, 'outcome': 'NOT RUN', 'reason': f'blocked by step {trial.step}'})
        trial.save('controller-exit.json', {'exit': trial.code, 'last_step': trial.step, 'runtime_seconds': time.monotonic() - trial.started})
        if trial.code == 0:
            trial.pass_step()
    return trial.code


if __name__ == '__main__':
    raise SystemExit(main())
