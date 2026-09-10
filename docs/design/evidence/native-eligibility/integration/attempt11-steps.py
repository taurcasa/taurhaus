"""Ordered paid trial actions. Run one numbered step; inspect and commit before next."""
import json
from pathlib import Path
import runpy
import secrets
import sys
import time
from attempt11_support import wait_receipt, validate_compaction, validate_tmux_activity
from attempt8_passive import complete_rows
B = Path(__file__).resolve().parent
OUT = B/'attempt11/run'
action = runpy.run_path(str(B/'attempt11-actions.py'))['action']

def mesh(argv, save): action({'op':'mesh','argv':argv,'save':save})
def capture(name): action({'op':'capture','name':name})
def read_json(path):
    for _ in range(20):
        try: return json.loads(path.read_text())
        except json.JSONDecodeError: time.sleep(.02)
    raise AssertionError('incomplete evidence JSON: '+str(path))
def events(): return complete_rows((OUT/'host-events.jsonl').read_text())
def wait_for(test, why, timeout=100):
    end=time.monotonic()+timeout
    while time.monotonic()<end:
        if any(r.get('kind')=='stopped' for r in complete_rows((OUT/'events.jsonl').read_text())): raise RuntimeError('controller stopped')
        if test(): return
        time.sleep(.25)
    raise AssertionError(why)
def reply(marker):
    return any(e.get('method')=='item/completed' and e.get('params',{}).get('item',{}).get('type')=='agentMessage' and marker in e['params']['item'].get('text','') for e in events())
def idle():
    return read_json(OUT/'hosted-transcript.json').get('thread',{}).get('status',{}).get('type')=='idle'
def wait_reply(marker):
    wait_for(lambda: reply(marker) and idle() and read_json(OUT/'cost-ledger.json')['metering_complete'], 'missing completed reply '+marker)
def send(marker, name): mesh(['send','seat','ACTION REQUIRED: Reply exactly '+marker+'. Do not execute tools.','--team','integration','--name','lead','--summary',name], name+'-send.txt')
def status(name): mesh(['team-daemon','status','--team','integration'],name+'-status.txt')
def journals():
    return [r for p in (OUT/'team/state/messaging-v2/segments').glob('*.jsonl') for r in complete_rows(p.read_text())]
def receipt_rows():
    return [read_json(p) for p in (OUT/'team/state/delivery').glob('*.json')]
def save(name,value): (OUT/name).write_text(json.dumps(value,indent=2)+'\n')
def current_receipts(name):
    action({'op':'snapshot'})
    sendrow=next(json.loads(l) for l in (OUT/(name+'-send.txt')).read_text().splitlines() if l.startswith('{'))
    return [r for r in journals() if r.get('payload',{}).get('message_id')==sendrow['message_id']]
def wait_pending(name):
    sendrow=next(json.loads(l) for l in (OUT/(name+'-send.txt')).read_text().splitlines() if l.startswith('{'))
    return wait_receipt(lambda:current_receipts(name), sendrow['message_id'])
def start(text, name):
    rec=json.loads((OUT/'team/runtime/seat.json').read_text())
    action({'op':'rpc','method':'coordination.hosted_input','params':{'team_name':'integration','member_name':'seat','generation':rec['attachmentGeneration'],'text':text},'save':name+'-active-start.json'})
def exposures(marker):
    items=[e['params']['item'] for e in events() if e.get('method')=='item/completed' and marker in json.dumps(e)]
    return list({i['id']:i for i in items}.values())

if __name__=='__main__':
    step=int(sys.argv[1]); action({'op':'step','step':step})
    try:
        if step==1:
            wait_for(lambda: idle() and read_json(OUT/'cost-ledger.json')['metering_complete'] and any('[taurhaus] recovery_card' in json.dumps(e) for e in events()), 'startup card did not complete', timeout=120)
            capture('step1-final');status('step1')
            rec=read_json(OUT/'team/runtime/seat.json')
            assert rec['terminalContract']==1 and rec['appServer']['instructionSources']
            assert '[taurhaus] recovery_card' in (OUT/'step1-final-pane-2.txt').read_text()
            assert rec['appServer']['threadId']==read_json(OUT/'step1-runtime.json')['appServer']['threadId']
        elif step==2:
            marker='saffron'+secrets.token_hex(3);save('step2-marker.json',marker)
            wait_for(idle, 'thread not idle')
            status('step2-before');send(marker,'step2');wait_reply(marker)
            status('step2-after');capture('step2')
            save('step2-journal-before-read.json',journals());save('step2-receipts.json',receipt_rows())
            assert marker in (OUT/'step2-pane-2.txt').read_text()
            assert 'consumed_by_read' not in json.dumps(journals())
            mesh(['read','--unread','--mark-read','--team','integration','--name','seat'],'step2-explicit-read.txt')
            save('step2-journal-after-read.json',journals())
            assert 'consumed_by_read' in json.dumps(journals())
        elif step==3:
            marker='juniper'+secrets.token_hex(3);save('step3-marker.json',marker)
            start('For this bounded transport timing probe, write exactly 80 numbered lines, each saying blue river stone. Do not execute tools.','step3')
            send(marker,'step3');status('step3-active');capture('step3-active')
            save('step3-pending.json',wait_pending('step3'))
            wait_reply(marker);status('step3-final');capture('step3-final')
            save('step3-receipts.json',current_receipts('step3'));save('step3-exposure.json',exposures(marker))
            assert len([i for i in exposures(marker) if i['type']=='userMessage'])==1
            assert len([i for i in exposures(marker) if i['type']=='agentMessage'])==1
        elif step==4:
            pending='cedar'+secrets.token_hex(3);typed='maple'+secrets.token_hex(3)
            save('step4-markers.json',{'socket':pending,'typed':typed})
            action({'op':'passive_start'})
            start('For this bounded input ordering probe, write exactly 80 numbered lines, each saying green valley oak. Do not execute tools.','step4')
            send(pending,'step4');save('step4-pending.json',wait_pending('step4'));capture('step4-pending')
            action({'op':'tmux','argv':['send-keys','-t','%2','-l','Reply exactly '+typed+'. Do not execute tools.']})
            action({'op':'tmux','argv':['send-keys','-t','%2','Enter']})
            wait_reply(pending);wait_reply(typed)
            capture('step4-final');status('step4-final');action({'op':'passive_end'})
            save('step4-receipts.json',current_receipts('step4'))
            save('step4-exposure.json',{'socket':exposures(pending),'typed':exposures(typed)})
            for marker in [pending,typed]:
                assert len([i for i in exposures(marker) if i['type']=='userMessage'])==1
                assert len([i for i in exposures(marker) if i['type']=='agentMessage'])==1
            replies=[e['params']['item']['text'] for e in events() if e.get('method')=='item/completed' and e['params']['item']['type']=='agentMessage']
            assert next(i for i,t in enumerate(replies) if typed in t)<next(i for i,t in enumerate(replies) if pending in t)
        elif step==5:
            wait_for(idle, 'thread not idle')
            action({'op':'snapshot'})
            before=read_json(OUT/'team/runtime/seat.json');save('step5-runtime-before.json',before)
            boundary_index=len(events())
            action({'op':'tmux','argv':['send-keys','-t','%2','-l','/compact']})
            action({'op':'tmux','argv':['send-keys','-t','%2','Enter']})
            time.sleep(2);capture('step5-compacting')
            def compacted():
                return any(e.get('method')=='item/completed' and e.get('params',{}).get('item',{}).get('type')=='contextCompaction' for e in events()[boundary_index:])
            wait_for(lambda:compacted() and idle(), 'compaction did not complete', timeout=120)
            capture('step5-compacted');action({'op':'snapshot'})
            after=read_json(OUT/'team/runtime/seat.json');save('step5-runtime-boundary.json',after)
            assert before['appServer']['threadId']==after['appServer']['threadId'], 'compaction changed thread identity'
            def recovered():
                action({'op':'snapshot'})
                rows=complete_rows((OUT/'taurhaus.log.jsonl').read_text())
                return any(r.get('event')=='compaction.codex_host.delivered' for r in rows) and idle()
            wait_for(recovered, 'daemon did not deliver compaction recovery', timeout=120)
            capture('step5-final')
            after=read_json(OUT/'team/runtime/seat.json');save('step5-runtime-after.json',after)
            boundary_events=events()[boundary_index:];save('step5-boundary-events.json',boundary_events)
            validate_compaction(before,after,complete_rows((OUT/'taurhaus.log.jsonl').read_text()),boundary_events,(OUT/'step5-final-pane-2.txt').read_text())
        elif step==6:
            wait_for(idle, 'thread not idle before restart')
            action({'op':'snapshot'})
            before=read_json(OUT/'team/runtime/seat.json');save('step6-runtime-before.json',before)
            action({'op':'restart_daemon','save':'step6-resume-result.json'})
            wait_for(idle, 'thread did not become idle after restart', timeout=120)
            action({'op':'snapshot'})
            after=read_json(OUT/'team/runtime/seat.json');save('step6-runtime-after.json',after)
            assert before['appServer']['threadId']==after['appServer']['threadId'], 'restart changed thread identity'
            marker='willow'+secrets.token_hex(3);save('step6-marker.json',marker)
            send(marker,'step6');wait_reply(marker)
            capture('step6-final');status('step6-final');save('step6-receipts.json',current_receipts('step6'))
            assert marker in (OUT/('step6-final-pane-'+after['paneId'][1:]+'.txt')).read_text()
        elif step==7:
            wait_for(idle, 'thread not idle before rollback')
            action({'op':'snapshot'})
            before=read_json(OUT/'team/runtime/seat.json');save('step7-runtime-before.json',before)
            mesh(['team','adapter','--member','seat','--mode','tmux','--team','integration','--name','lead'],'step7-inplace-refusal.txt')
            refusal=(OUT/'step7-inplace-refusal.txt').read_text()
            assert 'refused' in refusal or 'not applied' in refusal, 'in-place switch unexpectedly succeeded'
            action({'op':'host_poll','enabled':False})
            action({'op':'rpc','method':'stop_session','params':{'tmux_pane':before['paneId'],'cli_tool':'codex'},'save':'step7-stop.json'})
            action({'op':'operation','method':'coordination.remove_member','params':{'request':{'team_name':'integration','member_name':'seat'}},'save':'step7-remove.json'})
            agent={'name':'seat','cli_tool':'codex','model':'gpt-5.6-luna','reasoning_effort':'low','delivery':'tmux','project_id':before['project_path'],'instructions':'Isolated transport trial. Reply briefly. Never execute tools or commands.'}
            commands={'codex':{'fresh':'codex --sandbox read-only --ask-for-approval never','continue_cmd':'codex --sandbox read-only --ask-for-approval never','resume':'codex --sandbox read-only --ask-for-approval never resume'}}
            action({'op':'operation','method':'coordination.add_agent','params':{'request':{'team_name':'integration','agent':agent},'cli_commands':commands,'tmux_layout':'new_window'},'save':'step7-add.json'})
            value=read_json(OUT/'step7-add.json')
            assert value['outcome']['status']=='completed' and not value['outcome']['report'].get('failed_step'), value
            action({'op':'snapshot'})
            after=read_json(OUT/'team/runtime/seat.json');save('step7-runtime-after.json',after)
            assert after['terminalContract']==1 and not after.get('appServer') and not after.get('daemon_pid'), 'not a plain canonical tmux seat'
            capture('step7-plain');status('step7-before')
            def attributed():
                action({'op':'rpc','method':'get_runtime_session_snapshot','params':{},'save':'step7-activity-before.json'})
                try: validate_tmux_activity(read_json(OUT/'step7-activity-before.json'),after['paneId'],read_json(Path(after['activitySnapshotPath'])))
                except AssertionError: return False
                return True
            wait_for(attributed, 'rollback activity not attributed and idle', timeout=120)
            source=Path(after['activitySnapshotPath'])
            save('step7-mesh-activity.json',read_json(source))
            marker='aspen'+secrets.token_hex(3);save('step7-marker.json',marker);send(marker,'step7')
            def delivered():
                capture('step7-final');status('step7-final')
                action({'op':'rpc','method':'get_runtime_session_snapshot','params':{},'save':'step7-activity.json'})
                rows=current_receipts('step7')
                save('step7-receipts.json',rows)
                pane=(OUT/('step7-final-pane-'+after['paneId'][1:]+'.txt')).read_text()
                return any(r.get('payload',{}).get('stage')=='submitted' for r in rows) and marker in pane
            try:wait_for(delivered, 'plain tmux delivery stalled', timeout=120)
            except AssertionError:
                if 'activity not freshly idle' in (OUT/'step7-final-status.txt').read_text() and 'uncertain' in (OUT/'step7-activity.json').read_text():
                    raise AssertionError('FAIL — known defect (codex identity lane): pending: activity not freshly idle; activity uncertain')
                raise
        else: raise ValueError(step)
        save('step'+str(step)+'-outcome.json',{'step':step,'outcome':'PASS','at':time.time()})
    except BaseException as e:
        save('step'+str(step)+'-outcome.json',{'step':step,'outcome':'FAIL','error':str(e)})
        action({'op':'fail','reason':'step '+str(step)+': '+str(e)})
        raise
