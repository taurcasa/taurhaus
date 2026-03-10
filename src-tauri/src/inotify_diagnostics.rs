use serde_json::{Map, Value};

use crate::commands::logging::emit_global;

const WARNING_THRESHOLD_PCT: f64 = 75.0;
const ERROR_THRESHOLD_PCT: f64 = 90.0;
const _: () = assert!(WARNING_THRESHOLD_PCT < ERROR_THRESHOLD_PCT);

#[derive(Debug, Clone, PartialEq)]
struct InotifyTelemetrySnapshot {
    process_local_instances: u64,
    process_local_watch_descriptors: u64,
    daemon_listener_connections: Option<u64>,
    physical_watch_registrations: Option<u64>,
    logical_watch_subscriptions: Option<u64>,
    system_user_instances: Option<u64>,
    system_user_instance_limit: Option<u64>,
    system_user_instance_pct: Option<f64>,
}

pub(crate) fn emit_daemon_telemetry_with_counts(
    reason: &str,
    daemon_listener_connections: Option<u64>,
    physical_watch_registrations: Option<u64>,
    logical_watch_subscriptions: Option<u64>,
) {
    emit_process_telemetry(
        "daemon",
        "daemon",
        reason,
        daemon_listener_connections,
        physical_watch_registrations,
        logical_watch_subscriptions,
    );
}

pub(crate) fn emit_app_telemetry(reason: &str, logical_watch_subscriptions: usize) {
    emit_process_telemetry(
        "backend",
        "app",
        reason,
        None,
        None,
        Some(logical_watch_subscriptions as u64),
    );
}

fn emit_process_telemetry(
    component: &str,
    process_role: &str,
    reason: &str,
    daemon_listener_connections: Option<u64>,
    physical_watch_registrations: Option<u64>,
    logical_watch_subscriptions: Option<u64>,
) {
    let Some(snapshot) = collect_snapshot(
        daemon_listener_connections,
        physical_watch_registrations,
        logical_watch_subscriptions,
    ) else {
        return;
    };

    emit_global(
        "info",
        component,
        "inotify.telemetry",
        Some(format!("{process_role} inotify telemetry sample")),
        telemetry_fields(process_role, reason, &snapshot),
    );

    if let Some(instance_pct) = snapshot.system_user_instance_pct {
        let (level, event, severity) = if instance_pct >= ERROR_THRESHOLD_PCT {
            ("error", "inotify.capacity.error", "error")
        } else if instance_pct >= WARNING_THRESHOLD_PCT {
            ("warn", "inotify.capacity.warning", "warning")
        } else {
            return;
        };

        let mut fields = telemetry_fields(process_role, reason, &snapshot);
        fields.insert("severity".to_string(), Value::String(severity.to_string()));
        fields.insert(
            "warning_threshold_pct".to_string(),
            Value::from(WARNING_THRESHOLD_PCT),
        );
        fields.insert(
            "error_threshold_pct".to_string(),
            Value::from(ERROR_THRESHOLD_PCT),
        );
        emit_global(
            level,
            component,
            event,
            Some(format!(
                "{process_role} observed current-user inotify instance usage above {severity} threshold"
            )),
            fields,
        );
    }
}

fn collect_snapshot(
    daemon_listener_connections: Option<u64>,
    physical_watch_registrations: Option<u64>,
    logical_watch_subscriptions: Option<u64>,
) -> Option<InotifyTelemetrySnapshot> {
    let process_stats = crate::platform::process_inotify_stats(std::process::id())?;
    let user_stats = crate::platform::current_user_inotify_stats();
    Some(InotifyTelemetrySnapshot {
        process_local_instances: process_stats.instance_count,
        process_local_watch_descriptors: process_stats.watch_count,
        daemon_listener_connections,
        physical_watch_registrations,
        logical_watch_subscriptions,
        system_user_instances: user_stats.map(|stats| stats.instance_count),
        system_user_instance_limit: user_stats.and_then(|stats| stats.instance_limit),
        system_user_instance_pct: user_stats.and_then(|stats| stats.instance_pct),
    })
}

fn telemetry_fields(
    process_role: &str,
    reason: &str,
    snapshot: &InotifyTelemetrySnapshot,
) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert(
        "process_role".to_string(),
        Value::String(process_role.to_string()),
    );
    fields.insert("reason".to_string(), Value::String(reason.to_string()));
    fields.insert(
        "process_local_inotify_instances".to_string(),
        Value::from(snapshot.process_local_instances),
    );
    fields.insert(
        "process_local_inotify_watch_descriptors".to_string(),
        Value::from(snapshot.process_local_watch_descriptors),
    );
    if let Some(count) = snapshot.daemon_listener_connections {
        fields.insert(
            "daemon_listener_connections".to_string(),
            Value::from(count),
        );
    }
    if let Some(count) = snapshot.physical_watch_registrations {
        fields.insert(
            "physical_watch_registrations".to_string(),
            Value::from(count),
        );
    }
    if let Some(count) = snapshot.logical_watch_subscriptions {
        fields.insert(
            "logical_watch_subscriptions".to_string(),
            Value::from(count),
        );
    }
    if let Some(count) = snapshot.system_user_instances {
        fields.insert(
            "system_user_inotify_instances".to_string(),
            Value::from(count),
        );
    }
    if let Some(limit) = snapshot.system_user_instance_limit {
        fields.insert(
            "system_user_inotify_instance_limit".to_string(),
            Value::from(limit),
        );
    }
    if let Some(pct) = snapshot.system_user_instance_pct {
        fields.insert(
            "system_user_inotify_instance_pct".to_string(),
            Value::from(pct),
        );
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_fields_include_core_counts() {
        let snapshot = InotifyTelemetrySnapshot {
            process_local_instances: 66,
            process_local_watch_descriptors: 3284,
            daemon_listener_connections: Some(8),
            physical_watch_registrations: Some(17),
            logical_watch_subscriptions: Some(17),
            system_user_instances: Some(124),
            system_user_instance_limit: Some(512),
            system_user_instance_pct: Some(24.22),
        };

        let fields = telemetry_fields("daemon", "periodic", &snapshot);

        assert_eq!(fields["process_role"], "daemon");
        assert_eq!(fields["reason"], "periodic");
        assert_eq!(fields["process_local_inotify_instances"], 66);
        assert_eq!(fields["process_local_inotify_watch_descriptors"], 3284);
        assert_eq!(fields["daemon_listener_connections"], 8);
        assert_eq!(fields["physical_watch_registrations"], 17);
        assert_eq!(fields["logical_watch_subscriptions"], 17);
        assert_eq!(fields["system_user_inotify_instances"], 124);
        assert_eq!(fields["system_user_inotify_instance_limit"], 512);
        assert_eq!(fields["system_user_inotify_instance_pct"], 24.22);
    }

    #[test]
    fn warning_thresholds_stay_ordered() {
        assert_eq!(WARNING_THRESHOLD_PCT, 75.0);
        assert_eq!(ERROR_THRESHOLD_PCT, 90.0);
    }
}
