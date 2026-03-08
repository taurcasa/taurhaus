# Objective code quality assessment tools for taurhaus

Date: 2026-03-08
Task: #606
Scope: Rust backend + Svelte 5 / JS frontend + Tailwind v4

## Executive summary

There is no single tool that gives a trustworthy, local-first, private-repo-friendly "objective quality score" for a Rust + Svelte codebase.

For taurhaus, the best answer is a **stack**, not a single dashboard:

1. **Local structural analysis** for complexity, duplication, dependency drift, and maintainability hotspots
2. **Rust supply-chain and safety tooling** for advisories, unsafe usage, dead dependencies, and binary weight
3. **Frontend dependency and architecture tooling** for unused code, cycles, and module graphs
4. **Optional hosted overlays** for trend dashboards, security posture, or AI review

My bottom-line recommendation is:

- adopt a **local-first baseline** built from `cargo-deny`, `cargo-audit`, `cargo-geiger`, `cargo-udeps` or `cargo-machete`, `cargo tree -d`, `Knip`, `dependency-cruiser`, `Semgrep CE`, and `OSV-Scanner` or `Trivy`
- if taurhaus wants one paid "third opinion" on top of that stack, trial **CodeScene** first for maintainability/hotspots, or **Snyk** / **Semgrep Platform** first if the primary concern is security posture
- do **not** make SonarQube, Qodana, or GitHub Advanced Security the only answer for this repo, because the real frontend surface is `.svelte`, and Svelte coverage is still materially weaker than Rust or plain JS/TS coverage

## Evaluation criteria

I evaluated tools against the requirements in task `#606`:

- useful for **private repositories**
- preferably **local or self-hosted first**
- can say something meaningful about **Rust** and the **Svelte/JS frontend**
- adds signal beyond `cargo fmt`, `cargo check`, `clippy`, frontend typecheck, and unit tests
- useful as a genuine **independent second/third opinion**, not just a different wrapper around the same lints

## Recommended local-first baseline

### Tier 1: strongest fit for taurhaus right now

| Tool | Primary value | Rust | Svelte / JS | Deployment | Cost / license | Fit for taurhaus |
|---|---|---:|---:|---|---|---|
| `rust-code-analysis` | Complexity, duplication, maintainability metrics | Strong | Good for JS/TS; no Svelte-specific semantics | Local CLI | Open source, MPL-2.0 | Strong metrics collector |
| `Semgrep CE` | Cross-language static analysis for bug/security patterns | Strong | Good for JS/TS; Svelte weaker | Local CLI / CI | Open source CE; paid platform optional | Strong second-opinion scanner |
| `cargo-deny` | Advisories, licenses, bans, source policy | Strong | N/A | Local CLI | Open source | Must-have |
| `cargo-audit` | RustSec vulnerability scan | Strong | N/A | Local CLI | Open source | Must-have |
| `cargo-geiger` | Unsafe usage across crate graph | Strong | N/A | Local CLI | Open source | High-value audit input |
| `Knip` | Unused deps/files/exports, missing deps | N/A | Strong, including Svelte support | Local CLI | Open source, ISC | Must-have frontend hygiene tool |
| `dependency-cruiser` | Cycles, architecture rules, module graphs | N/A | Strong for JS/TS and supports `.svelte` | Local CLI | Open source, MIT | Must-have frontend architecture tool |
| `OSV-Scanner` or `Trivy` | Cross-ecosystem vulnerability scanning | Strong | Strong | Local CLI | Open source | Must-have cross-check |

### Tier 2: targeted local tools worth adding

| Tool | What it adds | Caveat | Recommendation |
|---|---|---|---|
| `cargo-udeps` | Most precise Rust unused-dependency detection | Nightly requirement | Recommended if nightly is acceptable |
| `cargo-machete` | Fast Rust unused-dependency pass | Less precise than `cargo-udeps` | Good fallback |
| `cargo tree -d` | Duplicate dependency versions | Narrow signal, but cheap | Recommended |
| `cargo-depgraph` | Rust dependency graph visualization | Visualization only | Optional |
| `cargo-bloat` | Binary-size contribution by crate/function | Performance/size lens more than maintainability lens | Optional |
| `cargo-outdated` | Dependency freshness | Not a quality metric by itself | Optional |
| `cargo-vet` | Trust and review workflow for third-party crates | Process-heavy | Long-term option |
| `madge` | Lightweight cycle detection / graphs | Less powerful than dependency-cruiser | Optional fallback |

## Why these local tools are the best baseline

### `rust-code-analysis`

Best fit when the goal is measurable structure rather than style. It gives complexity and duplication metrics that are meaningfully different from `clippy` output.

Use it to:

- rank backend and frontend hotspots by complexity
- compare complexity drift across releases
- detect duplicate logic after refactors

Example:

```bash
rust-code-analysis-cli --metrics --output-format json src-tauri/src > quality-rust.json
rust-code-analysis-cli --metrics --output-format json src/lib > quality-frontend.json
```

### `Semgrep CE`

Best cross-language local scanner when you want a rules-based second opinion on risky patterns.

Use it to:

- catch bug/security patterns that neither compiler nor unit tests catch
- add repo-specific rules later
- scan both backend and frontend in one pass

Example:

```bash
semgrep scan --config auto src-tauri/src src/lib
```

Important caveat:
- Semgrep is strongest on Rust and JS/TS.
- I did **not** find evidence of first-class Svelte analysis comparable to native JS/TS support, so treat `.svelte` coverage as partial.

### `cargo-deny` + `cargo-audit` + `cargo-geiger`

This is the strongest Rust-specific quality/risk trio beyond `clippy`.

- `cargo-deny`: policy and supply-chain guardrail
- `cargo-audit`: direct RustSec vulnerability signal
- `cargo-geiger`: objective unsafe-surface visibility

Examples:

```bash
cargo deny check advisories bans licenses sources
cargo audit
cargo geiger
```

### `Knip`

Knip is the most practical frontend hygiene tool for taurhaus.

Why it fits:

- it has Svelte support
- it understands dead dependencies, dead files, and dead exports
- it helps uncover false confidence in a clean-looking frontend tree
- it also understands Bun-oriented script parsing cases

Example:

```bash
bunx knip
```

### `dependency-cruiser`

Best local architecture-rule tool for the frontend side.

Why it fits:

- supports `.svelte`
- can detect cycles and orphan modules
- can enforce allowed dependency directions between folders
- produces useful graphs for architecture reviews

Examples:

```bash
bunx depcruise src --include-only '^src' --output-type err-long
bunx depcruise src --include-only '^src' --output-type dot | dot -Tsvg > dependency-graph.svg
```

### `OSV-Scanner` or `Trivy`

Rust-only dependency tools miss the Bun side. Taurhaus should have one cross-ecosystem scanner.

Recommendation split:

- choose **OSV-Scanner** if the main goal is a simple vulnerability cross-check on Cargo + Bun lockfiles
- choose **Trivy** if the team also wants a broader security scanner already common in DevSecOps workflows, including license scanning

Examples:

```bash
osv-scanner scan source --lockfile Cargo.lock --lockfile bun.lock
trivy fs .
trivy fs --scanners license .
```

## Hosted / SaaS / dashboard options

This section covers the current mainstream paid or hosted options that teams are actually likely to evaluate in 2026.

Important note:
- The popularity comments below are an **inference** from product maturity, official integration breadth, enterprise positioning, and ecosystem presence.
- They are **not** a market-share ranking.

### Strongest hosted candidates for taurhaus

| Product | Main value | Deployment model | Rust | Svelte / JS | Private repo fit | Recommendation |
|---|---|---|---:|---:|---|---|
| **CodeScene** | Code health, hotspots, temporal coupling, maintainability trends | Cloud or on-prem | Language-agnostic enough to be useful | Language-agnostic enough to be useful | Strong | Best paid maintainability dashboard to trial first |
| **Snyk** | Security platform: SAST + SCA + broader AppSec | SaaS; broker/flexible deployment options | Supported | JS/TS supported; Svelte not first-class | Strong, paid | Best security-first option |
| **Semgrep Platform** | Strong AppSec / SAST / policy platform on top of Semgrep | SaaS or CI-driven local scanning with metadata to service | Supported | JS/TS strong; Svelte partial | Strong, paid | Best if custom rules and AppSec workflow matter |
| **DeepSource** | SaaS code analysis + coverage + SCA + AI review | SaaS; self-hosted in Enterprise | Rust GA | JavaScript GA; no first-class Svelte docs found | Good | Worth a serious trial |
| **GitHub Advanced Security / CodeQL** | Native GitHub code scanning + dependency review + secrets | GitHub-hosted | Rust supported | JS/TS supported; Svelte partial | Good if already on paid GitHub plan | Good platform overlay, not full answer |
| **SonarQube / SonarQube Cloud** | Established dashboard for quality and security trends | Self-hosted Community Build or cloud | Rust supported now | JS/TS/CSS strong; `.svelte` still not first-class | Good | Partial fit only |
| **Qodana** | JetBrains inspection-powered static analysis and cloud reports | Local/CI + Qodana Cloud; self-hosted roadmap/improvements active | Rust not first-class yet; RustRover linter only on roadmap/EAP | JS/TS strong | Mixed | Not a good fit today for this exact stack |
| **CodeRabbit** | AI review of PRs / IDE / CLI | SaaS; enterprise self-hosted only at large scale | Model-based, language-agnostic review | Same | Good | Useful review assistant, not objective quality dashboard |

## Product notes

### CodeScene

This is the most compelling paid "objective quality" overlay for taurhaus.

Why:

- focuses on **code health**, **hotspots**, **complexity trends**, **temporal coupling**, and maintainability
- works as a dashboard instead of just a linter wrapper
- offers **on-prem / self-managed** deployment
- is less dependent on perfect framework-specific parsing than JS/Rust-native SAST tools

If taurhaus wants one paid trial focused on maintainability instead of security, this is the first product I would test.

### Snyk

Strongest security-first hosted option.

Why:

- supports Rust, JavaScript, and TypeScript across SCA and SAST
- has a mature Git/IDE/CLI platform story
- is clearly optimized for private-repo enterprise use

Limitations for taurhaus:

- this is primarily an **AppSec** answer, not a maintainability dashboard
- it will not answer questions like "which files are hardest to change" as well as CodeScene
- I did not find first-class Svelte-specific analysis claims, so treat Svelte as JS/TS-adjacent rather than natively understood

### Semgrep Platform

Best if taurhaus wants to grow from local Semgrep into a broader managed AppSec workflow.

Why:

- same local engine can stay in CI even if platform adoption grows
- strong custom-rule story
- source can remain local if scanning runs in CI/local and only metadata goes to Semgrep

Limitations:

- strongest for security/pattern scanning, not architecture health scoring
- Svelte story is still weaker than plain JS/TS

### DeepSource

DeepSource is more relevant than older assumptions would suggest.

Current fit:

- Rust analyzer is GA
- JavaScript analyzer is GA
- vulnerability scanning supports Cargo and JavaScript ecosystems
- Enterprise includes self-hosted deployment

Caveat:

- I found Rust and JavaScript support, but I did **not** find first-class Svelte-specific analysis documentation
- that makes it a plausible trial candidate, but not an obvious winner for taurhaus without validation on real `.svelte` files

### GitHub Advanced Security / CodeQL

This is a good choice only if taurhaus wants to stay deeply inside GitHub.

Strengths:

- Rust and JS/TS are supported in CodeQL
- dependency review and secret scanning integrate naturally with GitHub workflows
- good for organizations already paying for GitHub security features

Limitations:

- private repos require paid GitHub security licensing
- Svelte is not a first-class analysis language
- better at **security scanning** than at maintainability/coupling/architecture metrics

Practical conclusion:
- good overlay if taurhaus already has GitHub Advanced Security budget
- not the best standalone answer to the original question

### SonarQube / SonarQube Cloud

Sonar remains the most obvious dashboard product teams will compare against.

Current fit:

- self-hosted Community Build exists
- Rust support now exists in SonarQube
- JavaScript / TypeScript / CSS support is mature

But the key problem remains:

- I still found community guidance saying `.svelte` is not first-class supported

Practical conclusion:
- viable if taurhaus wants a familiar dashboard for Rust + JS/TS + CSS trends
- weak as the single source of truth for a Svelte-heavy frontend

### Qodana

Qodana is strong for JS/TS today, but taurhaus is the wrong stack to make it the primary third-opinion tool right now.

Why:

- Qodana for JS is mature and locally runnable
- but Rust is still on the 2026 roadmap as a RustRover linter EAP, not a mature first-class offering today

Practical conclusion:
- good for JS/TS shops already in the JetBrains ecosystem
- not the right primary stack-level answer for taurhaus yet

### CodeRabbit

CodeRabbit is best understood as an **AI reviewer**, not an objective code quality measurement system.

Use it for:

- PR review acceleration
- local review in IDE/CLI before opening PRs
- catching cross-file issues an LLM reviewer can infer from context

Do not use it as the only quality signal because:

- it does not produce the same kind of repeatable structural metrics as code-health or static-analysis tools
- it is reviewer augmentation, not a maintainability dashboard

## Lower-fit options

### Codacy

Codacy is more cloud-first than taurhaus wants, and its private-repo story is tied to hosted Git providers. It does have a self-hosted product, but the official system requirements show a fairly heavy Kubernetes/MicroK8s + PostgreSQL deployment footprint.

Recommendation:
- not the best first trial for this repo

### Qlty

Qlty is modern and GitHub-centric, with CLI, hooks, ratings, hotspots, churn, and AI integration. It looks promising if taurhaus wants a GitHub-native cloud quality layer.

Caveat:
- I did not find enough first-party evidence of strong Rust + Svelte-specific depth to rank it above CodeScene, DeepSource, or GitHub Advanced Security for this repo

Recommendation:
- interesting, but second-tier for taurhaus today

## Suggested taurhaus quality dashboard

### Option A: best local-first stack

Use this if taurhaus wants the highest signal with minimal code exfiltration and maximum private-repo control.

- `rust-code-analysis`
- `Semgrep CE`
- `cargo-deny`
- `cargo-audit`
- `cargo-geiger`
- `cargo-udeps` or `cargo-machete`
- `cargo tree -d`
- `Knip`
- `dependency-cruiser`
- `OSV-Scanner` or `Trivy`

This is my recommended default.

### Option B: local-first baseline plus one paid dashboard

Use this if taurhaus wants a stronger third-party view without going all-in on cloud scanning.

- local-first baseline from Option A
- plus **CodeScene** for code-health trends and hotspot prioritization

This is the best overall quality/governance setup for this repo.

### Option C: security-first hosted posture

Use this if the real concern is vulnerability/security regression rather than maintainability.

- local-first baseline minus `rust-code-analysis` if needed for cost/time
- plus **Snyk** or **Semgrep Platform**
- optionally **GitHub Advanced Security** if the team already pays for it

### Option D: GitHub-centric pragmatic stack

Use this only if taurhaus wants to consolidate into the GitHub workflow.

- `cargo-deny`
- `cargo-audit`
- `Knip`
- `dependency-cruiser`
- **GitHub Advanced Security / CodeQL**

This is practical, but weaker on Svelte-specific reality and weaker on maintainability metrics than Option B.

## Recommended adoption order

### Phase 1: highest signal, lowest friction

1. `cargo-deny`
2. `cargo-audit`
3. `cargo-geiger`
4. `Knip`
5. `dependency-cruiser`
6. `OSV-Scanner` or `Trivy`

### Phase 2: deeper structural insight

7. `rust-code-analysis`
8. `Semgrep CE`
9. `cargo-udeps` or `cargo-machete`
10. `cargo tree -d`
11. `cargo-bloat`

### Phase 3: paid trial if desired

12. `CodeScene` first
13. `Snyk` or `Semgrep Platform` if security governance becomes the main need
14. `GitHub Advanced Security` only if taurhaus wants a GitHub-native overlay and already has budget for private-repo security licensing

## Final recommendation

If the goal is an honest, independent assessment of taurhaus code quality, I would **not** start by buying a generic dashboard.

I would do this instead:

1. adopt the local-first baseline stack
2. review the first two weeks of results to identify which signal is actually missing
3. if a paid third-party opinion is still wanted, pilot **CodeScene** first
4. separately evaluate **Snyk** or **Semgrep Platform** only if the real problem is security posture rather than maintainability

That path gives taurhaus the best combination of:

- private-repo practicality
- real signal on both Rust and frontend code
- low vendor lock-in
- honest coverage of the Svelte-specific gap that most dashboard products still have

## Sources

Primary sources used for this assessment:

- rust-code-analysis: https://github.com/mozilla/rust-code-analysis
- Semgrep pricing and deployment notes: https://semgrep.dev/pricing/
- Semgrep supported languages: https://semgrep.dev/docs/supported-languages
- RustSec: https://rustsec.org/
- cargo-deny docs: https://embarkstudios.github.io/cargo-deny/
- cargo-geiger: https://github.com/geiger-rs/cargo-geiger
- Knip overview: https://knip.dev/
- Knip Svelte plugin: https://knip.dev/reference/plugins/svelte
- Knip compiler/file support: https://knip.dev/features/compilers
- dependency-cruiser repository and Svelte support: https://github.com/sverweij/dependency-cruiser
- OSV-Scanner: https://google.github.io/osv-scanner/
- Trivy filesystem scan: https://trivy.dev/docs/latest/target/filesystem/
- Cargo tree: https://doc.rust-lang.org/cargo/commands/cargo-tree.html
- cargo-depgraph: https://github.com/jplatte/cargo-depgraph
- cargo-bloat: https://github.com/RazrFalcon/cargo-bloat
- cargo-outdated: https://github.com/kbknapp/cargo-outdated
- cargo-vet: https://github.com/mozilla/cargo-vet
- SonarQube Community Build: https://www.sonarsource.com/open-source-editions/sonarqube-community-edition/
- Sonar supported languages: https://docs.sonarsource.com/sonarqube-community-build/analyzing-source-code/languages
- Sonar Rust announcement: https://www.sonarsource.com/blog/introducing-rust-in-sonarqube/
- Sonar community note on Svelte/SvelteKit support: https://community.sonarsource.com/t/sveltekit-support/48562/
- GitHub CodeQL: https://docs.github.com/en/enterprise-cloud@latest/code-security/concepts/code-scanning/codeql/about-code-scanning-with-codeql
- GitHub Advanced Security billing: https://docs.github.com/en/billing/concepts/product-billing/github-advanced-security
- Snyk supported languages: https://docs.snyk.io/supported-languages/supported-languages-package-managers-and-frameworks
- Snyk pricing: https://snyk.io/plans/
- Qodana overview: https://www.jetbrains.com/help/qodana/about-qodana.html
- Qodana linters: https://www.jetbrains.com/help/qodana/linters.html
- Qodana JS linter: https://www.jetbrains.com/help/qodana/js.html
- Qodana roadmap: https://www.jetbrains.com/help/qodana/roadmap.html
- CodeRabbit docs: https://docs.coderabbit.ai/
- CodeScene pricing and feature overview: https://codescene.com/pricing
- DeepSource analyzers: https://docs.deepsource.com/docs/platform/reference/languages
- DeepSource billing and self-hosted enterprise note: https://docs.deepsource.com/docs/platform/dashboard/team/team-settings
- Codacy pricing: https://www.codacy.com/pricing
- Codacy self-hosted system requirements: https://docs.codacy.com/chart/requirements/
- Qlty plans and features: https://docs.qlty.sh/cloud/billing/plans
