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
    LazyLock::new(|| Regex::new(r"\b([\p{L}]+),?\s+([\p{L}]+)\b").unwrap());
static RESTART: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(I want to|I would like to|I need to|we need to|we should|I'm going to),?\s+(I want to|I would like to|I need to|we need to|we should|I'm going to)\b").unwrap()
});
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
        loop {
            let before = out.clone();
            out = AGAIN
                .replace_all(&out, |c: &Captures| {
                    // "had had", "that that", emphatic "very very" can be grammatical.
                    if c[1].eq_ignore_ascii_case(&c[2])
                        && ["the", "a", "an", "to", "for", "and", "i", "we"]
                            .contains(&c[1].to_ascii_lowercase().as_str())
                    {
                        c[1].to_string()
                    } else {
                        c[0].to_string()
                    }
                })
                .into_owned();
            out = RESTART
                .replace_all(&out, |c: &Captures| {
                    if c[1].eq_ignore_ascii_case(&c[2]) {
                        c[1].to_string()
                    } else {
                        c[0].to_string()
                    }
                })
                .into_owned();
            if out == before {
                break;
            }
        }
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

/// Automatic cleanup must not silently drop a large tail or change identifiers.
/// Command Mode has separate, explicit authority to shorten/change selected text.
pub fn validate_cleanup(input: &str, edited: &str) -> Result<(), String> {
    let words = input.split_whitespace().count();
    if words >= 40 && edited.split_whitespace().count() * 100 < words * 65 {
        return Err(
            "The AI edit left out too much of your speech. Kept the original wording.".into(),
        );
    }
    let identifiers: std::collections::HashSet<_> = PROTECTED
        .find_iter(edited)
        .map(|m| {
            m.as_str()
                .trim_end_matches(['.', ',', ';', ':', ')', ']', '}'])
        })
        .collect();
    for identifier in PROTECTED.find_iter(input).map(|m| {
        m.as_str()
            .trim_end_matches(['.', ',', ';', ':', ')', ']', '}'])
    }) {
        if !identifiers.contains(identifier) {
            return Err(
                "The AI edit changed a technical identifier. Kept the original wording.".into(),
            );
        }
    }
    static NUMBERS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\bv?\d+(?:[./:-]\d+)*\b").unwrap());
    let numbers: std::collections::HashSet<_> =
        NUMBERS.find_iter(edited).map(|m| m.as_str()).collect();
    if NUMBERS
        .find_iter(input)
        .any(|m| !numbers.contains(m.as_str()))
    {
        return Err("The AI edit changed a number. Kept the original wording.".into());
    }
    Ok(())
}

/// Structural formatting runs locally; it only changes whitespace, list markers,
/// and sentence-initial case. Literal mode and non-English speech bypass it.
pub fn format(input: &str, layout: &str, mode: &Mode) -> String {
    if layout == "off" || *mode == Mode::Literal || input.trim().is_empty() {
        return input.to_string();
    }
    static HEADING: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^((?:(?:here are|these are) (?:my |the )?)?(?:my )?(?:meeting notes|notes|action items|shopping list|to.do list|bullet points|tasks|priorities|key points))\s*[.!:]?\s+").unwrap()
    });
    static ITEM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\b(first|second|third|fourth|fifth|sixth|seventh|eighth|ninth|tenth)(?:\s+(?:point|item))?(?:\s*[,.:]\s*|\s+)").unwrap()
    });
    let capital = |s: &str| {
        let s = s.trim();
        if PROTECTED.find(s).is_some_and(|m| m.start() == 0) {
            return s.to_string();
        }
        match s.chars().next() {
            Some(c) => c.to_uppercase().collect::<String>() + &s[c.len_utf8()..],
            None => String::new(),
        }
    };
    let heading = HEADING.captures(input);
    let body = heading
        .as_ref()
        .map(|h| &input[h.get(0).unwrap().end()..])
        .unwrap_or(input);
    let markers: Vec<_> = ITEM.captures_iter(body).collect();
    let ordered = markers.len() >= 2
        && markers.iter().enumerate().all(|(i, c)| {
            [
                "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth",
                "ninth", "tenth",
            ]
            .get(i)
            .is_some_and(|v| c[1].eq_ignore_ascii_case(v))
        })
        && body[..markers[0].get(0).unwrap().start()].trim().is_empty();
    if layout != "paragraphs" && ordered {
        let mut items = Vec::new();
        for (i, marker) in markers.iter().enumerate() {
            let end = markers
                .get(i + 1)
                .map(|c| c.get(0).unwrap().start())
                .unwrap_or(body.len());
            let item = body[marker.get(0).unwrap().end()..end].trim();
            if item.is_empty() {
                return input.to_string();
            }
            items.push(format!("• {}", capital(item)));
        }
        let prefix = heading
            .map(|h| format!("{}:\n", capital(&h[1])))
            .unwrap_or_default();
        return prefix + &items.join("\n");
    }
    // Keep explicit breaks while still formatting a long prose line within them.
    // Existing bullet items and fenced code keep their intentional structure.
    if input.contains('\n') {
        if input.contains("```") {
            return input.to_string();
        }
        return input
            .lines()
            .map(|line| {
                let trimmed = line.trim_start();
                if trimmed.starts_with(['•', '-', '*']) {
                    line.to_string()
                } else {
                    let formatted = format(line, layout, mode);
                    if formatted == line && layout == "auto" && input.chars().count() >= 240 {
                        format(line, "paragraphs", mode)
                    } else {
                        formatted
                    }
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    if let (Some(list_heading), true) = (heading.as_ref(), layout != "paragraphs") {
        let separator = if body.contains(';') {
            Some(';')
        } else if heading
            .as_ref()
            .is_some_and(|h| h[1].to_lowercase().contains("list"))
            && body.matches(',').count() >= 2
        {
            Some(',')
        } else {
            None
        };
        if let Some(separator) = separator {
            let protected: Vec<_> = PROTECTED.find_iter(body).map(|m| m.range()).collect();
            let mut items = Vec::new();
            let mut start = 0;
            for (index, c) in body.char_indices() {
                if c == separator && !protected.iter().any(|r| r.contains(&index)) {
                    items.push(&body[start..index]);
                    start = index + c.len_utf8();
                }
            }
            items.push(&body[start..]);
            if items.len() >= 2 && items.iter().all(|v| !v.trim().is_empty()) {
                let count = items.len();
                let bullets = items
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let mut value = v.trim();
                        if separator == ','
                            && i == count - 1
                            && value.to_lowercase().starts_with("and ")
                        {
                            value = &value[4..];
                        }
                        format!("• {}", capital(value))
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                return format!("{}:\n{}", capital(&list_heading[1]), bullets);
            }
        }
    }
    let parts = sentences(body);
    if (layout == "notes" || (heading.is_some() && layout == "auto")) && parts.len() >= 2 {
        let prefix = heading
            .map(|h| format!("{}:\n", capital(&h[1])))
            .unwrap_or_default();
        return prefix
            + &parts
                .iter()
                .map(|s| format!("• {}", capital(s)))
                .collect::<Vec<_>>()
                .join("\n");
    }
    if parts.len() < 2 || (layout == "auto" && input.chars().count() < 240) {
        return input.to_string();
    }
    let mut paragraphs = Vec::new();
    let mut paragraph = String::new();
    let mut count = 0;
    for sentence in sentences(input) {
        let lower = sentence.to_lowercase();
        let transition = [
            "separately,",
            "on another note,",
            "moving on,",
            "in other news,",
        ]
        .iter()
        .any(|v| lower.starts_with(v));
        if !paragraph.is_empty()
            && (count >= 3
                || paragraph.chars().count() >= 260
                || (count >= 2 && paragraph.len() >= 180)
                || transition)
        {
            paragraphs.push(std::mem::take(&mut paragraph));
            count = 0;
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(sentence);
        count += 1;
    }
    if !paragraph.is_empty() {
        paragraphs.push(paragraph);
    }
    paragraphs.join("\n\n")
}

fn sentences(input: &str) -> Vec<&str> {
    let protected: Vec<_> = PROTECTED.find_iter(input).map(|m| m.range()).collect();
    let mut result = Vec::new();
    let mut start = 0;
    for (index, c) in input.char_indices() {
        if !matches!(c, '.' | '!' | '?') || protected.iter().any(|r| r.contains(&index)) {
            continue;
        }
        let after = index + c.len_utf8();
        if after == input.len() || !input[after..].starts_with(char::is_whitespace) {
            continue;
        }
        let word = input[start..after]
            .split_whitespace()
            .next_back()
            .unwrap_or("")
            .to_ascii_lowercase();
        if [
            "mr.", "mrs.", "ms.", "dr.", "prof.", "e.g.", "i.e.", "vs.", "etc.",
        ]
        .contains(&word.as_str())
            || (word.len() == 2 && word.as_bytes()[0].is_ascii_alphabetic())
        {
            continue;
        }
        result.push(input[start..after].trim());
        start = after;
    }
    if !input[start..].trim().is_empty() {
        result.push(input[start..].trim());
    }
    result
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
    fn paragraphs_still_apply_when_whisper_returns_a_single_line_break() {
        let text = "We reviewed the current release carefully today. The build passed every check on this computer. We still need to test the installer on another machine. We should preserve the existing configuration during every update.\nThe endpoint is https://example.com/v2 and the variable is useState. We need to keep every identifier exactly as it was dictated. Maya will review the documentation on Thursday. The final deadline remains at 3 pm.";
        let result = format(text, "auto", &Mode::Clean);
        assert!(result.contains("\n\n"));
        assert_eq!(
            result.split_whitespace().collect::<Vec<_>>(),
            text.split_whitespace().collect::<Vec<_>>()
        );
    }
    #[test]
    fn naturally_introduced_lists_get_bullets() {
        assert_eq!(
            format(
                "Here are the tasks: review the draft; test the installer; send the email.",
                "auto",
                &Mode::Clean
            ),
            "Here are the tasks:\n• Review the draft\n• Test the installer\n• Send the email."
        );
        assert_eq!(
            format(
                "Shopping list: milk, eggs, and bread.",
                "auto",
                &Mode::Clean
            ),
            "Shopping list:\n• Milk\n• Eggs\n• Bread."
        );
        assert_eq!(format("First we should check the build. Second we should test it. Third we should send the email.","auto",&Mode::Clean),"• We should check the build.\n• We should test it.\n• We should send the email.");
    }
    #[test]
    fn repeated_stutters_and_restarts_do_not_leave_duplicate_fragments() {
        assert_eq!(
            normalize(
                "Um I, I, I need to, I need to write the report.",
                &Mode::Clean,
                &[]
            ),
            "I need to write the report."
        );
        assert_eq!(
            normalize(
                "I I I want to I want to send the the the report.",
                &Mode::Clean,
                &[]
            ),
            "I want to send the report."
        );
        assert_eq!(
            normalize(
                "I had had a very very good day, no no no.",
                &Mode::Clean,
                &[]
            ),
            "I had had a very very good day, no no no."
        );
    }
    #[test]
    fn automatic_ai_cleanup_rejects_missing_tail_identifiers_and_numbers() {
        assert!(validate_cleanup(
            "Send version v2.4.1 to maya@example.com at 3 pm.",
            "Send version v2.4.1 to maya@example.com at 4 pm."
        )
        .is_err());
        assert!(
            validate_cleanup("The variable is useState.", "The variable is useStates.").is_err()
        );
        assert!(validate_cleanup("The variable is useState.", "The variable is state.").is_err());
        let long = "This is a substantive sentence with several important words. ".repeat(12);
        assert!(validate_cleanup(&long, "This is a short summary.").is_err());
    }
    #[test]
    fn formats_explicit_notes_and_ordered_lists_without_an_ai_call() {
        assert_eq!(format("Meeting notes. First, ship the release. Second, email Maya. Third, update the docs.", "auto", &Mode::Clean), "Meeting notes:\n• Ship the release.\n• Email Maya.\n• Update the docs.");
        assert_eq!(
            format(
                "Notes: The build passed. The installer needs review.",
                "auto",
                &Mode::Clean
            ),
            "Notes:\n• The build passed.\n• The installer needs review."
        );
        assert_eq!(
            format(
                "First, check useState. Second, visit https://example.com/a.",
                "auto",
                &Mode::Clean
            ),
            "• Check useState.\n• Visit https://example.com/a."
        );
    }
    #[test]
    fn formatting_preserves_ordinary_speech_identifiers_and_literal_mode() {
        for text in [
            "The first time was the second test.",
            "Dr. Smith uses foo.bar and my_var.",
            "Email a@b.com. Visit https://example.com/x.",
        ] {
            assert_eq!(format(text, "auto", &Mode::Clean), text);
            assert_eq!(format(text, "notes", &Mode::Literal), text);
        }
        assert_eq!(
            format(
                "One sentence. Another sentence. A third sentence. Last sentence.",
                "paragraphs",
                &Mode::Clean
            ),
            "One sentence. Another sentence. A third sentence.\n\nLast sentence."
        );
    }
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
