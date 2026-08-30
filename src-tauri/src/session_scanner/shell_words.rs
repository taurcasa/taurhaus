//! One bounded shell-word tokenizer for launch-base classification.
//!
//! This reads quoting and escaping only. It never expands variables, tildes,
//! substitutions, globs, or aliases, and it never executes the command.

/// One shell word, decoded for comparison while retaining its source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Word {
    pub(crate) text: String,
    pub(crate) quoted: bool,
    pub(crate) start: usize,
    pub(crate) end: usize,
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
    pub(crate) fn assignment_name(&self) -> Option<&str> {
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

    pub(crate) fn is_assignment(&self) -> bool {
        self.assignment_name().is_some()
    }
}

/// Split a command into shell words without expanding or executing anything.
pub(crate) fn words(command: &str) -> Vec<Word> {
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
                        '\\' if matches!(characters.peek(), Some((_, '"' | '\\' | '$' | '`'))) => {
                            text.extend(characters.next().map(|(_, character)| character));
                        }
                        character => text.push(character),
                    }
                }
            }
            '\\' => {
                if !started {
                    start = index;
                    started = true;
                }
                quoted = true;
                name_quoted |= !text.contains('=');
                text.extend(characters.next().map(|(_, character)| character));
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
pub(crate) fn leading_assignments(command: &str) -> Vec<Word> {
    words(command)
        .into_iter()
        .take_while(Word::is_assignment)
        .collect()
}

/// The last leading assignment for `name`, which is the value the shell uses.
pub(crate) fn assignment_in_force(command: &str, name: &str) -> Option<Word> {
    leading_assignments(command)
        .into_iter()
        .filter(|word| word.assignment_name() == Some(name))
        .last()
}

/// The first word that is not a leading assignment.
pub(crate) fn head(command: &str) -> Option<Word> {
    words(command)
        .into_iter()
        .find(|word| !word.is_assignment())
}

#[cfg(test)]
mod tests {
    use super::{assignment_in_force, head, leading_assignments, words};

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
