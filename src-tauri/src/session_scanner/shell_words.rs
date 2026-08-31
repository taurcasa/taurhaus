//! One bounded shell-word tokenizer for launch-base classification.
//!
//! This reads quoting and escaping only. It never expands variables, tildes,
//! substitutions, globs, or aliases, and it never executes the command.

/// One shell word, decoded for comparison while retaining its source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    pub text: String,
    pub quoted: bool,
    pub start: usize,
    pub end: usize,
    name_quoted: bool,
}

impl Word {
    /// The assignment name when the shell reads this as `NAME=value`.
    ///
    /// A quote or escape that touches any part of `NAME=` makes the word a
    /// command head instead: `'NAME=value'` and `NA"ME"=value` are not
    /// assignments. Quotes and escapes after the unquoted `=` belong to the
    /// value and do not change the classification (`NAME='two words'`). This
    /// is the quoted-span rule shared by every launch-base consumer.
    pub fn assignment_name(&self) -> Option<&str> {
        if self.name_quoted {
            return None;
        }
        let (name, _) = self.text.split_once('=')?;
        (!name.is_empty()
            && name
                .starts_with(|character: char| character.is_ascii_alphabetic() || character == '_')
            && name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_'))
        .then_some(name)
    }

    pub fn is_assignment(&self) -> bool {
        self.assignment_name().is_some()
    }
}

/// Split a command into shell words without expanding or executing anything.
pub fn words(command: &str) -> Vec<Word> {
    let mut words = Vec::new();
    let mut text = String::new();
    let mut start = 0usize;
    let mut started = false;
    let mut quoted = false;
    let mut name_quoted = false;
    let mut characters = command.char_indices().peekable();

    while let Some((index, character)) = characters.next() {
        match character {
            character if character.is_whitespace() => {
                if started {
                    words.push(Word {
                        text: std::mem::take(&mut text),
                        quoted,
                        start,
                        end: index,
                        name_quoted,
                    });
                    started = false;
                    quoted = false;
                    name_quoted = false;
                }
            }
            '\'' => {
                if !started {
                    start = index;
                    started = true;
                }
                quoted = true;
                name_quoted |= !text.contains('=');
                for (_, character) in characters.by_ref() {
                    if character == '\'' {
                        break;
                    }
                    text.push(character);
                }
            }
            '"' => {
                if !started {
                    start = index;
                    started = true;
                }
                quoted = true;
                name_quoted |= !text.contains('=');
                while let Some((_, character)) = characters.next() {
                    match character {
                        '"' => break,
                        '\\' if matches!(characters.peek(), Some((_, '\n'))) => {
                            characters.next();
                        }
                        '\\' if matches!(characters.peek(), Some((_, '"' | '\\' | '$' | '`'))) => {
                            text.extend(characters.next().map(|(_, character)| character));
                        }
                        character => text.push(character),
                    }
                }
            }
            '\\' => {
                let escaped = characters.next();
                if matches!(escaped, Some((_, '\n'))) {
                    continue;
                }
                if !started {
                    start = index;
                    started = true;
                }
                quoted = true;
                name_quoted |= !text.contains('=');
                text.extend(escaped.map(|(_, character)| character));
            }
            character => {
                if !started {
                    start = index;
                    started = true;
                }
                text.push(character);
            }
        }
    }

    if started {
        words.push(Word {
            text,
            quoted,
            start,
            end: command.len(),
            name_quoted,
        });
    }
    words
}

/// The leading words a shell places in the command environment.
pub fn leading_assignments(command: &str) -> Vec<Word> {
    words(command)
        .into_iter()
        .take_while(Word::is_assignment)
        .collect()
}

/// The last leading assignment for `name`, which is the value the shell uses.
pub fn assignment_in_force(command: &str, name: &str) -> Option<Word> {
    leading_assignments(command)
        .into_iter()
        .rfind(|word| word.assignment_name() == Some(name))
}

/// The first word that is not a leading assignment.
pub fn head(command: &str) -> Option<Word> {
    words(command)
        .into_iter()
        .find(|word| !word.is_assignment())
}

#[cfg(test)]
mod tests {
    use super::{assignment_in_force, head, leading_assignments, words};

    #[derive(Debug)]
    struct CorpusCase {
        command: &'static str,
        assignments: &'static [&'static str],
        selector: Option<&'static str>,
        head: Option<(&'static str, bool)>,
    }

    // Regression: commit 89e73bd added a second assignment tokenizer beside
    // the one from 0f2bfbb, leaving quoted heads and escaped words classified
    // differently by launch-base, account, and renderer consumers.
    #[test]
    fn every_consumer_classifies_the_shell_word_corpus_identically() {
        let cases = [
            CorpusCase {
                command: r#"SELECTOR='two words' claude --flag"#,
                assignments: &["SELECTOR=two words"],
                selector: Some("SELECTOR=two words"),
                head: Some(("claude", false)),
            },
            CorpusCase {
                command: r#"SELECTOR=two\ words claude"#,
                assignments: &["SELECTOR=two words"],
                selector: Some("SELECTOR=two words"),
                head: Some(("claude", false)),
            },
            CorpusCase {
                command: "SELECTOR=one\\\ntwo claude",
                assignments: &["SELECTOR=onetwo"],
                selector: Some("SELECTOR=onetwo"),
                head: Some(("claude", false)),
            },
            CorpusCase {
                command: r#"SELECTOR='it'\''s' claude"#,
                assignments: &["SELECTOR=it's"],
                selector: Some("SELECTOR=it's"),
                head: Some(("claude", false)),
            },
            CorpusCase {
                command: "A=1 B=2 claude A=argument",
                assignments: &["A=1", "B=2"],
                selector: None,
                head: Some(("claude", false)),
            },
            CorpusCase {
                command: "SELECTOR=first OTHER=x SELECTOR=last claude",
                assignments: &["SELECTOR=first", "OTHER=x", "SELECTOR=last"],
                selector: Some("SELECTOR=last"),
                head: Some(("claude", false)),
            },
            CorpusCase {
                command: "SELECTOR=one ./opaque-wrapper --flag",
                assignments: &["SELECTOR=one"],
                selector: Some("SELECTOR=one"),
                head: Some(("./opaque-wrapper", false)),
            },
            CorpusCase {
                command: "SELECTOR=one 'claude' --flag",
                assignments: &["SELECTOR=one"],
                selector: Some("SELECTOR=one"),
                head: Some(("claude", true)),
            },
            CorpusCase {
                command: "'SELECTOR=not-env' claude",
                assignments: &[],
                selector: None,
                head: Some(("SELECTOR=not-env", true)),
            },
            CorpusCase {
                command: "SELECTOR=~/.tool claude",
                assignments: &["SELECTOR=~/.tool"],
                selector: Some("SELECTOR=~/.tool"),
                head: Some(("claude", false)),
            },
            CorpusCase {
                command: r#"SELECTOR=C:\\Users\\Example claude"#,
                assignments: &[r#"SELECTOR=C:\Users\Example"#],
                selector: Some(r#"SELECTOR=C:\Users\Example"#),
                head: Some(("claude", false)),
            },
            CorpusCase {
                command: r#"SELECTOR='C:\Users\Example User\.tool' 'C:\Program Files\tool.exe'"#,
                assignments: &[r#"SELECTOR=C:\Users\Example User\.tool"#],
                selector: Some(r#"SELECTOR=C:\Users\Example User\.tool"#),
                head: Some((r#"C:\Program Files\tool.exe"#, true)),
            },
            CorpusCase {
                command: "A=1 SELECTOR=two",
                assignments: &["A=1", "SELECTOR=two"],
                selector: Some("SELECTOR=two"),
                head: None,
            },
        ];

        for case in cases {
            // The module's own classification against the table.
            let leading = leading_assignments(case.command);
            let renderer_assignments = leading
                .iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>();
            assert_eq!(renderer_assignments, case.assignments, "{case:?}");
            let for_accounts = assignment_in_force(case.command, "SELECTOR");
            assert_eq!(
                for_accounts.as_ref().map(|word| word.text.as_str()),
                case.selector,
                "{case:?}"
            );
            assert_eq!(
                head(case.command)
                    .as_ref()
                    .map(|word| (word.text.as_str(), word.quoted)),
                case.head,
                "{case:?}"
            );

            // launch_base.rs's real consumer: the head it resolves.
            assert_eq!(
                crate::session_scanner::launch_base::head_word(case.command).as_deref(),
                case.head.map(|(text, _)| text),
                "{case:?}"
            );

            // accounts/mod.rs's real consumer: the selector in force.
            let account_value =
                crate::session_scanner::accounts::command_env_assignment(case.command, "SELECTOR");
            assert_eq!(account_value.is_some(), case.selector.is_some(), "{case:?}");
            if let (Some(Some(path)), Some(selector)) = (&account_value, case.selector) {
                let (_, raw_value) = selector.split_once('=').expect("selector assignment");
                if !raw_value.trim().starts_with('~') {
                    assert_eq!(
                        path,
                        &std::path::PathBuf::from(raw_value.trim()),
                        "{case:?}"
                    );
                }
            }

            // launch.rs's real consumer: rewriting the selector in force
            // leaves exactly one, and it is the new one.
            if case.selector.is_some() {
                let (rewritten, _) = crate::session_scanner::launch::replace_env_assignment(
                    case.command,
                    "SELECTOR",
                    "SELECTOR='/rewritten'",
                )
                .expect("a selector in force can be rewritten");
                let remaining = leading_assignments(&rewritten)
                    .into_iter()
                    .filter(|word| word.assignment_name() == Some("SELECTOR"))
                    .count();
                assert_eq!(remaining, 1, "{rewritten}");
                assert_eq!(
                    assignment_in_force(&rewritten, "SELECTOR").map(|word| word.text),
                    Some("SELECTOR=/rewritten".to_string()),
                    "{rewritten}"
                );
            }
        }
    }

    #[test]
    fn tokenizes_shell_words_with_decoded_text_and_original_spans() {
        let command = r#"  NAME='two words' clau"de" a\ b 'it'\''s'"#;

        let parsed = words(command);

        assert_eq!(
            parsed
                .iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>(),
            vec!["NAME=two words", "claude", "a b", "it's"]
        );
        assert_eq!(
            parsed
                .iter()
                .map(|word| &command[word.start..word.end])
                .collect::<Vec<_>>(),
            vec![
                r#"NAME='two words'"#,
                r#"clau"de""#,
                r#"a\ b"#,
                r#"'it'\''s'"#
            ]
        );
        assert!(parsed.iter().all(|word| word.quoted));
    }

    #[test]
    fn a_quoted_assignment_shaped_head_is_not_an_assignment() {
        let command = "'NAME=value' claude";

        assert!(leading_assignments(command).is_empty());
        let command_head = head(command).expect("quoted command head");
        assert_eq!(command_head.text, "NAME=value");
        assert!(command_head.quoted);
    }

    #[test]
    fn assignment_in_force_is_the_last_matching_leading_assignment() {
        let command = "NAME=one OTHER=x NAME='two words' claude NAME=argument";

        let assignment = assignment_in_force(command, "NAME").expect("leading NAME assignment");

        assert_eq!(assignment.text, "NAME=two words");
        assert_eq!(
            &command[assignment.start..assignment.end],
            "NAME='two words'"
        );
        assert_eq!(
            head(command).map(|word| word.text),
            Some("claude".to_string())
        );
    }
}
