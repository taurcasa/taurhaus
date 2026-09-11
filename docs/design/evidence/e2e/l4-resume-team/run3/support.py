"""Run 3 offline guards. Runtime source is the installation resolved by which(codex)."""
from pathlib import Path
import shutil


def copy_native_runtime(entry, destination):
    package=Path(entry).resolve().parents[1]
    candidates=list(package.glob('node_modules/@openai/codex-linux-x64/vendor/*/bin/codex'))
    assert len(candidates)==1, 'expected one installed native Codex runtime'
    native=candidates[0].parent
    sources=[native/name for name in ['codex','codex-code-mode-host']]
    assert all(p.is_file() for p in sources), 'native runtime sibling missing'
    for source in sources:
        target=destination/source.name
        shutil.copyfile(source,target); target.chmod(0o700)


def sanitize_log_rows(records):
    return [r for r in records if not r.get('event','').startswith('usage.')]


def classify_failure(reason):
    if any(s in reason for s in ['harness command','auth source','unapproved Codex','cap','headroom','metered','bwrap:', 'code-mode-host', 'Operation not permitted', 'Permission denied']):
        return 'harness'
    return 'mesh' if 'Mesh refusal' in reason or 'mesh team activation failed' in reason or 'team-daemon' in reason else 'taurhaus'


def reconciled_spend(ledger):
    complete=not ledger['unmetered']
    return {'total_usd':ledger['conservative_usd'] if complete else None,
            'metered_subtotal_usd':ledger['conservative_usd'],
            'cap_verified':complete and ledger['conservative_usd']<=.25,
            'unmetered_turns':ledger['unmetered'],
            'note':'Unknown total is never zero spend. Raw meter values are metered subtotals only.'}
