use crate::errors::sanitize_error;
use crate::ProviderState;

pub(crate) fn fail_fast_if_daemon_lane_is_busy(
    providers: &ProviderState,
    project_path: &str,
    operation: &str,
) -> Result<(), String> {
    if crate::provider::path::is_wsl_path(project_path)
        && providers
            .daemon
            .as_ref()
            .is_some_and(|daemon| daemon.is_connected() && daemon.is_busy())
    {
        return Err(sanitize_error(&format!(
            "Daemon transport error: foreground {operation} skipped because the shared daemon connection is busy"
        )));
    }

    Ok(())
}
