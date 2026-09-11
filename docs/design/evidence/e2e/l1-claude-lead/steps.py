"""Ordered continuation steps; one explicit invocation, no mutation/input retry."""
import json
from pathlib import Path
import sys
import time
from actions import action, OUT
from continuation_support import codex_ready, codex_submitted, alpha_attributed_idle, explicit_lead_read
A='L1_AMBER_42'; B='L1_BIRCH_73'; F='L1_FERN_91'

def read(name):
    try: return json.loads((OUT/name).read_text())
    except (FileNotFoundError,json.JSONDecodeError): return None

def wait(test,why,timeout=65):
    action({'op':'window','step':STEP,'seconds':timeout,'label':f'step{STEP}-window-{len(list(OUT.glob("*window*.json")))}'})
    start=time.monotonic()
    while time.monotonic()-start<timeout:
        if read('result.json'): raise RuntimeError('runtime controller already stopped')
        if test(): return
        time.sleep(1)
    raise why if isinstance(why, Exception) else AssertionError(why)

def send_input(tool,pane,text,recipient_inputs=0):
    action({'op':'input','step':STEP,'tool':tool,'pane':pane,'text':text,'recipient_inputs':recipient_inputs})

def claude_rows(): return read('claude-transcript.json') or []
def codex_rows(): return read('codex-transcript.json') or []
def text_rows(rows): return json.dumps(rows)
def journal():
    return [r for p in (OUT/'latest/team/state/messaging-v2/segments').glob('*.jsonl') for r in json.loads(p.read_text())]
def native(marker):
    return any(r.get('type')=='user' and '<teammate-message' in str(r.get('message',{}).get('content','')) and 'alpha' in str(r.get('message',{}).get('content','')) and marker in str(r.get('message',{}).get('content','')) for r in claude_rows())
def reply(marker):
    return any(r.get('type')=='assistant' and any(c.get('type')=='text' and marker in c.get('text','') for c in r.get('message',{}).get('content',[]) if isinstance(c,dict)) for r in claude_rows())
def snapshot(label): action({'op':'capture','step':STEP,'label':label})
def save(name,value): (OUT/name).write_text(json.dumps(value,indent=2)+'\n')

def journal_pages(label):
    cursor=None; index=0
    while True:
        argv=['journal','read']+(['--since',cursor] if cursor else [])
        name=f'{label}-journal-page-{index}'
        action({'op':'mesh','step':STEP,'argv':argv,'label':name})
        result=read(name+'.json'); assert result['exit']==0,result
        data=json.loads(result['output']); save(name+'-parsed.json',data)
        if data['done']: return
        cursor=data.get('next_cursor') or data.get('next') or data.get('cursor')
        assert cursor, 'missing journal cursor'
        index+=1

class OnboardingPending(AssertionError):
    """The spec's step-2 card-exposure timeout; ownership still requires facts."""


def classify_failure(error, facts):
    if isinstance(error, json.JSONDecodeError):
        return 'harness'
    if not all(facts.get(key) for key in ('daemon_alive', 'runtime_readable', 'activity_fresh')):
        return 'harness'
    if (isinstance(error, OnboardingPending) and facts.get('session_attributed')
            and facts.get('card_pending') and not facts.get('card_exposed')):
        return 'taurhaus'
    return 'product-owner-pending-evidence-review'


def alpha_native_ready():
    rows = codex_rows()
    if not codex_ready(rows):
        return False
    snapshot('step2-native-ready')
    try:
        pane = (OUT / 'step2-native-ready/pane-2.txt').read_text()
    except OSError:
        return False
    # Typed setup inputs also pass controller.input's new-turn confirmation.
    # For production onboarding, confirm its own started/completed turn here.
    return codex_submitted(pane, rows, set())


def prepare_alpha():
    if (OUT.name in ('run8', 'run9') or OUT.name == 'run10'):
        action({'op': 'wait_alpha_delivery', 'step': STEP})
    if not any(r.get('payload', {}).get('type') == 'task_started' for r in codex_rows()):
        send_input('codex', '%2', 'Controller setup check. Reply READY only. Do not execute tools, read messages, or send messages.')
    wait(alpha_native_ready, 'Codex native turn completion/empty composer not observed', timeout=65)
    snapshot('step2-alpha-observation')
    record = read('step2-alpha-observation/team/runtime/alpha.json') or {}
    activity = read('step2-alpha-observation/team/state/activity/alpha.json') or {}
    idle = alpha_attributed_idle(record, activity)
    save('step2-alpha-observation.json', {
        'runtime': record, 'activity': activity, 'attributed_idle': idle,
        'deviation': None if idle else 'Required derived attributed-idle observation absent; native turn readiness permits continuation.',
    })
    wait(lambda: '[taurhaus] recovery_card' in text_rows(codex_rows()),
         OnboardingPending('alpha onboarding card never delivered'), timeout=65)


def prepare_resumable_assignment():
    subject = 'Run6 native mailbox recovery'
    run7 = (OUT.name in ('run7', 'run8', 'run9') or OUT.name == 'run10')
    if run7: subject = OUT.name.capitalize() + ' native mailbox recovery'
    description = 'Preserve the lead role and this assignment across ordinary compaction, then handle the fresh native reply once.'
    if run7:
        # These are part of the real Mesh assignment, then copied verbatim on publication.
        description += '\nExecution mode: Measure\nFile-ownership boundary: []\nAdjacent-fix policy: No adjacent fixes.\nValidation expectation: Controller validates native recovery.\nResponse expectation: Respond once to fresh native mail.'
    action({'op': 'mesh', 'step': STEP, 'label': 'step5-task-create',
            'argv': ['task', 'create', '--subject', subject, '--json']})
    created = read('step5-task-create.json')
    assert created['exit'] == 0, created
    task_id = json.loads(created['output'])['id']
    action({'op': 'mesh', 'step': STEP, 'label': 'step5-task-assign', 'argv': [
        'task', 'assign', task_id, '--owner', 'lead', '--status', 'in_progress', '--json',
        '--description', description,
        '--deliverable', 'A brief native text response distinguishing the fresh reply after recovery; no files or tools.',
        '--first-step', 'Keep this assignment active through the controller compact; observe the next native teammate reply without tools.',
        '--completion-signal', 'Respond once in native text when the fresh reply arrives; the controller records completion.',
        '--review-route', 'Measure; controller acceptance with one independent Opus evidence review.']})
    assigned = read('step5-task-assign.json')
    assert assigned['exit'] == 0, assigned
    # Mesh warnings precede the JSON on this controller's merged stdout/stderr.
    output = assigned['output']
    assignment = json.loads(output[output.index('{'):])
    if run7:
        action({'op': 'publish_assignment', 'step': STEP, 'task_id': task_id,
                'assignment_id': assignment['assignment_id']})
    def ready():
        task = (read('latest/team/state/operational/lead.json') or {}).get('task', {})
        return task.get('id') == task_id and task.get('subject') == subject and task.get('status') == 'in_progress'
    wait(ready, 'production operational snapshot lacks the assigned in-progress task', timeout=65)
    snapshot('step5-assignment')
    operational = read('latest/team/state/operational/lead.json')
    record = read('latest/team/runtime/lead.json')
    result = {'task': operational['task'], 'operational_snapshot': operational,
              'assignment': assignment,
              'role': record['recovery']['last_delivered']['card_key']['contract']}
    save('step5-assignment-context.json', result)
    return result


def main(step):
    global STEP
    STEP = step
    try:
        if STEP==2:
            prepare_alpha()
            send_input('claude','%1',f'''Step 2. Use Bash to execute exactly these two mesh send commands; no reads. Do not send anything else. Then say SENT.
mesh send alpha 'ACTION REQUIRED: Remember marker {A}. Do not run tools or send a reply yet; the controller will request step 3.' --summary 'L1 marker A' --claude-dir "$CLAUDE_CONFIG_DIR" --team l1-claude-lead --name lead
mesh send alpha 'ACTION REQUIRED: Remember marker {B}. Do not run tools or send a reply yet; the controller will request step 3.' --summary 'L1 marker B' --claude-dir "$CLAUDE_CONFIG_DIR" --team l1-claude-lead --name lead''',recipient_inputs=2)
            wait(lambda: A in text_rows(journal()) and B in text_rows(journal()),'Claude sends did not reach the canonical journal')
            wait(lambda: A in text_rows(codex_rows()) and B in text_rows(codex_rows()),'alpha did not receive both marker exposures through production tmux delivery',timeout=65)
            snapshot('step2'); journal_pages('step2')
        elif STEP==3:
            send_input('codex','%2',f'''Step 3. Run mesh read --unread --json --mark-read --claude-dir "$CLAUDE_CONFIG_DIR" --team l1-claude-lead --name alpha. Follow any returned cursor with the same filters and --since until done. Then run exactly two commands:
mesh send lead 'RESULT: {A} received by alpha.' --summary 'L1 reply A' --claude-dir "$CLAUDE_CONFIG_DIR" --team l1-claude-lead --name alpha
mesh send lead 'RESULT: {B} received by alpha.' --summary 'L1 reply B' --claude-dir "$CLAUDE_CONFIG_DIR" --team l1-claude-lead --name alpha
Do nothing else.''')
            wait(lambda: 'RESULT: '+A in text_rows(journal()) and 'RESULT: '+B in text_rows(journal()),'alpha replies not accepted')
            wait(lambda: native(A) and native(B),'canonical seat replies never surfaced as native Claude teammate messages',timeout=65)
            wait(lambda: reply(A) and reply(B),'Claude did not distinguish both native markers',timeout=65)
            snapshot('step3-before-read'); journal_pages('step3')
        elif STEP==4:
            send_input('claude','%1','Step 4. Now explicitly run mesh read --unread --json --mark-read --claude-dir "$CLAUDE_CONFIG_DIR" --team l1-claude-lead --name lead. Continue with unchanged filters and the returned --since cursor until done, including empty unread pages. Report the read acknowledgments briefly. No sends.')
            wait(lambda: explicit_lead_read(journal()),'explicit lead read receipts absent')
            snapshot('step4');journal_pages('step4')
        elif STEP==5:
            assignment = prepare_resumable_assignment()
            before=read('latest/team/runtime/lead.json');save('step5-before.json',before)
            boundary=len(claude_rows()); log_boundary=len(read('daemon-events.json') or [])
            save('step5-boundary-index.json',{'transcript_rows':boundary,'daemon_rows':log_boundary})
            send_input('claude','%1','/compact')
            def recovered():
                after=read('latest/team/runtime/lead.json') or {}
                rows=claude_rows()[boundary:]
                events=(read('daemon-events.json') or [])[log_boundary:]
                delivered=all(any(r.get('event')=='compaction.claude_hook.'+kind for r in events)
                              for kind in ('received', 'resolved', 'delivered'))
                card=after.get('recovery',{}).get('last_delivered',{})
                contract=card.get('card_key',{}).get('contract',{})
                previous=before.get('recovery',{}).get('last_delivered',{}).get('card_key',{}).get('contract',{})
                return (after.get('contextGeneration')!=before['contextGeneration'] and delivered
                        and any('compact_boundary' in text_rows([r]) for r in rows)
                        and any('[taurhaus] recovery_card' in text_rows([r])
                                and 'lead on l1-claude-lead' in text_rows([r])
                                and assignment['task']['subject'] in text_rows([r]) for r in rows)
                        and contract.get('effective_role_revision')==previous.get('effective_role_revision')
                        and card.get('content_revision')!=before.get('recovery',{}).get('last_delivered',{}).get('content_revision'))
            # Real hook telemetry and runtime/card identity, not a prompted echo.
            wait(recovered,'missing real compaction boundary/generation/native recovery card',timeout=120)
            snapshot('step5'); save('step5-after.json',read('latest/team/runtime/lead.json'))
        elif STEP==6:
            send_input('codex','%2',f'''Run exactly mesh send lead 'RESULT: {F} after recovery.' --summary 'L1 fresh reply' --claude-dir "$CLAUDE_CONFIG_DIR" --team l1-claude-lead --name alpha. No other tools.''')
            wait(lambda:native(F) and reply(F),'fresh post-recovery native marker not acted on')
            snapshot('step6-before-read')
            send_input('claude','%1','Final explicit read/mark only. Run mesh read --unread --json --mark-read --claude-dir "$CLAUDE_CONFIG_DIR" --team l1-claude-lead --name lead. Follow the returned --since cursor with unchanged filters until done. No sends and no further action on messages; say MARKED only.')
            def fresh_read():
                ids = {r.get('payload', {}).get('message_id') for r in journal()
                       if F in ((r.get('payload') or {}).get('body') or '')}
                return any(r.get('payload', {}).get('message_id') in ids
                           and r.get('payload', {}).get('kind') == 'consumed_by_read'
                           and r.get('payload', {}).get('reader_name') == 'lead' for r in journal())
            wait(fresh_read, 'fresh native reply lacks final explicit read/mark')
            snapshot('step6');journal_pages('step6')
            action({'op':'stop','step':6,'result':{'outcome':'PASS','classification':None,'step':6,'reason':'all ordered runtime checks complete; final audit/review still required'}})
        else: raise ValueError('steps 2..6 only')
        if STEP!=6: action({'op':'pass','step':STEP,'evidence':f'step{STEP} snapshots, transcripts, journal pages'})
    except BaseException as error:
        if not read('result.json'):
            snapshot('step'+str(STEP)+'-failure')
            facts=read(f'step{STEP}-failure/onboarding-health.json') or {}
            classification=classify_failure(error, facts)
            save(f'step{STEP}-failure-classification.json', {'classification':classification, 'facts':facts})
            action({'op':'stop','step':STEP,'result':{'outcome':'UNAVAILABLE' if classification=='harness' else 'FAIL','classification':classification,'step':STEP,'reason':str(error)}})
        raise


if __name__ == '__main__':
    main(int(sys.argv[1]))
