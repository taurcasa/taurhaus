#!/usr/bin/env bash
# resource-monitor.sh — Dev tool: side-by-side resource usage for taurhaus
#
# Shows Windows app (taurhaus.exe) and WSL daemon (taurhaus-daemon) metrics.
# Run from WSL: ./scripts/resource-monitor.sh
# Press Ctrl+C to stop.

set -euo pipefail

INTERVAL=${1:-3}  # Refresh interval in seconds (default: 3)

# Colors
BOLD='\033[1m'
DIM='\033[2m'
GREEN='\033[32m'
YELLOW='\033[33m'
CYAN='\033[36m'
RED='\033[31m'
RESET='\033[0m'

format_bytes() {
    local bytes=$1
    if (( bytes >= 1073741824 )); then
        printf "%.1f GB" "$(echo "scale=1; $bytes / 1073741824" | bc)"
    elif (( bytes >= 1048576 )); then
        printf "%.1f MB" "$(echo "scale=1; $bytes / 1048576" | bc)"
    elif (( bytes >= 1024 )); then
        printf "%.0f KB" "$(echo "scale=0; $bytes / 1024" | bc)"
    else
        printf "%d B" "$bytes"
    fi
}

get_daemon_stats() {
    local pid
    pid=$(pgrep -f "taurhaus-daemon" 2>/dev/null | head -1) || true

    if [[ -z "$pid" ]]; then
        echo "not running"
        return
    fi

    # RSS from /proc (in KB)
    local rss_kb
    rss_kb=$(awk '/VmRSS/ {print $2}' "/proc/$pid/status" 2>/dev/null) || rss_kb=0
    local rss_bytes=$(( rss_kb * 1024 ))

    # CPU and threads from ps
    local cpu threads
    read -r cpu threads < <(ps -p "$pid" -o %cpu=,nlwp= 2>/dev/null) || { cpu=0; threads=0; }

    # Open file descriptors
    local fds
    fds=$(ls "/proc/$pid/fd" 2>/dev/null | wc -l) || fds=0

    # inotify watches — count "inotify wd:" lines across all fdinfo files
    local inotify_watches=0
    if [[ -d "/proc/$pid/fdinfo" ]]; then
        inotify_watches=$(grep -h 'inotify wd' "/proc/$pid/fdinfo/"* 2>/dev/null | wc -l) || inotify_watches=0
    fi

    # System inotify limits
    local max_watches
    max_watches=$(cat /proc/sys/fs/inotify/max_user_watches 2>/dev/null) || max_watches="?"
    local pct=0
    if [[ "$max_watches" != "?" && "$max_watches" -gt 0 ]]; then
        pct=$(( inotify_watches * 100 / max_watches ))
    fi

    printf "PID: %s\n" "$pid"
    printf "  RSS:       %s\n" "$(format_bytes $rss_bytes)"
    printf "  CPU:       %s%%\n" "$cpu"
    printf "  Threads:   %s\n" "$threads"
    printf "  Open FDs:  %s\n" "$fds"
    printf "  inotify:   %s watches / %s (%s%%)\n" "$inotify_watches" "$max_watches" "$pct"
}

get_windows_stats() {
    # Use PowerShell to get taurhaus.exe process info
    local ps_output
    ps_output=$(powershell.exe -NoProfile -Command '
        $p = Get-Process -Name "taurhaus" -ErrorAction SilentlyContinue
        if ($p) {
            $ws = $p.WorkingSet64
            $pm = $p.PrivateMemorySize64
            $cpu = $p.CPU
            $threads = $p.Threads.Count
            $handles = $p.HandleCount
            Write-Output "$($p.Id)|$ws|$pm|$cpu|$threads|$handles"
        } else {
            Write-Output "not_found"
        }
    ' 2>/dev/null | tr -d '\r') || ps_output="error"

    if [[ "$ps_output" == "not_found" || "$ps_output" == "error" || -z "$ps_output" ]]; then
        echo "not running"
        return
    fi

    IFS='|' read -r pid ws pm cpu threads handles <<< "$ps_output"

    printf "PID: %s\n" "$pid"
    printf "  Working Set:    %s\n" "$(format_bytes ${ws:-0})"
    printf "  Private Mem:    %s\n" "$(format_bytes ${pm:-0})"
    printf "  CPU (total):    %ss\n" "${cpu:-0}"
    printf "  Threads:        %s\n" "${threads:-0}"
    printf "  Handles:        %s\n" "${handles:-0}"
}

# Main loop
while true; do
    clear
    printf "${BOLD}${CYAN}═══ taurhaus Resource Monitor ═══${RESET}  ${DIM}(every ${INTERVAL}s, Ctrl+C to stop)${RESET}\n"
    printf "${DIM}%s${RESET}\n\n" "$(date '+%H:%M:%S')"

    printf "${BOLD}${GREEN}▸ WSL Daemon (taurhaus-daemon)${RESET}\n"
    get_daemon_stats
    echo

    printf "${BOLD}${YELLOW}▸ Windows App (taurhaus.exe)${RESET}\n"
    get_windows_stats
    echo

    # Quick system context
    printf "${DIM}── System ──${RESET}\n"
    printf "  WSL memory: %s\n" "$(free -h 2>/dev/null | awk '/Mem:/ {printf "%s used / %s total", $3, $2}')"

    sleep "$INTERVAL"
done
