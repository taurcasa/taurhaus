#!/usr/bin/env bash

set -u
set -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

cd "$ROOT" || exit 1

declare -a BG_JOBS=()

run_cmd() {
  local name="$1"
  shift
  local cmd="$*"
  bash -lc "$cmd" >"$TMP_DIR/${name}.out" 2>&1
  echo $? >"$TMP_DIR/${name}.status"
}

run_bg() {
  local name="$1"
  shift
  local cmd="$*"
  (
    bash -lc "$cmd" >"$TMP_DIR/${name}.out" 2>&1
    echo $? >"$TMP_DIR/${name}.status"
  ) &
  BG_JOBS+=("$!:${name}")
}

read_status() {
  local name="$1"
  if [[ -f "$TMP_DIR/${name}.status" ]]; then
    cat "$TMP_DIR/${name}.status"
  else
    echo "1"
  fi
}

strip_ansi() {
  local in_file="$1"
  local out_file="$2"
  sed -E 's/\x1B\[[0-9;]*[[:alpha:]]//g' "$in_file" >"$out_file"
}

commify() {
  local value="$1"
  if [[ ! "$value" =~ ^[0-9]+$ ]]; then
    echo "$value"
    return
  fi
  local out="$value"
  while [[ "$out" =~ ^([0-9]+)([0-9]{3})$ ]]; do
    out="${BASH_REMATCH[1]},${BASH_REMATCH[2]}"
  done
  echo "$out"
}

ratio_or_na() {
  local numerator="$1"
  local denominator="$2"
  if [[ ! "$numerator" =~ ^[0-9]+$ ]] || [[ ! "$denominator" =~ ^[0-9]+$ ]] || [[ "$denominator" -eq 0 ]]; then
    echo "n/a"
    return
  fi
  awk -v n="$numerator" -v d="$denominator" 'BEGIN { printf "%.1f", n / d }'
}

status_icon() {
  local exit_code="$1"
  if [[ "$exit_code" -eq 0 ]]; then
    echo "OK"
  else
    echo "FAIL (exit ${exit_code})"
  fi
}

# Run long frontend checks in parallel while Rust checks run.
run_bg "coverage" "npx vitest run --coverage"
run_bg "svelte" "npm run check"
run_bg "vite_build" "npm run build -- --manifest"

# Run Rust lanes in foreground (avoids cargo target lock thrashing).
run_cmd "rust_list" "cd src-tauri && cargo test -- --list"
run_cmd "clippy" "cd src-tauri && cargo clippy --all-targets"

# Static codebase metrics (fast, no build).
rust_loc="$(find src-tauri/src -type f -name '*.rs' -print0 | xargs -0 wc -l | tail -n1 | awk '{print $1}')"
frontend_loc="$(find src -type f \( -name '*.js' -o -name '*.svelte' \) -print0 | xargs -0 wc -l | tail -n1 | awk '{print $1}')"
e2e_spec_count="$(find e2e/specs -type f -name '*.js' | wc -l | tr -d ' ')"
e2e_it_count="$(grep -RhoE '\bit[[:space:]]*\(' e2e/specs --include='*.js' | wc -l | tr -d ' ')"

for entry in "${BG_JOBS[@]}"; do
  pid="${entry%%:*}"
  wait "$pid" || true
done

strip_ansi "$TMP_DIR/rust_list.out" "$TMP_DIR/rust_list.clean"
strip_ansi "$TMP_DIR/clippy.out" "$TMP_DIR/clippy.clean"
strip_ansi "$TMP_DIR/coverage.out" "$TMP_DIR/coverage.clean"
strip_ansi "$TMP_DIR/svelte.out" "$TMP_DIR/svelte.clean"
strip_ansi "$TMP_DIR/vite_build.out" "$TMP_DIR/vite_build.clean"

rust_status="$(read_status rust_list)"
clippy_status="$(read_status clippy)"
coverage_status="$(read_status coverage)"
svelte_status="$(read_status svelte)"
vite_status="$(read_status vite_build)"

rust_test_count="$(grep -c 'test$' "$TMP_DIR/rust_list.clean" 2>/dev/null || echo 0)"

sed -nE 's/^[[:space:]]*([[:alnum:]_]+(::[[:alnum:]_]+)*): test$/\1/p' "$TMP_DIR/rust_list.clean" \
  | awk -F'::' '{ counts[$1]++ } END { for (k in counts) printf "%s\t%d\n", k, counts[k] }' \
  | sort -t $'\t' -k2,2nr -k1,1 >"$TMP_DIR/rust_modules.tsv"

frontend_test_count="$(
  sed -nE 's/^[[:space:]]*Tests[[:space:]]+.*\(([0-9,]+)\)[[:space:]]*$/\1/p' "$TMP_DIR/coverage.clean" \
    | tail -n1 \
    | tr -d ',' \
    || true
)"
if [[ -z "$frontend_test_count" ]]; then
  frontend_test_count="$(
    grep -Eo '\([0-9,]+[[:space:]]+tests?\)' "$TMP_DIR/coverage.clean" \
      | tr -d '()' \
      | awk '{gsub(/,/, "", $1); sum += $1} END {print sum + 0}'
  )"
fi
if [[ -z "$frontend_test_count" ]]; then
  frontend_test_count=0
fi

coverage_line="$(grep -E '^All files' "$TMP_DIR/coverage.clean" | tail -n1 || true)"
coverage_branches="n/a"
coverage_functions="n/a"
coverage_lines="n/a"
if [[ -n "$coverage_line" ]]; then
  coverage_branches="$(echo "$coverage_line" | awk -F'|' '{gsub(/[[:space:]]/, "", $3); print $3}')"
  coverage_functions="$(echo "$coverage_line" | awk -F'|' '{gsub(/[[:space:]]/, "", $4); print $4}')"
  coverage_lines="$(echo "$coverage_line" | awk -F'|' '{gsub(/[[:space:]]/, "", $5); print $5}')"
fi

clippy_warnings="$(grep -c '^warning:' "$TMP_DIR/clippy.clean" 2>/dev/null || true)"
if [[ -z "$clippy_warnings" ]]; then
  clippy_warnings=0
fi
svelte_errors="$(grep -Eo 'found[[:space:]]+[0-9]+[[:space:]]+errors' "$TMP_DIR/svelte.clean" | tail -n1 | awk '{print $2}' || true)"
if [[ -z "$svelte_errors" ]]; then
  svelte_errors="n/a"
fi

vite_entry_kb="n/a"
vite_eager_over_500="n/a"
if [[ "$vite_status" -eq 0 ]]; then
  vite_metrics="$(node --input-type=module <<'NODE'
import fs from 'node:fs';
import path from 'node:path';

const manifestPath = path.join(process.cwd(), 'dist', '.vite', 'manifest.json');
if (!fs.existsSync(manifestPath)) {
  console.log('entry_kb=n/a');
  console.log('eager_over_500=n/a');
  process.exit(0);
}

const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
const entries = Object.entries(manifest).filter(([, value]) => value.isEntry);
if (entries.length === 0) {
  console.log('entry_kb=n/a');
  console.log('eager_over_500=n/a');
  process.exit(0);
}

const visited = new Set();
const queue = entries.map(([key]) => key);
while (queue.length > 0) {
  const key = queue.shift();
  if (visited.has(key)) continue;
  visited.add(key);
  const item = manifest[key];
  for (const dep of item.imports || []) {
    if (!visited.has(dep)) queue.push(dep);
  }
}

let entrySizeBytes = 0;
for (const [, item] of entries) {
  if (!item.file || !item.file.endsWith('.js')) continue;
  const filePath = path.join(process.cwd(), 'dist', item.file);
  if (!fs.existsSync(filePath)) continue;
  entrySizeBytes = Math.max(entrySizeBytes, fs.statSync(filePath).size);
}

let eagerOver500 = 0;
for (const key of visited) {
  const item = manifest[key];
  if (!item || !item.file || !item.file.endsWith('.js')) continue;
  const filePath = path.join(process.cwd(), 'dist', item.file);
  if (!fs.existsSync(filePath)) continue;
  const size = fs.statSync(filePath).size;
  if (size > 500 * 1024) eagerOver500 += 1;
}

console.log(`entry_kb=${(entrySizeBytes / 1024).toFixed(1)}`);
console.log(`eager_over_500=${eagerOver500}`);
NODE
)"
  vite_entry_kb="$(echo "$vite_metrics" | awk -F'=' '/^entry_kb=/{print $2}' | tail -n1)"
  vite_eager_over_500="$(echo "$vite_metrics" | awk -F'=' '/^eager_over_500=/{print $2}' | tail -n1)"
fi

total_tests=0
if [[ "$rust_test_count" =~ ^[0-9]+$ ]]; then
  total_tests=$((total_tests + rust_test_count))
fi
if [[ "$frontend_test_count" =~ ^[0-9]+$ ]]; then
  total_tests=$((total_tests + frontend_test_count))
fi
if [[ "$e2e_it_count" =~ ^[0-9]+$ ]]; then
  total_tests=$((total_tests + e2e_it_count))
fi

rust_loc_per_test="$(ratio_or_na "$rust_loc" "$rust_test_count")"
frontend_loc_per_test="$(ratio_or_na "$frontend_loc" "$frontend_test_count")"

echo "======================================="
echo "  taurhaus quality metrics"
echo "======================================="
echo
echo "TESTS"
echo "  Rust:      $(commify "$rust_test_count") tests (${rust_loc_per_test} LOC/test)"
echo "  Frontend:  $(commify "$frontend_test_count") tests (${frontend_loc_per_test} LOC/test)"
echo "  E2E:       $(commify "$e2e_it_count") tests ($(commify "$e2e_spec_count") specs)"
echo "  Total:     $(commify "$total_tests")"
echo
echo "  Rust tests by module area (top 12):"
if [[ -s "$TMP_DIR/rust_modules.tsv" ]]; then
  head -n 12 "$TMP_DIR/rust_modules.tsv" | while IFS=$'\t' read -r module count; do
    printf "    %-22s %8s\n" "$module" "$(commify "$count")"
  done
else
  echo "    n/a"
fi
echo
echo "COVERAGE (frontend)"
echo "  Lines:     ${coverage_lines}%"
echo "  Branches:  ${coverage_branches}%"
echo "  Functions: ${coverage_functions}%"
echo
echo "BUILD HEALTH"
echo "  Clippy:      $(commify "$clippy_warnings") warnings  [$(status_icon "$clippy_status")]"
echo "  Svelte:      ${svelte_errors} errors   [$(status_icon "$svelte_status")]"
echo "  Vite entry:  ${vite_entry_kb} kB       [$(status_icon "$vite_status")]"
echo "  >500k eager: ${vite_eager_over_500} chunks"
echo
echo "CODE SIZE"
echo "  Rust:      $(commify "$rust_loc") LOC"
echo "  Frontend:  $(commify "$frontend_loc") LOC"
echo "  Total:     $(commify "$((rust_loc + frontend_loc))") LOC"
echo
echo "RAW TOOL EXIT CODES"
echo "  cargo test -- --list:  $rust_status"
echo "  cargo clippy:          $clippy_status"
echo "  vitest --coverage:     $coverage_status"
echo "  svelte-check:          $svelte_status"
echo "  vite build:            $vite_status"
