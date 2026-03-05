#!/usr/bin/env python3
"""resource-monitor.py

Lightweight monitor for taurhaus ecosystem resources from WSL.

Default mode (interactive live table):
  ./scripts/resource-monitor.py
  ./scripts/resource-monitor.py --interval 1

CSV logging mode (background collection):
  ./scripts/resource-monitor.py --csv
  ./scripts/resource-monitor.py --csv logs/custom.csv --interval 2

Monitors:
  - WSL: taurhaus-daemon, mesh binaries
  - Windows: taurhaus.exe (queried via powershell.exe/pwsh.exe)

CSV columns:
  timestamp,process_name,pid,cpu_pct,rss_mb,threads,open_fds,inotify_watches,handles
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Tuple

PAGE_SIZE = os.sysconf("SC_PAGE_SIZE") if hasattr(os, "sysconf") else 4096
BYTES_PER_MB = 1024 * 1024
CSV_HEADERS = [
    "timestamp",
    "process_name",
    "pid",
    "cpu_pct",
    "rss_mb",
    "threads",
    "open_fds",
    "inotify_watches",
    "handles",
]


@dataclass
class SampleRow:
    timestamp: str
    process_name: str
    pid: int
    cpu_pct: Optional[float]
    rss_mb: float
    threads: Optional[int]
    open_fds: Optional[int]
    inotify_watches: Optional[int]
    handles: Optional[int]


@dataclass
class CpuState:
    total_ticks_prev: Optional[int]
    proc_ticks_prev: Dict[Tuple[str, int], int]
    windows_cpu_prev: Dict[int, Tuple[float, float]]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Monitor taurhaus process resources.")
    parser.add_argument(
        "-i",
        "--interval",
        type=float,
        default=2.0,
        help="Polling interval in seconds (default: 2)",
    )
    parser.add_argument(
        "--csv",
        "--log",
        dest="csv_path",
        nargs="?",
        const="auto",
        help="Enable CSV logging mode (optional output path)",
    )
    parser.add_argument(
        "--samples",
        type=int,
        default=None,
        help="Stop after N polling cycles (default: run until Ctrl+C)",
    )
    return parser.parse_args()


def now_timestamp() -> str:
    return dt.datetime.now().astimezone().isoformat(timespec="seconds")


def safe_read_text(path: Path) -> Optional[str]:
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None


def read_total_cpu_ticks() -> Optional[int]:
    stat = safe_read_text(Path("/proc/stat"))
    if not stat:
        return None
    first = stat.splitlines()[0].split()
    if not first or first[0] != "cpu":
        return None
    try:
        return sum(int(x) for x in first[1:])
    except ValueError:
        return None


def parse_proc_stat(pid: int) -> Optional[Tuple[int, int, int]]:
    """Return (cpu_ticks, threads, rss_pages) from /proc/<pid>/stat."""
    content = safe_read_text(Path(f"/proc/{pid}/stat"))
    if not content:
        return None
    close_paren = content.rfind(")")
    if close_paren == -1:
        return None

    after = content[close_paren + 2 :].split()
    # fields after comm/state start at stat field 3.
    # utime=14->idx11, stime=15->idx12, num_threads=20->idx17, rss=24->idx21
    try:
        utime = int(after[11])
        stime = int(after[12])
        threads = int(after[17])
        rss_pages = int(after[21])
    except (IndexError, ValueError):
        return None

    return utime + stime, threads, rss_pages


def count_open_fds(pid: int) -> Optional[int]:
    try:
        with os.scandir(f"/proc/{pid}/fd") as entries:
            return sum(1 for _ in entries)
    except OSError:
        return None


def count_inotify_watches(pid: int) -> Optional[int]:
    fdinfo_path = Path(f"/proc/{pid}/fdinfo")
    if not fdinfo_path.is_dir():
        return None

    total = 0
    try:
        for entry in fdinfo_path.iterdir():
            if not entry.is_file():
                continue
            try:
                with entry.open("r", encoding="utf-8", errors="replace") as handle:
                    for line in handle:
                        if line.startswith("inotify wd:"):
                            total += 1
            except OSError:
                continue
    except OSError:
        return None

    return total


def discover_wsl_targets() -> List[Tuple[str, int]]:
    targets: List[Tuple[str, int]] = []

    try:
        proc_entries = [entry for entry in Path("/proc").iterdir() if entry.name.isdigit()]
    except OSError:
        return targets

    for entry in proc_entries:
        pid = int(entry.name)
        comm = safe_read_text(entry / "comm")
        if comm:
            comm = comm.strip()

        cmdline_raw = None
        try:
            cmdline_raw = (entry / "cmdline").read_bytes()
        except OSError:
            pass

        exe_name = None
        if cmdline_raw:
            first_arg = cmdline_raw.split(b"\x00", 1)[0].decode("utf-8", errors="replace")
            exe_name = os.path.basename(first_arg)

        if comm == "taurhaus-daemon" or exe_name == "taurhaus-daemon":
            targets.append(("taurhaus-daemon", pid))
            continue
        if comm == "mesh" or exe_name == "mesh":
            targets.append(("mesh", pid))

    targets.sort(key=lambda item: (item[0], item[1]))
    return targets


def choose_windows_shell() -> Optional[str]:
    for candidate in ("powershell.exe", "pwsh.exe"):
        if shutil.which(candidate):
            return candidate
    return None


def query_windows_rows(powershell_bin: Optional[str], state: CpuState) -> List[SampleRow]:
    if not powershell_bin:
        return []

    command = r"""
$rows = Get-Process -Name taurhaus -ErrorAction SilentlyContinue |
  Select-Object @{Name='process_name';Expression={'taurhaus.exe'}},
                @{Name='IDProcess';Expression={$_.Id}},
                @{Name='CpuSeconds';Expression={$_.CPU}},
                @{Name='WorkingSet';Expression={$_.WorkingSet64}},
                @{Name='ThreadCount';Expression={$_.Threads.Count}},
                @{Name='HandleCount';Expression={$_.HandleCount}}
if ($rows) { $rows | ConvertTo-Csv -NoTypeInformation }
"""

    sample_monotonic = time.monotonic()
    cpu_count = os.cpu_count() or 1

    try:
        result = subprocess.run(
            [powershell_bin, "-NoProfile", "-Command", command],
            capture_output=True,
            text=True,
            check=False,
            timeout=6,
        )
    except (OSError, subprocess.SubprocessError):
        return []

    if result.returncode != 0 or not result.stdout.strip():
        return []

    lines = [line for line in result.stdout.replace("\r", "").splitlines() if line.strip()]
    if not lines:
        return []

    timestamp = now_timestamp()
    rows: List[SampleRow] = []
    windows_cpu_next: Dict[int, Tuple[float, float]] = {}

    try:
        reader = csv.DictReader(lines)
        for row in reader:
            pid_text = row.get("IDProcess")
            if not pid_text:
                continue
            pid = int(pid_text)
            cpu_seconds = float(row.get("CpuSeconds") or 0.0)
            cpu_pct: Optional[float] = None
            prev = state.windows_cpu_prev.get(pid)
            if prev is not None:
                prev_cpu_seconds, prev_monotonic = prev
                cpu_delta = cpu_seconds - prev_cpu_seconds
                time_delta = sample_monotonic - prev_monotonic
                if cpu_delta >= 0 and time_delta > 0:
                    cpu_pct = round((cpu_delta / time_delta) * 100.0 / cpu_count, 2)

            rss_mb = float(row.get("WorkingSet") or 0.0) / BYTES_PER_MB
            threads = int(row.get("ThreadCount") or 0)
            handles = int(row.get("HandleCount") or 0)
            windows_cpu_next[pid] = (cpu_seconds, sample_monotonic)
            rows.append(
                SampleRow(
                    timestamp=timestamp,
                    process_name=row.get("process_name") or "taurhaus.exe",
                    pid=pid,
                    cpu_pct=cpu_pct,
                    rss_mb=rss_mb,
                    threads=threads,
                    open_fds=None,
                    inotify_watches=None,
                    handles=handles,
                )
            )
    except (ValueError, csv.Error):
        return []

    state.windows_cpu_prev = windows_cpu_next
    return rows


def cpu_pct_from_delta(
    process_name: str,
    pid: int,
    proc_ticks: int,
    cpu_total_now: Optional[int],
    state: CpuState,
) -> Optional[float]:
    if cpu_total_now is None or state.total_ticks_prev is None:
        return None

    key = (process_name, pid)
    prev_proc_ticks = state.proc_ticks_prev.get(key)
    if prev_proc_ticks is None:
        return None

    total_delta = cpu_total_now - state.total_ticks_prev
    proc_delta = proc_ticks - prev_proc_ticks
    if total_delta <= 0 or proc_delta < 0:
        return None

    cpu_count = os.cpu_count() or 1
    pct = (proc_delta / total_delta) * cpu_count * 100.0
    return round(pct, 2)


def collect_wsl_rows(state: CpuState) -> List[SampleRow]:
    timestamp = now_timestamp()
    cpu_total_now = read_total_cpu_ticks()
    targets = discover_wsl_targets()
    rows: List[SampleRow] = []

    current_proc_ticks: Dict[Tuple[str, int], int] = {}

    for process_name, pid in targets:
        stat_values = parse_proc_stat(pid)
        if not stat_values:
            continue
        proc_ticks, threads, rss_pages = stat_values
        current_proc_ticks[(process_name, pid)] = proc_ticks

        rss_mb = (rss_pages * PAGE_SIZE) / BYTES_PER_MB
        cpu_pct = cpu_pct_from_delta(process_name, pid, proc_ticks, cpu_total_now, state)

        rows.append(
            SampleRow(
                timestamp=timestamp,
                process_name=process_name,
                pid=pid,
                cpu_pct=cpu_pct,
                rss_mb=round(rss_mb, 2),
                threads=threads,
                open_fds=count_open_fds(pid),
                inotify_watches=count_inotify_watches(pid),
                handles=None,
            )
        )

    state.total_ticks_prev = cpu_total_now
    state.proc_ticks_prev = current_proc_ticks

    return rows


def collect_sample(state: CpuState, powershell_bin: Optional[str]) -> List[SampleRow]:
    rows = collect_wsl_rows(state)
    rows.extend(query_windows_rows(powershell_bin, state))
    rows.sort(key=lambda row: (row.process_name, row.pid))
    return rows


def auto_csv_path() -> Path:
    timestamp = dt.datetime.now().strftime("%Y-%m-%d-%H%M")
    return Path("logs") / f"resource-monitor-{timestamp}.csv"


def ensure_csv_writer(csv_path: Path) -> Tuple[csv.DictWriter, object]:
    csv_path.parent.mkdir(parents=True, exist_ok=True)
    is_new = (not csv_path.exists()) or csv_path.stat().st_size == 0
    handle = csv_path.open("a", encoding="utf-8", newline="")
    writer = csv.DictWriter(handle, fieldnames=CSV_HEADERS)
    if is_new:
        writer.writeheader()
        handle.flush()
    return writer, handle


def row_to_dict(row: SampleRow) -> Dict[str, object]:
    return {
        "timestamp": row.timestamp,
        "process_name": row.process_name,
        "pid": row.pid,
        "cpu_pct": "" if row.cpu_pct is None else f"{row.cpu_pct:.2f}",
        "rss_mb": f"{row.rss_mb:.2f}",
        "threads": "" if row.threads is None else row.threads,
        "open_fds": "" if row.open_fds is None else row.open_fds,
        "inotify_watches": "" if row.inotify_watches is None else row.inotify_watches,
        "handles": "" if row.handles is None else row.handles,
    }


def render_live(rows: List[SampleRow], interval: float, powershell_bin: Optional[str]) -> None:
    print("\033[2J\033[H", end="")
    print(f"taurhaus Resource Monitor (every {interval:g}s, Ctrl+C to stop)")
    print(now_timestamp())
    print()

    if not rows:
        print("No target processes are currently running.")
        return

    print(
        f"{'process':<18} {'pid':>7} {'cpu%':>8} {'rss_mb':>10} {'threads':>8} {'fds':>8} {'inotify':>10} {'handles':>9}"
    )
    print("-" * 86)

    for row in rows:
        cpu = "-" if row.cpu_pct is None else f"{row.cpu_pct:>7.2f}"
        fds = "-" if row.open_fds is None else str(row.open_fds)
        inotify = "-" if row.inotify_watches is None else str(row.inotify_watches)
        handles = "-" if row.handles is None else str(row.handles)
        threads = "-" if row.threads is None else str(row.threads)
        print(
            f"{row.process_name:<18} {row.pid:>7} {cpu:>8} {row.rss_mb:>10.2f} {threads:>8} {fds:>8} {inotify:>10} {handles:>9}"
        )

    print()
    print("Notes:")
    print("- Windows RSS is the WorkingSet64 value from Get-Process.")
    if not powershell_bin:
        print("- Windows metrics unavailable (powershell.exe/pwsh.exe not found in PATH).")


def run_live_mode(args: argparse.Namespace, powershell_bin: Optional[str]) -> int:
    state = CpuState(total_ticks_prev=None, proc_ticks_prev={}, windows_cpu_prev={})
    cycles = 0

    try:
        while True:
            rows = collect_sample(state, powershell_bin)
            render_live(rows, args.interval, powershell_bin)
            cycles += 1
            if args.samples is not None and cycles >= args.samples:
                return 0
            time.sleep(args.interval)
    except KeyboardInterrupt:
        print("\nStopped.")
        return 0


def run_csv_mode(args: argparse.Namespace, powershell_bin: Optional[str]) -> int:
    csv_path = auto_csv_path() if args.csv_path == "auto" else Path(args.csv_path)
    writer, handle = ensure_csv_writer(csv_path)

    print(f"CSV logging mode: {csv_path}")
    print(f"Polling interval: {args.interval:g}s")
    print("Press Ctrl+C to stop.")

    state = CpuState(total_ticks_prev=None, proc_ticks_prev={}, windows_cpu_prev={})
    cycles = 0

    try:
        while True:
            rows = collect_sample(state, powershell_bin)
            for row in rows:
                writer.writerow(row_to_dict(row))
            handle.flush()

            cycles += 1
            if args.samples is not None and cycles >= args.samples:
                return 0
            time.sleep(args.interval)
    except KeyboardInterrupt:
        print("\nStopped.")
        return 0
    finally:
        handle.close()


def main() -> int:
    args = parse_args()
    if args.interval <= 0:
        print("--interval must be > 0", file=sys.stderr)
        return 2
    if args.samples is not None and args.samples <= 0:
        print("--samples must be > 0", file=sys.stderr)
        return 2

    powershell_bin = choose_windows_shell()

    if args.csv_path is None:
        return run_live_mode(args, powershell_bin)
    return run_csv_mode(args, powershell_bin)


if __name__ == "__main__":
    raise SystemExit(main())
