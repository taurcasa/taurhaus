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
    let attachment = record.app_server.as_ref().ok_or("NOT_HOSTED")?;
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
        && (attachment.state == "stopped" || attachment.state == "orphaned")
    {
        return Ok(serde_json::json!({"stopped":attachment.state == "stopped",
            "orphanProcessId":(attachment.state == "orphaned").then_some(attachment.process_id),
            "outcomeUnknown":record.host_input_unknown,
            "attachmentGeneration":generation}));
    }
    let params = params.clone();
    let mut result = hosts.operation(registry, team, member, generation, operation, params)?;
    result["attachmentGeneration"] = generation.into();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordination::hosted::tests::{input, running, saved, seat};
    use crate::coordination::hosted_process::tests::fixture;
    use serde_json::json;
    #[test]
    fn daemon_host_input_and_transcript_use_owned_member_and_generation() {
        let tmp = tempfile::tempdir().unwrap();
        let (registry, hosts) = running(tmp.path());
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
        // Regression: a9c8109b classified an unavailable host's pre-send failure as ambiguous.
        let error = handle(&hosts, &registry, "input", &params).unwrap_err();
        assert!(error.starts_with("failed:"), "{error}");
        assert_eq!(
            handle(&hosts, &registry, "transcript", &params).unwrap()["stopped"],
            true
        );
        assert!(!saved(tmp.path()).host_input_unknown);
    }
    #[test]
    fn owned_member_refuses_pane_conversion_and_does_not_adopt_after_owner_restart() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = seat(tmp.path());
        let launch = fixture(tmp.path());
        let hosts = HostedMembers::default();
        MemberRuntimeStore::update(tmp.path(), "team", "seat", |r| {
            r.pane_id = Some("%42".into())
        })
        .unwrap();
        assert!(hosts.launch(&registry, "team", "seat", &launch).is_err());
        MemberRuntimeStore::update(tmp.path(), "team", "seat", |r| r.pane_id = None).unwrap();
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let record = saved(tmp.path());
        let generation = record.attachment_generation;
        let restarted = HostedMembers::default();
        // Regression: 83077dad reported a live unowned child as stopped on owner restart.
        assert!(restarted.stop(&registry, "team", "seat").is_err());
        assert!(restarted
            .launch(&registry, "team", "seat", &launch)
            .is_err());
        assert!(input(&restarted, &registry, generation, "must not adopt").is_err());
        // Regression: a9c8109b hid a surviving prior owner's child behind unavailable.
        restarted.reconcile(&registry, "team", "seat").unwrap();
        let orphan = saved(tmp.path());
        assert_eq!(orphan.app_server.unwrap().state, "orphaned");
        assert_eq!(orphan.attachment_generation, generation + 1);
        let params = json!({"team_name":"team", "member_name":"seat"});
        let view = handle(&restarted, &registry, "transcript", &params).unwrap();
        assert_eq!(
            view["orphanProcessId"],
            record.app_server.unwrap().process_id
        );
        restarted.reconcile(&registry, "team", "seat").unwrap();
        assert_eq!(
            saved(tmp.path()).attachment_generation,
            orphan.attachment_generation
        );
        drop(hosts); // Only the original test owner kills and reaps its fake child.
        restarted.reconcile(&registry, "team", "seat").unwrap();
        assert_eq!(saved(tmp.path()).app_server.unwrap().state, "unavailable");
    }
    #[test]
    fn hosted_ambiguous_input_has_explicit_no_replay_recovery() {
        // Regression: a9c8109b persisted hostInputUnknown with no operator exit.
        let tmp = tempfile::tempdir().unwrap();
        let (registry, hosts) = running(tmp.path());
        let generation = saved(tmp.path()).attachment_generation;
        assert!(input(&hosts, &registry, generation, "disconnect").is_err());
        hosts.stop(&registry, "team", "seat").unwrap();
        let launch = fixture(tmp.path());
        assert!(hosts.launch(&registry, "team", "seat", &launch).is_err());
        let generation = saved(tmp.path()).attachment_generation;
        // An explicit operator decision abandons this input; it never resubmits it.
        handle(&hosts, &registry, "reconcile",
            &json!({"team_name":"team", "member_name":"seat", "generation":generation, "abandonUnknown":true})).unwrap();
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let persisted = std::fs::read_to_string(tmp.path().join("thread.json")).unwrap();
        assert_eq!(persisted.matches("disconnect").count(), 1);
        hosts.stop(&registry, "team", "seat").unwrap();
    }
}
