use super::*;
use crate::templates::agent_definitions::{export_agent_definitions, GENERATED_MARKER};

#[test]
fn list_roles_merges_sources_and_marks_read_only() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    write(
        &app_data.join("templates").join("roles").join("dev.yaml"),
        &agent_role_yaml("dev", "user override"),
    );

    let roles = store.list_roles().expect("list roles");
    let lead = roles
        .iter()
        .find(|role| role.template.role_id == "lead")
        .expect("lead role");
    assert_eq!(lead.source, TemplateSource::BuiltIn);
    assert!(lead.read_only);

    let dev = roles
        .iter()
        .find(|role| role.template.role_id == "dev")
        .expect("dev role");
    assert_eq!(dev.source, TemplateSource::User);
    assert!(!dev.read_only);
    assert_eq!(dev.template.instructions, "user override");
}

#[test]
fn get_role_prefers_user_override() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    write(
        &app_data.join("templates").join("roles").join("dev.yaml"),
        &agent_role_yaml("dev", "user override"),
    );

    let role = store.get_role("dev").expect("get role");
    assert_eq!(role.source, TemplateSource::User);
    assert_eq!(role.template.instructions, "user override");
}

#[test]
fn retired_tool_role_does_not_abort_the_catalog() {
    // Regression: commit 4cd067a removed the persisted third-harness wire value,
    // so one pre-18a role made the entire role catalog fail deserialization.
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");
    write(
        &app_data.join("templates/roles/gemini-ui-specialist.yaml"),
        &agent_role_yaml("gemini-ui-specialist", "legacy role")
            .replace("cli_tool: codex", "cli_tool: gemini"),
    );
    write(
        &app_data.join("templates/presets/legacy-google-team.yaml"),
        &preset_yaml_with_agent("legacy-google-team", "gemini-ui-specialist"),
    );

    let roles = store
        .list_roles()
        .expect("legacy role must not abort catalog");
    let retired = roles
        .iter()
        .find(|role| role.template.role_id == "gemini-ui-specialist")
        .expect("retired role remains visible for explicit migration");
    assert_eq!(retired.template.defaults.cli_tool.to_string(), "unknown");
    assert!(roles.iter().any(|role| role.template.role_id == "lead"));
    assert!(roles.iter().any(|role| role.template.role_id == "dev"));
    assert!(store
        .list_presets()
        .expect("legacy role reference must not abort presets")
        .iter()
        .any(|preset| preset.template.preset_id == "legacy-google-team"));
}

// Regression: ff40911 stripped legacy effort suffixes only at launch time,
// leaving template storage unable to preserve the requested effort separately.
#[test]
fn role_loaders_split_legacy_model_and_effort_for_builtins_and_user_files() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    write(
        &app_data.join("templates/roles/dev-dash.yaml"),
        &agent_role_yaml("dev-dash", "user dash").replace("gpt-5.4 high", "gpt-5.4-high"),
    );
    write(
        &app_data.join("templates/roles/dev-bare.yaml"),
        &agent_role_yaml("dev-bare", "user bare").replace("gpt-5.4 high", "gpt-5.4"),
    );

    let roles = store.list_roles().expect("list roles");
    for role_id in ["dev", "dev-dash"] {
        let defaults = &roles
            .iter()
            .find(|role| role.template.role_id == role_id)
            .expect("role")
            .template
            .defaults;
        assert_eq!(defaults.model, "gpt-5.4");
        assert_eq!(defaults.reasoning_effort.as_deref(), Some("high"));
    }

    let bare = &roles
        .iter()
        .find(|role| role.template.role_id == "dev-bare")
        .expect("bare role")
        .template
        .defaults;
    assert_eq!(bare.model, "gpt-5.4");
    assert_eq!(bare.reasoning_effort, None);
}

// Regression: ff40911 left the legacy model suffix in persisted role files,
// so an editor save could not preserve effort in its own schema field.
#[test]
fn save_after_legacy_load_writes_canonical_model_and_effort_keys() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let loaded = store.get_role("dev").expect("load legacy builtin");
    store
        .update_role("dev", &loaded.template)
        .expect("save canonical override");

    let raw =
        fs::read_to_string(app_data.join("templates/roles/dev.yaml")).expect("read saved role");
    assert!(raw.contains("model: gpt-5.4"));
    assert!(raw.contains("reasoning_effort: high"));
    assert!(!raw.contains("gpt-5.4 high"));
    assert!(!raw.contains("gpt-5.4-high"));
}

// Regression: ff40911 split legacy model/effort values at runtime, but the
// bundled role catalog kept combined values and could silently lose effort.
#[test]
fn bundled_roles_use_canonical_model_and_reasoning_effort() {
    let roles_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("templates")
        .join("roles");
    let mut paths = fs::read_dir(&roles_dir)
        .expect("read bundled roles")
        .map(|entry| entry.expect("read role entry").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("yaml"))
        .collect::<Vec<_>>();
    paths.sort();

    assert_eq!(paths.len(), 16, "bundled role count changed");

    let mut high_effort_roles = Vec::new();
    for path in paths {
        let raw = fs::read_to_string(&path).expect("read bundled role");
        assert!(
            raw.lines()
                .any(|line| line.trim_start().starts_with("reasoning_effort:")),
            "{} must declare defaults.reasoning_effort explicitly",
            path.display()
        );
        let role: RoleTemplate = serde_norway::from_str(&raw).expect("parse bundled role");
        let parsed = crate::session_scanner::launch::ModelSpec::parse_legacy(&role.defaults.model);

        assert_eq!(
            parsed.model.as_deref(),
            Some(role.defaults.model.as_str()),
            "{} must use a bare model slug",
            path.display()
        );
        assert_eq!(
            parsed.reasoning_effort,
            None,
            "{} still embeds reasoning effort in defaults.model",
            path.display()
        );

        if role.defaults.reasoning_effort.as_deref() == Some("high") {
            high_effort_roles.push(role.role_id);
        }
    }

    assert_eq!(high_effort_roles.len(), 10);
    assert!(high_effort_roles
        .iter()
        .any(|role| role == "v3-architect-codex"));
    assert!(high_effort_roles
        .iter()
        .any(|role| role == "claude-researcher"));
    assert!(high_effort_roles
        .iter()
        .any(|role| role == "adversarial-reviewer-claude"));
}

#[test]
fn create_role_validates_writes_and_commits() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let template = parse_role(&agent_role_yaml("qa", "qa role"));
    let result = store.create_role(&template).expect("create role");

    assert!(!result.committed);
    assert!(result.commit_id.is_none());
    let flushed = store.flush_pending_commits().expect("flush pending");
    assert!(flushed.is_some(), "flush should create commit");
    assert!(app_data
        .join("templates")
        .join("roles")
        .join("qa.yaml")
        .exists());
}

#[test]
fn create_role_blocks_built_in_collision() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data, builtins);

    let template = parse_role(&lead_role_yaml("lead", "override"));
    let err = store.create_role(&template).expect_err("should fail");
    assert!(matches!(err, TemplateStoreError::ReadOnly(_)));
}

#[test]
fn update_role_creates_user_override_for_built_in() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let template = parse_role(&lead_role_yaml("lead", "lead override"));
    let result = store
        .update_role("lead", &template)
        .expect("update built-in via override");

    assert!(!result.committed);
    let flushed = store.flush_pending_commits().expect("flush pending");
    assert!(flushed.is_some(), "flush should create commit");
    let role = store.get_role("lead").expect("get role");
    assert_eq!(role.source, TemplateSource::User);
    assert_eq!(role.template.instructions, "lead override");
    assert!(app_data
        .join("templates")
        .join("roles")
        .join("lead.yaml")
        .exists());
}

#[test]
fn update_role_fails_when_missing() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data, builtins);

    let template = parse_role(&agent_role_yaml("does-not-exist", "new"));
    let err = store
        .update_role("does-not-exist", &template)
        .expect_err("update should fail");
    assert!(matches!(err, TemplateStoreError::NotFound(_)));
}

#[test]
fn delete_role_blocks_when_referenced_by_preset() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let qa = parse_role(&agent_role_yaml("qa", "qa role"));
    store.create_role(&qa).expect("create qa");
    write(
        &app_data.join("templates").join("presets").join("qa.yaml"),
        &preset_yaml_with_agent("qa-preset", "qa"),
    );

    let err = store.delete_role("qa").expect_err("delete should fail");
    assert!(matches!(err, TemplateStoreError::Conflict(_)));
}

#[test]
fn delete_role_removes_user_template_and_commits() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let qa = parse_role(&agent_role_yaml("qa", "qa role"));
    store.create_role(&qa).expect("create qa");

    let result = store.delete_role("qa").expect("delete qa");
    assert!(!result.committed);
    let flushed = store.flush_pending_commits().expect("flush pending");
    assert!(flushed.is_some(), "flush should create commit");
    assert!(!app_data
        .join("templates")
        .join("roles")
        .join("qa.yaml")
        .exists());
}

#[test]
fn import_role_validates_and_writes_to_user_directory() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let external = app_data.join("external-role.yaml");
    write(&external, &agent_role_yaml("researcher", "research role"));

    let result = store.import_role(&external).expect("import role");
    assert!(!result.committed);
    let flushed = store.flush_pending_commits().expect("flush pending");
    assert!(flushed.is_some(), "flush should create commit");

    let role = store.get_role("researcher").expect("get imported role");
    assert_eq!(role.source, TemplateSource::User);
    assert_eq!(role.template.instructions, "research role");
}

#[test]
fn import_markdown_role_persists_provenance_and_records_import_commit_message() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let external = app_data.join("imported-role.md");
    write(
        &external,
        r#"---
name: Imported Reviewer
model: claude-opus-4-6
tools:
  - read
  - bash
---
Review structural changes and summarize risks.
"#,
    );

    store.import_role(&external).expect("import markdown role");
    let flushed = store.flush_pending_commits().expect("flush pending");
    assert!(flushed.is_some(), "flush should create commit");

    let role = store
        .get_role("imported-reviewer")
        .expect("get imported role");
    let provenance = role
        .template
        .provenance
        .as_ref()
        .expect("provenance should persist");
    assert_eq!(
        provenance.source_path.as_deref(),
        Some(external.to_string_lossy().as_ref())
    );
    assert_eq!(
        provenance.source_format,
        crate::templates::adapters::RoleExportFormat::ClaudeAgent
    );
    assert_eq!(
        latest_commit_message(&app_data.join("templates")),
        "templates: import role imported-reviewer from claude_agent"
    );
}

#[test]
fn import_role_conflicts_when_role_id_already_exists() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);

    let external = app_data.join("duplicate.md");
    write(
        &external,
        r#"---
name: Dev
description: Imported duplicate
model: gpt-5
---
Imported duplicate instructions.
"#,
    );

    let err = store
        .import_role(&external)
        .expect_err("duplicate import should fail");
    assert!(matches!(err, TemplateStoreError::Conflict(_)));
}

#[test]
fn list_roles_picks_up_external_files_added_to_roles_directory() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    write(
        &app_data.join("templates").join("roles").join("ext.yaml"),
        &agent_role_yaml("ext", "external file"),
    );

    let roles = store.list_roles().expect("list roles");
    let ext = roles
        .iter()
        .find(|role| role.template.role_id == "ext")
        .expect("external role present");
    assert_eq!(ext.source, TemplateSource::User);
    assert_eq!(ext.template.instructions, "external file");
}

// Regression: 9a6b9596 consolidated the bundled catalog without reconciling
// copies seeded by 0.8.5, so retired roles and stale role/preset bodies won.
#[test]
fn previous_release_builtins_reconcile_before_catalog_reads_and_export() {
    let (root, app_data, _fixture_builtins) = setup_dirs();
    let builtins = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("templates");
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    write(
        &app_data.join("templates/roles/claude-reviewer.yaml"),
        PREVIOUS_CLAUDE_REVIEWER,
    );
    write(
        &app_data.join("templates/roles/quick-dev-codex.yaml"),
        PREVIOUS_QUICK_DEV_CODEX,
    );
    write(
        &app_data.join("templates/presets/pair.yaml"),
        PREVIOUS_PAIR_PRESET,
    );

    let roles = store.list_roles().expect("list reconciled roles");
    let actual = roles
        .iter()
        .map(|record| record.template.role_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let expected = [
        "adversarial-reviewer-claude",
        "antigravity-orchestrator",
        "claude-design-lead",
        "claude-product-checker",
        "claude-researcher",
        "codex-orchestrator",
        "codex-qa",
        "docs-verifier-codex",
        "frontend-design-skill-developer",
        "quick-dev-codex",
        "v3-architect-codex",
        "v3-lead-claude",
        "v4-developer-agy",
        "v4-developer-claude",
        "v4-developer-codex",
        "v4-developer-grok",
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual, expected);

    let quick_dev = roles
        .iter()
        .find(|record| record.template.role_id == "quick-dev-codex")
        .expect("quick dev");
    assert_eq!(quick_dev.template.version, "2.0.0");
    assert_eq!(quick_dev.template.defaults.model, "gpt-5.6-sol");
    assert!(
        quick_dev
            .template
            .behavioral_contract
            .communication
            .join("\n")
            .contains("RESULT <id>")
    );

    let pair = store.get_preset("pair").expect("reconciled pair preset");
    assert_eq!(pair.template.version, "4.0.0");

    let project = root.path().join("project");
    fs::create_dir_all(project.join(".claude/agents")).expect("create agents dir");
    write(
        &project.join(".claude/agents/claude-reviewer.md"),
        &format!("---\nname: claude-reviewer\n---\n\n{GENERATED_MARKER}\nlegacy\n"),
    );
    let templates = roles
        .iter()
        .map(|record| record.template.clone())
        .collect::<Vec<_>>();
    let exported = export_agent_definitions(&templates, &project).expect("export definitions");
    assert_eq!(exported.removed, vec!["claude-reviewer"]);

    let history = store
        .get_history(Some(10), None)
        .expect("reconcile history");
    assert!(
        history
            .commits
            .iter()
            .any(|commit| commit.message.contains("reconcile built-in catalog")),
        "the automatic reconcile should be visible in template history"
    );
}

#[test]
fn previous_release_builtin_with_user_edits_is_preserved() {
    let (_root, app_data, _fixture_builtins) = setup_dirs();
    let builtins = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("templates");
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    let customized =
        PREVIOUS_CLAUDE_REVIEWER.replace("Claude Reviewer", "Claude Reviewer — locally customized");
    let target = app_data.join("templates/roles/claude-reviewer.yaml");
    write(&target, &customized);

    let roles = store.list_roles().expect("list roles");
    let preserved = roles
        .iter()
        .find(|record| record.template.role_id == "claude-reviewer")
        .expect("customized retired role remains available");
    assert_eq!(
        preserved.template.name,
        "Claude Reviewer — locally customized"
    );
    assert_eq!(
        fs::read_to_string(target).expect("read custom role"),
        customized
    );
}

const PREVIOUS_CLAUDE_REVIEWER: &str = r#"schema:
  kind: role_template
  version: 1

role_id: claude-reviewer
name: Claude Reviewer
version: 1.0.0
kind: agent

defaults:
  cli_tool: claude
  model: claude-opus-4-6
  reasoning_effort: null
  default_name_pattern: reviewer-{n}

instructions: |
  Perform code reviews focused on correctness, regressions, security risk, and
  missing tests. Prioritize actionable findings with severity and concrete
  references over general commentary.

focus_area: "Risk-focused review and validation"
context_summary: "Carries recent change history, risk hotspots, and test coverage concerns so review quality improves as more diffs pass through the role."
behavior_summary: "Reports findings by severity, blocks closure when validation is incomplete, and avoids implementing fixes unless explicitly requested."

communication_style: "Severity-ordered and evidence-backed. Leads with concrete findings, file references, and impact instead of general commentary."

quality_gates:
  - "High-risk regressions, unsafe assumptions, and missing tests are called out."
  - "The review scope has enough evidence to assess correctness and risk."
  - "Findings are actionable, prioritized, and tied to the changed behavior."

definition_of_done:
  - "The review states whether blocking issues were found."
  - "Residual risk and open questions are documented when certainty is incomplete."
  - "The report is ready for the developer or lead to act on immediately."

phase_scope:
  - "review"
  - "handoff"

mode: review

required_artifacts:
  - "review findings"
  - "risk summary"
  - "follow-up questions"

handoff_expectations:
  - "Report exact blocking findings or the explicit no-findings verdict with file references and reviewed evidence."
  - "Name the validation that was inspected, any coverage gaps that remain, and what would raise confidence."
  - "Leave the next owner knowing whether to fix code, add tests, or rerun a targeted check."

behavioral_contract:
  communication:
    - Start by confirming review scope (diff/feature/PR) and risk focus.
    - Report findings ordered by severity with file references and clear impact.
    - State explicitly when no critical findings are present, list residual risks, and say what the next lane should verify if certainty is incomplete.
  execution:
    - Evaluate behavior changes, edge cases, and test adequacy.
    - Check for violations of project conventions and unsafe assumptions.
    - "Keep review output structured: findings, open questions, suggested fixes, and missing validation."
    - Avoid implementing fixes unless explicitly requested.
  escalation:
    - Escalate high-risk defects immediately with reproduction details.
    - Call out uncertain conclusions and request targeted validation where needed.
    - If required context, repro steps, or validation evidence are missing, block review closure and request the exact artifacts needed.

capabilities: []

constraints:
  min_instances: 0
  max_instances: 6
  requires_lead_tool: null
  allowed_project_binding: any
"#;

const PREVIOUS_QUICK_DEV_CODEX: &str = r#"schema:
  kind: role_template
  version: 1

role_id: quick-dev-codex
name: Quick Dev (Codex)
version: 1.0.0
kind: agent

defaults:
  cli_tool: codex
  model: gpt-5.4
  reasoning_effort: high
  default_name_pattern: quick-dev-{n}

instructions: |
  You are the low-ceremony implementation lane for small, clear tasks. Move
  quickly, stay concrete, and avoid unnecessary explanation, but do not cut the
  validation or review loop.

  This role is for bounded execution, not architectural invention. If the task
  is obvious and local, implement it directly. If the task expands into design,
  cross-system coordination, or unclear scope, escalate instead of pretending it
  is still a quick change.

  Keep the implementation tight. Make the smallest real change that satisfies
  the request, run the promised checks, and hand the result off in a way that is
  easy to review. Speed comes from low ceremony and clear scope, not from
  skipping discipline.

  A quick task is still not done until it is review-ready. Every response must
  end with a mandatory review block containing exactly three things:

  1. CHANGED: which files were modified and what the change does (one line each)
  2. VERIFIED: what checks were run and their results (tests, lint, type check)
  3. VERIFY: what the reviewer should check manually (specific behavior to test)

  Never skip this block. A response without it is incomplete regardless of
  whether the implementation is correct.

focus_area: "Low-ceremony implementation for small, well-bounded tasks"
context_summary: "Carries the narrow task scope, local file set, and immediate verification lane so small changes can move quickly without losing review discipline."
behavior_summary: "Implements fast when scope is clear, avoids broad explanation, and always leaves the result ready for explicit final review."
communication_style: "Minimal. Reports what changed and what to verify. No explanation unless asked."

behavioral_contract:
  communication:
    - "Keep updates short: what changed, current status, and what still needs verification."
    - "Do not add background explanation unless the reviewer asks for it or the risk demands it."
    - "Close with a review-ready summary that points to changed files, validation, and the next owner or reviewer."
  execution:
    - "Implement directly when the task is small, local, and clearly specified."
    - "Use the smallest real change that solves the request without broad cleanup."
    - "Run the named quick gate before claiming readiness; use `just check-quick` when the task touches shared Rust/frontend surfaces or when the lead names it as the evidence bar."
    - "Leave the work in a state that a final reviewer can evaluate immediately."
  escalation:
    - "Escalate when the task expands beyond a small bounded change."
    - "Escalate when the request implies architecture, coordination, or cross-system redesign."
    - "Escalate when the quick gate fails for reasons outside the owned change or when the evidence is not review-ready."
    - "Escalate when the quick path is blocked by unclear ownership or missing context."

quality_gates:
  - "just check-quick passes"
  - "Changed files committed"
definition_of_done:
  - "Implementation complete"
  - "Tests pass"
  - "Ready for review"
phase_scope:
  - "implementation"
mode: "implementation"

required_artifacts:
  - "short diff summary"
  - "quick validation result"
  - "review handoff note"

handoff_expectations:
  - "Leave the reviewer with changed files, the quick gate result, and any exact rerun command."
  - "Call out the one or two highest-risk spots to inspect instead of making the reviewer rediscover them."
  - "Name any residual caveat or unblocker repair before handing the task off."

capabilities: []

constraints:
  min_instances: 0
  max_instances: 8
  requires_lead_tool: null
  allowed_project_binding: any
"#;

const PREVIOUS_PAIR_PRESET: &str = r#"schema:
  kind: team_preset
  version: 1

preset_id: pair
name: Pair
description: "One lead and one quick-delivery developer for the smallest scoped build-and-review loop."
version: "3.0.0"
lead_role_id: v3-lead-claude

agent_slots:
  - role_id: quick-dev-codex
    count: 1
    project_binding: lead_project
    overrides:
      name_pattern: quick-dev

defaults:
  team_name_pattern: "{project}-team"
  tmux_layout: tiled
"#;
