use super::*;

fn setup_config_json(effort_key: &str) -> serde_json::Value {
    serde_json::json!({
        "name": "dev-1",
        "cliTool": "codex",
        "model": "gpt-5.6-terra",
        effort_key: "xhigh",
        "projectId": "/projects/taurhaus",
    })
}

// Regression: PR 5a/5b split the reasoning effort out of the model string, but the
// frontend payload builders emit `reasoning_effort` next to `model` (the canonical
// request spelling) while the camelCase IPC contract emits `reasoningEffort`. Only
// one of them deserialized, so half the initialize payloads silently launched
// without the effort the user picked.
#[test]
fn agent_setup_config_accepts_both_reasoning_effort_spellings() {
    for key in ["reasoningEffort", "reasoning_effort"] {
        let config: AgentSetupConfig = serde_json::from_value(setup_config_json(key))
            .unwrap_or_else(|err| panic!("deserialize {key}: {err}"));
        assert_eq!(config.reasoning_effort.as_deref(), Some("xhigh"), "{key}");
        assert_eq!(config.model, "gpt-5.6-terra");
    }
}

#[test]
fn agent_setup_config_without_reasoning_effort_is_none() {
    let config: AgentSetupConfig = serde_json::from_value(serde_json::json!({
        "name": "dev-1",
        "cliTool": "codex",
        "model": "gpt-5.6-terra",
        "projectId": "/projects/taurhaus",
    }))
    .expect("deserialize without effort");
    assert!(config.reasoning_effort.is_none());
}
