"""Exact required continuation gates, isolated harness homes, no runtime launch."""
import os
from pathlib import Path
import signal
import tempfile
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent.parent))
import prepare
from build import cargo_slot

prepare.OUT=Path(__file__).resolve().parent/'gates'
prepare.OUT.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix='th-l4-gates12-') as temporary:
    env=dict(os.environ)
    env.pop('TMUX',None)
    env['CARGO_HOME']=os.environ.get('CARGO_HOME',str(Path.home()/'.cargo'))
    env['RUSTUP_HOME']=os.environ.get('RUSTUP_HOME',str(Path.home()/'.rustup'))
    env['CARGO_TARGET_DIR']=str(prepare.CHECKOUT/'src-tauri/target')
    env['CARGO_BUILD_JOBS']='1'
    for key in ['HOME','CODEX_HOME','CLAUDE_CONFIG_DIR','CLAUDE_DIR','TAURHAUS_CLAUDE_DIR','GROK_HOME','GEMINI_CLI_HOME','TAURHAUS_AGY_DIR','TAURHAUS_DATA_DIR','TMUX_TMPDIR']:
        path=Path(temporary)/key; path.mkdir(); env[key]=str(path)
    for sig in [signal.SIGTERM,signal.SIGINT]: signal.signal(sig,prepare.interrupted)
    resources=[prepare.CHECKOUT/'src-tauri/resources'/name for name in ['mesh','taurhaus-daemon','mesh.version','mesh.manifest.json']]
    absent=[p for p in resources if not p.exists()]
    codes=[]
    try:
        for recipe in ['check-quick','lint','test-contracts']:
            cargo_slot(prepare.OUT/(recipe+'-cargo-wait.json'))
            codes.append(prepare.run(['just',recipe],prepare.CHECKOUT,env,recipe))
    finally:
        for child in prepare.children:
            if child.poll() is None:
                os.killpg(child.pid,signal.SIGTERM); child.wait(timeout=15)
        for path in absent: path.unlink(missing_ok=True)

raise SystemExit(0 if all(code==0 for code in codes) else 1)
