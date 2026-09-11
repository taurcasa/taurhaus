"""Passive input exclusion for the bounded run8 controller; never takes a lock."""
from datetime import datetime
import json
from pathlib import Path


def delivery_input_facts(record, activity, journal, rows, lock_sample, now):
    record = record or {}; activity = activity or {}
    card_id = ((record.get('recovery') or {}).get('last_delivered') or {}).get('journal', {}).get('message_id')
    targets = {}
    for row in journal:
        payload = row.get('payload', {})
        if row.get('event_type') == 'message_accepted':
            for target in payload.get('delivery_targets', []):
                if target.get('recipient') == 'alpha':
                    targets[target['delivery_id']] = payload['message_id']
    stages = {}
    for row in journal:
        payload = row.get('payload', {})
        if row.get('event_type') in ('receipt', 'delivery_attempt') and payload.get('recipient') == 'alpha' and payload.get('stage'):
            stages[payload.get('delivery_id')] = payload['stage']
    submitted = {delivery for delivery, stage in stages.items() if stage == 'submitted'}
    pending = sorted(set(targets) - submitted)
    card_submitted = any(delivery in submitted and message == card_id for delivery, message in targets.items())
    card_read = bool(card_id) and any(r.get('payload', {}).get('message_id') == card_id
                and r['payload'].get('kind') == 'consumed_by_read'
                and r['payload'].get('reader_name') == 'alpha' for r in journal)
    for row in rows:
        payload = row.get('payload', {})
        if payload.get('type') in ('function_call_output', 'custom_tool_call_output'):
            output = str(payload.get('output', ''))
            card_read = card_read or bool(card_id and card_id in output and '[taurhaus] recovery_card' in output)
    try:
        age = now - datetime.fromisoformat(activity['observed_at'].replace('Z', '+00:00')).timestamp()
        fresh_idle = (0 <= age <= 120 and activity.get('activity_confidence') == 'idle'
                      and activity.get('pane_alive') is True and not activity.get('pane_foreign')
                      and bool(record.get('session_id') and record.get('jsonl_path')))
    except (KeyError, ValueError, TypeError):
        fresh_idle = False
    ready = bool(card_submitted and card_read and not pending and fresh_idle
                 and lock_sample['complete'] and not lock_sample['held'])
    return {'ready': ready, 'onboarding_message_id': card_id, 'onboarding_submitted': card_submitted,
            'onboarding_read': card_read, 'pending_delivery_ids': pending, 'fresh_idle': fresh_idle,
            'terminal_lock': lock_sample, 'observed_at': now}


def passive_terminal_lock(lock, identities, proc_root=Path('/proc')):
    sample = {'path': str(lock), 'complete': True, 'held': False, 'fdinfo': []}
    try:
        target = lock.stat(); sample['inode'] = target.st_ino
    except OSError:
        sample['complete'] = False
        return sample
    for identity in identities:
        proc = proc_root/str(identity['pid'])
        try:
            for fd in (proc/'fd').iterdir():
                try:
                    stat = fd.stat()
                    if (stat.st_dev, stat.st_ino) != (target.st_dev, target.st_ino): continue
                    content = (proc/'fdinfo'/fd.name).read_text()
                    held = any(line.startswith('lock:') for line in content.splitlines())
                    sample['held'] |= held
                    sample['fdinfo'].append({'pid': identity['pid'], 'fd': fd.name, 'text': content})
                except FileNotFoundError: pass  # fd closed during passive sampling
                except OSError: sample['complete'] = False
        except FileNotFoundError: pass  # owned process already exited
        except OSError: sample['complete'] = False
    return sample
