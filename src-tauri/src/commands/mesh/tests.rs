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
