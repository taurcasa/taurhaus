#!/usr/bin/env bash
# Mesh lock-flow release: fast-forward gated branches onto mesh master, bump the
# patch version, tag, build the release binary, then update taurhaus's lock files,
# bundle, verify and install the mesh on a release/mesh-<version>-lock worktree.
#
#   scripts/mesh-lock-release.sh <version> <branch> [<branch>...]
#
# Environment: MESH_PROJECT (default ~/projects/mesh), TAURHAUS_PROJECT (default the
# checkout containing this script), LOCK_WORKTREE (default ~/projects/taurhaus-lock),
# SESSION_URL (optional Claude-Session trailer).
#
# Speed rule (2026-09-13): a fast-forward leaves master's tree byte-identical to a
# branch tip that already passed its lane gates, so the combined-tree test run is
# skipped; a real merge of several branches is a new tree and is gated here.
# The mesh USAGE.md executor matrix is a manual edit before running this script.
set -euo pipefail

VERSION="${1:?usage: mesh-lock-release.sh <version> <branch>...}"
shift
[ "$#" -ge 1 ] || { echo "usage: mesh-lock-release.sh <version> <branch>..." >&2; exit 2; }
MESH="${MESH_PROJECT:-$HOME/projects/mesh}"
TH="${TAURHAUS_PROJECT:-$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)}"
LOCK_WT="${LOCK_WORKTREE:-$HOME/projects/taurhaus-lock}"
TRAILERS="Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>${SESSION_URL:+
Claude-Session: $SESSION_URL}"

echo "=== mesh lock release $VERSION start $(date -u)"
while pgrep -af '(^|/)[c]argo( |$)' >/dev/null; do echo "waiting for cargo…"; sleep 30; done

cd "$MESH"
[ "$(git rev-parse --abbrev-ref HEAD)" = master ] || { echo "mesh not on master" >&2; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "mesh tree dirty" >&2; exit 1; }
CURRENT="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)"
[ "$CURRENT" != "$VERSION" ] || { echo "mesh is already $VERSION" >&2; exit 1; }
BASE="$(git rev-parse HEAD)"
for b in "$@"; do
    git merge-base --is-ancestor "$BASE" "$b" || { echo "$b does not descend from master $BASE" >&2; exit 1; }
done

# 1. combine. A fast-forward reproduces a gated tree; anything else is a new tree.
NEEDS_GATE=0
for b in "$@"; do
    if git merge-base --is-ancestor HEAD "$b"; then
        git merge --ff-only "$b"
    else
        git merge --no-ff --no-edit -m "merge: $b into master for mesh $VERSION

$TRAILERS" "$b"
        NEEDS_GATE=1
    fi
done
MERGED="$(git rev-parse --short HEAD)"
echo "master now $MERGED (gate needed: $NEEDS_GATE)"
if [ "$NEEDS_GATE" = 1 ]; then
    just check-quick
    RUST_TEST_THREADS="${RUST_TEST_THREADS:-1}" just test
else
    echo "tree identical to gated branch tip $(git rev-parse --short "${!#}") — combined-tree test skipped"
fi

# 2. bump + commit
sed -i "s/^version = \"$CURRENT\"\$/version = \"$VERSION\"/" Cargo.toml
grep -q "^version = \"$VERSION\"\$" Cargo.toml || { echo "version bump failed" >&2; exit 1; }
cargo update -p mesh --offline 2>/dev/null || cargo update -p mesh
git add Cargo.toml Cargo.lock
git commit -q -m "release: mesh $VERSION

Master $MERGED plus the version bump. Protocol and schema unchanged.

$TRAILERS"
COMMIT="$(git rev-parse HEAD)"
echo "mesh master now $COMMIT"

# 3. release binary (build.rs stamps MESH_GIT_COMMIT=HEAD)
cargo build --release --bin mesh
BUILT="$(./target/release/mesh version --json | python3 -c 'import json,sys;print(json.load(sys.stdin)["git_commit"])')"
[ "$BUILT" = "$COMMIT" ] || { echo "built git_commit $BUILT != $COMMIT" >&2; exit 1; }
git tag -f "mesh-$VERSION" "$COMMIT"

# 4. taurhaus lock flow on a worktree from origin/main
cd "$TH"
git fetch -q origin
[ ! -e "$LOCK_WT" ] || { echo "$LOCK_WT already exists; remove it first" >&2; exit 1; }
git worktree add -q -b "release/mesh-$VERSION-lock" "$LOCK_WT" origin/main
cd "$LOCK_WT"
just update-mesh-lock "$VERSION" 1 1 "$COMMIT"
just bundle-mesh
just mesh-verify-lock
just install-mesh
"$HOME/.local/bin/mesh" version --json
git add src-tauri/resources/mesh.lock.json src-tauri/resources/mesh.manifest.json src-tauri/resources/mesh.version
git commit -q -m "Release: lock mesh $VERSION ($(git -C "$MESH" rev-parse --short HEAD))

Mesh master $COMMIT = $BASE + $* + the version bump; tag mesh-$VERSION.

$TRAILERS"
echo "=== mesh lock release $VERSION done $(date -u): lock commit $(git rev-parse --short HEAD) on release/mesh-$VERSION-lock in $LOCK_WT"
echo "next: push the branch, open the PR (short body, merge on three green checks), then just build-windows && just install-windows from $LOCK_WT"
