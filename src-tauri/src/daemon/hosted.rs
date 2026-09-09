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
    if operation == "reconcile" {
        if params["abandonUnknown"] != true {
            return Err("Explicit abandon decision required".into());
        }
        return hosts.abandon_unknown(registry, team, member, generation);
    }
    if operation == "transcript"
        && record
            .app_server
            .as_ref()
            .is_some_and(|a| a.state == "stopped")
    {
        return Ok(
            serde_json::json!({"stopped":true, "outcomeUnknown":record.host_input_unknown,
            "attachmentGeneration":generation}),
        );
    }
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
        let hosts = HostedMembers::default();
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let mut params = json!({"team_name":"team","member_name":"seat"});
        let view = handle(&hosts, &registry, "transcript", &params).unwrap();
        assert_eq!(view["thread"]["id"], "owned-thread");
        params["text"] = json!("daemon operator marker");
        assert!(handle(&hosts, &registry, "input", &params).is_err());
        params["generation"] = view["attachmentGeneration"].clone();
        assert!(handle(&hosts, &registry, "input", &params).is_ok());
        assert!(handle(&hosts, &registry, "transcript", &params)
            .unwrap()
            .to_string()
            .contains("daemon operator marker"));
        hosts.stop(&registry, "team", "seat").unwrap();
    }
    #[test]
    fn hosted_ambiguous_input_has_explicit_no_replay_recovery() {
        // Regression: 7921d720 persisted hostInputUnknown with no operator exit.
        let tmp = tempfile::tempdir().unwrap();
        let registry = crate::coordination::hosted::tests::seat(tmp.path());
        let launch = crate::coordination::hosted_process::tests::fixture(tmp.path());
        let hosts = HostedMembers::default();
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let generation = MemberRuntimeStore::load(tmp.path(), "team", "seat")
            .unwrap()
            .attachment_generation;
        assert!(hosts
            .operation(
                &registry,
                "team",
                "seat",
                generation,
                "input",
                json!({"text":"disconnect"})
            )
            .is_err());
        hosts.stop(&registry, "team", "seat").unwrap();
        assert!(hosts.launch(&registry, "team", "seat", &launch).is_err());
        let generation = MemberRuntimeStore::load(tmp.path(), "team", "seat")
            .unwrap()
            .attachment_generation;
        // An explicit operator decision abandons this input; it never resubmits it.
        handle(&hosts, &registry, "reconcile",
            &json!({"team_name":"team", "member_name":"seat", "generation":generation, "abandonUnknown":true})).unwrap();
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let persisted = std::fs::read_to_string(tmp.path().join("thread.json")).unwrap();
        assert_eq!(persisted.matches("disconnect").count(), 1);
        hosts.stop(&registry, "team", "seat").unwrap();
    }
}
