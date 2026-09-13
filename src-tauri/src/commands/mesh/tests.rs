use super::*;

// Regression: 9d09c883 used a frontend hash, so later Mesh releases lost support.
#[test]
fn canonical_capability_uses_installed_version_then_bundled_fallback() {
    let contract = |version: &str| MeshCompatibilityContract {
        version: version.into(),
        protocol_version: 1,
        schema_version: 1,
        git_commit: None,
    };
    for (version, supported) in [
        ("0.2.29", false),
        ("0.3.0", true),
        ("0.3.1", true),
        ("1.0.0", true),
        ("0.10.0", true),
        ("unparsable", false),
        ("0.3", false),
        ("0.3.0.1", false),
        ("0.3.0-preview", false),
    ] {
        for bundled in ["0.2.29", "1.0.0"] {
            let status = mesh_status_from_contract(
                &contract(bundled),
                Some(contract(version)),
                vec![],
                true,
                None,
            );
            let wire = serde_json::to_value(status).unwrap();
            assert_eq!(
                wire["canonicalMessagingSupported"], supported,
                "{version} with {bundled}"
            );
        }
        for status in [
            mesh_status_from_contract(&contract(version), None, vec![], true, None),
            mesh_status_not_installed(&contract(version), true, None),
            mesh_status_unrunnable(&contract(version), "fixture-mesh", "unreadable".into()),
        ] {
            let mut wire = serde_json::to_value(status).unwrap();
            assert_eq!(
                wire["canonicalMessagingSupported"], supported,
                "fallback {version}"
            );
            wire.as_object_mut()
                .unwrap()
                .remove("canonicalMessagingSupported");
            let old_status: MeshInstallStatus = serde_json::from_value(wire).unwrap();
            assert_eq!(
                serde_json::to_value(old_status).unwrap()["canonicalMessagingSupported"],
                false
            );
        }
    }
}

fn hosted_descriptor() -> serde_json::Value {
    use crate::session_scanner::launch::HostedDescriptor;
    let paired = HostedDescriptor::codex();
    serde_json::json!({"native_descriptors": [{
        "adapter": "app_server", "harness": "codex", "build": paired.build,
        "enabled": true, "host": HostedDescriptor::HOST,
        "configuration": HostedDescriptor::CONFIGURATION, "trust": HostedDescriptor::TRUST,
        "transport": paired.transport
    }]})
}

// Regression: 6398bfa3 exposed hosting but supplied no descriptor admission fact.
#[test]
fn hosted_capability_requires_mesh_floor_and_enabled_matching_codex_descriptor() {
    let descriptor = hosted_descriptor();
    // Regression: dogfood finding 6 (2026-09-13) — Codex 0.154.0 fell back to typed-pane
    // delivery because the verified build was compared with `==`; it is a floor.
    for mesh in ["0.2.29", "0.3.0", "unknown"] {
        for codex in [
            Some("0.153.4"),
            Some("0.153.5"),
            Some("0.154.0"),
            Some("0.153.3"),
            Some("0.154.0-alpha.1"),
            Some("wrong-build"),
            None,
        ] {
            assert_eq!(
                hosted_delivery_supported(mesh, codex, &descriptor),
                mesh == "0.3.0"
                    && matches!(codex, Some("0.153.4") | Some("0.153.5") | Some("0.154.0")),
                "mesh {mesh} codex {codex:?}"
            );
        }
    }
    let mut older_minimum = descriptor.clone();
    older_minimum["native_descriptors"][0]["build"] = serde_json::json!("0.150.0");
    assert!(hosted_delivery_supported(
        "0.3.0",
        Some("0.153.4"),
        &older_minimum
    ));
    let mut newer_minimum = descriptor.clone();
    newer_minimum["native_descriptors"][0]["build"] = serde_json::json!("0.160.0");
    assert!(!hosted_delivery_supported(
        "0.3.0",
        Some("0.154.0"),
        &newer_minimum
    ));
    for field in [
        "enabled",
        "host",
        "configuration",
        "trust",
        "transport",
        "build",
    ] {
        let mut refused = descriptor.clone();
        refused["native_descriptors"][0][field] = serde_json::Value::Null;
        assert!(!hosted_delivery_supported(
            "0.3.0",
            Some("0.153.4"),
            &refused
        ));
    }
    assert!(!hosted_delivery_supported(
        "0.3.0",
        Some("0.153.4"),
        &serde_json::Value::Null
    ));
}

// Regression: 6398bfa3 had no shared hosted fact across installed/bundled status paths.
#[test]
fn hosted_status_uses_same_contract_fallback_and_camel_case_wire() {
    let contract = MeshCompatibilityContract {
        version: "0.3.0".into(),
        protocol_version: 1,
        schema_version: 1,
        git_commit: None,
    };
    for status in [
        mesh_status_from_contract(&contract, Some(contract.clone()), vec![], true, None),
        mesh_status_not_installed(&contract, true, None),
        mesh_status_unrunnable(&contract, "fixture", "unreadable".into()),
    ] {
        let supported = with_hosted_delivery(status.clone(), Some("0.153.4"), &hosted_descriptor());
        assert!(supported.hosted_delivery_supported);
        let wire =
            serde_json::to_value(with_hosted_delivery(status, None, &serde_json::Value::Null))
                .unwrap();
        assert_eq!(wire["hostedDeliverySupported"], false);
        let mut old_wire = wire;
        old_wire
            .as_object_mut()
            .unwrap()
            .remove("hostedDeliverySupported");
        assert!(
            !serde_json::from_value::<MeshInstallStatus>(old_wire)
                .unwrap()
                .hosted_delivery_supported
        );
    }
}

// Regression: 2fcf7d65 trusted the probed build even when the daemon handshake rejects it.
// The daemon handshake (hosted_process.rs) admits any build at or above taurhaus's own
// verified minimum, so a Mesh descriptor and CLI that agree on a build BELOW that minimum
// must still be refused here; agreeing on a newer build is admitted (finding 6, 2026-09-13).
#[test]
fn hosted_capability_rejects_cli_and_mesh_matching_a_build_below_the_verified_minimum() {
    let mut capabilities = hosted_descriptor();
    capabilities["native_descriptors"][0]["build"] = serde_json::json!("0.150.0");
    assert!(!hosted_delivery_supported(
        "0.3.0",
        Some("0.150.0"),
        &capabilities
    ));
    capabilities["native_descriptors"][0]["build"] = serde_json::json!("0.153.5");
    assert!(hosted_delivery_supported(
        "0.3.0",
        Some("0.153.5"),
        &capabilities
    ));
}

#[test]
fn hosted_status_explains_build_mismatch_and_other_admission_failures() {
    // Regression: c36b1900 reported Codex mismatch before the prerequisite Mesh version.
    let mut contract = MeshCompatibilityContract {
        version: "0.3.0".into(),
        protocol_version: 1,
        schema_version: 1,
        git_commit: None,
    };
    for (version, codex, caps, expected) in [
        (
            "0.3.0",
            Some("0.153.3"),
            hosted_descriptor(),
            Some("installed Codex 0.153.3 is older than the verified minimum 0.153.4"),
        ),
        (
            "0.3.0",
            None,
            hosted_descriptor(),
            Some("installed Codex version is unavailable; verified minimum is 0.153.4"),
        ),
        (
            "0.3.0",
            Some("0.153.4"),
            serde_json::Value::Null,
            Some("Mesh has no enabled descriptor admitting the installed Codex build"),
        ),
        ("0.3.0", Some("0.153.4"), hosted_descriptor(), None),
        ("0.3.0", Some("0.154.0"), hosted_descriptor(), None),
        (
            "0.2.29",
            Some("0.154.0"),
            hosted_descriptor(),
            Some("Native delivery requires Mesh 0.3.0 or newer"),
        ),
    ] {
        contract.version = version.into();
        let status =
            mesh_status_from_contract(&contract, Some(contract.clone()), vec![], true, None);
        let wire = serde_json::to_value(with_hosted_delivery(status, codex, &caps)).unwrap();
        assert_eq!(wire["hostedDeliveryReason"].as_str(), expected);
    }
}
