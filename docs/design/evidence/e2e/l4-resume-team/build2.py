"""Continuation build: owned RC descriptor and uncommitted Tauri resource."""
import os
import shutil
import subprocess
import tempfile
import time
import json
import hashlib
from pathlib import Path
import prepare

BASE = Path(__file__).resolve().parent
prepare.OUT = BASE/'build2'
prepare.OUT.mkdir(exist_ok=True)
mesh = prepare.MESH
resource = prepare.CHECKOUT/'src-tauri/resources/mesh'
original = (mesh/prepare.DESCRIPTOR).read_text()
assert not subprocess.check_output(['git','-C',str(mesh),'status','--porcelain'],text=True).strip()
with tempfile.TemporaryDirectory(prefix='th-l4-build2-') as temporary:
    env = dict(os.environ)
    env['CARGO_BUILD_JOBS']='2'
    env['CARGO_HOME']=os.environ.get('CARGO_HOME',str(Path.home()/'.cargo'))
    env['RUSTUP_HOME']=os.environ.get('RUSTUP_HOME',str(Path.home()/'.rustup'))
    env['HOME']=temporary
    changed=False
    try:
        observations=[]; deadline=time.monotonic()+1800
        while True:
            p=subprocess.run(['pgrep','-af','(^|/)cargo( |$)'],capture_output=True,text=True)
            if not observations or observations[-1]['output']!=prepare.sanitize(p.stdout):
                observations.append({'at':time.time(),'exit':p.returncode,'output':prepare.sanitize(p.stdout)})
                (prepare.OUT/'cargo-wait.json').write_text(json.dumps(observations,indent=2)+'\n')
            if p.returncode==1: break
            assert p.returncode==0 and time.monotonic()<deadline, 'build wait exceeded 30 min'
            time.sleep(2)
        text=original.replace('host: "taurhaus-owned persistent thread; UNVERIFIED",','host: if build == "0.153.4" { "taurhaus-daemon-owned-thread/1" } else { "taurhaus-owned persistent thread; UNVERIFIED" },')
        text=text.replace('transport: TRANSPORT, configuration: "UNVERIFIED", trust: "UNVERIFIED",','transport: TRANSPORT, configuration: if build == "0.153.4" { "strict-config/1" } else { "UNVERIFIED" }, trust: if build == "0.153.4" { "daemon-owned/1" } else { "UNVERIFIED" },')
        text=text.replace('disposition: "disabled", enabled: false, strongest_receipt: "native_enqueued",','disposition: if build == "0.153.4" { "trial" } else { "disabled" }, enabled: build == "0.153.4", strongest_receipt: "native_enqueued",')
        changed=True
        (mesh/prepare.DESCRIPTOR).write_text(text)
        (prepare.OUT/'descriptor.diff').write_text(subprocess.check_output(['git','-C',str(mesh),'diff','--',prepare.DESCRIPTOR],text=True))
        env['CARGO_TARGET_DIR']=str(mesh/'target')
        assert prepare.run(['cargo','build','--bin','mesh'],mesh,env,'mesh')==0
        shutil.copyfile(mesh/'target/debug/mesh',resource)
        env['CARGO_TARGET_DIR']=str(prepare.CHECKOUT/'src-tauri/target')
        assert prepare.run(['just','build-daemon'],prepare.CHECKOUT,env,'daemon')==0
        binaries={}
        for name,p in [('mesh',mesh/'target/debug/mesh'),('daemon',prepare.CHECKOUT/'src-tauri/target/release/taurhaus-daemon')]:
            with p.open('rb') as f: digest=hashlib.file_digest(f,'sha256').hexdigest()
            binaries[name]={'sha256':digest,'path':str(p)}
        (prepare.OUT/'binaries.json').write_text(json.dumps(binaries,indent=2)+'\n')
    finally:
        for child in prepare.children:
            if child.poll() is None:
                os.killpg(child.pid,15)
                child.wait(timeout=15)
        if changed:
            prepare.run(['git','-C',str(mesh),'checkout','--',prepare.DESCRIPTOR],prepare.CHECKOUT,env,'restore')
        # The linked resource is scratch-only and is not needed to run the binary.
        resource.unlink(missing_ok=True)
