# README content plan

Date: 2026-03-07
Task: #596
Input: [readme-gap-analysis.md](/home/user/projects/taurhaus/docs/readme-gap-analysis.md)

## 1. Section outline

### 1. Hero

Open with one crisp product statement, one supporting paragraph, and one primary screenshot. This section should immediately position taurhaus as a desktop operations surface for developers running multiple AI tools and Mesh teams across many projects. Avoid jokes here; lead with control, visibility, and context recovery.

### 2. Why taurhaus

Summarize the three main reasons to use it: live session supervision, fast context recovery, and controlled multi-agent coordination. This section should make the problem concrete for power users who already live in tmux, editors, and AI CLIs.

### 3. Core workflows

Present taurhaus by workflow rather than by tab. Use short subsections such as “Watch live work,” “Recover context,” and “Coordinate a team,” each with a concise outcome-oriented explanation.

### 4. Product tour

Show the strongest screenshots in the order of actual user value, not implementation order. This should visually prove that taurhaus is not just a browser shell: include session-rich sidebar state, task/history visibility, and Mesh setup/runtime.

### 5. Mesh teams

Give Mesh its own section instead of burying it in a generic features list. Cover templates, launch/setup, runtime visibility, hot-add/remove, resume/recovery, and disband/re-onboard at a product level without falling into backend implementation detail.

### 6. Install and prerequisites

Keep installation practical and explicit for Windows and macOS, but make it more scannable than the current README. Preserve the platform-specific prerequisites, explain why WSL2 mirrored networking matters on Windows, and link to the detailed setup guide for troubleshooting depth.

### 7. First launch and quick start

Walk the reader through the first-run wizard and the first few meaningful actions. This section should make the time-to-value path obvious: register projects, launch or resume sessions, open Mesh if needed, and search across projects.

### 8. Development

Keep contributor guidance in the README, but make it obviously separate from user setup. Cover stack, main `just` recipes, Bun-only workflow, and point contributors to `CONTRIBUTING.md`, `ARCHITECTURE.md`, and docs for deeper detail.

### 9. Architecture at a glance

Retain a short architecture section with the system diagram lower in the page. This section should reassure technical readers that taurhaus is a serious native app with a daemon, SQLite, Tantivy, and tmux/session integration, without turning the README into an architecture manual.

### 10. License

Keep license at the end, minimal and standard.

## 2. Key messaging

The README should consistently communicate these value propositions:

1. **Operational visibility**: taurhaus gives you one place to see what Claude, Codex, Gemini, and Mesh agents are doing across projects right now.
2. **Context recovery**: taurhaus helps you recover project state quickly through README preview, commits, tasks, handoffs, history, and search.
3. **Multi-agent control**: taurhaus turns Mesh from a fragile terminal ritual into a visible, recoverable team workflow with setup, runtime status, and resume paths.
4. **Native developer tool**: taurhaus is not a cloud wrapper or browser dashboard; it is a native Tauri app integrated with tmux, local repos, local CLIs, and local storage.
5. **Power-user fit**: taurhaus is for people already running serious multi-project AI workflows, not for lightweight demo usage.

## 3. Tone guide

Tone target:

- professional but approachable
- confident, not self-conscious
- technically credible without sounding academic
- operator-focused and concrete

What to do:

- use direct product language: “launch,” “resume,” “inspect,” “coordinate,” “recover”
- prefer outcome statements over adjectives
- be specific about supported workflows and platform behavior
- assume the reader is technically fluent

What to avoid:

- self-deprecating jokes
- “we’re not here to judge” framing
- novelty-project voice
- overblown startup-style hype

## 4. Screenshot plan

Approximate placement in the new README:

1. **Hero screenshot** directly below the opening value statement
2. **Session + project supervision screenshot** inside Core workflows
3. **Task/search/context screenshot** inside Core workflows
4. **Mesh setup screenshot** at the start of the Mesh teams section
5. **Mesh runtime screenshot** in the same section, after setup
6. **Optional settings or onboarding screenshot** only if setup confidence needs visual proof

Important rule:

- prioritize screenshots that prove differentiated workflows
- do not spend screenshot budget on views that are already common in other tools unless they support a stronger workflow story

## 5. Feature organization

Do not organize the README by tab.

Recommended grouping:

### Watch live work

- project activity groups
- live session indicators
- hover previews
- terminal/session focus controls

### Recover context fast

- overview summaries
- README preview
- commit history and diffs
- task board and session history
- full-text search across files, sessions, and commits

### Coordinate Mesh teams

- role/preset templates
- setup and initialize
- runtime canvas and member actions
- hot-add/remove/re-onboard
- resume after restart or degraded state

### Install and operate

- platform prerequisites
- first launch
- settings and environment shaping

### Build and contribute

- stack
- `just` recipes
- contributor docs

## 6. What to cut or condense

Cut:

- most of the current “What this isn’t” section
- self-deprecating or apologetic framing
- the flat all-in-one feature bullet list

Condense:

- architecture detail in the main README
- repetitive installation prose that is already covered in [docs/getting-started.md](/home/user/projects/taurhaus/docs/getting-started.md)
- development commands that are better explained in [CONTRIBUTING.md](/home/user/projects/taurhaus/CONTRIBUTING.md)

Keep, but rewrite:

- installation prerequisites
- quick start
- architecture overview
- development section

## Recommended shape in one sentence

The new README should read like a mature workflow product page for serious multi-agent developers, grounded in real screenshots and organized around supervision, context recovery, and team coordination.
