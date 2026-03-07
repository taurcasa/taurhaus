#!/usr/bin/env bash
# Automated Rust target cleanup for projects under ~/projects.
#
# Custom cleanup strategy:
# - scans target/debug and target/release
# - only touches stale entries inside build/, deps/, incremental/, and .fingerprint/
# - never deletes outside those subdirectories
# - supports dry-run and timestamped logs
#
# Usage:
#   ./scripts/rust-cleanup.sh
#   ./scripts/rust-cleanup.sh --dry-run
#   ./scripts/rust-cleanup.sh --days 5
#   ./scripts/rust-cleanup.sh --root ~/projects
#
# Scheduling:
#   See `./scripts/rust-cleanup-install.sh --help`

set -euo pipefail

ROOT_DEFAULT="$HOME/projects"
DAYS_DEFAULT=2
LOG_ROOT_DEFAULT="${XDG_STATE_HOME:-$HOME/.local/state}/rust-cleanup"

usage() {
  cat <<'USAGE'
Usage: rust-cleanup.sh [options]

Options:
  --dry-run           Preview cleanup without deleting artifacts
  --days N            Delete entries older than N days (default: 2)
  --root PATH         Scan PATH recursively for Rust target directories
  --log-dir PATH      Write logs under PATH (default: ~/.local/state/rust-cleanup)
  -h, --help          Show this help

Notes:
  The script only considers entries inside:
    target/debug/build
    target/debug/deps
    target/debug/incremental
    target/debug/.fingerprint
    target/release/build
    target/release/deps
    target/release/incremental
    target/release/.fingerprint
USAGE
}

human_size() {
  local bytes="${1:-0}"
  if command -v numfmt >/dev/null 2>&1; then
    numfmt --to=iec-i --suffix=B "$bytes"
  else
    echo "${bytes}B"
  fi
}

DRY_RUN=0
DAYS="$DAYS_DEFAULT"
ROOT="$ROOT_DEFAULT"
LOG_ROOT="$LOG_ROOT_DEFAULT"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --days)
      DAYS="${2:-}"
      shift 2
      ;;
    --root)
      ROOT="${2:-}"
      shift 2
      ;;
    --log-dir)
      LOG_ROOT="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ ! "$DAYS" =~ ^[0-9]+$ ]]; then
  echo "Error: --days must be a non-negative integer" >&2
  exit 1
fi

ROOT="$(realpath -m "$ROOT")"
if [[ ! -d "$ROOT" ]]; then
  echo "Error: root directory does not exist: $ROOT" >&2
  exit 1
fi
if [[ "$ROOT" == "/" ]]; then
  echo "Error: refusing to run against /" >&2
  exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo "Error: python3 not found on PATH" >&2
  exit 1
fi

mkdir -p "$LOG_ROOT"
timestamp="$(date '+%Y%m%d-%H%M%S')"
log_file="$LOG_ROOT/rust-cleanup-${timestamp}-${DAYS}d.log"

python3 - "$ROOT" "$DAYS" "$DRY_RUN" "$log_file" <<'PY'
import json
import os
import shutil
import sys
import time
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(sys.argv[1]).resolve()
DAYS = int(sys.argv[2])
DRY_RUN = sys.argv[3] == "1"
LOG_FILE = Path(sys.argv[4])
CUTOFF = time.time() - DAYS * 86400
PROFILES = ("debug", "release")
CLEANABLE_SUBDIRS = ("build", "deps", "incremental", ".fingerprint")


def iter_target_dirs(root: Path):
    for dirpath, dirnames, _filenames in os.walk(root):
        path = Path(dirpath)
        if path.name == "target":
            yield path
            dirnames[:] = []
            continue
        dirnames[:] = [name for name in dirnames if name != "node_modules"]


def file_bytes(path: Path) -> int:
    if path.is_symlink() or path.is_file():
        try:
            return path.lstat().st_size
        except FileNotFoundError:
            return 0
    total = 0
    for child_root, _dirnames, filenames in os.walk(path):
        for filename in filenames:
            child = Path(child_root, filename)
            try:
                total += child.lstat().st_size
            except FileNotFoundError:
                continue
    return total


def newest_mtime(path: Path) -> float:
    try:
        latest = path.lstat().st_mtime
    except FileNotFoundError:
        return 0.0
    if path.is_symlink() or path.is_file():
        return latest
    for child_root, dirnames, filenames in os.walk(path):
        for name in dirnames + filenames:
            child = Path(child_root, name)
            try:
                latest = max(latest, child.lstat().st_mtime)
            except FileNotFoundError:
                continue
    return latest


def is_under(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def collect_actions(target: Path):
    actions = []
    for profile in PROFILES:
        profile_dir = target / profile
        if not profile_dir.is_dir():
            continue
        for subdir in CLEANABLE_SUBDIRS:
            subdir_path = profile_dir / subdir
            if not subdir_path.is_dir():
                continue
            for child in sorted(subdir_path.iterdir()):
                latest = newest_mtime(child)
                if latest >= CUTOFF:
                    continue
                size = file_bytes(child)
                if size <= 0:
                    continue
                if not is_under(child.resolve(strict=False), subdir_path.resolve(strict=False)):
                    continue
                actions.append({
                    "target": str(target),
                    "profile": profile,
                    "bucket": subdir,
                    "path": str(child),
                    "size": size,
                    "age_days": (time.time() - latest) / 86400,
                })
    return actions


def delete_path(path: Path):
    if path.is_symlink() or path.is_file():
        path.unlink(missing_ok=True)
    elif path.is_dir():
        shutil.rmtree(path)


def fmt_bytes(size: int) -> str:
    value = float(size)
    for unit in ("B", "KiB", "MiB", "GiB", "TiB"):
        if value < 1024.0 or unit == "TiB":
            return f"{value:.2f} {unit}"
        value /= 1024.0
    return f"{size} B"


def target_bytes(root: Path) -> int:
    total = 0
    for target in iter_target_dirs(root):
        total += file_bytes(target)
    return total


def log(line: str = ""):
    print(line)
    with LOG_FILE.open("a", encoding="utf-8") as fh:
        fh.write(line + "\n")


all_actions = []
for target in sorted(iter_target_dirs(ROOT)):
    all_actions.extend(collect_actions(target))

before_bytes = target_bytes(ROOT)
per_target = defaultdict(int)
per_bucket = defaultdict(int)
for action in all_actions:
    per_target[action["target"]] += action["size"]
    per_bucket[action["bucket"]] += action["size"]

log("== rust-cleanup ==")
log(f"timestamp: {datetime.now(timezone.utc).astimezone().isoformat(timespec='seconds')}")
log(f"root: {ROOT}")
log(f"days: {DAYS}")
log(f"dry_run: {int(DRY_RUN)}")
log(f"target_file_bytes_before: {before_bytes} ({fmt_bytes(before_bytes)})")
log(f"candidate_count: {len(all_actions)}")
log()

if all_actions:
    log("== candidates by target ==")
    for target, size in sorted(per_target.items(), key=lambda item: item[1], reverse=True):
        log(f"{fmt_bytes(size):>12}  {target}")
    log()
    log("== candidates by bucket ==")
    for bucket, size in sorted(per_bucket.items(), key=lambda item: item[1], reverse=True):
        log(f"{fmt_bytes(size):>12}  {bucket}")
    log()
    log("== candidate entries ==")
    for action in sorted(all_actions, key=lambda item: item["size"], reverse=True):
        log(
            f"{fmt_bytes(action['size']):>12}  {action['age_days']:5.1f}d  "
            f"{action['profile']}/{action['bucket']}  {action['path']}"
        )
else:
    log("No stale entries matched the cleanup policy.")

if not DRY_RUN:
    for action in all_actions:
        delete_path(Path(action["path"]))

after_bytes = target_bytes(ROOT)
reclaimed = sum(action["size"] for action in all_actions)
log()
log("== summary ==")
log(f"log_file: {LOG_FILE}")
log(f"target_file_bytes_after: {after_bytes} ({fmt_bytes(after_bytes)})")
if DRY_RUN:
    log(f"space_reclaimable: {reclaimed} ({fmt_bytes(reclaimed)})")
else:
    log(f"space_reclaimed: {reclaimed} ({fmt_bytes(reclaimed)})")
PY
