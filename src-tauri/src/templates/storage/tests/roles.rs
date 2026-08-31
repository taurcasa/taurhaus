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
    Repository::init(store.templates_dir()).expect("initialize previous store repository");
    store
        .recover_dirty_tree()
        .expect("commit previous store baseline")
        .expect("baseline commit");

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
    assert!(quick_dev
        .template
        .behavioral_contract
        .communication
        .join("\n")
        .contains("RESULT <id>"));

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

// Regression: 27c3e32e deleted retired built-in roles without considering
// user-authored presets that still referenced them, bricking preset reads.
#[test]
fn reconciliation_preserves_retired_role_referenced_by_user_preset() {
    let (_root, app_data, _fixture_builtins) = setup_dirs();
    let builtins = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("templates");
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    let retired_role = app_data.join("templates/roles/claude-reviewer.yaml");
    write(&retired_role, PREVIOUS_CLAUDE_REVIEWER);
    write(
        &app_data.join("templates/presets/my-team.yaml"),
        &preset_yaml_with_agent("my-team", "claude-reviewer")
            .replace("lead_role_id: lead", "lead_role_id: v3-lead-claude"),
    );

    let presets = store
        .list_presets()
        .expect("a referenced retired role should keep the preset catalog readable");
    assert!(presets
        .iter()
        .any(|preset| preset.template.preset_id == "my-team"));
    assert!(retired_role.exists(), "the referenced role was deleted");
    store
        .load_catalog()
        .expect("the merged catalog should remain readable");
}

// Regression: 27c3e32e recognized only 0.8.4+ bytes, so presets copied by
// 0.8.3 stayed stale and prevented the canonical catalog from loading.
#[test]
fn v0_8_3_seeded_presets_reconcile_to_the_canonical_catalog() {
    let (_root, app_data, _fixture_builtins) = setup_dirs();
    let builtins = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("templates");
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    for (name, contents) in [
        ("dev-team.yaml", V0_8_3_DEV_TEAM_PRESET),
        ("full-team.yaml", V0_8_3_FULL_TEAM_PRESET),
        ("grok-pair.yaml", V0_8_3_GROK_PAIR_PRESET),
        ("pair.yaml", PREVIOUS_PAIR_PRESET),
        ("research-team.yaml", V0_8_3_RESEARCH_TEAM_PRESET),
    ] {
        write(&app_data.join("templates/presets").join(name), contents);
    }

    let catalog = store
        .load_catalog()
        .expect("0.8.3 preset copies should reconcile");
    let role_ids = catalog
        .roles
        .iter()
        .map(|role| role.role_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let preset_ids = catalog
        .presets
        .iter()
        .map(|preset| preset.preset_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        role_ids,
        [
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
        .collect()
    );
    assert_eq!(
        preset_ids,
        [
            "dev-team",
            "full-team",
            "grok-pair",
            "pair",
            "research-team",
        ]
        .into_iter()
        .collect()
    );
    for name in [
        "dev-team.yaml",
        "full-team.yaml",
        "grok-pair.yaml",
        "pair.yaml",
        "research-team.yaml",
    ] {
        assert!(
            !app_data.join("templates/presets").join(name).exists(),
            "redundant shipped copy {name} should be removed"
        );
    }
}

#[test]
fn known_shipped_hashes_cover_pre_0_8_4_catalog_deltas() {
    for expected in [
        (
            "presets/dev-team.yaml",
            "0b21738499d30483be03427845cca63da1bd399caf4431d634790876749f28ed",
        ),
        (
            "presets/full-team.yaml",
            "d39d3082c563769a249246b769dd2f46c612eaea0f7877c1d122aafba67c44a0",
        ),
        (
            "presets/grok-pair.yaml",
            "869793213f7aeb8c204719ff1dc726c9b0211f1b0908b3d8c907996685c14c72",
        ),
        (
            "presets/research-team.yaml",
            "06be090b1c326440526554415f9967c2adf0436c8fc578b0a364fb0157c82021",
        ),
        (
            "roles/v3-lead-claude.yaml",
            "48ad6b77969c9e37deaa5fc466c4e644315d71a1f1c8d46536deb46193f5c014",
        ),
        (
            "roles/v3-lead-codex.yaml",
            "db8a1f434df71e4fcf145fbebd1aaf5722dbe5a01485829ecba14053fc0390ac",
        ),
    ] {
        assert!(
            PREVIOUS_BUNDLED_TEMPLATE_HASHES.contains(&expected),
            "missing known shipped fingerprint for {}",
            expected.0
        );
    }
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

const V0_8_3_DEV_TEAM_PRESET: &str = r#"schema:
  kind: team_preset
  version: 1

preset_id: dev-team
name: Dev Team
description: "One lead and two vertical-slice developers for parallel product-visible implementation with shared review gates."
version: "3.0.0"
lead_role_id: v3-lead-claude

agent_slots:
  - role_id: v3-developer-codex
    count: 2
    project_binding: lead_project

defaults:
  team_name_pattern: "{project}-team"
  tmux_layout: tiled
"#;

const V0_8_3_FULL_TEAM_PRESET: &str = r#"schema:
  kind: team_preset
  version: 1

preset_id: full-team
name: Full Team
description: "One lead, one architect, and two developers for structural guidance, implementation throughput, and stronger readiness checks."
version: "3.0.0"
lead_role_id: v3-lead-claude

agent_slots:
  - role_id: v3-architect-codex
    count: 1
    project_binding: lead_project
    overrides:
      name_pattern: architect
  - role_id: v3-developer-codex
    count: 2
    project_binding: lead_project

defaults:
  team_name_pattern: "{project}-team"
  tmux_layout: tiled
"#;

const V0_8_3_GROK_PAIR_PRESET: &str = r#"schema:
  kind: team_preset
  version: 1

preset_id: grok-pair
name: Grok Pair
description: "One lead and one Grok developer for the smallest scoped build-and-review loop on the xAI harness."
version: "1.0.0"
lead_role_id: v3-lead-claude

agent_slots:
  - role_id: grok-developer
    count: 1
    project_binding: lead_project
    overrides:
      name_pattern: grok-dev

defaults:
  team_name_pattern: "{project}-team"
  tmux_layout: tiled
"#;

const V0_8_3_RESEARCH_TEAM_PRESET: &str = r#"schema:
  kind: team_preset
  version: 1

preset_id: research-team
name: Research Team
description: "One lead, one researcher, and one developer for evidence gathering paired with implementation and decision-ready handoff."
version: "3.0.0"
lead_role_id: v3-lead-claude

agent_slots:
  - role_id: claude-researcher
    count: 1
    project_binding: lead_project
    overrides:
      name_pattern: researcher
  - role_id: v3-developer-codex
    count: 1
    project_binding: lead_project

defaults:
  team_name_pattern: "{project}-team"
  tmux_layout: tiled
"#;

// Regression: b0830f10's reference-gathering parsed every YAML in the user
// presets directory inline and propagated Parse errors from the first read,
// so one stray non-preset file took down every role read.
#[test]
fn a_stray_yaml_in_presets_does_not_fail_role_reads() {
    let (_root, app_data, _fixture_builtins) = setup_dirs();
    let builtins = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("templates");
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    write(
        &app_data.join("templates/presets/notes.yaml"),
        "just: a note\n",
    );

    store
        .list_roles()
        .expect("a stray presets file must not take the role catalog down");
    store.load_catalog().expect("catalog stays readable");
}

// Regression: delete_preset resolved through get_preset, whose validation
// skips a user preset naming a retired role — leaving exactly the presets
// most in need of cleanup reported "not found" while the file stayed on
// disk.
#[test]
fn an_invalid_user_preset_remains_deletable() {
    let (_root, app_data, _fixture_builtins) = setup_dirs();
    let builtins = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("templates");
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    let err = store
        .delete_preset("pair")
        .expect_err("a built-in preset stays protected");
    assert!(matches!(err, TemplateStoreError::ReadOnly(_)));
    let err = store
        .delete_preset("no-such-preset")
        .expect_err("an unknown id stays not-found");
    assert!(matches!(err, TemplateStoreError::NotFound(_)));

    let path = app_data.join("templates/presets/my-team.yaml");
    write(&path, INVALID_MY_TEAM_PRESET);
    store
        .delete_preset("my-team")
        .expect("an invalid user preset must remain deletable");
    assert!(!path.exists(), "the invalid preset file must be gone");
}

// Regression: the fingerprint table started at 0.8.0, so a store seeded by
// v0.4.5–v0.7.0 kept retired roles (gemini-orchestrator and friends) as
// user-owned copies forever.
#[test]
fn a_v0_7_0_seeded_retired_role_reconciles_away() {
    let (_root, app_data, _fixture_builtins) = setup_dirs();
    let builtins = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("templates");
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    write(
        &app_data.join("templates/roles/gemini-orchestrator.yaml"),
        V0_7_0_GEMINI_ORCHESTRATOR_ROLE,
    );

    let catalog = store.load_catalog().expect("v0.7.0 copies reconcile");
    assert!(
        catalog
            .roles
            .iter()
            .all(|role| role.role_id != "gemini-orchestrator"),
        "a v0.7.0-seeded retired role must be removed by reconciliation"
    );
}

const INVALID_MY_TEAM_PRESET: &str = r#"schema:
  kind: team_preset
  version: 1

preset_id: my-team
name: My Team
description: "A user preset naming a role that no longer exists."
version: "1.0.0"
lead_role_id: long-gone-role

agent_slots:
  - role_id: also-long-gone
    count: 1
    project_binding: lead_project

defaults:
  team_name_pattern: "{project}-team"
  tmux_layout: tiled
"#;

// The exact bytes v0.7.0 shipped (`git show
// v0.7.0:src-tauri/resources/templates/roles/gemini-orchestrator.yaml`);
// the fingerprint table must recognize them for the reconcile to fire.
const V0_7_0_GEMINI_ORCHESTRATOR_ROLE: &str = r#"schema:
  kind: role_template
  version: 1

role_id: gemini-orchestrator
name: Gemini Orchestrator
version: 1.0.0
kind: lead

defaults:
  cli_tool: gemini
  model: gemini-3.1-pro
  reasoning_effort: null
  default_name_pattern: lead-{project}

instructions: |
  Coordinate the team end to end: convert user requests into concrete tasks,
  assign clear owners, track blockers, and synthesize outcomes into user-facing
  updates. Keep momentum high by unblocking agents quickly and deciding on
  tradeoffs when ambiguity appears.

  Stay available for communication at all times. Your primary mode is delegation, not implementation. When work needs doing, assign it to the right team member:
  - Implementation tasks -> developers
  - Structural/pattern questions -> architect
  - Frontend design decisions -> UI specialist
  - Direction/scope questions -> decide yourself or consult the user

  Never do implementation work yourself unless all team members are occupied and the task is urgent. Your context is too valuable to spend on code -> spend it on coordination.

  Operate through Gemini CLI conventions while preserving the same team-lead contract: clear routing, explicit ownership, and concise status synthesis.

focus_area: "Team orchestration, delegation, and unblock decisions"
context_summary: "Carries the live map of team assignments, blockers, priorities, and handoffs so the next routing decision stays coherent after compaction."
behavior_summary: "Delegates by default, keeps momentum high, and handles direction-level decisions while routing specialized work to the right agent."

communication_style: "Short, directive, and priority-aware. Assigns concrete next actions, keeps lanes informed, and avoids narrative status chatter."

quality_gates:
  - "Every active lane has a clear owner, next action, and completion signal."
  - "Blockers, dependencies, and handoffs are visible in the task system."
  - "Specialized work is routed to the right role instead of being handled opportunistically."

definition_of_done:
  - "Assignments and handoffs are routed with exact deliverables and first actions."
  - "The team has no silent stalls or ambiguous ownership gaps."
  - "Outstanding blockers or risks are surfaced to the lead or next owner."

phase_scope:
  - "planning"
  - "execution"
  - "handoff"

mode: coordination

required_artifacts:
  - "task assignments"
  - "progress or blocker updates"
  - "handoff notes"

handoff_expectations:
  - "State the current owner, task id, next action, and completion signal for every active handoff."
  - "Call out the exact evidence or unblock decision still required before closure."
  - "Leave downstream lanes knowing who needs to be nudged, reviewed, or unblocked next."

behavioral_contract:
  communication:
    - Acknowledge new requests quickly and classify them as action, response, or informational.
    - Send concise assignment messages with acceptance criteria and expected evidence.
    - Request status updates when work runs long or dependencies shift.
    - Close each handoff by naming the next owner or lane explicitly.
  execution:
    - Break work into scoped tasks and keep each task aligned to one clear deliverable.
    - "Verify completion evidence before marking tasks done: changed paths, commands run, and outcomes."
    - Enforce project conventions (AGENTS.md/CLAUDE.md) and quality gates before closure.
    - Commit at milestone boundaries with descriptive messages when appropriate.
  escalation:
    - Surface blockers immediately with dependency context and decision options.
    - If conflicting reports arrive, resolve by requesting concrete evidence and choosing a single path.
    - Do not let blocked tasks stall silently; re-route or de-scope quickly.
    - "Route structural questions from developers to the architect, not yourself"
    - "Only handle direction-level decisions: new features, scope, priorities"
    - "Escalate when completion evidence, ownership, or downstream review routing is too ambiguous to advance safely."

capabilities: []

constraints:
  min_instances: 1
  max_instances: 1
  requires_lead_tool: null
  allowed_project_binding: lead_project
"#;
