//! Additive operator methods; older daemons answer UNKNOWN_METHOD.
use crate::coordination::hosted::HostedMembers;
use crate::coordination::stores::{MemberRuntimeStore, TeamRootRegistry};
use serde_json::Value;

pub(crate) fn handle(
    hosts: &HostedMembers,
    registry: &TeamRootRegistry,
    operation: &str,
    params: &Value,
) -> Result<Value, String> {
    let team = params["team_name"].as_str().ok_or("missing team_name")?;
    let member = params["member_name"]
        .as_str()
        .ok_or("missing member_name")?;
    let root = registry.resolve(team).map_err(|e| e.to_string())?;
    let record = MemberRuntimeStore::load(&root, team, member).map_err(|e| e.to_string())?;
    if record.app_server.is_none() {
        return Err("NOT_HOSTED".into());
    }
    let generation = if operation == "transcript" {
        record.attachment_generation
    } else {
        params["generation"]
            .as_u64()
            .ok_or("missing attachment generation")?
    };
    let mut result = hosts.operation(
        registry,
        team,
        member,
        generation,
        operation,
        params.clone(),
    )?;
    result["attachmentGeneration"] = generation.into();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn daemon_host_input_and_transcript_use_owned_member_and_generation() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = crate::coordination::hosted::tests::seat(tmp.path());
        let launch = crate::coordination::hosted_process::tests::fixture(tmp.path());
        let state = crate::coordination::state::CoordinationState::with_components_and_runtime(
            tmp.path().into(),
            crate::coordination::backend::BackendSelector::m0(),
            std::sync::Arc::new(|_, _| {
                Ok(std::sync::Arc::new(
                    crate::coordination::backend::fake::FakeBackend::default(),
                ))
            }),
            std::sync::Arc::new(|| {
                std::sync::Arc::new(
                    crate::coordination::runtime::RecordingCoordinationRuntime::default(),
                )
            }),
        );
        state
            .hosted
            .launch(&registry, "team", "seat", &launch)
            .unwrap();
        let mut params = json!({"team_name":"team","member_name":"seat"});
        let view = handle(&state.hosted, &registry, "transcript", &params).unwrap();
        assert_eq!(view["thread"]["id"], "owned-thread");
        params["text"] = json!("daemon operator marker");
        assert!(handle(&state.hosted, &registry, "input", &params).is_err());
        params["generation"] = view["attachmentGeneration"].clone();
        assert!(handle(&state.hosted, &registry, "input", &params).is_ok());
        assert!(handle(&state.hosted, &registry, "transcript", &params)
            .unwrap()
            .to_string()
            .contains("daemon operator marker"));
        state.hosted.stop(&registry, "team", "seat").unwrap();
    }
}
