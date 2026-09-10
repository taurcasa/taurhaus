"""Run 7 builds the shipped descriptor without any Mesh source mutation."""
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
sys.path.insert(0,str(Path(__file__).resolve().parent.parent))
import prepare

BASE=Path(__file__).resolve().parent

def cargo_slot(out):
    observations=[]; deadline=time.monotonic()+1800
    while True:
        p=subprocess.run(['pgrep','-af','(^|/)cargo( |$)'],capture_output=True,text=True)
        count=len(p.stdout.splitlines())
        observations.append({'at':time.time(),'exit':p.returncode,'count':count,'output':prepare.sanitize(p.stdout)})
        out.write_text(json.dumps(observations,indent=2)+'\n')
        assert p.returncode in [0,1], 'cargo probe failed'
        if count<3: return
        assert time.monotonic()<deadline, 'cargo slot unavailable after 30 minutes'
        time.sleep(30)

def main():
    assert Path.cwd()==prepare.CHECKOUT
    prepare.OUT=BASE/'build'; prepare.OUT.mkdir(exist_ok=True)
    assert subprocess.check_output(['git','-C',str(prepare.MESH),'rev-parse','--short','HEAD'],text=True).strip()=='310144d'
    assert not subprocess.check_output(['git','-C',str(prepare.MESH),'status','--porcelain'],text=True).strip()
    for sig in [signal.SIGINT,signal.SIGTERM]: signal.signal(sig,prepare.interrupted)
    with tempfile.TemporaryDirectory(prefix='th-l4-build7-') as temporary:
        env=dict(os.environ); env.pop('TMUX',None)
        env['CARGO_BUILD_JOBS']='1'
        for key,default in [('CARGO_HOME','.cargo'),('RUSTUP_HOME','.rustup')]: env[key]=os.environ.get(key,str(Path.home()/default))
        for key in ['HOME','CODEX_HOME','CLAUDE_CONFIG_DIR','CLAUDE_DIR','TAURHAUS_CLAUDE_DIR','GROK_HOME','GEMINI_CLI_HOME','TAURHAUS_AGY_DIR','TAURHAUS_DATA_DIR','TMUX_TMPDIR']:
            path=Path(temporary)/key; path.mkdir(); env[key]=str(path)
        resources=[prepare.CHECKOUT/'src-tauri/resources'/name for name in ['mesh','taurhaus-daemon','mesh.version','mesh.manifest.json']]
        absent=[p for p in resources if not p.exists()]
        try:
            cargo_slot(prepare.OUT/'cargo-wait-mesh.json')
            env['CARGO_TARGET_DIR']=str(prepare.MESH/'target')
            assert prepare.run(['cargo','build','--bin','mesh'],prepare.MESH,env,'mesh')==0
            assert prepare.run(['just','ensure-tauri-resources'],prepare.CHECKOUT,env,'resources')==0
            cargo_slot(prepare.OUT/'cargo-wait-daemon.json')
            env['CARGO_TARGET_DIR']=str(prepare.CHECKOUT/'src-tauri/target')
            assert prepare.run(['just','build-daemon'],prepare.CHECKOUT,env,'daemon')==0
            binaries={}
            for name,path in [('mesh',prepare.MESH/'target/debug/mesh'),('daemon',prepare.CHECKOUT/'src-tauri/target/release/taurhaus-daemon')]:
                with path.open('rb') as f: digest=hashlib.file_digest(f,'sha256').hexdigest()
                binaries[name]={'path':str(path),'sha256':digest}
            (prepare.OUT/'binaries.json').write_text(json.dumps(binaries,indent=2)+'\n')
        finally:
            for child in prepare.children:
                if child.poll() is None: os.killpg(child.pid,signal.SIGTERM); child.wait(timeout=15)
            for path in absent: path.unlink(missing_ok=True)

if __name__=='__main__': main()
