"""One independent, tool-free Opus evidence lens; separate bounded review spend."""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
from controller_support import sanitize
BASE=Path(__file__).resolve().parent

def run4_review_files(label, brief):
    names = ['review-brief.json'] if brief else ['report.md', 'final-disposition.json', 'spend-summary.json', 'teardown-audit.json', 'review-excerpts.json']
    return [f'{label}/{name}' for name in names]

def main():
    parser=argparse.ArgumentParser(); parser.add_argument('--continued',action='store_true'); parser.add_argument('--run3',action='store_true'); parser.add_argument('--brief',action='store_true'); parser.add_argument('--credential',type=Path,required=True); parser.add_argument('--binary',type=Path,required=True)
    args=parser.parse_args(); evidence=BASE/'run3/review-brief' if args.run3 and args.brief else BASE/'run3/review' if args.run3 else BASE/'continued-review-brief' if args.brief else BASE/'continued-review' if args.continued else BASE; evidence.mkdir(parents=True,exist_ok=True); root=Path(tempfile.mkdtemp(prefix='th-l1-opus-')); child=None
    label = os.environ.get('TRIAL_EVIDENCE_LABEL')
    if label in ('run4', 'run4c', 'run4d', 'run5', 'run6', 'run6/continuation', 'run7', 'run8', 'run9', 'run10'):
        evidence = BASE/label/'review'; evidence.mkdir(parents=True, exist_ok=True)
    def interrupted(sig,_frame): raise RuntimeError(f'reviewer interrupted {sig}')
    for sig in [signal.SIGTERM,signal.SIGINT]: signal.signal(sig,interrupted)
    try:
        for name in ['home','claude','tmp','bin']: (root/name).mkdir(mode=0o700)
        shutil.copyfile(args.binary,root/'bin/claude'); (root/'bin/claude').chmod(0o700)
        (root/'claude/.credentials.json').touch()
        (root/'claude/.claude.json').write_text('{"hasCompletedOnboarding":true,"theme":"dark"}')
        env={'TAURHAUS_TRIAL_ID':root.name,'CODEX_HOME':str(root/'codex'),'GROK_HOME':str(root/'grok'),'GEMINI_CLI_HOME':str(root/'gemini'),'TAURHAUS_AGY_DIR':str(root/'gemini'),'TAURHAUS_CLAUDE_DIR':str(root/'claude'),'HOME':str(root/'home'),'CLAUDE_CONFIG_DIR':str(root/'claude'),'CLAUDE_DIR':str(root/'claude'),'PATH':str(root/'bin')+':/usr/bin:/bin','TMPDIR':str(root/'tmp'),'DISABLE_AUTOUPDATER':'1','CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC':'1','LANG':'C.UTF-8','TERM':'dumb'}
        prompt='You are the independent Opus evidence reviewer for a small-change trial. Review the reported verdict against the actual evidence; a stopped run must never be called an E2E PASS. No tools. Return JSON {"verdict":"approve"|"fix_required","findings":[{"severity":"major"|"minor","finding":"...","fix":"..."}]}. fix_required requires a major. Assess truthful failure classification, spend, cleanup, secrets, evidence discipline. Missing runtime steps are intentional first-failure stop, not a pass claim. Be concise. One fix round may correct the report. The standing attempt budget rule is authoritative; this review launches no lane rerun.\n'
        files=['../l1-claude-lead.md','controller.py','controller_support.py','steps.py','continued-run/final-disposition.json','continued-run/cleanup.json','continued-run/final-cost-ledger.json','continued-run/initialize-result.json','continued-run/production-hooks-before-launch.json','continued-run/final/pane-1.txt','continued-run/final/pane-2.txt'] if args.continued else ['../l1-claude-lead.md','controller.py','controller_support.py','step1.py','run/result.json','run/cleanup.json','run/cost-ledger.json','run/initialize-result.json']
        if args.brief: files=['../l1-claude-lead.md','continued-run/final-disposition.json','continued-run/cleanup.json','continued-run/final-cost-ledger.json','continued-run/production-hooks-before-launch.json','continued-run/teardown-audit.json']
        if args.run3: files=['run3/report.md','run3/result.json','run3/final-disposition.json','run3/cleanup.json','run3/cost-ledger.json','run3/driver-steps.json','run3/log-retention.json','run3/evidence-index.json']
        if args.run3 and args.brief: files=['run3/report.md','run3/final-disposition.json','run3/cleanup.json','run3/cost-ledger.json','run3/review-summary.json']
        if label in ('run4', 'run4c', 'run4d', 'run5', 'run6', 'run6/continuation', 'run7', 'run8', 'run9', 'run10'):
            files = run4_review_files(label, args.brief)
        for name in files:
            path=BASE/name
            prompt+='\nFILE '+name+'\n'+path.read_text()+'\n'
        (evidence/'review-prompt.txt').write_text(prompt)
        bw=['bwrap','--die-with-parent','--unshare-pid','--ro-bind','/','/','--tmpfs','/home','--tmpfs','/tmp','--tmpfs','/run','--proc','/proc','--dev','/dev','--bind',str(root),str(root),'--ro-bind',str(args.credential),str(root/'claude/.credentials.json'),'--chdir',str(root/'home')]
        command=bw+[str(root/'bin/claude'),'-p','--model','claude-opus-4-6','--tools','','--max-budget-usd',('0.20' if args.brief or args.run3 else '0.65'),'--effort','low','--output-format','json','--no-session-persistence']
        (evidence/'review-command.json').write_text(json.dumps(sanitize(command),indent=2)+'\n')
        child=subprocess.Popen(command,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
        stdout,stderr=child.communicate(prompt.encode(),timeout=180)
        (evidence/'review-result.json').write_text(sanitize(stdout.decode(errors='replace')))
        (evidence/'review-exit.json').write_text(json.dumps({'exit':child.returncode,'stderr':sanitize(stderr.decode(errors='replace'))[-2000:]},indent=2)+'\n')
        print('Opus review exit',child.returncode)
    finally:
        if child and child.poll() is None:
            os.killpg(child.pid,signal.SIGTERM)
            try: child.wait(timeout=5)
            except subprocess.TimeoutExpired: os.killpg(child.pid,signal.SIGKILL); child.wait(timeout=5)
        zero=(root/'claude/.credentials.json').exists() and (root/'claude/.credentials.json').stat().st_size==0
        shutil.rmtree(root)
        (evidence/'review-cleanup.json').write_text(json.dumps({'child_exit':child.returncode if child else None,'root_removed':not root.exists(),'credential_placeholder_zero_bytes':zero},indent=2)+'\n')
if __name__=='__main__': main()
