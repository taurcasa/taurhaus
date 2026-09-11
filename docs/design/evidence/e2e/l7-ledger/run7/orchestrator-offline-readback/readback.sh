#!/bin/bash
set -u
S="$1"
BW=(bwrap --die-with-parent --ro-bind / / --tmpfs /home --tmpfs /tmp --tmpfs /run --proc /proc --dev /dev --ro-bind "$S" /tmp/offline --chdir /)
for v in current narrative; do
  "${BW[@]}" /tmp/offline/mesh ledger render --input-bundle /tmp/offline/bundle --view $v --format markdown > "$S/offline-$v.md" 2> "$S/offline-$v.err"; rc=$?
  ref=$([ $v = current ] && echo ledger.md || echo narrative.md)
  echo "$v: exit $rc, $(wc -c < "$S/offline-$v.md") bytes; err: $(head -c 200 "$S/offline-$v.err")"
  if cmp -s "$S/offline-$v.md" "$S/bundle/$ref"; then echo "  == matches bundle/$ref byte-for-byte"; else echo "  != differs from bundle/$ref"; diff "$S/offline-$v.md" "$S/bundle/$ref" | head -8; fi
done
"${BW[@]}" /tmp/offline/mesh ledger render --input-bundle /tmp/offline/bundle --format json > "$S/offline-render.json" 2> "$S/offline-json.err"; echo "json: exit $?, $(wc -c < "$S/offline-render.json") bytes; err: $(head -c 200 "$S/offline-json.err")"
