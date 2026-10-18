// SPDX-License-Identifier: MIT
//! Deliberately conservative local cleanup. Ambiguous self-corrections stay intact.
use crate::model::*;
use regex::{Captures, Regex};
use std::sync::LazyLock;
static PROTECTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://\S+|www\.\S+|[\p{L}\p{N}._%+\-]+@[\p{L}\p{N}.\-]+\.[\p{L}]{2,}|`[^`\n]*`|(?:[A-Za-z]:\\|/)[^\s]+|\b[A-Za-z_][A-Za-z0-9]*_[A-Za-z0-9_]*\b|\b[a-z][a-z0-9]*[A-Z][A-Za-z0-9]*\b|\b[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)+\b|\b[A-Za-z_][A-Za-z0-9_]*\.[A-Za-z_][A-Za-z0-9_.]*\b").unwrap()
});
static FILLER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:um+|uh+|erm)\b[, ]*").unwrap());
static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\S\r\n]{2,}").unwrap());
static BEFORE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+([,.!?;:])").unwrap());
static REVISION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(Monday|Tuesday|Wednesday|Thursday|Friday|Saturday|Sunday)(?:,?\s+actually[,]?\s+)(Monday|Tuesday|Wednesday|Thursday|Friday|Saturday|Sunday)\b").unwrap()
});
static AGAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([\p{L}]+)\s+([\p{L}]+)\b").unwrap());
static COMMANDS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"(?i)\bnew paragraph\b", "\n\n"),
        (r"(?i)\bnew line\b", "\n"),
        (r"(?i)\b(?:full stop|period mark)\b", "."),
        (r"(?i)\bcomma mark\b", ","),
        (r"(?i)\bquestion mark\b", "?"),
        (r"(?i)\bexclamation mark\b", "!"),
        (r"(?i)\b(?:bullet point|next bullet)\b", "\n• "),
    ]
    .into_iter()
    .map(|(a, b)| (Regex::new(a).unwrap(), b))
    .collect()
});

pub fn local_mode(mode: &Mode, explicit: &str, detected: Option<&str>) -> Mode {
    let english = if explicit.is_empty() {
        detected.is_some_and(|v| v.eq_ignore_ascii_case("en") || v.eq_ignore_ascii_case("english"))
    } else {
        explicit == "en"
    };
    if english {
        mode.clone()
    } else {
        Mode::Literal
    }
}
pub fn normalize(input: &str, mode: &Mode, entries: &[Entry]) -> String {
    let mut out = input.trim().to_owned();
    // Commands are opt-in by explicit phrases; ordinary words like "period" remain literal.
    if *mode != Mode::Literal {
        let mut protected = Vec::new();
        let mut prefix = "\u{e000}".to_string();
        while out.contains(&prefix) {
            prefix.push('\u{e000}');
        }
        out = PROTECTED
            .replace_all(&out, |capture: &Captures| {
                let index = protected.len();
                protected.push(capture[0].to_string());
                format!("{prefix}{index}\u{e001}")
            })
            .into_owned();
        for (re, to) in COMMANDS.iter() {
            out = re.replace_all(&out, *to).into_owned();
        }
        out = FILLER.replace_all(&out, "").into_owned();
        out = REVISION.replace_all(&out, "$2").into_owned();
        out = AGAIN
            .replace_all(&out, |c: &Captures| {
                // "had had", "that that", emphatic "very very" can be grammatical.
                if c[1].eq_ignore_ascii_case(&c[2])
                    && ["the", "a", "an", "to", "for", "and"]
                        .contains(&c[1].to_ascii_lowercase().as_str())
                {
                    c[1].to_string()
                } else {
                    c[0].to_string()
                }
            })
            .into_owned();
        out = SPACE.replace_all(&out, " ").into_owned();
        out = BEFORE.replace_all(&out, "$1").into_owned();
        out = out
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
        // Don't alter casing in URLs, identifiers, acronyms, paths, or code.
        if !out.contains(['_', '/', '\\', '@', '`']) {
            if let Some(c) = out.chars().next() {
                out = c.to_uppercase().collect::<String>() + &out[c.len_utf8()..];
            }
        }
        for (index, value) in protected.iter().enumerate() {
            out = out.replace(&format!("{prefix}{index}\u{e001}"), value);
        }
    }
    // Single pass substitutions: an entry's output cannot trigger another rule.
    let originals = out.clone();
    let mut matches: Vec<(usize, usize, &str)> = Vec::new();
    for entry in entries
        .iter()
        .filter(|e| !e.source.trim().is_empty() && !e.replacement.is_empty())
    {
        let pattern = format!(r"(?i)(?:{})", regex::escape(&entry.source));
        if let Ok(re) = Regex::new(&pattern) {
            for m in re.find_iter(&originals) {
                let boundary = |c: char| c.is_alphanumeric() || c == '_';
                if originals[..m.start()]
                    .chars()
                    .next_back()
                    .is_some_and(boundary)
                    || originals[m.end()..].chars().next().is_some_and(boundary)
                {
                    continue;
                }
                matches.push((m.start(), m.end(), &entry.replacement));
            }
        }
    }
    matches.sort_by_key(|(s, e, _)| (*s, std::cmp::Reverse(*e)));
    let mut cursor = 0;
    let mut result = String::new();
    for (start, end, replacement) in matches {
        if start < cursor {
            continue;
        }
        result.push_str(&originals[cursor..start]);
        result.push_str(replacement);
        cursor = end;
    }
    result.push_str(&originals[cursor..]);
    result
}

pub fn expand(input: &str, snippets: &[Snippet]) -> Option<(String, bool)> {
    let trigger = input.trim().trim_end_matches(['.', '!', '?']).trim();
    snippets
        .iter()
        .find(|s| s.trigger.trim().eq_ignore_ascii_case(trigger))
        .map(|s| {
            let now = chrono::Local::now();
            (
                s.content
                    .replace("{{date}}", &now.format("%Y-%m-%d").to_string())
                    .replace("{{time}}", &now.format("%H:%M").to_string()),
                s.confirm,
            )
        })
}
pub fn validate_entries(
    entries: &[Entry],
    snippets: &[Snippet],
    styles: &[Style],
) -> Result<(), String> {
    if entries.len() > 5000 || snippets.len() > 1000 || styles.len() > 200 {
        return Err("Collection exceeds the supported entry count.".into());
    }
    for e in entries {
        if e.source.trim().is_empty() || e.source.len() > 256 || e.replacement.len() > 2048 {
            return Err("Dictionary entries need a word (up to 256 characters); replacements may be up to 2048 characters.".into());
        }
    }
    let mut triggers = std::collections::HashSet::new();
    for s in snippets {
        if s.trigger.trim().is_empty()
            || s.trigger.len() > 256
            || s.content.len() > 32000
            || !triggers.insert(s.trigger.trim().to_lowercase())
        {
            return Err(
                "Snippets need unique, nonempty triggers and content under 32,000 characters."
                    .into(),
            );
        }
    }
    for s in styles {
        if s.name.trim().is_empty() || s.instruction.len() > 4000 || s.app_match.len() > 256 {
            return Err("Styles need a name and instructions under 4,000 characters.".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn english_disfluency_rules_do_not_remove_portuguese_words() {
        let mode = local_mode(&Mode::Clean, "pt", Some("portuguese"));
        assert_eq!(normalize("um pedido", &mode, &[]), "um pedido");
        assert_eq!(local_mode(&Mode::Clean, "", Some("english")), Mode::Clean);
        assert_eq!(local_mode(&Mode::Clean, "", None), Mode::Literal);
    }
    #[test]
    fn cleanup_never_treats_identifier_contents_as_fillers() {
        for value in [
            "https://example.com/um",
            "uh@example.com",
            "`um new paragraph uh`",
            "C:\\work\\uh",
            "useState",
            "std::vec",
            "foo.bar",
        ] {
            assert_eq!(normalize(value, &Mode::Clean, &[]), value);
        }
        assert_eq!(
            normalize("um email uh@example.com and useState", &Mode::Clean, &[]),
            "Email uh@example.com and useState"
        );
    }
    #[test]
    fn literal_preserves_voice() {
        assert_eq!(
            normalize("um, hello actually hi", &Mode::Literal, &[]),
            "um, hello actually hi"
        );
    }
    #[test]
    fn removes_only_safe_disfluencies() {
        assert_eq!(
            normalize(
                "um the the meeting for Tuesday, actually Thursday.",
                &Mode::Clean,
                &[]
            ),
            "The meeting for Thursday."
        );
        assert_eq!(
            normalize("I had had enough, you know.", &Mode::Clean, &[]),
            "I had had enough, you know."
        );
    }
    #[test]
    fn identifiers_and_meaning_survive() {
        assert_eq!(
            normalize("my_var and https://example.com/a", &Mode::Clean, &[]),
            "my_var and https://example.com/a"
        );
        assert_eq!(
            normalize(
                "ship version one, actually wait for approval",
                &Mode::Clean,
                &[]
            ),
            "Ship version one, actually wait for approval"
        );
    }
    #[test]
    fn substitutions_are_whole_and_not_chained() {
        let e = vec![
            Entry {
                id: "1".into(),
                source: "groak".into(),
                replacement: "Groq".into(),
            },
            Entry {
                id: "2".into(),
                source: "Groq".into(),
                replacement: "wrong".into(),
            },
        ];
        assert_eq!(
            normalize("groak groaker", &Mode::Literal, &e),
            "Groq groaker"
        );
    }
    #[test]
    fn snippet_requires_entire_utterance() {
        let s = vec![Snippet {
            id: "1".into(),
            trigger: "my signature".into(),
            content: "Regards,\nChahatesh".into(),
            confirm: true,
        }];
        assert!(expand("please update my signature", &s).is_none());
        assert!(expand("My signature.", &s).unwrap().1);
    }
}
