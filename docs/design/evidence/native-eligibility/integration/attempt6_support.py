"""Lossless overlapping-buffer export; paid accounting uses the audited helper."""
from attempt5_support import clean, ledger


def new_events(previous, current):
    for overlap in range(min(len(previous), len(current)), 0, -1):
        if previous[-overlap:] == current[:overlap]:
            return current[overlap:]
    return current
