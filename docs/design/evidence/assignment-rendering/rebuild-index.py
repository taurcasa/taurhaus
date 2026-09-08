import collections as co
import hashlib, json, math, statistics, tarfile
from datetime import datetime, timedelta
from pathlib import Path

archive = Path("/home/mstie/projects/taurjob/docs/wave-1/mesh-archive/taurjob-team-archive.tar.gz")
assert hashlib.sha256(archive.read_bytes()).hexdigest() == (
    "60bd9ba90b941211821c14b36b10dc583ddf21e961ff80547ff223141d2643e1"
)
primary = {
    "heavy-implementer": {3:4, 4:4, 5:4, 7:8, 8:8, 9:4, 10:8, 11:11, 12:11, 14:11},
    "heavy-implementer-1": {3:5, 4:5, 5:5, 7:9, 8:9, 9:5, 10:9},
    "architect": {3:1, 4:1, 5:1, 6:1, 7:1, 9:4, 10:5, 11:1, 12:1, 13:1,
                  14:8, 15:1, 16:1, 18:1, 19:10},
    "implementer-1": {3:3, 4:3, 5:3, 6:3, 8:3, 9:10, 10:10, 12:10},
    "judge-astra-1": {2:9, 3:9, 4:9, 5:9, 7:9},
    "lead-taurjob": {0:11, 1:2},
    "team-lead": {5:7, 6:7, 8:6, 9:7, 12:6, 16:6, 17:6, 18:1,
                  20:6, 21:2, 22:7, 24:7, 25:7, 28:2, 31:2},
}
broadcasts = {"Commit hygiene: pathspec commits only",
              "Directory unified: taurjob is now a symlink to taurjobs"}
rows = []
with tarfile.open(archive) as tar:
    for member in tar.getmembers():
        if "/inboxes/" not in member.name or not member.name.endswith(".json"):
            continue
        seat = Path(member.name).stem
        inbox = json.load(tar.extractfile(member))
        print("inbox", seat, len(inbox), sum(len(r["text"]) for r in inbox),
              sum(not r["read"] for r in inbox))
        for index, original in enumerate(inbox):
            task = primary.get(seat, {}).get(index)
            category = ("T" if task else "O" if original.get("summary") == "operator_notice"
                        else "B" if original.get("summary") in broadcasts else "D")
            rows.append(dict(original, seat=seat, index=index, task=task,
                             category=category, te=len(original["text"])/4,
                             ts=datetime.fromisoformat(original["timestamp"].replace("Z", "+00:00"))))
    journals = {}
    for name in ("workflow_events", "protocol_index", "task_mutations"):
        body = tar.extractfile(f"taurjob-team/state/{name}.jsonl").read()
        journals[name] = [json.loads(line) for line in body.splitlines() if line.strip()]
        print("journal", name, len(journals[name]))
for category in "TBOD":
    group = [r for r in rows if r["category"] == category]
    print("category", category, len(group), sum(r["te"] for r in group))
assert len(rows) == 106 and sum(r["te"] for r in rows) == 50047.5
exact = co.Counter(r["text"] for r in rows)
print("exact", len(exact), sum(n-1 for n in exact.values()),
      sum(len(body)*(n-1) for body, n in exact.items()))
lines = co.defaultdict(set)
for r in rows:
    for line in set(r["text"].splitlines()):
        if len(line) >= 80:
            lines[line].add(r["seat"])
shared = {line: seats for line, seats in lines.items() if len(seats) > 1}
print("shared lines", len(shared), sum(map(len, shared)),
      sum(len(line)*(len(seats)-1) for line, seats in shared.items()))
sent = [r for r in journals["workflow_events"] if r["eventType"] == "message_sent"]
ids = {r.get("id") or r.get("msg_id") for r in rows}
print("sent/matched", len(sent), sum(r["message_id"] in ids for r in sent))
print("unmatched destinations", co.Counter(r["recipient"] for r in sent if r["message_id"] not in ids))
print("structured links", sum(bool(r.get("taskId")) for r in rows))

# Authored contract index, generated card index: duplicated assignment purpose.
pairs = {"architect": [(3,6)], "heavy-implementer": [(3,5),(8,7),(11,12)],
         "heavy-implementer-1": [(3,5),(8,7)],
         "implementer-1": [(3,6),(10,9)], "judge-astra-1": [(3,2)]}
prose_chars = card_chars = 0
lookup = {(r["seat"], r["index"]): r for r in rows}
for seat, entries in pairs.items():
    for prose_index, card_index in entries:
        prose_chars += len(lookup[seat, prose_index]["text"])
        card_chars += len(lookup[seat, card_index]["text"])
print("assignment prose/card characters", prose_chars, card_chars)
assert (prose_chars, card_chars) == (15616, 7485)

tasks = [r for r in rows if r["task"]]
assert len(tasks) == 62 and sum(r["te"] for r in tasks) == 21131

output=Path('/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0')
output.joinpath('archive-rows.json').write_text(json.dumps(rows,default=str,indent=2))
output.joinpath('archive-journals.json').write_text(json.dumps(journals,indent=2))
selected=[]
for seat, entries in pairs.items():
    for pi,ci in entries:
        p,c=lookup[seat,pi],lookup[seat,ci]
        selected.append(dict(seat=seat,prose_index=pi,card_index=ci,prose=p,card=c))
output.joinpath('nine-pairs.json').write_text(json.dumps(selected,default=str,indent=2))
with output.joinpath('nine-pairs-source.txt').open('w') as f:
    for n,x in enumerate(selected,1):
        f.write(f"PAIR {n}: {x['seat']} prose[{x['prose_index']}] card[{x['card_index']}]\n")
        for kind in ['prose','card']:
            r=x[kind]
            f.write(f"{kind.upper()} {r['timestamp']} {len(r['text'])} characters\n{r['text']}\n\n")
print('assertions passed: 15616/7485; 62 task bodies; 84524 task characters')
print('archive_sha256',hashlib.sha256(archive.read_bytes()).hexdigest())
