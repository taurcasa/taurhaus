"""Run9 host-PID launch, passive registry evidence and owned-process teardown."""
import json
import signal

def launch_wrapper(root, credential):
    return ['bwrap','--die-with-parent','--ro-bind','/','/','--tmpfs','/home',
            '--tmpfs','/tmp','--tmpfs','/run','--dev','/dev','--bind',str(root),str(root),
            '--ro-bind',str(credential),str(root/'claude/.credentials.json'),
            '--chdir',str(root/'project')]

def retain_registry(root, save):
    source=root/'claude/sessions'
    files=sorted(p for p in source.rglob('*') if p.is_file())
    save('sessions/index.json', {'directory_exists':source.is_dir(),
                               'files':[str(p.relative_to(source)) for p in files if p.suffix=='.json'],
                               'all_files':[str(p.relative_to(source)) for p in files]})
    for path in files:
        try: value=json.loads(path.read_text())
        except (ValueError, OSError): value=path.read_text(errors='replace')
        if isinstance(value,dict) and 'peerToken' in value:
            value.pop('peerToken'); value['peer_token_redacted']=True
        save('sessions/'+str(path.relative_to(source)),value)

def registry_facts(record, activity, files):
    host_pid=activity.get('pid') or record.get('pid')
    present=bool(host_pid and f'{host_pid}.json' in files)
    attributed=bool(record.get('session_id') and activity.get('activity_attribution')=='attributed')
    return {'host_pid':host_pid,'registry_present':present,'runtime_session_id':record.get('session_id'),
            'activity_attribution':activity.get('activity_attribution'),
            'ready':present and attributed,
            'classification':None if present and attributed else 'taurhaus' if present else 'harness/environment',
            'reason':None if present and attributed else 'host_registry_present_lead_unattributed' if present else 'claude_registry_absent' if not files else 'claude_registry_host_pid_mismatch',
            'continue_without_attribution':not files}

def stop_owned(owned, identities, kill, sleep):
    """Only reap still-matching runtime descendants; no namespace-init dependency."""
    expected={(p['pid'],p['start_ticks']) for p in owned}
    for sig in (signal.SIGTERM,signal.SIGKILL):
        alive=[p for p in identities() if (p['pid'],p['start_ticks']) in expected]
        if not alive:return
        for process in alive:
            # Revalidate immediately before signalling to exclude PID reuse.
            if any((p['pid'],p['start_ticks'])==(process['pid'],process['start_ticks']) for p in identities()):
                try:kill(process['pid'],sig)
                except ProcessLookupError:pass
        sleep(2)
