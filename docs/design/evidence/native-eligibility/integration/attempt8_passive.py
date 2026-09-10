"""Read-only kernel host-operation holder evidence; no host RPC or process control."""
import json
from pathlib import Path
import time


def sample(proc, inode, trial_id):
    result = []
    for line in (proc/'locks').read_text().splitlines():
        fields = line.split()
        if '->' in fields or len(fields) < 6 or fields[1:4] != ['FLOCK','ADVISORY','WRITE']:
            continue
        if not fields[5].endswith(':'+str(inode)): continue
        pid = int(fields[4]); directory = proc/str(pid)
        try:
            if ('TAURHAUS_TRIAL_ID='+trial_id).encode()+b'\0' not in (directory/'environ').read_bytes(): continue
            info = [p.read_text() for p in (directory/'fdinfo').iterdir()]
            info = [s for s in info if f'ino:\t{inode}\n' in s and 'lock:' in s]
            result.append({'pid':pid, 'start_ticks':(directory/'stat').read_text().rsplit(')',1)[1].split()[19],
                'argv':(directory/'cmdline').read_bytes().decode().split('\0'),
                'proc_lock':line, 'fdinfo':info})
        except (FileNotFoundError, PermissionError, ProcessLookupError): continue
    return result


def observe(root, output, stop):
    lock = root/'claude/teams/integration/state/app-server/seat.lock'
    inode = lock.stat().st_ino
    previous = None
    with output.open('w', buffering=1) as stream:
        while not stop.is_set():
            rows = sample(Path('/proc'), inode, root.name)
            if rows != previous:
                stream.write(json.dumps({'at':time.time(), 'inode':inode, 'holders':rows})+'\n')
                previous = rows
            stop.wait(.02)


def complete_rows(text):
    rows = []
    for line in text.splitlines(keepends=True):
        if not line.endswith('\n'): continue
        try: rows.append(json.loads(line))
        except ValueError: continue
    return rows
