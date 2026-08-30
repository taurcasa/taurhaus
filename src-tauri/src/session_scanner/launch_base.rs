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

    let opaque_head = head_word(&command).filter(|head| !runs_the_tool(tool, head));
    ResolvedBase {
        command,
        expansions,
        opaque_head,
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

/// Asks the pane's own interactive shell what a word means.
pub struct ShellAliasProbe {
    shell: String,
}

impl ShellAliasProbe {
    /// The shell a launched pane runs: tmux's `default-shell`, else `$SHELL`.
    pub fn for_pane() -> Self {
        Self {
            shell: pane_shell(),
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
        is_alias_name(name).then(|| shell_alias(&self.shell, name))?
    }
}

/// Ask one interactive shell what a single validated word means.
#[cfg(not(test))]
fn shell_alias(shell: &str, name: &str) -> Option<String> {
    let script = format!("alias -- {name}");
    let output = super::process::run_with_timeout_within(shell, &["-ic", &script], PROBE_TIMEOUT)?;
    parse_alias_output(name, &output)
}

#[cfg(test)]
fn shell_alias(_shell: &str, name: &str) -> Option<String> {
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

fn unquote_alias_body(body: &str) -> Option<String> {
    let body = body.trim();
    if body.is_empty() || body.starts_with("$'") {
        // `$'...'` carries C escapes this parser does not speak.
        return None;
    }
    if let Some(inner) = body.strip_prefix('\'').and_then(|b| b.strip_suffix('\'')) {
        return Some(inner.replace("'\\''", "'"));
    }
    if let Some(inner) = body.strip_prefix('"').and_then(|b| b.strip_suffix('"')) {
        let mut unescaped = String::with_capacity(inner.len());
        let mut characters = inner.chars().peekable();
        while let Some(character) = characters.next() {
            match character {
                '\\' if matches!(characters.peek(), Some('"' | '\\' | '$' | '`')) => {
                    unescaped.extend(characters.next());
                }
                character => unescaped.push(character),
            }
        }
        return Some(unescaped);
    }
    (!body.contains(['\'', '"'])).then(|| body.to_string())
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

        let resolved = resolve_base_command(
            "claude2 --dangerously-skip-permissions",
            CliTool::Claude,
            &probe,
        );

        assert_eq!(
            resolved.command,
            "CLAUDE_CONFIG_DIR=~/.claude-account2 claude --dangerously-skip-permissions"
        );
        assert_eq!(
            resolved.expansions,
            vec![AliasExpansion {
                name: "claude2".to_string(),
                body: "CLAUDE_CONFIG_DIR=~/.claude-account2 claude".to_string(),
            }]
        );
        assert_eq!(resolved.opaque_head, None);
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
}
