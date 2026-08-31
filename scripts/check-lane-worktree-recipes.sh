#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
tmp_dir=$(mktemp -d)
source_checkout="$tmp_dir/source"
origin_repo="$tmp_dir/origin.git"
lane_path="$tmp_dir/lane"
unmerged_lane_path="$tmp_dir/unmerged-lane"
relative_invocation_dir="$source_checkout/nested"
relative_lane_path="$relative_invocation_dir/relative-lane"
relative_remove_lane_path="$relative_invocation_dir/relative-remove-lane"
misplaced_relative_lane_path="$source_checkout/relative-lane"
stacked_lane_path="$tmp_dir/stacked-lane"
failed_install_lane_path="$tmp_dir/failed-install-lane"
test_home="$tmp_dir/home"
fake_bin="$tmp_dir/bin"
git_log="$tmp_dir/git.log"
bun_log="$tmp_dir/bun.log"
bunx_log="$tmp_dir/bunx.log"
real_git=$(command -v git)

cleanup() {
    if [ -d "$source_checkout/.git" ]; then
        "$real_git" -C "$source_checkout" worktree remove "$lane_path" --force 2>/dev/null || true
        "$real_git" -C "$source_checkout" worktree remove "$unmerged_lane_path" --force 2>/dev/null || true
        "$real_git" -C "$source_checkout" worktree remove "$relative_lane_path" --force 2>/dev/null || true
        "$real_git" -C "$source_checkout" worktree remove "$relative_remove_lane_path" --force 2>/dev/null || true
        "$real_git" -C "$source_checkout" worktree remove "$misplaced_relative_lane_path" --force 2>/dev/null || true
        "$real_git" -C "$source_checkout" worktree remove "$stacked_lane_path" --force 2>/dev/null || true
        "$real_git" -C "$source_checkout" worktree remove "$failed_install_lane_path" --force 2>/dev/null || true
        "$real_git" -C "$source_checkout" branch -D lane-provision-smoke 2>/dev/null || true
        "$real_git" -C "$source_checkout" branch -D lane-unmerged-smoke 2>/dev/null || true
        "$real_git" -C "$source_checkout" branch -D lane-relative-smoke 2>/dev/null || true
        "$real_git" -C "$source_checkout" branch -D lane-relative-remove-smoke 2>/dev/null || true
        "$real_git" -C "$source_checkout" branch -D lane-stacked-smoke 2>/dev/null || true
        "$real_git" -C "$source_checkout" branch -D lane-failed-install-smoke 2>/dev/null || true
        "$real_git" -C "$source_checkout" branch -D stacked-parent-smoke 2>/dev/null || true
    fi
    rm -rf "$tmp_dir"
}
trap cleanup EXIT

mkdir -p "$source_checkout" "$relative_invocation_dir" "$test_home" "$fake_bin"
"$real_git" init --bare --initial-branch=main "$origin_repo" >/dev/null 2>&1
"$real_git" -C "$source_checkout" init --initial-branch=main >/dev/null 2>&1
"$real_git" -C "$source_checkout" config user.email "lane-test@example.invalid"
"$real_git" -C "$source_checkout" config user.name "Lane recipe test"
printf '%s\n' '{"name":"lane-recipe-fixture","packageManager":"bun@1.2.20"}' > "$source_checkout/package.json"
cp "$repo_root/.gitignore" "$source_checkout/.gitignore"
"$real_git" -C "$source_checkout" add package.json .gitignore
"$real_git" -C "$source_checkout" commit -m "fixture base" >/dev/null 2>&1
"$real_git" -C "$source_checkout" remote add origin "$origin_repo"
"$real_git" -C "$source_checkout" push -u origin main >/dev/null 2>&1
printf '%s\n' 'provision from this HEAD' > "$source_checkout/head-marker.txt"
"$real_git" -C "$source_checkout" add head-marker.txt
"$real_git" -C "$source_checkout" commit -m "fixture head" >/dev/null 2>&1
"$real_git" -C "$source_checkout" push origin main >/dev/null 2>&1

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
    'if [ "${LANE_TEST_BUN_FAIL:-0}" = "1" ]; then exit 42; fi' \
    > "$fake_bin/bun"
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    'printf '\''cwd=%s target=%s args=%s\n'\'' "$PWD" "${CARGO_TARGET_DIR:-<unset>}" "$*" >> "$LANE_TEST_BUNX_LOG"' \
    > "$fake_bin/bunx"
chmod +x "$fake_bin/git" "$fake_bin/bun" "$fake_bin/bunx"

run_just_from() {
    local invocation_dir="$1"
    local output
    local status=0
    shift
    output=$(
        cd "$invocation_dir"
        HOME="$test_home" \
            LANE_TEST_REAL_GIT="$real_git" \
            LANE_TEST_GIT_LOG="$git_log" \
            LANE_TEST_BUN_LOG="$bun_log" \
            LANE_TEST_BUNX_LOG="$bunx_log" \
            LANE_TEST_BUN_FAIL="${LANE_TEST_BUN_FAIL:-0}" \
            PATH="$fake_bin:$PATH" \
            just --justfile "$repo_root/justfile" --working-directory "$source_checkout" "$@" 2>&1
    ) || status=$?
    if [ "$status" -ne 0 ]; then
        [ -z "$output" ] || printf '%s\n' "$output" >&2
        return "$status"
    fi
}

run_just() {
    run_just_from "$source_checkout" "$@"
}

regression_failures=0
record_regression_failure() {
    echo "$1" >&2
    regression_failures=$((regression_failures + 1))
}

# // Regression: cf6f7d55 redirected lane artifacts while consumers retained checkout-local paths.
grep -Fq '"$cargo_target_dir/release/$DAEMON_BIN"' "$repo_root/justfile" || \
    record_regression_failure "install-daemon does not use Cargo's resolved target directory"
grep -Fq '"$cargo_target_dir/release/taurhaus-daemon"' "$repo_root/justfile" || \
    record_regression_failure "bundle-daemon does not use Cargo's resolved target directory"
grep -Fq "resolve(cargoTargetDir, 'debug', 'taurhaus')" "$repo_root/e2e/wdio.conf.js" || \
    record_regression_failure "E2E app launch does not use Cargo's resolved target directory"
grep -Fq "resolve(cargoTargetDir, 'debug', 'taurhaus-daemon')" "$repo_root/e2e/wdio.conf.js" || \
    record_regression_failure "E2E daemon launch does not use Cargo's resolved target directory"
grep -Fq 'target_directory' "$repo_root/e2e/wdio.conf.js" || \
    record_regression_failure "E2E configuration does not resolve Cargo metadata target_directory"

# // Regression: cf6f7d55 excluded the lane-only Cargo redirect with an unanchored rsync
# // pattern, which matches a .cargo basename at any depth and therefore dropped the
# // tracked src-tauri/.cargo/audit.toml from both --delete platform syncs.
recipe_excludes() {
    awk -v recipe="$1" '
        $0 ~ "^" recipe ":" { inside = 1; next }
        inside && /^[^ \t]/ { inside = 0 }
        inside && /--exclude=/ { print }
    ' "$repo_root/justfile" | sed -n "s/.*--exclude='\([^']*\)'.*/\1/p"
}

assert_sync_excludes() {
    local recipe="$1"
    local fixture="$tmp_dir/sync-$recipe"
    local args=()
    local pattern
    while IFS= read -r pattern; do
        args+=("--exclude=$pattern")
    done < <(recipe_excludes "$recipe")
    if [ "${#args[@]}" -eq 0 ]; then
        record_regression_failure "$recipe declares no rsync excludes"
        return
    fi
    rm -rf "$fixture"
    mkdir -p "$fixture/src/.cargo" "$fixture/src/src-tauri/.cargo" "$fixture/src/node_modules" "$fixture/dst"
    printf '%s\n' 'lane-only redirect' > "$fixture/src/.cargo/config.toml"
    printf '%s\n' 'tracked audit config' > "$fixture/src/src-tauri/.cargo/audit.toml"
    printf '%s\n' 'junk' > "$fixture/src/node_modules/marker"
    rsync -a --delete "${args[@]}" "$fixture/src/" "$fixture/dst/"
    if [ ! -f "$fixture/dst/src-tauri/.cargo/audit.toml" ]; then
        record_regression_failure "$recipe drops the tracked src-tauri/.cargo/audit.toml from the build sync"
    fi
    if [ -e "$fixture/dst/.cargo/config.toml" ]; then
        record_regression_failure "$recipe syncs the lane-only .cargo/config.toml into the platform build"
    fi
    if [ -e "$fixture/dst/node_modules" ]; then
        record_regression_failure "$recipe no longer excludes node_modules"
    fi
}

if command -v rsync >/dev/null 2>&1; then
    assert_sync_excludes sync-windows
    assert_sync_excludes sync-macos
else
    for sync_recipe in sync-windows sync-macos; do
        recipe_excludes "$sync_recipe" | grep -Fxq '/.cargo/' || record_regression_failure \
            "$sync_recipe does not anchor its lane-only .cargo exclude (rsync missing, behavioural check skipped)"
    done
fi

# // Regression: cf6f7d55 resolved the E2E binaries from Cargo's target directory, so every
# // provisioned lane launched one shared debug build and a concurrent lane's compile replaced
# // the app underneath a live run. The E2E recipes must pin their artifacts to the checkout.
: > "$bunx_log"
e2e_target="$source_checkout/src-tauri/target"
for e2e_recipe in build-e2e test-e2e test-e2e-full; do
    run_just "$e2e_recipe" || record_regression_failure "$e2e_recipe failed with a stubbed bunx"
done
run_just test-e2e-spec mesh-workflow || record_regression_failure "test-e2e-spec failed with a stubbed bunx"
e2e_invocations=$(grep -c '^cwd=' "$bunx_log" || true)
if [ "$e2e_invocations" -ne 4 ]; then
    record_regression_failure "expected four stubbed bunx invocations from the E2E recipes, saw $e2e_invocations"
fi
if grep -Fvq "target=$e2e_target " "$bunx_log"; then
    record_regression_failure "an E2E recipe did not pin CARGO_TARGET_DIR to the checkout-local target"
    sed -n '1,10p' "$bunx_log" >&2
fi

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
grep -Fi "Cargo's own locking serializes concurrent lane compiles" "$config_path" >/dev/null || {
    echo "worktree Cargo config omits the concurrent-lane locking note" >&2
    exit 1
}
grep -F 'last-writer-wins' "$config_path" >/dev/null || {
    echo "worktree Cargo config omits the shared-artifact caveat" >&2
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
# // Regression: cf6f7d55 left the generated lane Cargo config permanently untracked.
if ! "$real_git" -C "$lane_path" check-ignore --quiet .cargo/config.toml; then
    record_regression_failure "provision-worktree generated a Cargo config that Git does not ignore"
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

# // Regression: cf6f7d55 resolved relative lane paths from the justfile directory.
run_just_from "$relative_invocation_dir" provision-worktree relative-lane lane-relative-smoke HEAD
if [ ! -d "$relative_lane_path" ]; then
    record_regression_failure "provision-worktree did not resolve a relative path from the invocation directory"
fi
relative_remove_status=0
run_just_from "$relative_invocation_dir" remove-worktree relative-lane || relative_remove_status=$?
if [ "$relative_remove_status" -ne 0 ]; then
    record_regression_failure "remove-worktree did not resolve a relative path from the invocation directory"
    actual_relative_lane="$relative_lane_path"
    [ -d "$actual_relative_lane" ] || actual_relative_lane="$misplaced_relative_lane_path"
    run_just remove-worktree "$actual_relative_lane"
fi
run_just provision-worktree "$relative_remove_lane_path" lane-relative-remove-smoke HEAD
relative_remove_status=0
run_just_from "$relative_invocation_dir" remove-worktree relative-remove-lane || relative_remove_status=$?
if [ "$relative_remove_status" -ne 0 ]; then
    record_regression_failure "remove-worktree did not resolve an existing relative path from the invocation directory"
    run_just remove-worktree "$relative_remove_lane_path"
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

# // Regression: be9d2897 treated a stacked feature HEAD as proof that a lane reached main.
run_just provision-worktree "$stacked_lane_path" lane-stacked-smoke HEAD
printf '%s\n' 'stacked lane work' > "$stacked_lane_path/stacked.txt"
"$real_git" -C "$stacked_lane_path" add stacked.txt
"$real_git" -C "$stacked_lane_path" commit -m "stacked lane work" >/dev/null 2>&1
"$real_git" -C "$source_checkout" branch stacked-parent-smoke lane-stacked-smoke
"$real_git" -C "$source_checkout" switch stacked-parent-smoke >/dev/null 2>&1
stacked_output="$tmp_dir/stacked-remove.log"
stacked_status=0
run_just remove-worktree "$stacked_lane_path" >"$stacked_output" 2>&1 || stacked_status=$?
if [ "$stacked_status" -eq 0 ]; then
    record_regression_failure "remove-worktree deleted a lane merged only into the current stacked feature HEAD"
else
    if ! grep -Fq 'origin/main' "$stacked_output"; then
        record_regression_failure "remove-worktree did not name origin/main in its integration guard message"
    fi
    FORCE_BRANCH=1 run_just remove-worktree "$stacked_lane_path"
fi
"$real_git" -C "$source_checkout" switch main >/dev/null 2>&1
"$real_git" -C "$source_checkout" branch -D stacked-parent-smoke >/dev/null 2>&1

# // Regression: cf6f7d55 wrote the lane Cargo config only after Bun succeeded.
failed_install_output="$tmp_dir/failed-install.log"
failed_install_status=0
LANE_TEST_BUN_FAIL=1 run_just provision-worktree "$failed_install_lane_path" lane-failed-install-smoke HEAD \
    >"$failed_install_output" 2>&1 || failed_install_status=$?
if [ "$failed_install_status" -eq 0 ]; then
    echo "provision-worktree unexpectedly accepted the seeded Bun install failure" >&2
    exit 1
fi
if [ ! -f "$failed_install_lane_path/.cargo/config.toml" ]; then
    record_regression_failure "a failed Bun install left the lane without its Cargo config"
fi
run_just remove-worktree "$failed_install_lane_path"

printf '%s\n' 'discardable Cargo artifact' > "$shared_target/artifact.txt"
run_just clean-lane-target
if [ -e "$shared_target" ]; then
    echo "clean-lane-target left the shared Cargo target on disk" >&2
    exit 1
fi
run_just clean-lane-target

if ! grep -Fq '`just provision-worktree PATH BRANCH [BASE]`' "$repo_root/CONTRIBUTING.md"; then
    echo "CONTRIBUTING.md does not name the lane provisioning recipe" >&2
    exit 1
fi
if ! grep -Fq '**Cached lane worktrees**' "$repo_root/CHANGELOG.md"; then
    echo "CHANGELOG.md does not record cached lane worktrees" >&2
    exit 1
fi

if [ "$regression_failures" -ne 0 ]; then
    echo "$regression_failures lane worktree regression check(s) failed." >&2
    exit 1
fi

echo "lane worktree recipe guard passed."
