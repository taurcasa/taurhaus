#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
tmp_dir=$(mktemp -d)
source_checkout="$tmp_dir/source"
origin_repo="$tmp_dir/origin.git"
lane_path="$tmp_dir/lane"
unmerged_lane_path="$tmp_dir/unmerged-lane"
test_home="$tmp_dir/home"
fake_bin="$tmp_dir/bin"
git_log="$tmp_dir/git.log"
bun_log="$tmp_dir/bun.log"
real_git=$(command -v git)

cleanup() {
    if [ -d "$source_checkout/.git" ]; then
        "$real_git" -C "$source_checkout" worktree remove "$lane_path" --force 2>/dev/null || true
        "$real_git" -C "$source_checkout" worktree remove "$unmerged_lane_path" --force 2>/dev/null || true
        "$real_git" -C "$source_checkout" branch -D lane-provision-smoke 2>/dev/null || true
        "$real_git" -C "$source_checkout" branch -D lane-unmerged-smoke 2>/dev/null || true
    fi
    rm -rf "$tmp_dir"
}
trap cleanup EXIT

mkdir -p "$source_checkout" "$test_home" "$fake_bin"
"$real_git" init --bare --initial-branch=main "$origin_repo" >/dev/null
"$real_git" -C "$source_checkout" init --initial-branch=main >/dev/null
"$real_git" -C "$source_checkout" config user.email "lane-test@example.invalid"
"$real_git" -C "$source_checkout" config user.name "Lane recipe test"
printf '%s\n' '{"name":"lane-recipe-fixture","packageManager":"bun@1.2.20"}' > "$source_checkout/package.json"
"$real_git" -C "$source_checkout" add package.json
"$real_git" -C "$source_checkout" commit -m "fixture base" >/dev/null
"$real_git" -C "$source_checkout" remote add origin "$origin_repo"
"$real_git" -C "$source_checkout" push -u origin main >/dev/null
printf '%s\n' 'provision from this HEAD' > "$source_checkout/head-marker.txt"
"$real_git" -C "$source_checkout" add head-marker.txt
"$real_git" -C "$source_checkout" commit -m "fixture head" >/dev/null

printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    'printf '\''%s\n'\'' "$*" >> "$LANE_TEST_GIT_LOG"' \
    'exec "$LANE_TEST_REAL_GIT" "$@"' \
    > "$fake_bin/git"
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    'printf '\''cwd=%s\nargs=%s\n'\'' "$PWD" "$*" > "$LANE_TEST_BUN_LOG"' \
    > "$fake_bin/bun"
chmod +x "$fake_bin/git" "$fake_bin/bun"

run_just() {
    HOME="$test_home" \
        LANE_TEST_REAL_GIT="$real_git" \
        LANE_TEST_GIT_LOG="$git_log" \
        LANE_TEST_BUN_LOG="$bun_log" \
        PATH="$fake_bin:$PATH" \
        just --justfile "$repo_root/justfile" --working-directory "$source_checkout" "$@"
}

run_just provision-worktree "$lane_path" lane-provision-smoke HEAD

config_path="$lane_path/.cargo/config.toml"
shared_target="$test_home/.cache/taurhaus-lane-target"
if [ ! -f "$config_path" ]; then
    echo "provision-worktree did not write $config_path" >&2
    exit 1
fi
if [ ! -d "$shared_target" ]; then
    echo "provision-worktree did not create $shared_target" >&2
    exit 1
fi
grep -Fx "target-dir = \"$shared_target\"" "$config_path" >/dev/null || {
    echo "worktree Cargo config does not name the shared target directory" >&2
    exit 1
}
grep -F 'Deleting ~/.cache/taurhaus-lane-target is always safe' "$config_path" >/dev/null || {
    echo "worktree Cargo config omits the safe-deletion note" >&2
    exit 1
}
grep -Fi "Cargo's own locking serializes concurrent lane builds" "$config_path" >/dev/null || {
    echo "worktree Cargo config omits the concurrent-lane locking note" >&2
    exit 1
}
grep -F 'main checkout and release builds keep src-tauri/target untouched' "$config_path" >/dev/null || {
    echo "worktree Cargo config does not preserve the main/release target" >&2
    exit 1
}
if [ -e "$source_checkout/.cargo/config.toml" ]; then
    echo "provision-worktree wrote Cargo config into the main checkout" >&2
    exit 1
fi
if [ ! -f "$lane_path/head-marker.txt" ]; then
    echo "provision-worktree did not use the requested HEAD base" >&2
    exit 1
fi
if [ "$(sed -n '1p' "$git_log")" != "fetch origin" ]; then
    echo "provision-worktree did not fetch origin first" >&2
    sed -n '1,20p' "$git_log" >&2
    exit 1
fi
if [ "$(sed -n '2p' "$git_log")" != "worktree add $lane_path -b lane-provision-smoke HEAD" ]; then
    echo "provision-worktree did not use the proven worktree-add sequence" >&2
    sed -n '1,20p' "$git_log" >&2
    exit 1
fi
grep -Fx "cwd=$lane_path" "$bun_log" >/dev/null || {
    echo "provision-worktree did not run Bun inside the lane" >&2
    exit 1
}
grep -Fx 'args=install --frozen-lockfile' "$bun_log" >/dev/null || {
    echo "provision-worktree did not use the frozen Bun install" >&2
    exit 1
}

run_just remove-worktree "$lane_path"
if [ -e "$lane_path" ]; then
    echo "remove-worktree left the merged lane on disk" >&2
    exit 1
fi
if "$real_git" -C "$source_checkout" show-ref --verify --quiet refs/heads/lane-provision-smoke; then
    echo "remove-worktree left the merged lane branch behind" >&2
    exit 1
fi

run_just provision-worktree "$unmerged_lane_path" lane-unmerged-smoke HEAD
printf '%s\n' 'unmerged lane work' > "$unmerged_lane_path/unmerged.txt"
"$real_git" -C "$unmerged_lane_path" add unmerged.txt
"$real_git" -C "$unmerged_lane_path" commit -m "unmerged lane work" >/dev/null

unmerged_output="$tmp_dir/unmerged-remove.log"
unmerged_status=0
run_just remove-worktree "$unmerged_lane_path" >"$unmerged_output" 2>&1 || unmerged_status=$?
if [ "$unmerged_status" -eq 0 ]; then
    echo "remove-worktree accepted an unmerged branch without FORCE_BRANCH=1" >&2
    exit 1
fi
if ! grep -Fq 'is not merged' "$unmerged_output" || ! grep -Fq 'FORCE_BRANCH=1' "$unmerged_output"; then
    echo "remove-worktree did not explain the unmerged-branch guard" >&2
    sed -n '1,80p' "$unmerged_output" >&2
    exit 1
fi
if [ ! -d "$unmerged_lane_path" ]; then
    echo "remove-worktree deleted an unmerged lane before refusing it" >&2
    exit 1
fi
if ! "$real_git" -C "$source_checkout" show-ref --verify --quiet refs/heads/lane-unmerged-smoke; then
    echo "remove-worktree deleted an unmerged branch before refusing it" >&2
    exit 1
fi

FORCE_BRANCH=1 run_just remove-worktree "$unmerged_lane_path"
if [ -e "$unmerged_lane_path" ]; then
    echo "forced remove-worktree left the lane on disk" >&2
    exit 1
fi
if "$real_git" -C "$source_checkout" show-ref --verify --quiet refs/heads/lane-unmerged-smoke; then
    echo "forced remove-worktree left the lane branch behind" >&2
    exit 1
fi

echo "lane worktree recipe guard passed."
