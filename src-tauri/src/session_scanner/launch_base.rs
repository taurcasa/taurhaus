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
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime};

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
    let expansions = probe_alias_expansions(base, probe);
    resolve_base_command_from_expansions_in(base, tool, &expansions, home)
}

fn probe_alias_expansions(base: &str, probe: &dyn AliasProbe) -> Vec<AliasExpansion> {
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
    expansions
}

fn resolve_base_command_from_expansions_in(
    base: &str,
    tool: CliTool,
    expansions: &[AliasExpansion],
    home: Option<&Path>,
) -> ResolvedBase {
    let mut command = base.to_string();
    let mut applied = Vec::with_capacity(expansions.len());
    for expansion in expansions {
        let Some(head) = words(&command)
            .into_iter()
            .find(|word| !word.is_assignment())
        else {
            break;
        };
        if head.quoted || head.value != expansion.name {
            break;
        }
        command.replace_range(head.start..head.end, &expansion.body);
        applied.push(expansion.clone());
    }

    let command = expand_selector_home(&command, tool, home);
    let opaque_head = head_word(&command).filter(|head| !runs_the_tool(tool, head));
    ResolvedBase {
        command,
        expansions: applied,
        opaque_head,
    }
}

/// Rewrite this tool's leading `SELECTOR=~/…` assignments as absolute paths.
///
/// Resolution runs where the pane shell runs — in the WSL daemon on Windows —
/// so `home` is the home that shell would expand the tilde against. The app
/// never has to guess it from its own side of the boundary, which it cannot do:
/// a Windows profile path names none of the accounts the daemon detects. Every
/// consumer downstream compares this selector against absolute account dirs.
///
/// Only the leading assignment run is rewritten, and only when the value is one
/// the shell would have nothing left to do with: past the command name every
/// word is an argument the program receives verbatim, and a value carrying an
/// expansion is the shell's to read, not this module's to freeze.
fn expand_selector_home(command: &str, tool: CliTool, home: Option<&Path>) -> String {
    let (Some(selector), Some(home)) = (spec(tool).capabilities.account_selector, home) else {
        return command.to_string();
    };
    let prefix = format!("{selector}=");
    let mut rewritten = command.to_string();
    // Back to front, so each remaining word keeps its span in the original.
    for word in leading_assignments(command).into_iter().rev() {
        // A shell leaves a quoted tilde literal, and so does this.
        if word.quoted {
            continue;
        }
        let Some(value) = word.value.strip_prefix(&prefix) else {
            continue;
        };
        // `~/${PROFILE}` names a directory only the launching shell can name.
        // Substituting the home would mean quoting the whole value, which stops
        // that shell expanding the rest, so the word goes through as typed and
        // the account the user chose is rendered in front of it instead.
        if value.contains(['$', '`']) {
            continue;
        }
        let Some(expanded) = expand_tilde(value, home) else {
            continue;
        };
        rewritten.replace_range(
            word.start..word.end,
            &format!("{prefix}{}", super::launch::shell_escape(&expanded)),
        );
    }
    rewritten
}

/// The run of `NAME=value` words a shell puts in the environment: the ones in
/// front of the command name.
fn leading_assignments(command: &str) -> Vec<Word> {
    let mut words = words(command);
    let assignments = words
        .iter()
        .position(|word| !word.is_assignment())
        .unwrap_or(words.len());
    words.truncate(assignments);
    words
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

/// Resolve `base`, reusing the alias chain for this shell and head word.
///
/// Probing runs an interactive shell, so a launch, its preview and the settings
/// page must not each pay for one.
pub fn resolve_base_command_cached(
    base: &str,
    tool: CliTool,
    probe: &dyn AliasProbe,
) -> ResolvedBase {
    resolve_base_command_cached_at(base, tool, probe, Instant::now())
}

/// Shell answers remain useful until an rc file changes, with a hard upper cap.
const CACHE_TTL: Duration = Duration::from_secs(10 * 60);
/// A missing/failed alias answer self-heals quickly after a transient timeout.
const NEGATIVE_CACHE_TTL: Duration = Duration::from_secs(60);

type CacheKey = (u64, String, String);

#[derive(Clone, PartialEq, Eq)]
struct RcFingerprint(Vec<(PathBuf, Option<SystemTime>)>);

struct CachedAliasChain {
    observed_at: Instant,
    rc_fingerprint: RcFingerprint,
    resolution: Arc<InFlightAliasChain>,
}

impl CachedAliasChain {
    fn ttl(&self) -> Duration {
        let result = self
            .resolution
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match result.as_deref() {
            Some([]) => NEGATIVE_CACHE_TTL,
            _ => CACHE_TTL,
        }
    }
}

#[derive(Default)]
struct InFlightAliasChain {
    result: Mutex<Option<Vec<AliasExpansion>>>,
    ready: Condvar,
}

impl InFlightAliasChain {
    fn complete(&self, expansions: Vec<AliasExpansion>) {
        let mut result = self
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        *result = Some(expansions);
        self.ready.notify_all();
    }

    fn wait_for(&self, timeout: Duration) -> Vec<AliasExpansion> {
        let started = Instant::now();
        let mut result = self
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        while result.is_none() {
            let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
                return Vec::new();
            };
            let (next, wait) = self
                .ready
                .wait_timeout(result, remaining)
                .unwrap_or_else(|error| error.into_inner());
            result = next;
            if wait.timed_out() && result.is_none() {
                return Vec::new();
            }
        }
        result.clone().unwrap_or_default()
    }
}

static CACHE_GENERATION: AtomicU64 = AtomicU64::new(0);
static CACHE: LazyLock<Mutex<HashMap<CacheKey, CachedAliasChain>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// A successful settings save makes every earlier shell answer stale.
pub fn invalidate_base_command_cache() {
    CACHE_GENERATION.fetch_add(1, Ordering::AcqRel);
}

fn resolve_base_command_cached_at(
    base: &str,
    tool: CliTool,
    probe: &dyn AliasProbe,
    now: Instant,
) -> ResolvedBase {
    resolve_base_command_cached_at_in(base, tool, probe, now, dirs::home_dir().as_deref())
}

fn resolve_base_command_cached_at_in(
    base: &str,
    tool: CliTool,
    probe: &dyn AliasProbe,
    now: Instant,
    home: Option<&Path>,
) -> ResolvedBase {
    resolve_base_command_cached_at_in_generation(
        base,
        tool,
        probe,
        now,
        home,
        CACHE_GENERATION.load(Ordering::Acquire),
    )
}

fn resolve_base_command_cached_at_in_generation(
    base: &str,
    tool: CliTool,
    probe: &dyn AliasProbe,
    now: Instant,
    home: Option<&Path>,
    generation: u64,
) -> ResolvedBase {
    let Some(head) = alias_head(base) else {
        return resolve_base_command_from_expansions_in(base, tool, &[], home);
    };
    let key = (generation, probe.shell().to_string(), head);
    let rc_fingerprint = shell_rc_fingerprint(probe.shell(), home);
    let (resolution, owns_probe) = {
        let mut cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
        let reusable = cache.get(&key).filter(|entry| {
            entry.rc_fingerprint == rc_fingerprint
                && now
                    .checked_duration_since(entry.observed_at)
                    .is_some_and(|age| age < entry.ttl())
        });
        if let Some(entry) = reusable {
            (Arc::clone(&entry.resolution), false)
        } else {
            let resolution = Arc::new(InFlightAliasChain::default());
            cache.insert(
                key,
                CachedAliasChain {
                    observed_at: now,
                    rc_fingerprint,
                    resolution: Arc::clone(&resolution),
                },
            );
            let actual_now = Instant::now();
            cache.retain(|_, entry| {
                actual_now
                    .checked_duration_since(entry.observed_at)
                    .is_none_or(|age| age < CACHE_TTL)
            });
            (resolution, true)
        }
    };

    if owns_probe {
        // The shell runs outside the map lock; joiners wait only on this head.
        let expansions = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            probe_alias_expansions(base, probe)
        }))
        .unwrap_or_default();
        resolution.complete(expansions);
    }
    let expansions = resolution.wait_for(RESOLUTION_BUDGET);
    resolve_base_command_from_expansions_in(base, tool, &expansions, home)
}

fn alias_head(command: &str) -> Option<String> {
    let head = words(command)
        .into_iter()
        .find(|word| !word.is_assignment())?;
    (!head.quoted && is_alias_name(&head.value)).then_some(head.value)
}

fn shell_rc_fingerprint(shell: &str, home: Option<&Path>) -> RcFingerprint {
    let Some(home) = home else {
        return RcFingerprint(Vec::new());
    };
    let shell = Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(shell);
    let relative = match shell {
        "zsh" => Some(".zshrc"),
        "bash" => Some(".bashrc"),
        "fish" => Some(".config/fish/config.fish"),
        "ksh" | "mksh" => Some(".kshrc"),
        "csh" => Some(".cshrc"),
        "tcsh" => Some(".tcshrc"),
        _ => None,
    };
    RcFingerprint(
        relative
            .map(|relative| {
                let path = home.join(relative);
                let modified = std::fs::metadata(&path)
                    .and_then(|metadata| metadata.modified())
                    .ok();
                (path, modified)
            })
            .into_iter()
            .collect(),
    )
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
        invalidate_base_command_cache();
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
    invalidate_base_command_cache();
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
    fn a_selector_value_the_shell_expands_is_left_for_the_shell() {
        // Regression: a3afcfe substituted the home into every selector word and
        // shell-quoted the whole value, so `CLAUDE_CONFIG_DIR=~/${PROFILE}`
        // resolved to `CLAUDE_CONFIG_DIR='/home/mstie/${PROFILE}'`. The quotes
        // stop the shell expanding PROFILE, so the launch ran on a literal
        // directory of that name and silently left the chosen account behind.
        let probe = FakeProbe::new(&[]);

        let resolved = resolve_base_command_in(
            "CLAUDE_CONFIG_DIR=~/${PROFILE} claude --resume",
            CliTool::Claude,
            &probe,
            Some(Path::new("/home/mstie")),
        );

        assert_eq!(
            resolved.command, "CLAUDE_CONFIG_DIR=~/${PROFILE} claude --resume",
            "only the shell can read this value, so it reaches the shell as typed"
        );
    }

    #[test]
    fn a_selector_shaped_argument_keeps_its_tilde() {
        // Regression: a3afcfe normalized this tool's selector in every word of
        // the line, so the argument in `claude --append-system-prompt
        // CLAUDE_CONFIG_DIR=~/.literal` was rewritten to an absolute path the
        // user never typed. A shell hands every word past the command name to
        // the program verbatim, tilde and all.
        let probe = FakeProbe::new(&[]);

        let resolved = resolve_base_command_in(
            "claude --append-system-prompt CLAUDE_CONFIG_DIR=~/.literal",
            CliTool::Claude,
            &probe,
            Some(Path::new("/home/mstie")),
        );

        assert_eq!(
            resolved.command,
            "claude --append-system-prompt CLAUDE_CONFIG_DIR=~/.literal"
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
    fn the_cache_answers_for_ten_minutes_and_then_asks_again() {
        let home = tempfile::tempdir().expect("temporary shell home");
        let probe = FakeProbe::new(&[("cached2", "CLAUDE_CONFIG_DIR=/homes/two claude")]);
        let start = Instant::now();

        let first = resolve_base_command_cached_at_in_generation(
            "cached2 --resume",
            CliTool::Claude,
            &probe,
            start,
            Some(home.path()),
            u64::MAX - 10,
        );
        let second = resolve_base_command_cached_at_in_generation(
            "cached2 --resume",
            CliTool::Claude,
            &probe,
            start + Duration::from_secs(599),
            Some(home.path()),
            u64::MAX - 10,
        );
        assert_eq!(first, second);
        assert_eq!(
            probe.asked(),
            vec!["cached2".to_string(), "claude".to_string()],
            "the second call reused the answer"
        );

        resolve_base_command_cached_at_in_generation(
            "cached2 --resume",
            CliTool::Claude,
            &probe,
            start + Duration::from_secs(601),
            Some(home.path()),
            u64::MAX - 10,
        );
        assert_eq!(probe.asked().len(), 4, "an expired entry is asked again");
    }

    // Regression: 3c5b6cd9 let cache contract tests read the process-global
    // generation while alias-override tests changed it in parallel. An
    // unrelated bump then looked like a cache miss and made this lane flaky.
    #[test]
    fn an_isolated_cache_test_ignores_the_ambient_generation() {
        let _serial = ALIAS_OVERRIDE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let home = tempfile::tempdir().expect("temporary shell home");
        let probe = FakeProbe::new(&[("isolated2", "claude")]);
        let start = Instant::now();

        resolve_base_command_cached_at_in_generation(
            "isolated2 --fresh",
            CliTool::Claude,
            &probe,
            start,
            Some(home.path()),
            u64::MAX - 11,
        );
        invalidate_base_command_cache();
        resolve_base_command_cached_at_in_generation(
            "isolated2 --resume",
            CliTool::Claude,
            &probe,
            start + Duration::from_secs(1),
            Some(home.path()),
            u64::MAX - 11,
        );

        assert_eq!(
            probe.asked(),
            vec!["isolated2", "claude"],
            "another test's cache generation must not disturb this contract"
        );
    }

    // Regression: 0.8.4 / PR #75 keyed shell resolution by the whole launch
    // command, so fresh, continue and resume probed the same `claude2` alias
    // separately during every Settings refresh.
    #[test]
    fn commands_that_share_a_head_share_one_probe() {
        let home = tempfile::tempdir().expect("temporary shell home");
        let probe = FakeProbe::new(&[("shared2", "CLAUDE_CONFIG_DIR=/homes/two claude")]);
        let start = Instant::now();

        let fresh = resolve_base_command_cached_at_in_generation(
            "shared2 --fresh",
            CliTool::Claude,
            &probe,
            start,
            Some(home.path()),
            u64::MAX - 12,
        );
        let continued = resolve_base_command_cached_at_in_generation(
            "shared2 --continue",
            CliTool::Claude,
            &probe,
            start,
            Some(home.path()),
            u64::MAX - 12,
        );
        let resumed = resolve_base_command_cached_at_in_generation(
            "shared2 --resume session-1",
            CliTool::Claude,
            &probe,
            start,
            Some(home.path()),
            u64::MAX - 12,
        );

        assert!(fresh.command.ends_with("claude --fresh"), "{fresh:?}");
        assert!(
            continued.command.ends_with("claude --continue"),
            "{continued:?}"
        );
        assert!(
            resumed.command.ends_with("claude --resume session-1"),
            "{resumed:?}"
        );
        assert_eq!(
            probe.asked(),
            vec!["shared2".to_string(), "claude".to_string()],
            "one head under one shell must cost one alias-chain probe"
        );
    }

    // Regression: 0.8.4 / PR #75 cached shell answers for a fixed 60 seconds,
    // leaving a changed alias stale and then paying for unchanged rc files on
    // the next account refresh.
    #[test]
    fn changing_the_shell_rc_mtime_invalidates_the_cached_head() {
        let home = tempfile::tempdir().expect("temporary shell home");
        let rc = home.path().join(".zshrc");
        std::fs::write(&rc, "alias rc2=claude").expect("write initial rc");
        let initial_mtime = std::fs::metadata(&rc)
            .and_then(|metadata| metadata.modified())
            .expect("initial rc mtime");
        let probe = FakeProbe::new(&[("rc2", "claude")]);
        let start = Instant::now();

        resolve_base_command_cached_at_in_generation(
            "rc2 --fresh",
            CliTool::Claude,
            &probe,
            start,
            Some(home.path()),
            u64::MAX - 13,
        );
        let changed_mtime = initial_mtime + Duration::from_secs(2);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&rc)
            .and_then(|file| file.set_times(std::fs::FileTimes::new().set_modified(changed_mtime)))
            .expect("set deterministic changed rc mtime");

        resolve_base_command_cached_at_in_generation(
            "rc2 --resume",
            CliTool::Claude,
            &probe,
            start + Duration::from_secs(1),
            Some(home.path()),
            u64::MAX - 13,
        );

        assert_eq!(
            probe.asked(),
            vec!["rc2", "claude", "rc2", "claude"],
            "a changed rc file must be observed before the ten-minute cap"
        );
    }

    // Regression: 0.8.4 / PR #75 kept resolved aliases after the operator
    // saved a different CLI command, so Settings could describe the command it
    // had just replaced until the old 60-second entry expired.
    #[test]
    fn settings_invalidation_discards_cached_alias_answers() {
        let _serial = ALIAS_OVERRIDE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let home = tempfile::tempdir().expect("temporary shell home");
        let probe = FakeProbe::new(&[("saved2", "claude")]);
        let start = Instant::now();

        resolve_base_command_cached_at_in(
            "saved2 --fresh",
            CliTool::Claude,
            &probe,
            start,
            Some(home.path()),
        );
        invalidate_base_command_cache();
        resolve_base_command_cached_at_in(
            "saved2 --resume",
            CliTool::Claude,
            &probe,
            start + Duration::from_secs(1),
            Some(home.path()),
        );

        assert_eq!(
            probe.asked(),
            vec!["saved2", "claude", "saved2", "claude"],
            "a settings save must make the earlier alias answer unreachable"
        );
    }

    struct BlockingProbe {
        entered: std::sync::mpsc::Sender<()>,
        released: std::sync::Mutex<bool>,
        release_changed: std::sync::Condvar,
    }

    impl BlockingProbe {
        fn release(&self) {
            *self.released.lock().expect("release lock") = true;
            self.release_changed.notify_all();
        }
    }

    impl AliasProbe for BlockingProbe {
        fn shell(&self) -> &str {
            "/fake/zsh"
        }

        fn alias(&self, name: &str) -> Option<String> {
            if name != "joined2" {
                return None;
            }
            self.entered.send(()).expect("report entered probe");
            let mut released = self.released.lock().expect("release lock");
            while !*released {
                released = self.release_changed.wait(released).expect("release wait");
            }
            Some("claude".to_string())
        }
    }

    struct PanickingProbe;

    impl AliasProbe for PanickingProbe {
        fn shell(&self) -> &str {
            "/fake/zsh"
        }

        fn alias(&self, _name: &str) -> Option<String> {
            panic!("synthetic probe failure")
        }
    }

    // Regression: 3c5b6cd9 published an in-flight cache entry before probing,
    // but a probe panic never completed it. Every later caller for that head
    // then waited forever instead of receiving the normal fail-soft literal.
    #[test]
    fn a_panicking_probe_completes_the_cache_fail_soft() {
        let home = tempfile::tempdir().expect("temporary shell home");

        let resolved = resolve_base_command_cached_at_in_generation(
            "panic2 --fresh",
            CliTool::Claude,
            &PanickingProbe,
            Instant::now(),
            Some(home.path()),
            u64::MAX - 15,
        );

        assert_eq!(resolved.command, "panic2 --fresh");
        assert!(resolved.expansions.is_empty());
    }

    // Regression: 3c5b6cd9 made joiners use an unbounded Condvar wait. If an
    // owner disappeared without completing its entry, an IPC worker could be
    // retained forever instead of returning the fail-soft literal answer.
    #[test]
    fn an_abandoned_in_flight_probe_has_a_bounded_wait() {
        let resolution = InFlightAliasChain::default();
        let started = Instant::now();

        let expansions = resolution.wait_for(Duration::from_millis(2));

        assert!(expansions.is_empty());
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    // Regression: 0.8.4 / PR #75 probed outside the cache lock without an
    // in-flight entry, so concurrent callers duplicated the same slow shell.
    #[test]
    fn a_probe_in_flight_is_joined() {
        let home = tempfile::tempdir().expect("temporary shell home");
        let home = std::sync::Arc::new(home);
        let (entered, observed) = std::sync::mpsc::channel();
        let probe = std::sync::Arc::new(BlockingProbe {
            entered,
            released: std::sync::Mutex::new(false),
            release_changed: std::sync::Condvar::new(),
        });
        let start = Instant::now();

        let first_probe = std::sync::Arc::clone(&probe);
        let first_home = std::sync::Arc::clone(&home);
        let first = std::thread::spawn(move || {
            resolve_base_command_cached_at_in_generation(
                "joined2 --fresh",
                CliTool::Claude,
                first_probe.as_ref(),
                start,
                Some(first_home.path()),
                u64::MAX - 1,
            )
        });
        observed
            .recv_timeout(Duration::from_secs(1))
            .expect("first probe started");

        let second_probe = std::sync::Arc::clone(&probe);
        let second_home = std::sync::Arc::clone(&home);
        let second = std::thread::spawn(move || {
            resolve_base_command_cached_at_in_generation(
                "joined2 --resume",
                CliTool::Claude,
                second_probe.as_ref(),
                start,
                Some(second_home.path()),
                u64::MAX - 1,
            )
        });
        let duplicated = observed.recv_timeout(Duration::from_millis(150)).is_ok();
        probe.release();

        let first = first.join().expect("first resolution");
        let second = second.join().expect("joined resolution");
        assert!(!duplicated, "the second caller started a duplicate probe");
        assert!(first.command.ends_with("claude --fresh"), "{first:?}");
        assert!(second.command.ends_with("claude --resume"), "{second:?}");
    }

    // Regression: 3c5b6cd9 gave an empty result the same ten-minute lifetime
    // as a resolved alias. A transient shell timeout on Windows therefore
    // ignored Retry and kept the literal command stale for ten minutes.
    #[test]
    fn a_failed_probe_is_cached_too() {
        let home = tempfile::tempdir().expect("temporary shell home");
        let probe = FakeProbe::new(&[]);
        let start = Instant::now();

        resolve_base_command_cached_at_in_generation(
            "uncached-tool --x",
            CliTool::Claude,
            &probe,
            start,
            Some(home.path()),
            u64::MAX - 14,
        );
        resolve_base_command_cached_at_in_generation(
            "uncached-tool --x",
            CliTool::Claude,
            &probe,
            start + Duration::from_secs(59),
            Some(home.path()),
            u64::MAX - 14,
        );
        resolve_base_command_cached_at_in_generation(
            "uncached-tool --x",
            CliTool::Claude,
            &probe,
            start + Duration::from_secs(61),
            Some(home.path()),
            u64::MAX - 14,
        );

        assert_eq!(
            probe.asked().len(),
            2,
            "a failed shell answer is retried after the old one-minute TTL"
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
