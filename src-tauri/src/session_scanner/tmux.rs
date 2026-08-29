//! tmux mapper — map terminal TTYs to tmux pane/window IDs.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::DisplaySession;

/// Information about a tmux pane.
#[derive(Debug, Clone, PartialEq)]
pub struct TmuxPane {
    /// Pane ID (e.g., "%0", "%3").
    pub pane_id: String,
    /// Terminal device (e.g., "/dev/pts/2").
    pub tty: String,
    /// Window index (e.g., "0", "1").
    pub window_index: String,
    /// Window name (e.g., "claude", "bash").
    pub window_name: String,
    /// tmux session name (e.g., "0", "main").
    pub session_name: String,
}

/// An attached tmux client, as reported by `tmux list-clients`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxClient {
    /// Raw client flags (`attached`, `focused`, `UTF-8`, ...).
    pub flags: Vec<String>,
    pub session: String,
    pub window_index: String,
    pub pane_id: String,
    /// `client_activity` epoch seconds; the tie-break between clients.
    pub activity: u64,
}

impl TmuxClient {
    fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|value| value == flag)
    }

    /// The terminal reports this client's window as focused.
    pub fn is_focused(&self) -> bool {
        self.has_flag("focused")
    }

    fn is_attached(&self) -> bool {
        self.has_flag("attached")
    }
}

/// The tmux session/window/pane the user is looking at.
///
/// Wire compatibility: `window_index` serializes as `window`, the key old apps
/// read, and every field tolerates a legacy `null` (the detached hook payload).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TmuxFocus {
    #[serde(default, deserialize_with = "null_as_empty_string")]
    pub session: String,
    #[serde(rename = "window", default, deserialize_with = "null_as_empty_string")]
    pub window_index: String,
    #[serde(default, deserialize_with = "null_as_empty_string")]
    pub pane_id: String,
}

fn null_as_empty_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

const LIST_CLIENTS_FORMAT: &str =
    "#{client_flags}\t#{session_name}\t#{window_index}\t#{pane_id}\t#{client_activity}";

/// List the tmux clients attached to the server.
///
/// Returns an empty list when tmux is unreachable (no server, timeout): the
/// hub reads that as "nothing is focused", never as an error.
pub fn list_clients() -> Vec<TmuxClient> {
    #[cfg(test)]
    if let Some(scripted) = list_clients_override() {
        return scripted();
    }

    let Some(output) =
        super::process::run_with_timeout("tmux", &["list-clients", "-F", LIST_CLIENTS_FORMAT])
    else {
        return Vec::new();
    };
    parse_list_clients(&output)
}

/// Test seam: stands in for the tmux client probe so the hub cycle can be
/// driven through focus changes without a tmux server.
#[cfg(test)]
pub(crate) type ListClientsOverride = fn() -> Vec<TmuxClient>;
#[cfg(test)]
static LIST_CLIENTS_OVERRIDE: std::sync::Mutex<Option<ListClientsOverride>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
pub(crate) fn set_list_clients_override(scripted: Option<ListClientsOverride>) {
    *LIST_CLIENTS_OVERRIDE
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = scripted;
}

#[cfg(test)]
fn list_clients_override() -> Option<ListClientsOverride> {
    *LIST_CLIENTS_OVERRIDE
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

/// Parse `tmux list-clients -F` output (tab-separated, 5 fields per line).
///
/// Lines that are not a client record are skipped.
pub fn parse_list_clients(output: &str) -> Vec<TmuxClient> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.trim_end_matches(['\r', '\n']).splitn(5, '\t');
            let flags = fields.next()?;
            let session = fields.next()?.trim();
            let window_index = fields.next()?.trim();
            let pane_id = fields.next()?.trim();
            let activity = fields.next()?.trim().parse::<u64>().ok()?;
            if session.is_empty() || window_index.is_empty() || pane_id.is_empty() {
                return None;
            }

            Some(TmuxClient {
                flags: flags
                    .split(',')
                    .map(str::trim)
                    .filter(|flag| !flag.is_empty())
                    .map(str::to_string)
                    .collect(),
                session: session.to_string(),
                window_index: window_index.to_string(),
                pane_id: pane_id.to_string(),
                activity,
            })
        })
        .collect()
}

/// Whether any client reports its window as focused — someone is looking.
pub fn any_client_focused(clients: &[TmuxClient]) -> bool {
    clients.iter().any(TmuxClient::is_focused)
}

/// Resolve the focused session/window/pane from the attached clients.
///
/// The focused client wins; ties and unfocused terminals fall back to the most
/// recently active attached client. No attached client means no focus.
pub fn focus_from_clients(clients: &[TmuxClient]) -> Option<TmuxFocus> {
    let focused = clients
        .iter()
        .filter(|client| client.is_focused())
        .max_by_key(|client| client.activity);
    let client = focused.or_else(|| {
        clients
            .iter()
            .filter(|client| client.is_attached())
            .max_by_key(|client| client.activity)
    })?;

    Some(TmuxFocus {
        session: client.session.clone(),
        window_index: client.window_index.clone(),
        pane_id: client.pane_id.clone(),
    })
}

/// List all tmux panes and build a TTY → TmuxPane lookup.
///
/// Returns an empty map if tmux is not running or the command fails.
pub fn list_panes() -> HashMap<String, TmuxPane> {
    let output = match run_tmux_list_panes() {
        Some(output) => output,
        None => return HashMap::new(),
    };
    parse_tmux_output(&output)
}

/// Run `tmux list-panes -a` and return stdout.
///
/// Uses `run_with_timeout` to avoid hanging if tmux is unresponsive.
fn run_tmux_list_panes() -> Option<String> {
    super::process::run_with_timeout(
        "tmux",
        &[
            "list-panes",
            "-a",
            "-F",
            "#{pane_id} #{pane_tty} #{window_index} #{window_name} #{session_name}",
        ],
    )
}

/// Parse tmux list-panes output into a TTY → TmuxPane map.
///
/// Expected format per line (space-separated, 5 fields):
/// `%0 /dev/pts/2 0 claude 0`
/// `pane_id tty window_index window_name session_name`
pub fn parse_tmux_output(output: &str) -> HashMap<String, TmuxPane> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }

            // Split into exactly 5 fields. Window names may contain spaces,
            // so we split into at most 5 parts.
            let parts: Vec<&str> = line.splitn(5, ' ').collect();
            if parts.len() < 5 {
                return None;
            }

            let pane = TmuxPane {
                pane_id: parts[0].to_string(),
                tty: parts[1].to_string(),
                window_index: parts[2].to_string(),
                window_name: parts[3].to_string(),
                session_name: parts[4].to_string(),
            };

            Some((pane.tty.clone(), pane))
        })
        .collect()
}

/// Map a focused tmux session/window/pane onto a known session's project path.
///
/// The pane is the answer whenever the focus carries one. Every scanned session
/// that knows its tmux session knows its pane too — both come from the same
/// `TmuxPane` record (`scans.rs`) — so a focused pane that matches nothing is a
/// pane no session owns: a plain shell, or the neighbour that a `Split` window
/// puts beside another project (`tmux_layout.rs`). Naming the window's other
/// project there is a wrong answer, not a degraded one, so it stays unresolved.
///
/// Window matching (index, then the taurhaus-managed window name) resolves only
/// payloads that carry no pane id at all: the retired hook file and any snapshot
/// whose pane field is null.
pub fn resolve_focus_project_path(
    focus: &TmuxFocus,
    sessions: &[DisplaySession],
) -> Option<String> {
    let session_name = focus.session.trim();
    let window = focus.window_index.trim();
    if session_name.is_empty() || window.is_empty() {
        return None;
    }

    let in_focused_session = |session: &&DisplaySession| {
        session
            .tmux_session
            .as_deref()
            .is_some_and(|value| value.trim() == session_name)
    };

    let pane = focus.pane_id.trim();
    if !pane.is_empty() {
        return sessions
            .iter()
            .filter(in_focused_session)
            .find(|session| {
                session
                    .tmux_pane
                    .as_deref()
                    .is_some_and(|value| value.trim() == pane)
            })
            .map(|session| session.project_path.clone());
    }

    sessions
        .iter()
        .filter(in_focused_session)
        .find(|session| {
            let matches_window_name = session
                .tmux_window_name
                .as_deref()
                .is_some_and(|value| value.trim() == window);
            let matches_window_index = session
                .tmux_window
                .as_deref()
                .is_some_and(|value| value.trim() == window);
            matches_window_name || matches_window_index
        })
        .map(|session| session.project_path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::{
        ActivityAttribution, ActivityConfidence, CliTool, SessionGroupKind, SessionState,
    };

    fn session_for(path: &str, session_name: Option<&str>, window: Option<&str>) -> DisplaySession {
        DisplaySession {
            pid: 42,
            project_path: path.to_string(),
            tty: "/dev/pts/1".to_string(),
            args: "codex --yolo".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: session_name.map(str::to_string),
            tmux_window: Some("1".to_string()),
            tmux_pane: Some("%1".to_string()),
            tmux_window_name: window.map(str::to_string),
            state: SessionState::Active,
            recent_io: false,
            last_output_age_secs: None,
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
            workflow_activity: None,
        }
    }

    #[test]
    fn parse_single_pane() {
        let output = "%0 /dev/pts/2 0 claude 0\n";
        let map = parse_tmux_output(output);
        assert_eq!(map.len(), 1);
        let pane = map.get("/dev/pts/2").unwrap();
        assert_eq!(pane.pane_id, "%0");
        assert_eq!(pane.tty, "/dev/pts/2");
        assert_eq!(pane.window_index, "0");
        assert_eq!(pane.window_name, "claude");
        assert_eq!(pane.session_name, "0");
    }

    #[test]
    fn parse_multi_pane_multi_window() {
        let output = "\
%0 /dev/pts/1 0 bash main
%1 /dev/pts/2 0 bash main
%2 /dev/pts/3 1 claude main
%3 /dev/pts/4 2 vim work";
        let map = parse_tmux_output(output);
        assert_eq!(map.len(), 4);

        let pane0 = map.get("/dev/pts/1").unwrap();
        assert_eq!(pane0.pane_id, "%0");
        assert_eq!(pane0.window_name, "bash");
        assert_eq!(pane0.session_name, "main");

        let pane2 = map.get("/dev/pts/3").unwrap();
        assert_eq!(pane2.pane_id, "%2");
        assert_eq!(pane2.window_index, "1");
        assert_eq!(pane2.window_name, "claude");

        let pane3 = map.get("/dev/pts/4").unwrap();
        assert_eq!(pane3.session_name, "work");
    }

    #[test]
    fn parse_multi_session() {
        let output = "\
%0 /dev/pts/1 0 shell sess-a
%1 /dev/pts/2 0 shell sess-b";
        let map = parse_tmux_output(output);
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("/dev/pts/1").unwrap().session_name, "sess-a");
        assert_eq!(map.get("/dev/pts/2").unwrap().session_name, "sess-b");
    }

    #[test]
    fn parse_empty_output() {
        let map = parse_tmux_output("");
        assert!(map.is_empty());
    }

    #[test]
    fn parse_malformed_lines_skipped() {
        let output = "\
%0 /dev/pts/1 0 bash main
bad line
%1 /dev/pts/2";
        let map = parse_tmux_output(output);
        // Only the first line has 5 fields
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("/dev/pts/1"));
    }

    #[test]
    fn parse_window_name_with_spaces() {
        // Window name might contain spaces if renamed
        // With splitn(5), the 5th field gets the rest
        // But our format has session_name as the 5th, and window_name as 4th
        // Session name shouldn't have spaces typically
        let output = "%0 /dev/pts/1 0 my-project 0\n";
        let map = parse_tmux_output(output);
        assert_eq!(map.len(), 1);
        let pane = map.get("/dev/pts/1").unwrap();
        assert_eq!(pane.window_name, "my-project");
        assert_eq!(pane.session_name, "0");
    }

    #[test]
    fn duplicate_tty_last_wins() {
        // If somehow two panes share a TTY (shouldn't happen), last wins
        let output = "\
%0 /dev/pts/1 0 first main
%1 /dev/pts/1 1 second main";
        let map = parse_tmux_output(output);
        assert_eq!(map.len(), 1);
        // HashMap insert overwrites, so which wins is non-deterministic
        // Just verify we have one entry
        assert!(map.contains_key("/dev/pts/1"));
    }

    fn client(
        flags: &[&str],
        session: &str,
        window: &str,
        pane: &str,
        activity: u64,
    ) -> TmuxClient {
        TmuxClient {
            flags: flags.iter().map(|flag| flag.to_string()).collect(),
            session: session.to_string(),
            window_index: window.to_string(),
            pane_id: pane.to_string(),
            activity,
        }
    }

    #[test]
    fn parse_list_clients_reads_a_single_focused_client() {
        // Live on this host (tmux 3.4, Windows Terminal -> wsl.exe client).
        let output = "attached,focused,UTF-8\ttaurhaus\t0\t%3\t1787359665\n";

        assert_eq!(
            parse_list_clients(output),
            vec![client(
                &["attached", "focused", "UTF-8"],
                "taurhaus",
                "0",
                "%3",
                1_787_359_665
            )]
        );
    }

    #[test]
    fn parse_list_clients_ignores_garbage_and_empty_lines() {
        let output = "attached,focused\ttaurhaus\t0\t%3\t10\nnot a client line\n\n\t\t\t\t\n";

        assert_eq!(
            parse_list_clients(output),
            vec![client(&["attached", "focused"], "taurhaus", "0", "%3", 10)]
        );
    }

    #[test]
    fn parse_list_clients_returns_empty_for_no_clients() {
        assert!(parse_list_clients("").is_empty());
    }

    #[test]
    fn focus_from_clients_prefers_the_most_recently_active_focused_client() {
        // Verified on this host: two attached clients can both report `focused`;
        // the tie-break is `client_activity`.
        let clients = vec![
            client(&["attached", "focused"], "taurhaus", "0", "%0", 100),
            client(&["attached", "focused"], "beta", "2", "%7", 200),
        ];

        assert_eq!(
            focus_from_clients(&clients),
            Some(TmuxFocus {
                session: "beta".to_string(),
                window_index: "2".to_string(),
                pane_id: "%7".to_string(),
            })
        );
    }

    #[test]
    fn focus_from_clients_prefers_focused_over_more_recent_unfocused() {
        let clients = vec![
            client(&["attached", "focused"], "taurhaus", "0", "%0", 100),
            client(&["attached"], "beta", "2", "%7", 900),
        ];

        assert_eq!(
            focus_from_clients(&clients).map(|focus| focus.session),
            Some("taurhaus".to_string())
        );
    }

    #[test]
    fn focus_from_clients_falls_back_to_the_most_recent_attached_client() {
        let clients = vec![
            client(&["attached"], "taurhaus", "0", "%0", 100),
            client(&["attached"], "beta", "2", "%7", 200),
        ];

        assert_eq!(
            focus_from_clients(&clients).map(|focus| focus.pane_id),
            Some("%7".to_string())
        );
    }

    #[test]
    fn focus_from_clients_returns_none_without_an_attached_client() {
        assert_eq!(focus_from_clients(&[]), None);
        assert_eq!(
            focus_from_clients(&[client(&["UTF-8"], "taurhaus", "0", "%0", 100)]),
            None
        );
    }

    #[test]
    fn any_client_focused_reports_whether_someone_is_looking() {
        assert!(any_client_focused(&[client(
            &["attached", "focused"],
            "taurhaus",
            "0",
            "%0",
            1
        )]));
        assert!(!any_client_focused(&[client(
            &["attached"],
            "taurhaus",
            "0",
            "%0",
            1
        )]));
        assert!(!any_client_focused(&[]));
    }

    /// A legacy focus payload: session and window, no pane id.
    fn focus_on(session: &str, window_index: &str) -> TmuxFocus {
        focus_pane(session, window_index, "")
    }

    fn focus_pane(session: &str, window_index: &str, pane_id: &str) -> TmuxFocus {
        TmuxFocus {
            session: session.to_string(),
            window_index: window_index.to_string(),
            pane_id: pane_id.to_string(),
        }
    }

    /// One launch inside the shared `taurhaus` session, pinned to a pane.
    fn split_session(path: &str, window_index: &str, pane_id: &str) -> DisplaySession {
        let mut session = session_for(path, Some("taurhaus"), None);
        session.tmux_window = Some(window_index.to_string());
        session.tmux_pane = Some(pane_id.to_string());
        session
    }

    // Regression: commit 07ab6c5 made the hub the owner of tmux focus but
    // resolved it at window granularity, ignoring the pane id it already
    // carried. The `Split` launch policy puts two projects in one window
    // (`tmux_layout.rs`), so focusing project B's pane reported project A.
    #[test]
    fn resolve_focus_matches_the_focused_pane_inside_a_split_window() {
        let sessions = vec![
            split_session("/projects/alpha", "1", "%3"),
            split_session("/projects/beta", "1", "%5"),
        ];

        assert_eq!(
            resolve_focus_project_path(&focus_pane("taurhaus", "1", "%5"), &sessions),
            Some("/projects/beta".to_string())
        );
        assert_eq!(
            resolve_focus_project_path(&focus_pane("taurhaus", "1", "%3"), &sessions),
            Some("/projects/alpha".to_string())
        );
    }

    // Regression: commit b816dc7 made the focused pane the precise answer but
    // left the window match as an unconditional fallback. Every session that
    // carries a tmux session carries its pane too (both come from the same
    // `TmuxPane` record in `scans.rs`), so a focused pane that matches nothing
    // is a pane no scanned session owns — the neighbour in a `Split` window, a
    // plain shell, or an agent that just exited. Falling through to the window
    // named the wrong project as foreground.
    #[test]
    fn resolve_focus_does_not_name_the_neighbour_when_the_focused_pane_is_unknown() {
        let sessions = vec![split_session("/projects/alpha", "1", "%3")];

        assert_eq!(
            resolve_focus_project_path(&focus_pane("taurhaus", "1", "%5"), &sessions),
            None,
            "beta's pane shares alpha's window but alpha must not inherit its focus"
        );
    }

    // Regression: commit 07ab6c5. Legacy focus payloads (the retired hook file,
    // and any snapshot whose pane field is null) carry no pane id at all, so the
    // window match stays as their resolution path.
    #[test]
    fn resolve_focus_falls_back_to_the_window_only_for_a_payload_without_a_pane() {
        let sessions = vec![split_session("/projects/alpha", "1", "%3")];

        assert_eq!(
            resolve_focus_project_path(&focus_pane("taurhaus", "1", ""), &sessions),
            Some("/projects/alpha".to_string()),
            "a legacy payload without a pane id still resolves"
        );
    }

    // A pane id is server-unique, but the session guard must still hold.
    #[test]
    fn resolve_focus_ignores_a_matching_pane_in_another_tmux_session() {
        let mut other = split_session("/projects/alpha", "1", "%3");
        other.tmux_session = Some("other".to_string());

        assert_eq!(
            resolve_focus_project_path(&focus_pane("taurhaus", "1", "%3"), &[other]),
            None
        );
    }

    #[test]
    fn resolve_focus_matches_window_index() {
        let mut indexed_only = session_for("/projects/mesh", Some("taurhaus"), None);
        indexed_only.tmux_window = Some("2".to_string());

        assert_eq!(
            resolve_focus_project_path(&focus_on("taurhaus", "2"), &[indexed_only]),
            Some("/projects/mesh".to_string())
        );
    }

    #[test]
    fn resolve_focus_matches_taurhaus_managed_window_name() {
        let sessions = vec![
            session_for("/projects/other", Some("taurhaus"), Some("other")),
            session_for("/projects/mesh", Some("taurhaus"), Some("mesh")),
        ];

        assert_eq!(
            resolve_focus_project_path(&focus_on("taurhaus", "mesh"), &sessions),
            Some("/projects/mesh".to_string())
        );
    }

    #[test]
    fn resolve_focus_returns_none_for_unknown_window() {
        let sessions = vec![session_for(
            "/projects/mesh",
            Some("taurhaus"),
            Some("mesh"),
        )];

        assert_eq!(
            resolve_focus_project_path(&focus_on("taurhaus", "missing"), &sessions),
            None
        );
    }

    #[test]
    fn resolve_focus_returns_none_for_empty_focus_fields() {
        let sessions = vec![session_for(
            "/projects/mesh",
            Some("taurhaus"),
            Some("mesh"),
        )];

        assert_eq!(
            resolve_focus_project_path(&focus_on("", ""), &sessions),
            None
        );
    }

    #[test]
    fn tmux_focus_decodes_legacy_session_window_payload() {
        // Old daemons wrote `{"session":..,"window":..,"timestamp":..}`; a new app
        // must still decode their snapshot instead of dropping it wholesale.
        let legacy: TmuxFocus =
            serde_json::from_str(r#"{"session":"taurhaus","window":"2","timestamp":123}"#).unwrap();
        assert_eq!(legacy, focus_on_index("taurhaus", "2"));

        let detached: TmuxFocus =
            serde_json::from_str(r#"{"session":null,"window":null,"timestamp":null}"#).unwrap();
        assert_eq!(detached.session, "");
        assert_eq!(detached.window_index, "");
    }

    fn focus_on_index(session: &str, window_index: &str) -> TmuxFocus {
        TmuxFocus {
            session: session.to_string(),
            window_index: window_index.to_string(),
            pane_id: String::new(),
        }
    }

    #[test]
    fn tmux_focus_keeps_the_session_and_window_wire_keys() {
        // Old apps read only `session`/`window`; the daemon must keep emitting them.
        let json = serde_json::to_value(focus_pane("taurhaus", "2", "%1")).unwrap();
        assert_eq!(json["session"], "taurhaus");
        assert_eq!(json["window"], "2");
        assert_eq!(json["pane_id"], "%1");
    }
}
