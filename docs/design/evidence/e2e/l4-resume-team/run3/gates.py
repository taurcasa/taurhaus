"""Exact required continuation gates, isolated harness homes, no runtime launch."""
import os
from pathlib import Path
import signal
import tempfile
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent.parent))
import prepare

prepare.OUT=Path(__file__).resolve().parent/'gates'
prepare.OUT.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix='th-l4-gates3-') as temporary:
    env=dict(os.environ)
    env.pop('TMUX',None)
    env['CARGO_HOME']=os.environ.get('CARGO_HOME',str(Path.home()/'.cargo'))
    env['RUSTUP_HOME']=os.environ.get('RUSTUP_HOME',str(Path.home()/'.rustup'))
    env['CARGO_TARGET_DIR']=str(prepare.CHECKOUT/'src-tauri/target')
    env['CARGO_BUILD_JOBS']='2'
    for key in ['HOME','CODEX_HOME','CLAUDE_CONFIG_DIR','CLAUDE_DIR','TAURHAUS_CLAUDE_DIR','GROK_HOME','GEMINI_CLI_HOME','TAURHAUS_AGY_DIR','TAURHAUS_DATA_DIR','TMUX_TMPDIR']:
        path=Path(temporary)/key; path.mkdir(); env[key]=str(path)
    try:
        for recipe in ['check-quick','lint','test-contracts']:
            prepare.run(['just',recipe],prepare.CHECKOUT,env,recipe)
    finally:
        for child in prepare.children:
            if child.poll() is None:
                os.killpg(child.pid,signal.SIGTERM); child.wait(timeout=15)
        (prepare.CHECKOUT/'src-tauri/resources/mesh').unlink(missing_ok=True)
