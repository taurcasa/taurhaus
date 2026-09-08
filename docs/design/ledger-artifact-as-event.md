# Artifact-native ledger intake

## Result

**[I — proposed design, not shipped behavior] Reuse the submission, not merely the file.** Ordinary completion, progress, and ruling facts stay in their current authorities and are projected without an authored ledger event. When an existing review, NOTES, or result document contains a genuinely additional claim, a small `ledger` block selects that claim from the same document. The explicit artifact path in the seat's existing submission invokes the ledger adapter synchronously. The seat neither creates a second delivery file nor runs a second ingest command. This requires the submission integration specified below; a standalone file-ingest verb alone does **not** satisfy this requirement. Basis: `docs/design/mesh-next-adjudication.md:34–47`; `docs/design/ledger-append-log-research.md:179–184`; Mesh `src/cli.rs:498–567`, `src/main.rs:4015–4043`.

**[S] Round-1 design revision completed; acceptance pending.** Read-only branch/HEAD queries in this round returned `main` and `9dd8427e2c084aa1a47a63d338083f1df21c5f78` (commands: `git branch --show-current`, `git rev-parse HEAD`). The eight quoted Mesh probes were rerun successfully as observations, including expected parser rejections; see `mesh-artifact-fix-probes.json:1` and the Evidence transcript. Only this permitted output directory was written. No product test, build, Git write, real harness, daemon, tmux or team operation was run. This is a completed document-fix lane, not an increment-C/D gate or review approval. Prior corpus observations are retained below with their provenance and date; they were not freshly re-inventoried in this round. Direction: `d1-opus-findings.md:5–35`.

### Evidence contract and authority

| Label | Meaning and limit |
|---|---|
| S | Source or retained bytes inspected; a fresh count is a measurement of the stated population. A source assertion of PASS is not a PASS independently replayed here. Command observations prove only the exercised path. |
| D | Opened primary external documentation. No D evidence was needed or consulted in this local design. |
| P | Prior reported measurement or seat testimony, with source; not independently reproduced as operational behavior. |
| I | Proposed requirement, interpretation or manual classification. **UNVERIFIED as implementation** until the specified acceptance work succeeds. |
| U | **UNVERIFIED**: evidence or authority is missing; the open-question table gives the safe default and what settles it. |

**[I] Compound-label convention:** the first label identifies the evidence basis (S = inspected bytes/command, P = retained prior report); subsequent labels distinguish reported material (P), interpretation (I), or unresolved inference (U). Thus S/I is measurement plus proposed meaning, S/U is observation plus an unresolved conclusion, and P/I/U is reported evidence plus design plus a stated gap. All I and U conclusions are **UNVERIFIED as implementation**; candidate execution or the named authority settles them. No compound implies an independently replayed artifact PASS.

**[I] Label scope:** a label on a paragraph, table introduction, or example applies to its entire block. Source paths prefixed `Mesh` mean `/home/mstie/projects/mesh/`; `taurjob` means `/home/mstie/projects/taurjob/`; otherwise repository paths are relative to `/home/mstie/projects/taurhaus/`. Evidence file names without a directory refer to this report's output directory. Line ranges identify inspected context; SHA-256 identities in the Evidence section bind the mutable corpus bytes. The design below is I throughout, even where it uses imperative requirements. Fix-round binding directions are `d1-opus-findings.md:5–35`; retained external-corpus examples are P unless explicitly remeasured here. Each §10 corpus population is the retained as-of 2026-09-08 04:05 UTC snapshot, hash-identified by `artifact-inventory.json`; no cross-file atomic snapshot is implied. Evidence contract follows `docs/design/ledger-append-log-research.md:11–19` and the assignment at `docs/design/ledger-artifact-as-event-brief.md:17–33`.

**[S/P] Authority:** the adjudication explicitly governs inconsistent studies until their revision (`mesh-next-adjudication.md:3–7`). It adopts artifact intake, preserves the journal/four kinds/CAS/pull rendering, and requires a narrative view (:34–47). Its visibility and retention corrections also bind (:51–65). The operator prohibits prose escaping and allows Markdown files or literal stdin (`ledger-append-log-brief.md:92–115`); the study's amendment chooses `--file` (:560–573). The nine-seat synthesis reports eight objections to duplicate ledger authoring and the lead's narrative condition (taurjob `docs/wave-2/concept-review/summary.md:33–47`); that is P as survey testimony, not measured savings.

### 1. Qualifying artifacts and discovery

**[I] One adapter, explicit paths only.** Qualifying classes are committed review/acceptance/certification documents, producer NOTES, measurement or implementation RESULT artifacts regardless of filename, a RESULT submitted as literal body, an exceptional dedicated declaration, and programmatic JSON. File class never implies a verdict, lifecycle transition, permission, or payload kind. Neither directory membership nor a `RESULT` prefix creates a ledger event. No recursive discovery, watcher, Git hook, remote fetch, or parsing of prose citations into filesystem reads. Basis: brief :72–77; study :171–186; actual inventory `artifact-measurements.md:5–13`.

**[I] Two explicit entry points share normalization and admission.** Standalone authored intake is future `mesh ledger entry --file <artifact.md>` (file or literal stdin); programmatic emitters use the mutually exclusive explicit `--json <file>` option, never attachment sniffing or JSON-escaped authored prose. The normal route is the existing completion/review/progress submission with its explicit artifact path. Only paths ending in case-sensitive `.md` are candidates for a bounded opening-front-matter sniff. Other attachments, including JSON and binaries, retain today’s string-only handling and are never ledger errors. For `.md`, verify an in-root regular file without symlink traversal before opening: missing/unreadable/out-of-root/directory/symlink paths retain string-only handling with a warning line and no ledger read. No opening `---`, invalid UTF-8/binary content, malformed/unclosed/over-bound front matter or no parsed `ledger` namespace means no ledger input (warn for a failed sniff, do not fail delivery). Only an actually parsed but invalid ledger namespace causes a ledger error, after source commit under §1’s failure contract. Do not infer a namespace from a textual `ledger:` fragment in unparseable YAML. Ruling `ref` remains opaque. Basis: `d1-opus-findings.md:20–21`; Mesh `src/task_lifecycle.rs:550–568`.

**[I] Named spec-delta SD1 — one-submission integration.** Extend the completion/review/progress artifact handling to call this adapter, and add a mutually exclusive file/stdin alternative to the existing summary argument. Call the future alternative `--summary-file`, with `-` meaning stdin; apply the same normalization to a future RESULT-body submission surface. Existing plain summaries remain valid and unchanged. This is an input adapter and synchronous call into the ledger writer, not a new payload kind or authority. SD1 is necessary: current paths only append metadata and the current parser rejects the summary-file option (Evidence transcript; Mesh `src/main.rs:3959–4017`). Do not claim the zero-second-step objective is met if increment C ships only standalone intake.

**[I] Submission failure contract for SD1 — source-commit-first:** perform only the cheap syntactic pre-check (bounded front matter parses; namespace present); capture those bytes and the existing authenticated frozen assignment context. Commit the lifecycle mutation and its existing delivery route exactly as today, without waiting for ledger schema, bounds, reference, authority, audience or CAS validation. Only after that source commit validate and admit all intended events under the ledger lock. If the existing source operation itself fails, preserve its existing error behavior and do not admit ledger events. A subsequent ledger rejection or storage failure returns the **source receipt plus ledger error and a NONZERO exit**, explicitly saying `source committed; ledger rejected` or `source committed; ledger durability unknown`. Delivery is committed; only the optional ledger part needs repair. Name the standalone repair: `mesh ledger entry --file <the same artifact>` (with the original frozen authority context and normal explicit identity/root options), idempotent by event UUID; never advise rerunning task complete/review/progress to repair the ledger. A plain file without the namespace needs no opt-out flag. **Dropped:** durable submission-receipt store, coordinator lock, and task/workflow-writer per-submission idempotency changes. The split receipt is command output from the existing source operation, not a new durable store or cross-authority transaction. If SD4 is absent, the review is still delivered; its restricted ledger part returns `source_incomplete`, reason `audience_proof_unavailable`, exit 5, with the same standalone repair once proof exists. A lost response/process failure has no new source-level exactly-once guarantee; inspect the existing source authority and repair only ledger effects. Basis: `d1-opus-findings.md:5–6,33`; current separate writes/complete-again guard: Mesh `src/main.rs:3978–4068`.

### 2. Parser-precise authored contract

**[I] Markdown transport grammar (adapter version 1).** UTF-8 only; optional UTF-8 BOM is accepted but remains in the artifact digest. After the optional BOM the first line must be exactly `---`, terminated by LF or CRLF. A closing line exactly `---` must occur within 16 KiB of front matter. No `...` document ending, multiple YAML documents, tabs for indentation, duplicate mapping keys, tags, anchors, aliases, or merge keys. Decode the YAML 1.2 core scalar types; schema identifiers/enums/UUIDs/path values must be strings, version must be integer 1. Reject wrong types instead of coercing booleans/numbers to strings. A Markdown file with no opening delimiter is ordinary attachment input, but an explicit standalone ledger intake reports a missing-namespace schema error. On explicit standalone intake, malformed YAML fails explicitly. On attachment sniffing, malformed or over-bound front matter that never yields a parsed namespace is warning-only string metadata, as §1 specifies; parsed namespace schema errors are deferred until after lifecycle commit. Strict ledger constraints such as duplicate keys inside a parsed namespace are admission checks, not lifecycle preconditions. Basis: operator literal-prose constraint; the house precedent parses `---`-delimited YAML but trims body bytes (`src-tauri/src/session/parser.rs:54–60,121–145`), so reuse the concept, not its permissive delimiter/trim behavior.

**[I] Namespacing:** the sole ledger root is `ledger: {adapter_version: 1, events: [...]}`. `events` is a nonempty sequence of at most 16 mappings. Reject unknown keys anywhere in that namespace. Other top-level artifact metadata is opaque and never enters authentication or the canonical event; an existing top-level `author` is ignored as artifact metadata. `schema_version`, `author`, `sequence`, `committed_at`, `entry_id`, `ledger_id`, `team_incarnation_id`, `log_epoch`, and `audience_ref` are forbidden authored envelope fields anywhere in the ledger namespace. `adapter_version` names only the input adapter; canonical JSONL and its envelope `schema_version` remain exclusively writer output. The manifest/actor/entry head supplies them. `operation` defaults to `entry`; allowed values are `entry`, `amend`, `tombstone`. `occurred_at` is an optional RFC3339 observation timestamp and never determines order. Basis: study :160–169,221–245. Multi-event input is adapter batching, not a fifth kind or an event-list field in canonical JSONL.

**[I] Required event input:** `event_id` (canonical lowercase UUID), `entry_key`, `references`, and `payload` except for tombstone. On the **submission route**, the seat authors exactly `{kind, slot}` in `entry_key`; mesh DERIVES `wave` and `scope` from the frozen assignment, never from a display label, filename or cwd. An explicit wave/scope on that route is `invalid_input` (do not offer competing authority). Canonical `entry_key` is `{wave, scope, kind, slot}`, with kind exactly `outcome|remaining|decision|note`; slot is stable, caller-selected, actor-specific for non-authoritative observations. Supply primary scope (project/wave/scope/packet revision) and delivery task/assignment references from the same frozen assignment; conflicting authored refs fail. **Standalone declarations** author wave/scope and required scope/assignment references from the named approved wave packet / frozen assignment record: the smallest existing seam is the committed packet linked by the assignment, not the human ledger table (delivery standard :19–27; study :217–221). **Standalone repair** may keep the same kind/slot-only artifact when the caller supplies that original frozen assignment context; the split receipt must name its existing authority locator/identity, and resolution uses that explicit record, never a directory search. Missing machine-readable IDs or a missing immutable assignment locator is `source_incomplete`; no current guaranteed mapping is claimed (Open questions). Identifier/slot limit 128 UTF-8 bytes, path limit 1024, maximum 64 expanded references. Basis: `d1-opus-findings.md:11–12`; study :166–186,217–229.

**[I] Reference vocabulary:** use the study's typed `authority` plus `role`, not naked Markdown links. Normalized readable artifact refs carry mesh-resolved `repo_id`, repo-relative `path`, mesh-computed raw-byte SHA-256 `digest`, optional fully resolved `revision` and Git `blob`, optional `locator`; retained non-Git evidence carries `bundle` and digest. Task refs carry incarnation/task ID and assignment ID where relevant. Scope refs carry project/wave/scope/packet revision. Source events carry authority/source identity/event ID, or captured digest plus exact byte interval for a legacy record; ruling refs carry task identity and integer sequence. `assessment` is exactly `source_checked|accepted_on_citation|unmeasured|unavailable`; require a literal `limitation` for the latter three. Resolving bytes never upgrades the declared assessment. `candidate`, `landing`, `rubric`, `instrument`, `result_artifact`, `review_artifact`, `completion`, `ruling`, and `prior_observation` are roles, not authorities. The current artifact shorthand is `{authority: artifact, role: ..., path: self, assessment: ...}`; it expands to verified repo/path/digest, never the literal path `self`. Other artifact references use real paths and identities. A file literally named `self` is addressed as `./self`; recognize the sentinel before stripping `./`, so the escaped path remains a real file reference after normalization. Basis: study :171–173; concrete retention failure testimony in taurjob `concept-review/heavy-implementer-2.md:24`.

**[I] Per-reference-kind required/optional inputs; the seat never hand-computes a digest.** Mesh computes every readable content digest; a supplied digest is only an assertion and must match. `assessment` and its required literal `limitation` describe evidence quality separately. Required authority/candidate evidence must resolve even if an unavailable assessment is supplied. Basis: `d1-opus-findings.md:14–15`; study :171–173.

| Reference kind | Required authored/resolved identity | Optional input / mesh work |
|---|---|---|
| `artifact` with `path: self` | Author role, assessment; mesh computes repo_id/path/digest from the submitted artifact and frozen repo identity | No authored repo/path digest needed. Optional revision/blob/locator must verify. Literal stdin uses source-body identity/digest rather than an invented repo file. |
| Other in-root repo-relative `artifact` (including `./self`) | Author path, role, assessment; repo identity resolves from frozen scope | Mesh resolves the safe path and fills digest when readable. If unreadable/missing, require `assessment: unavailable` plus limitation; omit unknown digest. A supplied digest must be checked, so unavailable bytes cannot validate one. Optional revision/blob/locator must verify. No symlink or external-root reads. |
| Retained non-Git bundle evidence | Authorized bundle locator and role/assessment | Mesh computes/verifies content digest when bytes are readable; otherwise unavailable plus limitation, never `archived` without bundled bytes. |
| Task / assignment | Team incarnation + task ID; assignment ID required for a delivery-specific claim | Submission derives these from frozen assignment; standalone supplies authoritative record references. No author-computed digest. |
| Scope | Project/wave/scope IDs + packet revision | Submission derives them; standalone uses approved packet/assignment record; mesh computes any packet digest. |
| Source event / completion | Authority + source identity + event ID | For legacy records mesh computes captured-source digest and verifies exact byte interval; no invented event ID. |
| Ruling | Task identity + integer sequence | Resolve against the source authority; note/ref text does not confer authority. |

**[I] Canonical normalization and reference equality:** decode UTF-8, normalize strings and mapping keys to NFC, normalize CRLF or CR to LF in decoded prose, omit null-valued mapping members (`absent == null`), reject duplicate keys after normalization, and serialize mappings with keys sorted by Unicode scalar value, compact JSON, UTF-8 and no insignificant whitespace. Preserve scalar types and array order except the outer `references` set: normalize, deduplicate exact objects, then sort by canonical bytes. Required members remain required after null omission; null array entries are invalid where objects are required. For paths, recognize `self` versus escaped `./self` first, use `/` separators, strip leading `./`, collapse redundant separators and `.` components, preserve case, remove trailing slash, and reject `..` traversal/absolute escapes before opening. Do not Unicode-normalize a path into permission to open a different raw filesystem object: resolve the permitted object, then record its normalized locator; ambiguous identities fail. Reference-valued payload fields (`authority_ref`, `prior_decision`, `target_task`, `qualifies`) are expanded by this same resolver and must equal a complete normalized object in `references`, byte for byte after canonical serialization, including assessment/limitation and any verified digest. This object-membership predicate is distinct from retry equality’s self-digest exclusion in §4. Formatting/key order/`./` spelling alone cannot create a new semantic reference. Basis: `d1-opus-findings.md:14–15,29`.

**[I] Payload field spelling and conditions** below make the study's prose field descriptions concrete; they introduce no new semantic kinds. All authored prose fields (`body`, `reason`, `description`, `consequence`, `question`, `decision`, `revisit_condition`, each limitation, and evidence `limitation`) use YAML literal block scalars (`|`, `|-`, or `|+`, with standard indentation/chomping) or, for `body` only, the designated body selector. Empty body and empty arrays are the exceptions. Quoted/folded prose scalars are rejected in Markdown intake, so quotes, pipes, backslashes and newlines require no escape layer. The explicit `--json <file>` transport accepts a JSON object with the same `ledger: {adapter_version, events}` shape and no trailing-body form; it is already-decoded machine data and does not impose the authored-scalar restriction. Canonical writer JSONL is never accepted as authored input. `body` additionally accepts the section selector below. Reject unknown fields, lifecycle fields, or verdict/score authority in the ledger payload. Basis: study :177–186.

| Kind | Required payload fields and validation |
|---|---|
| outcome | Nonempty `body`; `scope_disposition: fulfilled|partial|stopped|unknown`; `limitations` list (explicit empty allowed); `remaining_status: none|items|unknown`. `remaining_entry_ids` required and nonempty only with `items`. Those roots must exist or be created in this batch as remaining items. `none` is rejected against known open items and never closes one. |
| remaining | Nonempty `item_id`, `description`, `consequence`, `revisit_condition`; `disposition: open|routed|deferred|resolved|withdrawn`. Optional `owner`, `target_task` reference, and SD5 literal-context `body`. `routed` requires target task/owner or an explicit routing-authority ref. Deferred/resolved/withdrawn dispositions require resolution/decision evidence in `references`; resolved/withdrawn use `revisit_condition` to say no further action and the applicable exception/reopen condition. One obligation per event. Item identity is stable across amendments. |
| decision | Nonempty `question`, `decision`, `consequence`; `authority_ref` naming a reference in the event; `applicability` naming its bounded scope. Optional `prior_decision` reference and SD5 literal-context `body`. Budget/GO/acceptance effects must already exist in their original authority. Policy activation uses the study's reserved slot and `authority_change: {previous_digest,next_digest}` only under its special old-policy authorization rule. |
| note | `body`, explicit or supplied by the SD5 trailing-body form (may be empty for a pure typed association); at least one evidence/relationship ref in addition to primary scope. Optional `qualifies` reference naming the claim being qualified. A blank note with only its own file and scope is invalid. |

**[I] Named spec-delta SD5 — literal trailing body and section selection.** A dedicated declaration uses front-matter fields above literal prose: **everything after the closing front matter is the body**, normalized and bounded by the same 4096-byte rule. A literal RESULT through `--summary-file` has the same trailing-body default. For an existing review/NOTES artifact, select its existing prose explicitly rather than ingesting the whole report. Explicit `payload.body` wins over the trailing default for RESULT and existing-artifact input; on a dedicated file, setting `body` while trailing prose is non-empty is `invalid_input`, so two authored bodies cannot silently compete. For standalone repair, an explicitly resolved frozen submission record naming this artifact preserves its existing-artifact/RESULT form, including an explicit literal body. Otherwise a standalone file with a section selector is an existing-artifact selection; an unbound standalone declaration with no selector is the dedicated form. No filename or prose-title inference is used. For a multi-event literal RESULT, every outcome/note lacking an explicit body receives the trailing body and is independently bounded; remaining/decision required fields remain explicit and are never inferred from prose. Their optional trailing `body` is retained as literal context under SD5 and included in the combined 4096-byte prose bound. Basis: `d1-opus-findings.md:17–18`; study :560–573.

**[I] Body selection, with no copied summary obligation:** `body` is either a literal string or exactly `{section: <exact heading text>}`. Resolve a section against Markdown *after* the front matter. Recognize column-zero ATX headings consisting of 1–6 `#`, one ASCII space, and nonempty text; trim trailing horizontal whitespace, but perform no slugging/case-folding/inline Markdown rendering or closing-`#` stripping. Ignore headings inside fenced blocks: an opener has up to three spaces then at least three backticks or tildes; the closer uses the same character, at least opener length, and only trailing whitespace. Section text starts after the matching heading line and ends before the next recognized heading of equal/lower level or EOF, including nested subheadings. Require exactly one heading with that text; duplicate/missing/unclosed-fence cases fail validation. Return literal section bytes with CRLF normalized to LF and without trimming; apply the shared NFC normalization for canonical content. It is not a general Markdown inference engine. An existing section can therefore be referenced once; otherwise write one short literal `body: |-` in front matter, which is the sole authored summary. Never choose the first section, infer PASS from a title, or summarize using a model. Basis: 86 of 92 measured review files exceed 4096 bytes (`artifact-measurements.md:9–10`); NOTES set already has a limits section (taurjob `imagery/set/NOTES.md:24–28`).

**[I] Reference-valued payload fields:** `authority_ref`, `prior_decision`, `target_task` and `qualifies` use complete typed reference objects matching an object in `references` after normalization, not undocumented array indices. `remaining_entry_ids` contains canonical root UUIDs, not display labels. `applicability` is a nonempty sequence of authorized scope IDs already present in scope references. Reference `locator` is an optional object with either `heading` (string) or `lines` (two positive inclusive line numbers), never both; it documents evidence position and is not the body-selection mechanism. Arbitrary prose within reference locators is not executable. These concretize the study’s typed-reference intent (:171–182).

**[I] Bound all claim prose, not just a field named body.** Keep the study's 4096-byte UTF-8 body bound and 32768-byte canonical event/64-ref limits. For remaining/decision kinds, apply 4096 bytes to the concatenation of their required prose fields in table order followed by any SD5 body; also apply 4096 to combined outcome body plus limitations so a qualification cannot evade the bound. Measure after scalar decoding/section extraction and LF normalization, before JSON escaping; then check the canonical serialized event separately. Artifact bytes have a separate 1 MiB input bound, enough for the measured maximum 66097-byte review. Do not inline or inspect binary attachments; link their digests. Oversize errors report actual/limit and the offending field, never a clipped accepted event. These are adapter-bound clarifications (SD2a), with later tuning contingent on measured examples, not claims of optimal sizes. Basis: study :186; inventory :9–10.

### 3. Identity, commit order, and retention

**[I] Normal seat sequence:** author the existing artifact including any optional ledger block; commit it through the already-required delivery workflow; submit its explicit path through the existing lifecycle submission. Intake does not run Git writes and never inserts its own receipt/hash into that artifact. The receipt carries event ID/sequence outside the document, so there is no self-hash or commit-message loop. The author's UUID precedes the commit and is not a commit hash. Basis: delivery standard :9–13,35–41,103–115; study :161–171,378.

**[I] Ledger input reads one regular file without following symlinks under an explicitly allowed repository/submission root.** Attachment sniffing uses §1’s warning-only exclusions; strict standalone intake rejects traversal, out-of-root paths, symlinks, directories/devices and identity/size changes during bounded read. Hash the exact bytes read, including front matter and original line endings, but **do not copy artifact bytes into an evidence store at ingest**. Mesh computes digests for self and every readable in-root reference; a caller-supplied digest must match (§2 table). Resolve optional revision locally to a full commit and verify its path/blob bytes before recording it; missing commit/dirty bytes never acquire an inferred HEAD revision. A literal stdin result uses its existing source-body locator/digest and remains referenced until a boundary bundles bytes; digest alone is not retained evidence. Repo identity comes from the frozen project manifest, never a credential-bearing origin URL. Never fetch or follow prose citations into external/private-home paths. Basis: `d1-opus-findings.md:14–15,20–21,34`; study :171,217,374–376.

**[I] Later revision binding is optional and separate.** If a working-tree/stdio submission later lands, its digest-addressed identity remains valid. A later landing association can reference both original digest and verified commit; it does not amend old provenance silently or require the producer to resubmit the identical claim. Re-ingesting a changed document is governed by §4, not by filename continuity. Distinguish source, gated, reviewed and landed identities; a captures-only append does not authorize a wider product review. Basis: adjudication :22–25,63–68; taurjob NOTES set :9–11,97–105; review T49 :29.

**[I] Named spec-delta SD3 — boundary retention metadata.** Record `retention: {state: referenced|archived, retain_until: <boundary-id>}` on evidence references. In v1, `retention.state` defaults to **referenced** at ingest; the content digest proves identity, not availability. It becomes **archived only when a boundary snapshot/export bundles the bytes** and records the actual content-addressed bundle locator. This boundary/export assessment does not rewrite earlier journal provenance. No ingest evidence store or per-event byte copy. Closure export refuses required referenced-only evidence it cannot resolve and bundle; optional unavailable evidence remains referenced with `assessment: unavailable` and limitation. Required authority/candidate evidence cannot be waived by marking it unavailable. Deletion needs boundary release, not an elapsed timer. Basis: `d1-opus-findings.md:34`; adjudication :57–60; study :374–376.

### 4. Idempotency, amendments, and multi-fact artifacts

**[I] Client event IDs live in the artifact.** The seat/editor supplies one UUID per intended event; it is the ledger-local retry lookup key. Retry equality is over the **event’s own normalized content**: `entry_key`, operation, decoded/extracted/LF-normalized payload and normalized references **excluding the submitted self artifact digest**, plus the event’s own optional `occurred_at`, `previous_event_id`, `reason` and authority-change fields when present. Authentication/entitlement remains mandatory, not an invocation-time input to hash. The submitted artifact’s digest is provenance on its expanded self/result_artifact (or review_artifact) reference, not equality; strip that same digest from any matching reference-valued payload object when computing equality. Do not strip other evidence digests. Re-sort/deduplicate the reference set after these exclusions before comparing canonical bytes. Writer fields, adapter transport spelling and retention/export bookkeeping do not enter equality. Same UUID and equal normalized content returns the original receipt/provenance before current-head checks, without rewriting its old artifact digest. Changes to selected section text or the event’s own semantic front-matter fields conflict under the same UUID; formatting/key order/line endings with identical normalization do not. **Any other artifact bytes**, including unrelated NOTES stages, image prompts, other top-level metadata and other events’ metadata, do not conflict for this event. A reused ID with changed normalized event content is `idempotency_conflict`; a new ID at an occupied key is `entry_exists`. No auto-regenerated UUID on retry. Basis: `d1-opus-findings.md:8–9,14–15`; study :161,241–245.

**[I] Named spec-delta SD6 — expected head transport.** Standalone intake accepts `--expect-head` OR `previous_event_id` in front matter for an amend/tombstone; both supplied must agree or return `invalid_input`. On the submission route front matter supplies the expected head; no caller/default silently overrides it (any conflicting supplied context is invalid_input). For a standalone multi-event batch, a single flag is legal only for one amend/tombstone; otherwise require per-event heads in front matter. Basis: `d1-opus-findings.md:25`; study :167–169,328–329.

**[I] Changed artifacts do not silently amend.** To revise an operative claim, the new file carries `operation: amend`, a fresh event UUID, the same entry key, an explicit expected head (`previous_event_id`, or standalone `--expect-head` per SD6), a nonempty literal `reason`, and the complete replacement payload. The writer resolves the root from the key, but does **not** substitute the current head for the authored expected head. Under lock it verifies equality, permissions and all references; stale head returns `head_mismatch`. This chooses the brief's rejected-conflict option for unmarked changes and preserves real CAS: “find current head and automatically overwrite it” would erase concurrent work. Tombstone likewise requires expected head/reason, has no payload, and permanently reserves the key. Basis: study :167–169,241–247.

**[I] Multi-fact documents use the events array, not new files.** Current front matter describes only events intended for this submission, not an append-only history of prior headers. Old submissions remain in Git and the ledger. An updated NOTES stage can replace its old header with the new stage's events, keeping earlier image bytes/provenance in the Markdown body. A correction of the same claim is explicit amend; a new observation/stage has a new slot. Every event carries the entire submitted artifact digest as provenance only; its equality follows §4’s own-content predicate. After lifecycle source commit on the submission route, take the ledger lock, validate all entries and reject duplicate IDs/keys inside the batch, then allocate consecutive sequences and publish the entire batch in one manifest update; no partially admitted header. References to earlier/later entries in the batch are validated as a set, with no cyclic remaining-item relationships. An all-identical batch retry returns all original receipts; a partially matching retry batch fails as a conflict, not a mix of “old accepted” and “new maybe.” Atomic multi-entry admission is SD2b, a bounded adapter/writer transaction extension using the existing manifest boundary, not per-event snapshots. Basis: study :239–245,292–300; NOTES stage growth evidence at taurjob set :97–105 and concept-review asset :47–55.

### 5. RESULT body and the exact do-nothing default

**[I] Zero authoring is the default:** if the submitted summary already states the claimed outcome and qualifications, display that retained source text and the linked result artifact. Do not require a ledger UUID, front matter, disposition boilerplate, or another summary. Completion is lifecycle evidence, not scope acceptance or `remaining: none`. A normal RESULT body with no `ledger` namespace is retained as the submission summary exactly as supplied, subject to its submission bound; absence of metadata is not an error. A completion short enough to contain the result needs no event. Basis: study :67,179–184; the seven measured retained completion summaries in Evidence.

**[I] Explicit opt-in, not NLP:** a ledger event is required only when the authorized actor needs to assert a durable claim not already carried with sufficient identity by the original sources: (a) scope fulfillment/partial/stopped qualification beyond a task terminal state; (b) an independently tracked remaining obligation/disposition/revisit condition; (c) a non-task decision within mandate; or (d) a missing typed evidence assessment, limitation, or completion-to-review association. A review score, a budget raise or a GO already recorded as a ruling is projected, not authored again. A report's full existing limitations can remain a linked artifact when no machine-operable qualification is needed. “Cannot carry” here means missing typed scope/relation/disposition, not an inability of a summary string to contain prose. Basis: study :171–184,225–231; delivery standard :90–115.

**[I] Literal RESULT intake:** a file containing only ordinary RESULT prose supplies the complete summary via SD1, including quotes/backslashes/newlines without argv or JSON string construction. For a RESULT with a ledger header, strip only the header for the submission body; normalize LF for the summary and record the original-byte digest with referenced retention; boundary export must bundle those bytes for archived retention. That same in-memory capture supplies the SD5 trailing body or explicit section selector. Long reports remain artifacts with a compact summary; never truncate them to force a completion field. A future direct-message submission uses the same file/stdin parser and authenticated source-message identity, but no messaging-journal dependency is needed for the lifecycle adapter. Dedicated intake is not a replacement for a required directed handoff. Basis: operator constraint :92–115; adjudication :14–19,39–45.

### 6. Validation and author-visible errors

**[I] All ledger error payloads use `{error, field, detail, actual?, limit?, current_head?}`**, without submitted prose echoes. On the submission route **all semantic ledger errors below occur after source commit** and return its source receipt alongside the ledger error and nonzero exit; they never roll back or prevent the delivered review/completion/progress. Standalone errors have no lifecycle effect. Failed attachment sniffing without an actually parsed namespace is warning-only (§1); strict standalone intake reports malformed/missing input. `invalid_input` is the separate vocabulary delta **SD2c**. Basis: `d1-opus-findings.md:5–6,20–24`; study :339.

| Error / proposed exit | Trigger and repair |
|---|---|
| invalid_input / 2 | Missing key/namespace, duplicate YAML key, ambiguous body section, invalid UTF-8/type, empty required reason, forbidden authored writer field, oversize body/artifact/record/ref count. Report field and limits; select a bounded existing section or link full artifact. No truncation. |
| unsupported_schema / 2 | Unknown adapter/event version; install the compatible reader/writer through the governed release, never guess. |
| entry_exists / 3 | A new UUID tries to create an occupied immutable slot; inspect intended meaning and explicitly amend or use a genuinely new observation slot. |
| head_mismatch / 3 | Expected head stale; return current head only if audience allows it, re-read authorized history, author a deliberate replacement. |
| idempotency_conflict / 3 | Same UUID, changed normalized event content; restore that event’s content (unrelated artifact edits are allowed), or author an explicit new event/amendment. Repair only with standalone ledger intake. |
| wrong_authority / 4 | Root/incarnation/ledger/policy mismatch, including policy change since capture. Revalidate the current frozen assignment; never root-scan. |
| unauthorized / 4 | Forged authorship, unauthorized scope fulfillment/override, forbidden audience widening or disclosure. The authenticated actor's authority decides. |
| missing_reference / 5 | Required source, section's target artifact, Git object, digest match or source-event identity cannot resolve. Correct identity or retain authorized evidence. A matching digest with unavailable optional evidence must be explicitly declared unavailable, not claimed checked. |
| source_incomplete / 5 | Required retained source cut/audience proof/frozen assignment missing; absent SD4 uses reason `audience_proof_unavailable`. Preserve the gap; do not mint acceptance or read mutable live fallback. |
| committed_corruption / 5 | Invalid committed prefix; stop before authoritative folding/appending. |
| durability_unknown / 5 | Sync/manifest/storage failure leaves durability unconfirmed. Preserve the source/ledger receipts and retry the same event through standalone ledger intake; never return success on a best-effort write. |

**[I] Validation order:** submission attachment syntactic sniff → existing lifecycle mutation/delivery → ledger lock/recovery → strict adapter/schema/bounds and frozen actor/root/assignment/policy validation → safe reference resolution/digests and event normalization → authorized UUID retry lookup → scope/audience/CAS and reference validity → canonical record bounds → append/sync/manifest publication → ledger receipt. Parse the bounded captured input only; no scanning. Ledger batch admission is atomic, but source and ledger are not a transaction. Unavailable audience proof rejects restricted ledger intake after delivery. Standalone follows the same ledger stages with no source mutation. Unauthorized diagnostics disclose no head IDs, restricted counts or details. Basis: `d1-opus-findings.md:5–6,33`; study :239–245,292–308.

### 7. Independent-review audience

**[I] Named spec-delta SD4 — immutable audience proof in the envelope.** Increment C records this proof from authenticated submission context and rejects restricted ledger intake without it; increment D owns projection/export enforcement described here. No proof means `source_incomplete` / `audience_proof_unavailable` for the ledger part, while the review remains delivered (§1). Add `audience_ref` referencing the frozen review/submission audience policy, including review round, reviewer assignment, locked-verdict generation and permitted actor incarnations. The writer derives it from authenticated submission context; front matter cannot supply a recipient list or widen audience. A review event remains visible to its author and authorized adjudication owner, but no unlocked peer reviewer for that scope/round may see the verdict, its summary, score, title, limitation-derived conclusion or acceptance aggregate. All projections, history, search, JSON/Markdown export, snapshots, receipt details and artifact link resolution apply the same predicate *before* joins and rendering. Derived facts inherit the intersection of their sources' permitted audiences. A generic “restricted evidence withheld” marker may be shown, without count, ID, title, timing or verdict. Basis: adjudication :51–56; study :233,376; taurjob concept-review judge :10,19,24; review T49 :3–10.

**[I] Locking one's own verdict does not itself broadcast peer material.** Release requires the review authority's recorded policy/release event at the input cut. Unknown policy/lock state means withhold. An artifact mixing producer data and appended reviewer material is treated at the stricter whole-artifact audience unless a separately frozen, explicitly approved producer snapshot exists; do not simply expose the whole file through a harmless-looking NOTES reference. A public export is a permitted derived source-facts bundle and excludes restricted raw sources. This amends the study's earlier team-visible-only ledger assumption; without SD4, safe default is to reject only the restricted ledger intake after committing review delivery, never publish it as team-visible. Recording a proof in C is not evidence that D’s consumer enforcement has shipped. Filesystem access outside mesh remains outside this cooperative integrity guarantee. Basis: study :233,374–376; adjudication :51–56; taurjob concept-review asset :83–87.

### 8. Narrative renderer

**[I] FUTURE surface:** `mesh ledger render --view current|narrative`, default `current`; both views use the same input cut, root/reader authority, audience predicate and `--format markdown|json` options. This is a renderer requirement, not a successful command invocation (current absence: Evidence probe 6). Basis: `d1-opus-findings.md:26`; study :320–378.

**[I] Provide both current state and a deterministic authored narrative view per task and per recorded day.** Current rows link to anchored outcome/remaining/decision/note blocks. Narrative blocks show the original source summary or selected literal body, author, exact scope/candidate, evidence assessment, visible limitations and unresolved items, with source links beside each assertion. For rulings, show the original question/note/value and consequence when recorded; do not invent rationale from timestamps. Group ledger entries by committed day, retain recorded observation time as a labeled field, and preserve per-authority sequence. Cross-authority ordering uses the study's stable source-rank/position tuple with causal links, never claims total causal time. An event missing time appears in an undated group. Basis: lead testimony taurjob concept-review lead :48–53; study :272–284,345–378.

**[I] Operative narrative is not history pasted into giant cells.** Default shows current claims plus unresolved contradictions; history expands superseded claims and amendment reasons. Earlier FAIL and later clean evidence remain distinguishable. Gate/method limits, commit-or-none, numbered findings, separate questions, exact score fields, per-root zero versus unmeasured, and source/landing identity remain reachable in a complete artifact view. Missing artifacts are visible evidence gaps. No summarizer seat; source text is authored once. A lead's new cross-task interpretation is an optional attributed decision/note at a real boundary, not mandatory prose per event. Rendering is pull-only and read-only; boundary snapshots alone retain a reproducible vector cut and renderer version. Basis: study :264–282,345–378; delivery standard :9–13,64–115.

### 9. Seat text: three rules, after execution validation

**[I — FUTURE; not active role text]** The proposed role addition is only these three rules, plus one validated example and the verb's own help link. Basis: brief :113–117; study :337–341; taurjob concept-review lead :54–56.

1. Deliver the normal RESULT/review/NOTES; if it already says the fact, author no ledger entry.
2. For a genuinely new ledger claim, put the ledger block in that same artifact, select its existing prose, and submit its explicit path in the usual completion/review submission.
3. If delivery committed but ledger failed, use standalone ledger repair on the same artifact; preserve the event UUID/content for a retry, or deliberately amend with a new UUID, expected head and reason.

**[I — FUTURE help link]** Field details and the validated example belong at `mesh ledger entry --help`, not in additional role rules; SD4 enforces the audience structurally. Basis: `d1-opus-findings.md:30`.

**[P/I/U] Increment D must execute parser and handler success/failure examples with fixture identities against the candidate binary before these enter role YAML. Current help/parser results below prove only that 0.2.29 lacks the proposed intake; they are not an activation gate. No new role file is written here. Basis: study :341; brief :115–117.

### 10. Measurement plan and authoring-cost estimator

**[P] Retained baseline, as-of 2026-09-08 04:05 UTC:** immediate review Markdown files total 54/586909 bytes in wave 1 and 38/381736 bytes in wave 2; 0/92 begin with front matter and 0/92 begin RESULT. There are two asset NOTES: gate 33511 bytes and set 30870 bytes, both without front matter. Review medians are 7643.5 and 8487.5 bytes; 86/92 exceed the proposed 4096-byte body ceiling. Seven archived completion summaries total 1833 bytes and range 131–572 bytes. These are artifact/storage lengths, not authored bytes per fact. Evidence: `artifact-measurements.md`, `artifact-inventory.json`, `completion-census.json`.

**[I] Population and estimator (future closed generation; not the as-of 2026-09-08 04:05 UTC baseline):** after wave 2 is exported as a closed generation, enumerate its submissions across all nine seats, stratifying ordinary completion, review, NOTES stage, implementation result, correction/diff-confirm, and genuinely independent decision/remaining claim. Match by full task/assignment/stage/candidate identity, not `#9` alone. Manually code atomic claims and their authoritative carriers; two independent coders record disagreements rather than force a percentage. For each fact count UTF-8 bytes actually authored in its directed RESULT (`M`), ledger edit span (`L`), new metadata (`F`), genuinely new prose (`N`) and correction/retry repair (`R`). Compare `old = M + L` with `new = M + F + N + R`; existing artifact prose cancels. If an existing completion summary covers the fact, `F=N=R=0`. Count delivery actions and seat-authored files separately; generated event bytes, copied artifact bytes, exports and renderer output are not authoring. Basis: brief :118–121; study :406; explicit current artifacts in Evidence.

**[S/I] One re-priced proposed example, no savings claim:** after the Round-1 changes, `future-notes-header.txt:1` is exactly 426 UTF-8 bytes and 18 lines (reproduction in `artifact-fix-verification.py:1`). Wave/scope are derived, and the header uses `adapter_version`. It selects the existing NOTES conformance section instead of duplicating its prose. The retained as-of 2026-09-08 04:05 UTC wave-2 ledger population’s complete #9 row is 1297 bytes (:122); #49 is 614 (:161); #51 is 263 (:162). The 426-byte header is smaller than the retained 1297-byte #9 row and larger than the 263-byte #51 row; neither comparison measures a per-fact saving. A row contains several facts and multiple revisions. The projection might remove a lead's `L` while increasing an artifact producer's `F`; report both seats' costs, not only aggregate net. Evidence: `future-notes-header.txt:1`; `wave2-row-census.json:1`; `artifact-measurements.md:29–34`. Row counts are P retained measurements, not a fresh live census; source hashes are in `artifact-inventory.json:1`. The lens later reported 382368 wave-2 review bytes and #9/#49/#51 row sizes 1286/609/260 (`d1-opus-findings.md:27`, P); those are a different live population and do not replace this dated baseline.

**[U — UNVERIFIED] No quantitative savings rate is warranted.** The retained journal is an early wave-1 cut and does not include final wave-2 submission bodies. Its inbox archive contains RESULT-labelled idle echoes, not proven original directed RESULT texts. The as-of 2026-09-08 04:05 UTC ledger row lengths are not edit spans; malformed table widths also prevent naive task-row parsing. Final wave-2 source exports plus path-scoped read-only commit-diff measurements and claim coding would settle `M`, `L`, and whole-population savings. Safe default: ordinary entries must add zero files, zero metadata bytes and zero commands; optional annotated entries must add zero delivery steps. Measure field generation/correction time and front-matter error rates in increment D before making a performance or token claim. Evidence: completion and row censuses; `mesh-next-adjudication.md:34–47`.

## Evidence

### Worked examples from real artifacts

**[I] All transformations below are FUTURE parser/design examples, not commands that have succeeded or events admitted into a historical wave.** UUIDs in the proposed header are synthetic; authority identities must come from a real frozen assignment in an execution fixture. `study` above denotes `docs/design/ledger-append-log-research.md`; `brief` denotes `docs/design/ledger-artifact-as-event-brief.md`. Abbreviated taurjob `concept-review/`, `imagery/`, and unqualified wave-2 review names denote paths beneath `docs/wave-2/`; T26 names denote `docs/wave-1/reviews/`. The following full paths and hashes disambiguate the concrete examples. The prior lane read source bytes; this round retains those P examples and does not claim replay of their underlying test/runtime assertions.

**[P] Retained artifact identities from `artifact-inventory.json:1` (as-of 2026-09-08 04:05 UTC):**

| Class / source in taurjob | Bytes | SHA-256 |
|---|---:|---|
| Review: `docs/wave-2/reviews/T49-review-judge.md` | 3853 | `19efc29acd12b58dfecf1dff475c1a521f7cf1113740adacd61f243cabbcabfb` |
| NOTES: `docs/wave-2/imagery/set/NOTES.md` | 30870 | `743e5c5e0f59ce8e666248d3c42b4e5ebb94eb493568724031db7e028121c257` |
| NOTES gate: `docs/wave-2/imagery/gate/NOTES.md` | 33511 | `446990ea0326c7c1e9842756b4382557a659e64912b004b837a3f9f7060f773e` |
| Result/evidence: `docs/wave-1/reviews/T26-integration-evidence.md` | 8775 | `9fed863b715a5136735425c817457395357f36b23f2a0f866ffd39403dc567a5` |
| Certification result: `docs/wave-1/reviews/T26-certification.md` | 6765 | `cad6d579b01386713572ee907a0a32d6e7257a543b5315da877ba74e0d9167df` |
| Acceptance: `docs/wave-1/reviews/T26-cert-product-acceptance.md` | 6078 | `ac976e7c2390d11e922945ddadc89ab0f341865e08ef53a41d6f91d4cdf6de8e` |
| JSON evidence: `docs/wave-2/reviews/evidence/t15/SHA256.json` | See source file | `c833822d432807f6c639d5d8da0de4babc276a4973c7bd53846b0e3df033a2c6` |
| Retained team archive: `docs/wave-1/mesh-archive/taurjob-team-archive.tar.gz` | See archive | `60bd9ba90b941211821c14b36b10dc583ddf21e961ff80547ff223141d2643e1` |

**[P/I] Review — preserve verdict authority, optionally add a qualification link.** T49's report records ACCEPT at :3, candidate/base at :4, numbered evidence and scope qualifications at :8–10, score/evidence table at :12–19, and limits/source-versus-landing at :27–29. An ordinary review submission needs no ledger metadata if those fields already reach the result/ruling projection. If the missing fact is a typed qualification of the native capture claim, submit a `note` scoped to the frozen #49/#51 delivery, reference the review artifact as `review_artifact`, link the underlying candidate/ruling identities, and use a bounded literal body such as “Native binary identity was not independently reproduced.” That is proposed paraphrase of :9, not a new observed defect. Preserve the complete numbered findings and exact table in the linked artifact; do not turn this note into another ACCEPT. Audience is the review policy, not “public because a .md file exists.” No required per-finding event: the existing report is already the findings authority.

**[P/I] NOTES — select existing prose without repeating the generation record.** The set records task and assignment at :3; decision/rulings and stage ownership at :7–11; exact image hashes at :19–22; conformance limits at :24–28; separate later delivery at :101–105. Its phrase “embedded sRGB conformance is unverified” (:28) is a qualified producer observation even though a deviation was accepted. Future input below selects that entire existing short conformance section. The header is added to the *same* file; no body is appended or duplicated. Scope/task/assignment/policy references come from the already frozen submission, and self expands to the digest of the **new submitted bytes**, not this report's original-corpus digest. The consumer retains the images' distinct hashes and does not confuse a NOTES append with changed image bytes. Source identity: table above.

```yaml
---
ledger:
  adapter_version: 1
  events:
    - event_id: 877bdbad-7a91-4bd1-b048-c72516365b23
      operation: entry
      entry_key:
        kind: note
        slot: asset-generator-E1-limitations
      references:
        - authority: artifact
          role: result_artifact
          path: self
          assessment: source_checked
      payload:
        body:
          section: Conformance and accepted deviations
---
```

**[I] NOTES normalization example:** under the corresponding authorized submission, the event retains that UUID/key, writer actor and sequence, expanded primary scope/task/assignment refs, and the self artifact's actual repo/path/SHA-256. `payload.body` becomes exactly the conformance section's normalized text; image generation prompts elsewhere in the 30 KiB file never enter the event. The fixture must measure the extracted bytes, preserve both the accepted-deviation qualification and the unverified-conformance qualification, and reject it if over bound. Merely showing this transformation is not an executed parser test. If a completion/result projection already carries the section, the correct normal header is **no header**. Basis: set NOTES :24–28; study :179–186.

**[P/I] RESULT artifact — title is not parser type.** T26 integration evidence is an actual producer result document under `reviews/`; :3 explicitly defers the combined rerun, :17–24 mixes PASS/FAIL/DEFER, and :53–60 identifies unresolved obligations and void attempts. A future `outcome` can select a newly authored short summary in that same result file with `scope_disposition: partial`, explicit limitations, and `remaining_status: unknown` if item mapping is unavailable; it must not convert all rows to PASS. A separately tracked R29 obligation from :57 is a `remaining` event in the same header batch: `item_id` and slot identify the cumulative-counter obligation; `description` states the terminal overwrite; disposition `open`; consequence is lost accumulated counts; revisit condition is verification on the combined fixing candidate; task/ruling references must be resolved first. This is not a dedicated entry-file exception: the fact already has a result artifact. Source hash: table above. No separately named literal `RESULT.md` appeared in the inspected review populations; functional producer result documents satisfy the class, not a guessed filename.

**[P/I] Acceptance/certification — qualifications remain beside PASS.** T26 product acceptance :3 pins judged report, pre-rebase/landed identity, app, scanner and rubric; :5 scopes PASS; :33 counts four rows and distinguishes nine independently confirmed from two accepted-on-citation checklist items. F1 at :35–44 says the Run-side clause appears only after relaunch and the evidence cannot settle the explanation. A `note` can select `F1 — 25r's Run-side clause is confirmed only after relaunch` as its body and qualify the exact certification/ruling reference. The rest of the certification is linked, not copied into 4 KiB. The acceptance source remains authoritative; a later observation does not erase retained R5c FAIL (:46–52) or broaden the “not judged” scope (:54–56). A caller supplying the pre-rebase hash must actually resolve it; otherwise retain the document by content digest and report that revision unavailable, not assume tree equivalence from prose.

**[P/I] RESULT message — the early archive demonstrates both reuse and retention limits.** The retained workflow's completion for task 8 contains its accepted artifact tip, 77/80 line count, build/dev timings and the explicit sentence “Visual quality, production build, and native Windows support not validated.” That whole 307-byte summary can be rendered as-is: **zero ledger event**, zero typed `fulfilled` declaration, zero new file. Task 6's 572-byte completion already includes its unresolved privacy seam; preserve it without requiring a duplicate `remaining` item unless someone needs an independently actionable disposition. Sources: extracted `workflow_events.jsonl` in the recorded scratch directory, corresponding rows identified below; `completion-census.json`.

**[P/U] The archive's original-message limit matters:** none of 106 raw inbox `text` fields starts with RESULT. Six nested `idle_notification.summary` fields have a `[to …] RESULT` label and an associated nested `result` turn-end body; e.g. `taurjob-team/inboxes/team-lead.json`, array index 8, labels T6's `7bc4b74` and the body reports the 47-check artifact plus open questions. That is retained RESULT-related prose, **not proof of the exact sent message body**. Use it as an imported source with archive/member/field/index/digest, never fabricate an authenticated original-message event ID. Future ordinary file/stdin submission names the exact submitted-byte digest and existing source locator; SD3 boundary export bundles bytes for retained replay, not ingest-time copying. Evidence: `result-echo-census.json`; archive hash above. A closed export of the actual messages would settle exact RESULT-to-completion overlap.

**[P/I/U] Exceptional dedicated declaration — worked rejection prevents reintroducing ceremony.** NOTES set :7 already records the direction-B pick and cites #9 ruling sequence 2. Copying that into a dedicated `decision` file must be rejected as a required workflow step: projection of the existing authority is sufficient; if a typed link is missing, attach it to NOTES. Likewise T26 :57's remaining item belongs in its existing result. No real qualifying dedicated-entry file or genuinely artifact-absent historical declaration was identified in this population. It would be dishonest to invent one and call it a real artifact example. The allowed future exception is an authorized scope declaration or standalone decision for which no result/review/NOTES/retained ruling already exists; its first and only Markdown file is the artifact, using the SD5 dedicated form: the same header above a literal body consisting of everything after the front matter, bounded to 4096 bytes; explicit body plus non-empty trailing prose is invalid_input. Safe default: do not create that exception during ordinary submission. This is a worked negative case for the class and a U population gap; an operator-supplied real artifact-absent decision would supply the positive fixture. Basis: adjudication :41–44; brief :127–130.

**[P/I] Programmatic JSON — emit a link, not re-encode authored review prose.** The actual t15 `SHA256.json:2–12` maps evidence filenames to SHA-256 digests (including `README.md`, mutation patch, gate/capture logs). It has no ledger envelope and is **not** a valid event input. An explicit machine emitter may emit a `note` with empty `body`, primary task/scope refs, a `result_artifact` reference to this immutable JSON manifest and evidence refs for files it truly checked. These normalize to the same event schema and limits as Markdown. Do not invent PASS or `source_checked` merely from a hash table. Mesh computes the target digest; missing optional evidence requires `assessment: unavailable` plus a limitation, while required evidence is `missing_reference` even with that assessment. No JSON emitter ran here and no hash-table target logs were replayed. This real JSON artifact anchors the future emitter example without confusing an existing manifest with a shipped ledger entry. Source hash: table above; current artifact handler stores path strings only (Mesh `src/task_lifecycle.rs:550–568`).

### Inspected source seams and preserved obligations

**[S] Source inspection identifies these existing carriers; runtime conformance beyond the parser was not exercised:**

| Carrier | Verified source and consequence |
|---|---|
| Task metadata | Mesh `src/types.rs:181–208`: optional arbitrary JSON metadata plus flattened unknown top-level fields. `src/task_lifecycle.rs:216–232,713–736`: object metadata is cloned and selected keys merged/cleared; non-object metadata becomes empty. Therefore arbitrary preservation is true for object metadata, not every possible JSON value. |
| Completion summary | Mesh `src/task_lifecycle.rs:368–378`: exact input string written to completion_summary. `src/main.rs:4035–4043`: same summary copied to workflow task_completed. No ledger event derives from it today. |
| Progress | Mesh `src/task_lifecycle.rs:318–328`: summary stored as last_progress and timestamp; may advance pending to in_progress. It is an existing lifecycle authority, not a neutral ledger read. |
| Ruling | Mesh `src/task_lifecycle.rs:445–481`: sequenced entry records kind/value/by/at and optional ref/note, and preserves ruling array; score/verdict update designated scalar mirrors. Ref is a string, not a validated artifact identity. |
| Explicit artifacts | Mesh `src/cli.rs:508–510,539–541,559–561`; `src/task_lifecycle.rs:550–568`: path/kind/added_by/added_at appended, no file read or content hashing in that helper. |
| Existing completion route | Mesh `src/main.rs:4064–4068`: completion packet rendered and fan-out invoked, followed by activity. Existing lifecycle behavior must remain in its own authority. |
| House front matter (P: prior inspection) | `src-tauri/src/session/parser.rs:37–60,119–145`: Markdown/YAML handoff parser; stronger literal-byte and duplicate-key rules are proposed here, not inherited evidence. `ARCHITECTURE.md:78,334` references handoff summaries/events; the detailed handoff claim is in `CLAUDE.md` Architecture Summary, not a dedicated ARCHITECTURE “Session handoffs” heading at this checkout. |
| Literal prompt files (P: prior inspection) | `.claude/workflows/research-sweep.js:570–593,663–669`: prompt path and stdin redirection; task text written verbatim to file. Source precedent only; no workflow or real CLI was launched. |
| Delivery obligations | `docs/team-delivery-standard.md:9–13,64–94,103–119`: measure/diagnose can have no commit; budgets have a fixed counting basis; review manifest freezes evidence/scope; findings/questions/scores remain distinct; one acceptance owner. Intake must not restate or broaden these. |

**[P, grounded seat testimony] All nine seat files were inventoried by hash; their relevant mechanisms, rather than a popularity inference, constrain the design:**

| Seat source under taurjob `docs/wave-2/concept-review/` | Evidence used |
|---|---|
| design-lead.md:40–44 | Committed review is already the fact; a separate file duplicates it. |
| altitude-reviewer.md:42–46 | Reviewer authored zero ledger rows; lead mirrored reports; avoid moving that transcription to reviewers. |
| ui-implementer.md:40–50 | Diff-confirm append must not become RESULT + file + command; card cannot substitute for directed delivery. |
| heavy-implementer-1.md:15–16 | Ordinary small lanes must not acquire a reporting cycle; peer verdicts need projection protection. |
| heavy-implementer-2.md:16,24–25 | New command must replace an action; retained pointers are not retained evidence; correct serialization does not certify science. |
| judge-astra.md:17,24–27 | No normal-RESULT ritual; safe card audience; explicit evidence-retention and uncertainty. |
| architect.md:19,26–31 | No duplicate answer transcription; distinguish informational RESULT from GO; retain evidence lineage. |
| asset-generator.md:28–42,47–55,71–87 | NOTES already contains hashes/limits; stage growth is legitimate; stale producer status differs from current acceptance. |
| lead-taurjob.md:48–56 | Narrative per task/day, and a worked example, are required for adoption. |

**[P] Survey bytes are identified in `artifact-inventory.json` and the identity table in `artifact-measurements.md`.** The synthesis's own SHA-256 is `1e7bfc58540dce772868302a1b4c2b2a641afbc5197e2a244c3847b5e9a89cad`; it is not treated as proof that every referenced incident was remeasured. Individual source hashes are retained so future document changes cannot silently change this population.

### Isolated command observations

**[S] Fresh fix-round observations.** Every invocation below used `/home/mstie/.local/bin/mesh`, `env={}`, closed stdin, the same temporary cwd and explicit `--claude-dir`, and a ten-second timeout. The runner kills only its own still-running child in `finally`; all eight children exited, the root stayed empty, and the temporary root was removed. No lifecycle handler or product gate was exercised. Binary SHA-256: `408c7e0cc7ed5302f4e5fb9a2be3b8e47f75b4c14ab9e2127e6bad4572f2f672`. Version fields match `src-tauri/resources/mesh.lock.json:2–5`; the hash identifies the binary, not a hash field in that lock. Reproduction: `python3 .check-logs/mesh-next-phase0/artifact-fix-probes.py` from the specified checkout. Runner: `artifact-fix-probes.py:1`; exact argv/exits/stdout/stderr: `mesh-artifact-fix-probes.json:1`.

**[S] Probe 1 — exit 0.**

```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/artifact-fix-probe-wwf0ggps version
mesh 0.2.29 (commit 6789201c5511b51be704fe30c6e4d025f3e64f8c, protocol 1, schema 1, dirty: false)
```

**[S] Probe 2 — exit 0.**

```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/artifact-fix-probe-wwf0ggps task complete --help
Complete with required --summary evidence; an unstarted task records an implied start and completion in one action

Usage: mesh task complete [OPTIONS] --summary <SUMMARY> <ID>

Arguments:
  <ID>  Task ID

Options:
      --summary <SUMMARY>            Completion summary
      --team <TEAM>                  Team name [env: MESH_TEAM=]
      --assignment <ASSIGNMENT>      Optionally require the current assignment ID
      --name <NAME>                  Agent name (your handle) [env: MESH_NAME=]
      --artifact <ARTIFACT>          Evidence path to append to the task record
      --claude-dir <CLAUDE_DIR>      Claude config directory [env: CLAUDE_DIR=]
      --as-lead                      Confirm this is an explicit team-lead repair mutation
      --admin-reason <ADMIN_REASON>  Explicit admin reason for a team-lead repair mutation
  -h, --help                         Print help
```

**[S] Probe 3 — exit 0.**

```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/artifact-fix-probe-wwf0ggps task progress --help
Record progress for the current owner; prior start is not required

Usage: mesh task progress [OPTIONS] --summary <SUMMARY> <ID>

Arguments:
  <ID>  Task ID

Options:
      --summary <SUMMARY>        Progress summary
      --team <TEAM>              Team name [env: MESH_TEAM=]
      --assignment <ASSIGNMENT>  Optionally require the current assignment ID
      --name <NAME>              Agent name (your handle) [env: MESH_NAME=]
      --artifact <ARTIFACT>      Evidence path to append to the task record
      --claude-dir <CLAUDE_DIR>  Claude config directory [env: CLAUDE_DIR=]
  -h, --help                     Print help
```

**[S] Probe 4 — exit 0.**

```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/artifact-fix-probe-wwf0ggps task review --help
Record a review or handoff request (requires start --assignment first, unless --as-lead)

Usage: mesh task review [OPTIONS] --summary <SUMMARY> <ID>

Arguments:
  <ID>  Task ID

Options:
      --summary <SUMMARY>            Review summary
      --team <TEAM>                  Team name [env: MESH_TEAM=]
      --assignment <ASSIGNMENT>      Optionally require the current assignment ID
      --name <NAME>                  Agent name (your handle) [env: MESH_NAME=]
      --artifact <ARTIFACT>          Evidence path to append to the task record
      --claude-dir <CLAUDE_DIR>      Claude config directory [env: CLAUDE_DIR=]
      --as-lead                      Confirm this is an explicit team-lead repair mutation
      --admin-reason <ADMIN_REASON>  Explicit admin reason for a team-lead repair mutation
  -h, --help                         Print help
```

**[S] Probe 5 — exit 0.**

```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/artifact-fix-probe-wwf0ggps task ruling --help
Append an attributed, sequenced ruling to a task

Usage: mesh task ruling [OPTIONS] --kind <KIND> --value <VALUE> <ID>

Arguments:
  <ID>  Task ID

Options:
      --kind <KIND>              Ruling kind: verdict, score, ruling, or note
      --team <TEAM>              Team name [env: MESH_TEAM=]
      --name <NAME>              Agent name (your handle) [env: MESH_NAME=]
      --value <VALUE>            Exact ruling value
      --claude-dir <CLAUDE_DIR>  Claude config directory [env: CLAUDE_DIR=]
      --field <FIELD>            Optional field the ruling addresses
      --ref <REFERENCE>          Optional commit, packet, or assignment reference
      --note <NOTE>              Optional explanatory note
  -h, --help                     Print help
```

**[S] Probe 6 — exit 2.**

```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/artifact-fix-probe-wwf0ggps ledger --help
error: unrecognized subcommand 'ledger'

  tip: some similar subcommands exist: 'leave', 'nudge', 'lease'

Usage: mesh [OPTIONS] <COMMAND>

For more information, try '--help'.
```

**[S] Probe 7 — exit 2.**

```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/artifact-fix-probe-wwf0ggps ledger entry --file artifact.md --team fixture --name seat
error: unrecognized subcommand 'ledger'

  tip: some similar subcommands exist: 'leave', 'nudge', 'lease'

Usage: mesh [OPTIONS] <COMMAND>

For more information, try '--help'.
```

**[S] Probe 8 — exit 2.**

```text
/home/mstie/.local/bin/mesh --claude-dir /home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/artifact-fix-probe-wwf0ggps task complete 1 --summary-file RESULT.md --team fixture --name seat
error: unexpected argument '--summary-file' found

  tip: a similar argument exists: '--summary'

Usage: mesh task complete --summary <SUMMARY> <ID>

For more information, try '--help'.
```


### Retained completion positions

**[P]** The prior lane extracted only the allowlisted archived workflow member to the scratch root (no archive-wide extraction). Member `taurjob-team/state/workflow_events.jsonl`, SHA-256 `907075d8fbaa2d9cbaa5424ecbc17880bf7aff135821134585d8d6b56c3dbe38`. The following one-based line numbers refer to `/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/artifact-research-7d2jw9oc/workflow_events.jsonl`; the archive identity remains the authority. Full selected summaries are retained in `completion-census.json`.

| Task | Workflow line | Summary bytes |
|---|---:|---:|
| 3 | 131 | 290 |
| 4 | 152 | 138 |
| 5 | 172 | 247 |
| 6 | 182 | 572 |
| 8 | 238 | 307 |
| 7 | 307 | 131 |
| 9 | 315 | 148 |

**[P/I] Prior reproduction method (not re-executed in this fix round):** inventory immediate `reviews/*.md` in each wave; count recursive regular files separately without following symlinks; UTF-8 bytes via byte length, lines via splitlines, header flags via literal first line; SHA-256 per file with inode/size/mtime checked around each read. Load only the allowlisted archived workflow member and JSON inbox members; count raw RESULT-first separately from decoded idle summaries matching `[to ...] RESULT`. Completion counts select exact `eventType == task_completed`. Numeric pipe-line counting is a syntactic census, deliberately not a Markdown/task parser. No source logs, test executables, referenced private paths or live task roots were needed. The JSON census files preserve the measurements; the semantic classifications and proposed counterfactual header are I, not measured native events.

### Artifact-class census

**[P/I] Taxonomy is a reproducible filename heuristic, not a parser permission or a claim of exclusive semantic classes:** plan/register/triage wins first; exact T26-certification.md is a producer certification; evidence-named files and T26-enrichment.md are producer evidence; remaining acceptance-named files are acceptance reviews; all others are other review/audit. Counts and byte sums are P retained measurements from `artifact-inventory.json`; classification is I. NOTES and RESULT-message populations are counted separately above. No renamed file would acquire a different ingestion authority.

| Wave | Heuristic class | Files | Bytes | Front matter | RESULT-first |
|---|---|---:|---:|---:|---:|
| wave-1 | acceptance review | 12 | 98247 | 0 | 0 |
| wave-1 | certification producer result | 1 | 6765 | 0 | 0 |
| wave-1 | other review/audit | 36 | 384749 | 0 | 0 |
| wave-1 | plan/register/triage | 2 | 74727 | 0 | 0 |
| wave-1 | producer evidence | 3 | 22421 | 0 | 0 |
| wave-2 | other review/audit | 36 | 334283 | 0 | 0 |
| wave-2 | plan/register/triage | 2 | 47453 | 0 | 0 |

### Round-1 document verification

**[S] Retained-evidence recount and document checks:** command `python3 .check-logs/mesh-next-phase0/artifact-fix-verification.py` (checkout cwd; only this output directory written) produced the following stdout with exit 0. The runner recounts the saved inventory/row bytes, compares the report’s header and command quotations to their evidence files, checks the three seat rules and the 16 fix-record entries. These are document consistency checks, not parser/handler acceptance or an independent review. Sources: `artifact-fix-verification.py:1`, `artifact-fix-verification.json:1`; source population hashes remain in `artifact-inventory.json:1`.

```text
Document/evidence checks passed; header=426 bytes/18 lines; retained inventories=54/586909,38/381736; rows=1297,614,263; mesh exits=0,0,0,0,0,2,2,2; fix records=16; seat rules=3. No product gate or approval claimed.
```

## Recommendation

**[I] Increment C boundary (binding OQ-A):** implement the Markdown/JSON adapter, validation, reference resolution, writer/fold/CAS/manifest from the study, the adapter call from existing completion/review/progress submission, and `--summary-file` (`-` = stdin). SD1 has exactly those two submission additions. C records SD4 `audience_ref` from authenticated context and rejects restricted ledger input without audience proof, while review delivery commits. **Not in C:** submission-receipt store and task-writer idempotency changes (both DROPPED), or export-side audience enforcement (increment D). SD2a/b/c, SD5 and SD6 define the bounded intake contract; SD3 records referenced retention at ingest and archives only at boundary export. Preserve writer-only canonical JSONL, four kinds, 4 KiB prose, CAS and pull rendering. Basis: `d1-opus-findings.md:5–35`; study :152–186,239–245.

**[I] SD1 implementation boundary:** the existing lifecycle handler owns lifecycle effects. Cheap syntactic pre-check → existing source commit/delivery → ledger validation/admission under its own lock. No new submission coordinator, durable receipt store, coordinator lock or task/workflow idempotency protocol. Source and ledger receipts are returned together as the command result. If ledger fails after source commit, return nonzero and direct repair to `mesh ledger entry --file <the same artifact>`, never a repeat lifecycle mutation. For restricted review without SD4 proof, delivery succeeds and ledger returns `source_incomplete` / `audience_proof_unavailable`; D separately implements projection enforcement. Basis: `d1-opus-findings.md:6,33`; Mesh `src/main.rs:3978–4068`.

**[I] Recovery acceptance criterion:** observe source delivery committed before a ledger YAML-schema/bounds/reference/authority/CAS/audience/storage failure and a nonzero split receipt. Standalone repair must have zero lifecycle effects and use event UUID plus normalized own-content equality; unrelated NOTES edits do not conflict. Storage uncertainty retries that event unchanged via standalone intake, returning the original ledger receipt if committed. Semantic corrections require deliberate corrected event content/new UUID and expected head/reason for amendments; never auto-rebase. The source delivery remains complete even while ledger repair is outstanding. There is no source-level crash/retry exactly-once promise and no new receipt store. Exceptional repair contributes estimator `R`; normal valid annotation remains one submission. Basis: `d1-opus-findings.md:6,9,33`; study :242–247,295–308.

**[I] Bounded increment-D validation plan, not work performed in this lane:** use isolated temp roots, synthetic identities/actors and corpus-derived nonsecret fixtures. Execute ordinary completion with no ledger header (zero extra ledger events); one submission with NOTES selection and one restricted review; multi-event outcome/remaining batch; standalone ledger retry after each ledger durable boundary and split source/ledger failure; unrelated NOTES append retry; duplicate-ID/content conflict; simultaneous stale amendments; missing/oversize/duplicate-key/ambiguous-heading inputs; warning-only attachment sniff failures and explicit JSON discrimination; SD5 trailing-body/selector ambiguity and SD6 flag/front-matter disagreement; boundary-bundled replay after removing the original artifact, and closure refusal for missing required referenced-only bytes; unauthorized scope claim and pre-lock peer-view denial across Markdown/JSON/history/export; post-lock authorized release; narrative replay of earlier FAIL/later PASS without losing limitations. Verify original artifact bytes and task/ruling authority are untouched by ledger-only operations. No load or paid CLI lane is needed. Only after those executions pass should role examples and the pin change through the normal release route. Basis: study :436–447 and :341; this lane executed none of those tests.

### Open questions, safe defaults and settlement evidence

**[U — UNVERIFIED unless labeled I] These are first-class acceptance items, not silent assumptions:**

| Question | Safe default / decided behavior | What settles it |
|---|---|---|
| Does the candidate implement source-first split receipts and standalone repair? | I: source commit precedes ledger admission; no task-writer idempotency or submission store. A lost response requires inspecting existing source state, not blind lifecycle retry. | Increment-D fault fixtures observe source receipt plus ledger error/nonzero, then UUID-idempotent standalone repair with zero task/workflow mutations. |
| Which concrete frozen record supplies wave/scope and standalone-repair context today? | I: the approved wave packet linked by the assignment is the authority seam (delivery standard :19–27; study :217–221). S: inspected Mesh Task is generic metadata, not typed wave/scope identity (Mesh `src/types.rs:181–208`). U: a deployed immutable mapping/locator and standalone context-binding syntax have not been verified. Reject `source_incomplete`, never guess from cwd/display labels or scan tasks. | Orchestrator names an approved packet/frozen assignment containing actual IDs and a stable locator; C binds explicit standalone context to it, and D runs the same kind/slot-only artifact through source submission then standalone repair. No durable submission store is permitted. |
| Which frozen review-policy artifact supplies audience proof? | I: authenticated assignment/review context supplies `audience_ref`; without it deliver review and reject its restricted ledger part with `audience_proof_unavailable`. | Authority owner names the existing immutable policy seam; C fixtures verify capture, D fixtures verify policy change/reassignment/lock/release and every export. |
| Can the review audience be enforced on every consumer/export? | I: fail closed, including derivatives and mixed NOTES/review files; team-visible-only mode rejects restricted input. | Cross-surface denial/release fixtures, including offline bundles and errors. Shared filesystem access is not a promised security sandbox. |
| Does the final wave-2 completion source already contain most outcomes/remaining prose? | Unknown. Seven early wave-1 summaries prove feasibility of reuse, not a full-wave percentage. | Closed final wave-2 source export, matched by assignment/stage, and independently coded proposition overlap. |
| Do exact directed RESULT messages survive somewhere in the permitted archive? | Inspected inboxes supply zero raw RESULT-first bodies and six labelled idle echoes. Do not treat echoes as exact message submissions. | An authorized immutable message export with original IDs/body bytes; no live-root search. |
| Is there a positive real dedicated-entry example? | None identified; use the real NOTES pick and T26 remaining item as negative duplicate-authoring examples. No fabricated historical entry. | A commissioned fact absent from all existing artifacts/rulings; its first authorized declaration supplies the positive fixture. |
| Are 16 events, 16 KiB YAML and 1 MiB Markdown sufficient, and is 4096 prose bytes practical? | I: fixed conservative bounds, explicit oversize error. Measured reviews fit artifact bound; this does not validate all future source formats. | Corpus-derived parser fixtures and full-wave metadata-byte/error census; amend limits explicitly if needed, no truncation. |
| Where does the boundary bundle survive root/account moves? | I: SD3 defaults referenced; archived only after boundary export bundles bytes. No ingest evidence store or remote fetching; closure refuses unresolved required evidence. | Name the durable boundary-export destination and retain-until authority, then remove original root in a fixture and verify authorized offline replay from that bundle. |
| Can a head/authority conflict after source commit be hidden behind a success? | No. I: split receipt with source delivery complete and ledger repair outstanding, deliberate corrected standalone request; ordinary source lifecycle is never rolled back by a ledger error. | Candidate fault/concurrency fixtures verify no duplicate task completion and visible repair accounting. |
| Has this design or its future syntax passed the appointed review? | Round-1 findings required fixes; this revision does not claim their acceptance. Fresh probes 6–8 reject ledger and summary-file syntax. | Orchestrator verification of this single Round-1 fix (no third round), followed by increment-D parser/handler execution before activation. |
| What exact model deployment identifier ran this research? | The session identifies the assistant as GPT-6; no tool-observed deployment identifier was supplied. Do not infer the brief's requested Astra label as a verified runtime fact. | Caller/orchestrator execution metadata, not a filesystem or account-root search. |

**[I] Scope boundary:** no product implementation, tests, corpus migration, role edits, historical native-event backfill, ledger snapshots per event, summarizer, watcher, directory scan, Git hook, runtime launch, build-host coordinator, or replacement task database belongs to this research deliverable. The artifact author retains the existing delivery standard; the new mechanism only removes duplicate ledger authoring and records explicitly missing typed claims. Basis: brief :123–143; study :410–412,449.

### Round-1 fix record — 2026-09-08

**[S/I] Applied document fixes, not an acceptance verdict.** The directions in `d1-opus-findings.md:5–35` are represented at the exact locations below. S covers the edited text and reproducible evidence; I covers the still-future behavior. Orchestrator verification is pending; no third review round or product gate was run.

| Finding | Direction applied | Exact fix location in this document |
|---|---|---|
| 1 | Source-first lifecycle/delivery, split receipt/nonzero, standalone UUID repair; receipt store/coordinator/task-writer changes dropped; interim restricted-review reason named. | [line 33](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:33); [line 433](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:433) |
| 2 | Retry equality uses normalized event content, excludes self digest; unrelated artifact bytes do not conflict. | [line 88](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:88); [line 114](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:114) |
| 3 | Derive wave/scope on submission; standalone packet/assignment seam and fail-closed mapping gap; header re-priced. | [line 41](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:41); [line 156](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:156); [line 446](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:446) |
| 4 | Mesh computes digests; required/optional reference table and canonical/reference-field equality published. | [line 45](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:45); [line 57](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:57) |
| 5 | SD5 names selectors and retains literal trailing body, explicit precedence and dedicated-body conflict. | [line 68](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:68); [line 214](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:214) |
| 6 | .md-only bounded attachment sniff; unsafe/unreadable/unparsed inputs warn without ledger read/error; parsed invalid namespace fails after delivery; explicit --json. | [line 29](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:29); [line 37](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:37) |
| 7 | SD2a bounds, SD2b atomic batch, SD2c error vocabulary split. | [line 74](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:74); [line 94](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:94); [line 106](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:106) |
| 8 | Authored adapter_version replaces schema_version; writer envelope fields forbidden. | [line 39](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:39); [line 186](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:186) |
| 9 | SD6 expected head flag/front matter, disagreement invalid_input, submission front matter authority. | [line 90](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:90) |
| 10 | Current/narrative bound to render --view, same cut and formats, default current. | [line 132](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:132) |
| 11 | Every §10 corpus population dated as-of 2026-09-08 04:05 UTC with inventory hashes; later lens numbers separate. | [line 152](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:152); [line 156](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:156) |
| 12 | Compound provenance/interpretation labels defined; retained corpus examples marked P. | [line 19](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:19) |
| 13 | ./self escape recognized before path normalization. | [line 43](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:43); [line 57](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:57) |
| 14 | §9 reduced to three rules plus help/example. | [line 138](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:138); [line 146](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:146) |
| OQ-A | C scope restated; D owns export/projection enforcement; C captures proof and rejects restricted ledger intake when absent. | [line 431](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:431); [line 126](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:126) |
| OQ-B | No ingest evidence store/copies; referenced default; archived only at boundary bundling; closure refuses unresolved required evidence. | [line 80](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:80); [line 84](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:84); [line 453](/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/ledger-artifact-as-event.md:453) |
