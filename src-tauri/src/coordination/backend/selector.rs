//! Backend selector for coordination operations.

use super::BackendKind;
use crate::session_scanner::cli_tool::CliTool;

/// Resolves which coordination backend to use for a given CLI tool.
///
/// For M0: forced MeshBridged for all tools. The auto-detect path exists
/// for post-M0 when ClaudeNative becomes available.
#[derive(Debug, Clone, Copy, Default)]
pub struct BackendSelector {
    pub override_kind: Option<BackendKind>,
    pub force_mesh: bool,
}

impl BackendSelector {
    /// M0 selector: forces MeshBridged for all tools.
    pub fn m0() -> Self {
        Self {
            override_kind: None,
            force_mesh: true,
        }
    }

    /// Select backend using override → force_mesh → CLI-tool auto-detection.
    pub fn select(&self, cli_tool: CliTool) -> BackendKind {
        if let Some(kind) = self.override_kind {
            return kind;
        }
        if self.force_mesh {
            return BackendKind::MeshBridged;
        }
        if crate::session_scanner::cli_tool::spec(cli_tool)
            .capabilities
            .native_inbox_poller
        {
            BackendKind::ClaudeNative
        } else {
            BackendKind::MeshBridged
        }
    }

    pub fn select_floor(&self) -> BackendKind {
        self.override_kind.unwrap_or(BackendKind::MeshBridged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m0_forces_mesh_for_all_tools() {
        let selector = BackendSelector::m0();
        assert_eq!(selector.select(CliTool::Claude), BackendKind::MeshBridged);
        assert_eq!(selector.select(CliTool::Codex), BackendKind::MeshBridged);
        assert_eq!(selector.select(CliTool::Agy), BackendKind::MeshBridged);
    }

    #[test]
    fn default_auto_detects_by_cli_tool() {
        let selector = BackendSelector::default();
        assert_eq!(selector.select(CliTool::Claude), BackendKind::ClaudeNative);
        assert_eq!(selector.select(CliTool::Codex), BackendKind::MeshBridged);
        assert_eq!(selector.select(CliTool::Agy), BackendKind::MeshBridged);
    }

    #[test]
    fn override_takes_precedence() {
        let selector = BackendSelector {
            override_kind: Some(BackendKind::ClaudeNative),
            force_mesh: true,
        };
        assert_eq!(selector.select(CliTool::Codex), BackendKind::ClaudeNative);
    }

    #[test]
    fn override_can_force_mesh_for_claude_tool() {
        let selector = BackendSelector {
            override_kind: Some(BackendKind::MeshBridged),
            force_mesh: false,
        };
        assert_eq!(selector.select(CliTool::Claude), BackendKind::MeshBridged);
    }
}
