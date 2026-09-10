"""Ordered paid trial actions. Run one numbered step; inspect and commit before next."""
import json
from pathlib import Path
import runpy
import secrets
import sys
import time
B = Path(__file__).resolve().parent
OUT = B/'attempt8/run'
action = runpy.run_path(str(B/'attempt8-actions.py'))['action']

def mesh(argv, save): action({'op':'mesh','argv':argv,'save':save})
def capture(name): action({'op':'capture','name':name})
def events(): return [json.loads(l) for l in (OUT/'host-events.jsonl').read_text().splitlines() if l]
def wait_for(test, why, timeout=100):
    end=time.monotonic()+timeout
    while time.monotonic()<end:
        if any(r.get('kind')=='stopped' for r in [json.loads(l) for l in (OUT/'events.jsonl').read_text().splitlines()]): raise RuntimeError('controller stopped')
        if test(): return
        time.sleep(.25)
    raise AssertionError(why)
def reply(marker):
    return any(e.get('method')=='item/completed' and e.get('params',{}).get('item',{}).get('type')=='agentMessage' and marker in e['params']['item'].get('text','') for e in events())
def idle():
    return json.loads((OUT/'hosted-transcript.json').read_text()).get('thread',{}).get('status',{}).get('type')=='idle'
def wait_reply(marker):
    wait_for(lambda: reply(marker) and idle() and json.loads((OUT/'cost-ledger.json').read_text())['metering_complete'], 'missing completed reply '+marker)
def send(marker, name): mesh(['send','seat','ACTION REQUIRED: Reply exactly '+marker+'. Do not execute tools.','--team','integration','--name','lead','--summary',name], name+'-send.txt')
def status(name): mesh(['team-daemon','status','--team','integration'],name+'-status.txt')
def journals():
    return [json.loads(l) for p in (OUT/'team/state/messaging-v2/segments').glob('*.jsonl') for l in p.read_text().splitlines()]
def receipt_rows():
    return [json.loads(p.read_text()) for p in (OUT/'team/state/delivery').glob('*.json')]
def save(name,value): (OUT/name).write_text(json.dumps(value,indent=2)+'\n')
def start(text, name):
    rec=json.loads((OUT/'team/runtime/seat.json').read_text())
    action({'op':'rpc','method':'coordination.hosted_input','params':{'team_name':'integration','member_name':'seat','generation':rec['attachmentGeneration'],'text':text},'save':name+'-active-start.json'})
def exposures(marker):
    items=[e['params']['item'] for e in events() if e.get('method')=='item/completed' and marker in json.dumps(e)]
    return list({i['id']:i for i in items}.values())

if __name__=='__main__':
    step=int(sys.argv[1]); action({'op':'step','step':step})
    try:
        if step==2:
            marker='saffron'+secrets.token_hex(3);save('step2-marker.json',marker)
            assert idle()
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
            save('step3-pending.json',receipt_rows())
            assert 'thread_active' in json.dumps(receipt_rows()), 'no active-thread deferral'
            wait_reply(marker);status('step3-final');capture('step3-final')
            save('step3-receipts.json',receipt_rows());save('step3-exposure.json',exposures(marker))
            assert len([i for i in exposures(marker) if i['type']=='userMessage'])==1
            assert len([i for i in exposures(marker) if i['type']=='agentMessage'])==1
        elif step==4:
            pending='cedar'+secrets.token_hex(3);typed='maple'+secrets.token_hex(3)
            save('step4-markers.json',{'socket':pending,'typed':typed})
            action({'op':'passive_start'})
            start('For this bounded input ordering probe, write exactly 80 numbered lines, each saying green valley oak. Do not execute tools.','step4')
            send(pending,'step4');capture('step4-pending');save('step4-pending.json',receipt_rows())
            assert 'thread_active' in json.dumps(receipt_rows()), 'no pending socket delivery'
            action({'op':'tmux','argv':['send-keys','-t','%2','-l','Reply exactly '+typed+'. Do not execute tools.']})
            action({'op':'tmux','argv':['send-keys','-t','%2','Enter']})
            wait_reply(pending);wait_reply(typed)
            capture('step4-final');status('step4-final');action({'op':'passive_end'})
            save('step4-receipts.json',receipt_rows())
            save('step4-exposure.json',{'socket':exposures(pending),'typed':exposures(typed)})
            for marker in [pending,typed]:
                assert len([i for i in exposures(marker) if i['type']=='userMessage'])==1
                assert len([i for i in exposures(marker) if i['type']=='agentMessage'])==1
            replies=[e['params']['item']['text'] for e in events() if e.get('method')=='item/completed' and e['params']['item']['type']=='agentMessage']
            assert next(i for i,t in enumerate(replies) if typed in t)<next(i for i,t in enumerate(replies) if pending in t)
        else: raise ValueError(step)
        save('step'+str(step)+'-outcome.json',{'step':step,'outcome':'PASS','at':time.time()})
    except BaseException as e:
        save('step'+str(step)+'-outcome.json',{'step':step,'outcome':'FAIL','error':str(e)})
        action({'op':'fail','reason':'step '+str(step)+': '+str(e)})
        raise
