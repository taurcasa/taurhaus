# Team delivery standard

This standard scales delivery ceremony to the kind of work being done. Role definitions name the work kinds they perform; assignments link here instead of repeating these rules. Repository instructions and a committed product or design packet still take precedence when they impose a narrower constraint.

## Work kinds

| Work kind | Default evidence and artifact | Red-first policy | Review depth |
|---|---|---|---|
| **Measure** | The measurement is the artifact. Record the method, result, and declared instrument limits. No commit, screenshot, quoted-copy block, or duplicate evidence report is required unless that evidence is itself the measurement. | Not required; measurement establishes the baseline. | One acceptance-owner check by default. |
| **Diagnose** | A concise cause statement backed by reproducible observations, relevant file or runtime evidence, and the smallest useful probe. Do not change production behavior unless the assignment is reclassified or explicitly authorizes it. | Required when the diagnosis adds a regression test for broken behavior; otherwise optional. | One review by the acceptance owner or the owner of the affected surface. |
| **Implement** | A commit plus the tests and focused validation that prove the assigned behavior. Visible product surfaces include rendered evidence when that evidence is needed to judge the change. | Required for behavior changes and regression risk. It may be skipped for scaffolding or copy-only changes; state the skip and why in the result. | One review by default. Two independent reviews only when the wave declares the affected area a hero surface. |
| **Review** | Numbered, standalone findings followed by the applicable score table. Each finding names evidence, impact, and the action needed; separate questions from defects. Prose is optional. | Not required unless the review assignment explicitly includes writing a regression test. | The review is the review step; an additional review is required only for a declared hero surface or an explicit acceptance route. |
| **Spec-delta** | A small, committed packet or specification edit with the changed ruling and its consequence stated plainly. | Not required for copy or scaffolding. Use red-first only when the delta changes an executable contract with regression risk. | No review beyond the named acceptance owner. |

A wave declares its hero surfaces before implementation or in the committed packet. Undeclared surfaces receive one review. Double review is insurance for the product areas where reviewer error would materially damage the outcome, not a default for every change.

## Assignment contract

An assignment is five lines:

1. **Objective:** one sentence describing the outcome.
2. **Deliverable:** the exact path, artifact, or output contract.
3. **First action:** an imperative verb plus the first concrete file, command, or probe.
4. **Completion signal:** the task state or message that means the work is ready.
5. **Review route:** the work kind, acceptance owner, required reviewer(s), and any seam or handoff.

The committed packet is the specification. Link it and this standard; do not paste their doctrine into the assignment. A correction supersedes the earlier instruction rather than appending a competing version.

## Message conventions

The completion-signal line also states the response expectation. Prefix a request that requires execution with `ACTION REQUIRED:`. Prefix context that needs no action with `INFO ONLY:` and end it with `no response needed`. Do not send a pure acknowledgment; execute the first action, then report through the named completion signal.

## Results and reviewer artifacts

Results are compact: give the commit hash (or `none — measure/diagnose only`) and the findings or outcome bullets. Include only evidence needed to assess the claim, plus any red-first skip required by the work-kind table. Essays and repeated gate transcripts make the decision harder to find and are discouraged.

Validation follows repository instructions: run the repository's per-task gate when one is named. The full serialized gate belongs to the lead unless narrower repository instructions explicitly assign it elsewhere.

A reviewer returns:

1. numbered findings in severity order, each able to stand alone;
2. open questions, clearly separated from confirmed defects; and
3. the score table required by the assignment or role, with values copied as fields rather than paraphrased in relay.

`No findings` is a valid result. Optional prose may explain residual risk but must not hide a finding or alter a score.

## Ownership and decision rights

Each surface has exactly one accountable implementer and one acceptance owner. The implementer decides in-scope implementation details and owns the delivered change. The acceptance owner decides whether the evidence satisfies the packet and may accept, reject, or request a bounded correction. Other specialists advise through the review route; they do not silently override either owner.

Assignments state cross-surface seams and handoffs up front, including who may commit in each area and who accepts the handback. A lane that needs to cross an ownership boundary asks that surface's owner unless its role or assignment grants standing authority.

## Optional deadline and effort overrides

Deadlines and model effort are optional overrides, not required assignment fields. Use a deadline for genuinely time-sensitive or long-running work where automated escalation is useful. When set, the daemon deadline pass sends one nudge at half the interval — suppressed while the member is visibly active — and marks the task stale once at the deadline. Without that desired escalation, omit the deadline.

Use an effort override only when the task is meaningfully model-effort-sensitive. No `--why` explanation is required for either override.

## Communication economy

Broadcast only when the information changes work for every recipient. Prefer targeted routing for surface-specific evidence and decisions. Corrections supersede older notices; completion evidence is reported once at its canonical task or artifact location rather than copied into several messages.
