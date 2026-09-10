"""Replay run 3's manifest against complete events; no runtime/credential access.

Default checks committed exports without rewriting evidence. --write recreates
the exports and removes only byte-identical aliases declared in the manifest.
This frozen replay covers run 3's two capture rounds (initial and teardown).
"""
import argparse
import json
from pathlib import Path

from support import clean

BASE = Path(__file__).resolve().parent


def replay(events, manifest):
    raw = {}
    round_index = -1
    command = None
    for row in events:
        if row['kind'] == 'runtime':
            raw['team/runtime/alpha.json'] = json.dumps(row['value'], indent=2) + '\n'
            raw['final-runtime.json'] = raw['team/runtime/alpha.json']
        if row['kind'] == 'command':
            command = row['argv']
            if 'list-panes' in command:
                round_index += 1
        elif row['kind'] == 'command_result' and command:
            if 'capture-pane' in command:
                if row['exit'] != 0 or round_index not in (0, 1):
                    raise ValueError('Unexpected capture result/round')
                label = ('step1-initial', 'final')[round_index]
                pane = command[command.index('-t') + 1].removeprefix('%')
                raw[f'{label}-pane-{pane}.txt'] = clean('\n'.join(row['output'].splitlines()[-60:])) + '\n'
            command = None
    if round_index != 1:
        raise ValueError('Expected initial and teardown capture rounds')
    for alias, canonical in manifest['aliases'].items():
        if raw[alias] != raw[canonical]:
            raise ValueError(f'Alias not byte-identical: {alias}')
        del raw[alias]
    for item in manifest['pane_normalization']:
        name = item['path']
        lines = raw[name].splitlines()
        if len(lines) != item['before_lines']:
            raise ValueError(f'Original line count differs: {name}')
        while lines and not lines[-1].strip():
            lines.pop()
        if len(lines) != item['after_lines']:
            raise ValueError(f'Normalized line count differs: {name}')
        raw[name] = '\n'.join(lines) + '\n'
    return raw


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    out = BASE / 'run'
    events = [json.loads(line) for line in (out / 'events.jsonl').read_text().splitlines()]
    manifest = json.loads((BASE / 'export-manifest.json').read_text())
    exports = replay(events, manifest)
    for name, value in exports.items():
        path = out / name
        if args.write:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(value)
        elif path.read_text() != value:
            raise ValueError(f'Export differs from replay: {name}')
    for name in manifest['aliases']:
        path = out / name
        if args.write:
            path.unlink(missing_ok=True)
        elif path.exists():
            raise ValueError(f'Duplicate alias still exists: {name}')
    print(json.dumps({'exit': 0, 'exports_verified': len(exports),
                      'aliases_verified': len(manifest['aliases']), 'write': args.write}))


if __name__ == '__main__':
    main()
