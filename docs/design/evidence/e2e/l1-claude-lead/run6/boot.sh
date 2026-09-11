#!/bin/bash
set -eu
tmux -D -f /dev/null &
for i in {1..100}; do test -S "$TMUX_TMPDIR/tmux-$(id -u)/default" && break; sleep .1; done
tmux set-option -g default-shell /bin/bash
tmux new-session -d -s taurhaus -x 140 -y 48 /bin/bash
/tmp/th-l1-runtime-u73aa6he/home/.local/bin/taurhaus-daemon --port 48637 --data-dir /tmp/th-l1-runtime-u73aa6he/data &
wait $!
