//! What the pane's shell will actually run.
//!
//! A launch is typed into an interactive shell, so the first word of a base
//! command is whatever that shell says it is: `claude2` may be an alias for
//! `CLAUDE_CONFIG_DIR=~/.claude-account2 claude`. Taking the base literally
//! hides an account selector the user never sees, so resolution and rendering
//! both read the resolved form instead.
//!
//! Resolution is fail-soft by construction: a probe that cannot answer leaves
//! the base exactly as configured, which is what taurhaus did before this
//! module existed.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::cli_tool::{spec, CliTool};

/// One alias the pane shell expands before the command runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasExpansion {
    pub name: String,
    pub body: String,
}

/// A configured base command as the pane shell reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedBase {
    /// The command with every expanded alias substituted in place.
    pub command: String,
    /// The aliases that were expanded, outermost first.
    #[serde(default)]
    pub expansions: Vec<AliasExpansion>,
    /// The head word when it is not the tool's own executable. A function or a
    /// wrapper script is opaque by design; taurhaus does not run it to find out.
    #[serde(default)]
    pub opaque_head: Option<String>,
}

/// What the pane shell says a word means. Implementations never run the word.
pub trait AliasProbe {
    /// The shell being asked. Part of the resolution cache key.
    fn shell(&self) -> &str;

    /// The alias body for `name`, or `None` when `name` is not an alias.
    fn alias(&self, name: &str) -> Option<String>;
}

/// A shell expands an alias whose body is itself an alias, but not forever.
const MAX_EXPANSIONS: usize = 3;

/// Resolve `base` the way `probe`'s shell would read its first word.
pub fn resolve_base_command(base: &str, tool: CliTool, probe: &dyn AliasProbe) -> ResolvedBase {
    resolve_base_command_in(base, tool, probe, dirs::home_dir().as_deref())
}

/// The same, against an explicit home for the shell that will run the launch.
fn resolve_base_command_in(
    base: &str,
    tool: CliTool,
    probe: &dyn AliasProbe,
    home: Option<&Path>,
) -> ResolvedBase {
    let mut command = base.to_string();
    let mut expansions = Vec::new();
    let mut expanded: HashSet<String> = HashSet::new();

    while expansions.len() < MAX_EXPANSIONS {
        let words = words(&command);
        let Some(head) = words.into_iter().find(|word| !word.is_assignment()) else {
            break;
        };
        // A quoted head is never alias-expanded by the shell either.
        if head.quoted || !is_alias_name(&head.value) || expanded.contains(&head.value) {
            break;
        }
        let Some(body) = probe.alias(&head.value).map(|body| body.trim().to_string()) else {
            break;
        };
        if body.is_empty() {
            break;
        }
        expanded.insert(head.value.clone());
        expansions.push(AliasExpansion {
            name: head.value,
            body: body.clone(),
        });
        command.replace_range(head.start..head.end, &body);
    }

    let command = expand_selector_home(&command, tool, home);
    let opaque_head = head_word(&command).filter(|head| !runs_the_tool(tool, head));
    ResolvedBase {
        command,
        expansions,
        opaque_head,
    }
}

/// Rewrite this tool's `SELECTOR=~/…` assignments as absolute paths.
///
/// Resolution runs where the pane shell runs — in the WSL daemon on Windows —
/// so `home` is the home that shell would expand the tilde against. The app
/// never has to guess it from its own side of the boundary, which it cannot do:
/// a Windows profile path names none of the accounts the daemon detects. Every
/// consumer downstream compares this selector against absolute account dirs.
fn expand_selector_home(command: &str, tool: CliTool, home: Option<&Path>) -> String {
    let (Some(selector), Some(home)) = (spec(tool).capabilities.account_selector, home) else {
        return command.to_string();
    };
    let prefix = format!("{selector}=");
    let mut rewritten = command.to_string();
    // Back to front, so each remaining word keeps its span in the original.
    for word in words(command).into_iter().rev() {
        // A shell leaves a quoted tilde literal, and so does this.
        if word.quoted {
            continue;
        }
        let Some(expanded) = word
            .value
            .strip_prefix(&prefix)
            .and_then(|value| expand_tilde(value, home))
        else {
            continue;
        };
        rewritten.replace_range(
            word.start..word.end,
            &format!("{prefix}{}", super::launch::shell_escape(&expanded)),
        );
    }
    rewritten
}

/// `~` and `~/tail` against `home`, or `None` for anything else.
///
/// `~someone` is another user's home, which only that shell can name. The tail
/// is joined with `/` rather than `Path::join`, because the shell that reads
/// this line is a POSIX one wherever the app itself happens to run.
fn expand_tilde(value: &str, home: &Path) -> Option<String> {
    let rest = value.strip_prefix('~')?;
    let home = home.to_string_lossy();
    let home = home.trim_end_matches('/');
    if rest.is_empty() {
        return Some(home.to_string());
    }
    match rest.strip_prefix('/')?.trim_start_matches('/') {
        "" => Some(home.to_string()),
        tail => Some(format!("{home}/{tail}")),
    }
}

/// Resolve `base`, reusing an answer this process got in the last minute.
///
/// Probing runs an interactive shell, so a launch, its preview and the settings
/// page must not each pay for one.
#[cfg(not(test))]
pub fn resolve_base_command_cached(
    base: &str,
    tool: CliTool,
    probe: &dyn AliasProbe,
) -> ResolvedBase {
    resolve_base_command_cached_at(base, tool, probe, Instant::now())
}

/// Under test the process-global cache is a channel between tests: one test's
/// answer would outlive the alias table the next one installs.
#[cfg(test)]
pub fn resolve_base_command_cached(
    base: &str,
    tool: CliTool,
    probe: &dyn AliasProbe,
) -> ResolvedBase {
    resolve_base_command(base, tool, probe)
}

/// One resolution per (shell, tool, base), including a failed probe: a shell
/// that could not answer must not be asked again on every keystroke.
const CACHE_TTL: Duration = Duration::from_secs(60);

type CacheKey = (String, CliTool, String);

static CACHE: LazyLock<Mutex<HashMap<CacheKey, (Instant, ResolvedBase)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn resolve_base_command_cached_at(
    base: &str,
    tool: CliTool,
    probe: &dyn AliasProbe,
    now: Instant,
) -> ResolvedBase {
    let key = (probe.shell().to_string(), tool, base.to_string());
    if let Some(resolved) = cache_read(&key, now) {
        return resolved;
    }
    // Probing outside the lock: a shell that hangs for its whole budget must
    // not hold every other launch behind it.
    let resolved = resolve_base_command(base, tool, probe);
    let mut cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
    // Settings can be edited into any number of distinct base commands; an
    // entry nobody can read again has no reason to stay.
    cache.retain(|_, (observed_at, _)| now.duration_since(*observed_at) < CACHE_TTL);
    cache.insert(key, (now, resolved.clone()));
    resolved
}

fn cache_read(key: &CacheKey, now: Instant) -> Option<ResolvedBase> {
    let cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
    let (observed_at, resolved) = cache.get(key)?;
    (now.duration_since(*observed_at) < CACHE_TTL).then(|| resolved.clone())
}

/// Whether the head word invokes the tool's own executable, by name or path.
fn runs_the_tool(tool: CliTool, head: &str) -> bool {
    let tool_spec = spec(tool);
    // A tool with no argv signature has nothing to be told apart from.
    tool_spec.argv_signatures.is_empty() || tool_spec.matches_argv_token(head)
}

/// The first word that is not a leading `NAME=value` assignment.
fn head_word(command: &str) -> Option<String> {
    words(command)
        .into_iter()
        .find(|word| !word.is_assignment())
        .map(|word| word.value)
}

/// Alias names taurhaus is willing to hand to a shell.
///
/// The name reaches an interactive `-c` script, so anything outside this set —
/// a path, a substitution, whitespace — is not asked about at all.
fn is_alias_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_.+-".contains(character))
}

/// One word of a shell command: where it sits, and what it means unquoted.
struct Word {
    start: usize,
    end: usize,
    value: String,
    quoted: bool,
}

impl Word {
    fn is_assignment(&self) -> bool {
        let Some((name, _)) = self.value.split_once('=') else {
            return false;
        };
        !name.is_empty()
            && name
                .starts_with(|character: char| character.is_ascii_alphabetic() || character == '_')
            && name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
    }
}

/// Split a command into words, keeping each word's span in the original line.
///
/// The quoting rules are the ones a POSIX shell applies before it looks a word
/// up as an alias; nothing here expands, substitutes, or executes.
fn words(command: &str) -> Vec<Word> {
    let mut words = Vec::new();
    let mut value = String::new();
    let mut start = 0usize;
    let mut started = false;
    let mut quoted = false;
    let mut characters = command.char_indices().peekable();

    while let Some((index, character)) = characters.next() {
        match character {
            character if character.is_whitespace() => {
                if started {
                    words.push(Word {
                        start,
                        end: index,
                        value: std::mem::take(&mut value),
                        quoted,
                    });
                    started = false;
                    quoted = false;
                }
            }
            '\'' => {
                if !started {
                    start = index;
                    started = true;
                }
                quoted = true;
                for (_, character) in characters.by_ref() {
                    if character == '\'' {
                        break;
                    }
                    value.push(character);
                }
            }
            '"' => {
                if !started {
                    start = index;
                    started = true;
                }
                quoted = true;
                while let Some((_, character)) = characters.next() {
                    match character {
                        '"' => break,
                        '\\' if matches!(characters.peek(), Some((_, '"' | '\\' | '$' | '`'))) => {
                            value.extend(characters.next().map(|(_, character)| character));
                        }
                        character => value.push(character),
                    }
                }
            }
            '\\' => {
                if !started {
                    start = index;
                    started = true;
                }
                quoted = true;
                value.extend(characters.next().map(|(_, character)| character));
            }
            character => {
                if !started {
                    start = index;
                    started = true;
                }
                value.push(character);
            }
        }
    }

    if started {
        words.push(Word {
            start,
            end: command.len(),
            value,
            quoted,
        });
    }
    words
}

/// How long an interactive shell gets to answer what one word means.
#[cfg(not(test))]
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// The whole of one resolution — finding the pane shell, then every probe it
/// answers — fits inside this.
///
/// A caller on the other side of the WSL boundary sizes its request around this
/// number: a resolution that outlives the request asking for it reads as a
/// failure, and a failure puts the literal base back, alias selector and all.
/// Exhausting the budget is fail-soft in exactly the same way, so it is set
/// where one slow shell still answers and a chain of them stops trying.
pub const RESOLUTION_BUDGET: Duration = Duration::from_secs(8);

/// Asks the pane's own interactive shell what a word means.
pub struct ShellAliasProbe {
    shell: String,
    /// When this resolution stops asking. Probing is the slow part, and the
    /// budget is the whole probe's, not each question's.
    deadline: Instant,
}

impl ShellAliasProbe {
    /// The shell a launched pane runs: tmux's `default-shell`, else `$SHELL`.
    pub fn for_pane() -> Self {
        Self::for_pane_until(Instant::now() + RESOLUTION_BUDGET)
    }

    /// The same probe against an explicit deadline.
    fn for_pane_until(deadline: Instant) -> Self {
        Self {
            // Finding the shell is inside the budget: it runs tmux to do it.
            shell: pane_shell(),
            deadline,
        }
    }
}

#[cfg(not(test))]
fn pane_shell() -> String {
    tmux_default_shell()
        .or_else(|| std::env::var("SHELL").ok())
        .map(|shell| shell.trim().to_string())
        .filter(|shell| !shell.is_empty())
        .unwrap_or_else(|| "/bin/sh".to_string())
}

/// A unit test never starts an interactive shell and never reads the
/// developer's own rc file.
#[cfg(test)]
fn pane_shell() -> String {
    "test".to_string()
}

impl AliasProbe for ShellAliasProbe {
    fn shell(&self) -> &str {
        &self.shell
    }

    fn alias(&self, name: &str) -> Option<String> {
        // A shell started with less time than it needs answers nothing useful,
        // and the caller is no longer waiting for it.
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return None;
        }
        is_alias_name(name).then(|| shell_alias(&self.shell, name, remaining))?
    }
}

/// Ask one interactive shell what a single validated word means.
#[cfg(not(test))]
fn shell_alias(shell: &str, name: &str, within: Duration) -> Option<String> {
    let script = format!("alias -- {name}");
    let output = super::process::run_with_timeout_within(
        shell,
        &["-ic", &script],
        within.min(PROBE_TIMEOUT),
    )?;
    parse_alias_output(name, &output)
}

#[cfg(test)]
fn shell_alias(_shell: &str, name: &str, _within: Duration) -> Option<String> {
    ALIAS_OVERRIDE
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .and_then(|aliases| aliases.get(name).cloned())
}

#[cfg(not(test))]
fn tmux_default_shell() -> Option<String> {
    super::process::run_with_timeout("tmux", &["show-options", "-gv", "default-shell"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Read one `alias` builtin line: `name='body'` (zsh) or `alias name='body'`
/// (bash). An interactive rc may print its own noise first, so every line is
/// considered and anything unparseable simply is not an alias.
fn parse_alias_output(name: &str, output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let line = line.trim();
        let line = line.strip_prefix("alias ").unwrap_or(line);
        let (found, body) = line.split_once('=')?;
        (found.trim() == name).then_some(body)?;
        unquote_alias_body(body)
    })
}

/// Read an `alias` builtin's value as the shell wrote it: a run of adjacent
/// segments, each single-quoted, double-quoted, backslash-escaped or bare.
///
/// The whole value is not one wrapped string. zsh stops quoting at a body's
/// final quote (`say='echo '\''hi there'\'`) and opens with a bare escape when
/// the body starts with one (`q=\''foo'\'' bar'`), so anything that assumes an
/// outer pair either corrupts the body or drops it. Concatenating segments is
/// what a shell does with the value anyway.
fn unquote_alias_body(body: &str) -> Option<String> {
    let body = body.trim();
    if body.is_empty() {
        return None;
    }
    let mut unquoted = String::with_capacity(body.len());
    let mut rest = body;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('\'') {
            let (inner, tail) = after.split_once('\'')?;
            unquoted.push_str(inner);
            rest = tail;
        } else if let Some(after) = rest.strip_prefix('"') {
            rest = push_double_quoted(after, &mut unquoted)?;
        } else if let Some(after) = rest.strip_prefix('\\') {
            let mut characters = after.chars();
            unquoted.push(characters.next()?);
            rest = characters.as_str();
        } else {
            let end = rest.find(['\'', '"', '\\']).unwrap_or(rest.len());
            let (bare, tail) = rest.split_at(end);
            // `$'...'` carries C escapes this parser does not speak.
            if bare.ends_with('$') && tail.starts_with('\'') {
                return None;
            }
            unquoted.push_str(bare);
            rest = tail;
        }
    }
    Some(unquoted)
}

/// Consume one double-quoted segment, `after` starting just past its opening
/// quote. Returns what follows the closing quote, or `None` when there is none.
fn push_double_quoted<'a>(after: &'a str, unquoted: &mut String) -> Option<&'a str> {
    let mut characters = after.chars();
    while let Some(character) = characters.next() {
        match character {
            '"' => return Some(characters.as_str()),
            '\\' => {
                let mut escaped = characters.clone();
                match escaped.next() {
                    Some(next @ ('"' | '\\' | '$' | '`')) => {
                        unquoted.push(next);
                        characters = escaped;
                    }
                    _ => unquoted.push('\\'),
                }
            }
            character => unquoted.push(character),
        }
    }
    None
}

#[cfg(test)]
static ALIAS_OVERRIDE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

#[cfg(test)]
static ALIAS_OVERRIDE_LOCK: Mutex<()> = Mutex::new(());

/// Keeps a test-owned alias table installed for `ShellAliasProbe::for_pane`.
#[cfg(test)]
pub(crate) struct AliasOverrideGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl Drop for AliasOverrideGuard {
    fn drop(&mut self) {
        *ALIAS_OVERRIDE
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
    }
}

/// Install what the pane shell would say, without running one.
#[cfg(test)]
pub(crate) fn install_alias_override(aliases: &[(&str, &str)]) -> AliasOverrideGuard {
    let lock = ALIAS_OVERRIDE_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    *ALIAS_OVERRIDE
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some(
        aliases
            .iter()
            .map(|(name, body)| (name.to_string(), body.to_string()))
            .collect(),
    );
    AliasOverrideGuard { _lock: lock }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::cell::RefCell;

    /// Answers from a table. No shell, no process, no user rc file.
    struct FakeProbe {
        shell: &'static str,
        aliases: HashMap<String, String>,
        asked: RefCell<Vec<String>>,
    }

    impl FakeProbe {
        fn new(aliases: &[(&str, &str)]) -> Self {
            Self {
                shell: "/fake/zsh",
                aliases: aliases
                    .iter()
                    .map(|(name, body)| (name.to_string(), body.to_string()))
                    .collect(),
                asked: RefCell::new(Vec::new()),
            }
        }

        fn asked(&self) -> Vec<String> {
            self.asked.borrow().clone()
        }
    }

    impl AliasProbe for FakeProbe {
        fn shell(&self) -> &str {
            self.shell
        }

        fn alias(&self, name: &str) -> Option<String> {
            self.asked.borrow_mut().push(name.to_string());
            self.aliases.get(name).cloned()
        }
    }

    #[test]
    fn expands_the_head_alias_and_keeps_the_rest_of_the_line() {
        // Regression: 0.8.3 read the base command literally, so the operator's
        // `claude2` alias hid a CLAUDE_CONFIG_DIR the account resolution never
        // saw and every launch ran on the wrong subscription.
        let probe = FakeProbe::new(&[("claude2", "CLAUDE_CONFIG_DIR=~/.claude-account2 claude")]);

        let resolved = resolve_base_command_in(
            "claude2 --dangerously-skip-permissions",
            CliTool::Claude,
            &probe,
            Some(Path::new("/home/operator")),
        );

        assert_eq!(
            resolved.command,
            "CLAUDE_CONFIG_DIR='/home/operator/.claude-account2' claude --dangerously-skip-permissions"
        );
        assert_eq!(
            resolved.expansions,
            vec![AliasExpansion {
                name: "claude2".to_string(),
                body: "CLAUDE_CONFIG_DIR=~/.claude-account2 claude".to_string(),
            }],
            "the alias body stays as the user wrote it"
        );
        assert_eq!(resolved.opaque_head, None);
    }

    #[test]
    fn a_tilde_selector_expands_against_the_home_the_pane_shell_has() {
        // Regression: a86f3b0 left `~` in the resolved command for the app to
        // expand with its own `dirs::home_dir()`. On Windows that is the
        // Windows profile and never the WSL home the daemon's accounts sit
        // under, so a base-owned alias selector matched no detected account and
        // the launch kept no account identity at all. The probe already runs
        // where the pane shell runs, so it is the side that knows the home.
        let probe = FakeProbe::new(&[("claude2", "CLAUDE_CONFIG_DIR=~/.claude-account2 claude")]);

        let resolved = resolve_base_command_in(
            "claude2 --dangerously-skip-permissions",
            CliTool::Claude,
            &probe,
            // A WSL home, resolved while the app itself runs on Windows.
            Some(Path::new("/home/mstie")),
        );

        assert_eq!(
            resolved.command,
            "CLAUDE_CONFIG_DIR='/home/mstie/.claude-account2' claude --dangerously-skip-permissions"
        );
        assert!(
            !resolved.command.contains('~'),
            "every consumer downstream compares this against absolute account dirs"
        );
    }

    #[test]
    fn only_an_unquoted_leading_tilde_of_this_tools_selector_is_expanded() {
        let probe = FakeProbe::new(&[]);
        let home = Some(Path::new("/home/two words"));

        // A home a shell would need quoted stays one word.
        assert_eq!(
            resolve_base_command_in(
                "CLAUDE_CONFIG_DIR=~/.claude claude",
                CliTool::Claude,
                &probe,
                home
            )
            .command,
            "CLAUDE_CONFIG_DIR='/home/two words/.claude' claude"
        );
        // A shell leaves a quoted tilde literal, and so does this.
        assert_eq!(
            resolve_base_command_in(
                "CLAUDE_CONFIG_DIR='~/.claude' claude",
                CliTool::Claude,
                &probe,
                home
            )
            .command,
            "CLAUDE_CONFIG_DIR='~/.claude' claude"
        );
        // `~someone` is another user's home; only the launching one is known.
        assert_eq!(
            resolve_base_command_in(
                "CLAUDE_CONFIG_DIR=~other/.claude claude",
                CliTool::Claude,
                &probe,
                home
            )
            .command,
            "CLAUDE_CONFIG_DIR=~other/.claude claude"
        );
        // Another tool's selector is not this tool's to rewrite.
        assert_eq!(
            resolve_base_command_in("CODEX_HOME=~/.codex claude", CliTool::Claude, &probe, home)
                .command,
            "CODEX_HOME=~/.codex claude"
        );
    }

    #[test]
    fn keeps_leading_assignments_and_expands_the_first_real_word() {
        let probe = FakeProbe::new(&[("c1", "CLAUDE_CONFIG_DIR=/homes/one claude")]);

        let resolved = resolve_base_command("FOO=bar c1 --resume", CliTool::Claude, &probe);

        assert_eq!(
            resolved.command,
            "FOO=bar CLAUDE_CONFIG_DIR=/homes/one claude --resume"
        );
        // The shell looks the substituted head up as well; `claude` is not an
        // alias, so expansion stops there.
        assert_eq!(probe.asked(), vec!["c1".to_string(), "claude".to_string()]);
    }

    #[test]
    fn expands_three_levels_and_stops() {
        let probe = FakeProbe::new(&[
            ("one", "two"),
            ("two", "three"),
            ("three", "four"),
            ("four", "claude"),
        ]);

        let resolved = resolve_base_command("one --x", CliTool::Claude, &probe);

        assert_eq!(resolved.command, "four --x");
        assert_eq!(resolved.expansions.len(), 3);
        assert_eq!(resolved.opaque_head.as_deref(), Some("four"));
    }

    #[test]
    fn stops_on_an_alias_cycle() {
        let probe = FakeProbe::new(&[("claude", "claude --flag")]);

        let resolved = resolve_base_command("claude --resume", CliTool::Claude, &probe);

        assert_eq!(resolved.command, "claude --flag --resume");
        assert_eq!(resolved.expansions.len(), 1, "a name expands once");
        assert_eq!(resolved.opaque_head, None);
    }

    #[test]
    fn reports_a_head_that_is_not_the_tool_binary() {
        let probe = FakeProbe::new(&[]);

        let resolved = resolve_base_command("my-claude-wrapper --yolo", CliTool::Claude, &probe);

        assert_eq!(resolved.command, "my-claude-wrapper --yolo");
        assert_eq!(resolved.opaque_head.as_deref(), Some("my-claude-wrapper"));
    }

    #[test]
    fn a_path_to_the_tool_binary_is_not_opaque() {
        let probe = FakeProbe::new(&[]);

        let resolved = resolve_base_command("/usr/local/bin/codex --yolo", CliTool::Codex, &probe);

        assert_eq!(resolved.opaque_head, None);
    }

    #[test]
    fn a_quoted_head_is_not_alias_expanded() {
        let probe = FakeProbe::new(&[("claude2", "CLAUDE_CONFIG_DIR=/homes/two claude")]);

        let resolved = resolve_base_command("'claude2' --resume", CliTool::Claude, &probe);

        assert_eq!(resolved.command, "'claude2' --resume");
        assert!(resolved.expansions.is_empty());
        assert_eq!(probe.asked(), Vec::<String>::new());
    }

    #[test]
    fn a_probe_that_cannot_answer_leaves_the_base_literal() {
        let probe = FakeProbe::new(&[]);

        let resolved = resolve_base_command(
            "claude --dangerously-skip-permissions",
            CliTool::Claude,
            &probe,
        );

        assert_eq!(resolved.command, "claude --dangerously-skip-permissions");
        assert!(resolved.expansions.is_empty());
        assert_eq!(resolved.opaque_head, None);
    }

    #[test]
    fn a_head_that_cannot_be_an_alias_name_never_reaches_a_shell() {
        let probe = FakeProbe::new(&[]);

        resolve_base_command("$(echo claude) --resume", CliTool::Claude, &probe);
        resolve_base_command("cla;ude --resume", CliTool::Claude, &probe);
        resolve_base_command("/usr/bin/claude --resume", CliTool::Claude, &probe);

        assert_eq!(probe.asked(), Vec::<String>::new());
    }

    #[test]
    fn parses_zsh_and_bash_alias_output() {
        assert_eq!(
            parse_alias_output(
                "claude2",
                "claude2='CLAUDE_CONFIG_DIR=~/.claude-account2 claude'"
            ),
            Some("CLAUDE_CONFIG_DIR=~/.claude-account2 claude".to_string())
        );
        assert_eq!(
            parse_alias_output("claude2", "alias claude2='CLAUDE_CONFIG_DIR=~/.c2 claude'"),
            Some("CLAUDE_CONFIG_DIR=~/.c2 claude".to_string())
        );
        assert_eq!(
            parse_alias_output("say", r#"say='echo '\''hi there'\'''"#),
            Some("echo 'hi there'".to_string())
        );
        assert_eq!(
            parse_alias_output("dq", r#"dq="claude --title \"my session\"""#),
            Some(r#"claude --title "my session""#.to_string())
        );
        assert_eq!(
            parse_alias_output("plain", "plain=claude"),
            Some("claude".to_string())
        );
    }

    // Regression: 0f2bfbb parsed an alias body by stripping a presumed pair of
    // outer quotes, but zsh does not re-open the wrapper after a body's final
    // quote — it prints `say='echo '\''hi there'\'`, where the trailing `\'` is
    // escaped data. Stripping the last character removed that backslash's quote
    // and left the backslash behind, so an alias ending in a single-quoted
    // argument reached the launch command corrupted. A body whose first word is
    // single-quoted (zsh opens with a bare `\'`) was dropped as unparseable for
    // the same reason.
    #[test]
    fn parses_the_zsh_forms_that_do_not_wrap_the_whole_body() {
        // Captured from `zsh -f`: alias a1="echo 'hi there'" and friends.
        assert_eq!(
            parse_alias_output("say", r"say='echo '\''hi there'\'"),
            Some("echo 'hi there'".to_string())
        );
        assert_eq!(
            parse_alias_output("quoted", r"quoted=\''foo'\'' bar'"),
            Some("'foo' bar".to_string())
        );
        assert_eq!(
            parse_alias_output("trailing", r"trailing='just'\'"),
            Some("just'".to_string())
        );

        // End to end: the operator's own alias, as their zsh prints it, must
        // reach the launch command with its final argument intact.
        let body = parse_alias_output(
            "claude2",
            r"claude2='CLAUDE_CONFIG_DIR=/homes/two claude --append-system-prompt '\''use account two'\'",
        )
        .expect("the pane shell's own output is an alias");
        let probe = FakeProbe::new(&[("claude2", &body)]);

        let resolved = resolve_base_command_in(
            "claude2 --dangerously-skip-permissions",
            CliTool::Claude,
            &probe,
            Some(Path::new("/home/operator")),
        );

        assert_eq!(
            resolved.command,
            "CLAUDE_CONFIG_DIR=/homes/two claude --append-system-prompt 'use account two' --dangerously-skip-permissions"
        );
        assert_eq!(resolved.opaque_head, None);
    }

    #[test]
    fn ignores_rc_noise_and_unparseable_alias_output() {
        assert_eq!(
            parse_alias_output(
                "claude2",
                "welcome back!\nclaude2='CLAUDE_CONFIG_DIR=/homes/two claude'"
            ),
            Some("CLAUDE_CONFIG_DIR=/homes/two claude".to_string())
        );
        assert_eq!(parse_alias_output("claude2", "claude2: not found"), None);
        assert_eq!(parse_alias_output("claude2", ""), None);
        assert_eq!(parse_alias_output("claude2", "other='claude'"), None);
        assert_eq!(parse_alias_output("weird", "weird=$'claude\\n--x'"), None);
    }

    #[test]
    fn the_cache_answers_for_a_minute_and_then_asks_again() {
        let probe = FakeProbe::new(&[("cached2", "CLAUDE_CONFIG_DIR=/homes/two claude")]);
        let start = Instant::now();

        let first =
            resolve_base_command_cached_at("cached2 --resume", CliTool::Claude, &probe, start);
        let second = resolve_base_command_cached_at(
            "cached2 --resume",
            CliTool::Claude,
            &probe,
            start + Duration::from_secs(59),
        );
        assert_eq!(first, second);
        assert_eq!(
            probe.asked(),
            vec!["cached2".to_string(), "claude".to_string()],
            "the second call reused the answer"
        );

        resolve_base_command_cached_at(
            "cached2 --resume",
            CliTool::Claude,
            &probe,
            start + Duration::from_secs(61),
        );
        assert_eq!(probe.asked().len(), 4, "an expired entry is asked again");
    }

    #[test]
    fn a_failed_probe_is_cached_too() {
        let probe = FakeProbe::new(&[]);
        let start = Instant::now();

        for _ in 0..3 {
            resolve_base_command_cached_at("uncached-tool --x", CliTool::Claude, &probe, start);
        }

        assert_eq!(
            probe.asked().len(),
            1,
            "a shell that said no is not re-asked"
        );
    }

    /// Regression: bc4457a put the resolution behind a daemon request whose
    /// timeout was the 5 s ping. Three probes of 5 s each plus finding the pane
    /// shell outlast that request, and a request that times out puts the
    /// literal base back — the alias goes invisible again and its own selector
    /// overrides the account the operator chose. The resolution is bounded so
    /// the caller can size a request around it.
    #[test]
    fn no_probe_answers_once_the_resolution_budget_is_spent() {
        let _aliases =
            install_alias_override(&[("claude2", "CLAUDE_CONFIG_DIR=/homes/two claude")]);

        let fresh = ShellAliasProbe::for_pane();
        assert_eq!(
            fresh.alias("claude2").as_deref(),
            Some("CLAUDE_CONFIG_DIR=/homes/two claude"),
            "a probe inside its budget asks the shell"
        );

        let spent = ShellAliasProbe::for_pane_until(Instant::now());
        assert_eq!(
            spent.alias("claude2"),
            None,
            "a probe past the budget never starts a shell it cannot wait for"
        );

        let resolved = resolve_base_command_in("claude2 --resume", CliTool::Claude, &spent, None);
        assert_eq!(
            resolved.command, "claude2 --resume",
            "an exhausted budget is fail-soft: the base is exactly as configured"
        );
        assert!(resolved.expansions.is_empty());
    }
}
