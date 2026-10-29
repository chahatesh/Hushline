// SPDX-License-Identifier: MIT
use crate::model::Shortcuts;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Push,
    Release,
    HandsFree,
    Command,
    ImprovePrompt,
    Paste,
    Copy,
    Open,
    Scratchpad,
    Cancel,
    Quick(crate::quick::QuickSelection),
    MenuFailed,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chord {
    pub modifiers: u8,
    pub key: Option<u16>,
}
pub const CTRL: u8 = 1;
pub const WIN: u8 = 2;
pub const ALT: u8 = 4;
pub const SHIFT: u8 = 8;
pub fn parse(input: &str) -> Result<Chord, String> {
    let mut chord = Chord {
        modifiers: 0,
        key: None,
    };
    for token in input.split('+').map(str::trim) {
        let m = match token.to_ascii_lowercase().as_str() {
            "ctrl" => CTRL,
            "win" => WIN,
            "alt" => ALT,
            "shift" => SHIFT,
            _ => 0,
        };
        if m != 0 {
            if chord.modifiers & m != 0 {
                return Err("Duplicate shortcut modifier.".into());
            }
            chord.modifiers |= m;
            continue;
        }
        let key=match token.to_ascii_lowercase().as_str() { "space"=>0x20,"escape"=>0x1b,"mouse4"=>5,"mouse5"=>6,
            x if x.len()==1 && x.as_bytes()[0].is_ascii_alphanumeric()=>x.to_ascii_uppercase().as_bytes()[0] as u16,
            x if x.starts_with('f')=>x[1..].parse::<u16>().ok().filter(|n| (1..=24).contains(n)).map(|n| 0x6f+n).ok_or("Use F1–F24.")?,
            _=>return Err(format!("Unsupported shortcut key: {token}. Use Ctrl, Win, Alt, Shift, A–Z, 0–9, F1–F24, Space, Escape, Mouse4 or Mouse5.")) };
        if chord.key.replace(key).is_some() {
            return Err("Use one primary key per shortcut.".into());
        }
    }
    if chord.modifiers == 0
        && chord.key != Some(0x1b)
        && chord.key != Some(5)
        && chord.key != Some(6)
    {
        return Err(
            "Shortcuts need a modifier; Escape and mouse buttons can be used alone.".into(),
        );
    }
    if chord.key.is_none() && chord.modifiers.count_ones() < 2 {
        return Err("Modifier-only shortcuts need at least two modifiers.".into());
    }
    Ok(chord)
}
pub fn bindings(s: &Shortcuts) -> Vec<(&str, Action)> {
    vec![
        (&s.hands_free, Action::HandsFree),
        (&s.command, Action::Command),
        (&s.improve_prompt, Action::ImprovePrompt),
        (&s.paste, Action::Paste),
        (&s.copy, Action::Copy),
        (&s.open, Action::Open),
        (&s.scratchpad, Action::Scratchpad),
        (&s.cancel, Action::Cancel),
        (&s.push, Action::Push),
    ]
}
pub fn validate(s: &Shortcuts) -> Result<(), String> {
    let mut seen = HashSet::new();
    for (v, _) in bindings(s) {
        let c = parse(v)?;
        if !seen.insert(c) {
            return Err("Two Hushline actions share the same shortcut.".into());
        }
    }
    if parse(&s.push)?.key == Some(0x1b) {
        return Err("Escape is reserved for cancellation.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_valid() {
        assert!(validate(&Shortcuts::default()).is_ok());
    }
    #[test]
    fn detects_equivalent_conflicts() {
        let s = Shortcuts {
            copy: "Win+Ctrl".into(),
            ..Shortcuts::default()
        };
        assert!(validate(&s).is_err());
    }
    #[test]
    fn rejects_ambiguous_keys() {
        for c in ["Ctrl+A+B", "Ctrl+Ctrl+A", "Win", "A", "Ctrl+F25", ""] {
            assert!(parse(c).is_err(), "{c}");
        }
    }
    #[test]
    fn supports_mouse_and_modifiers() {
        assert_eq!(parse("Ctrl+Mouse4").unwrap().key, Some(5));
        assert!(parse("Alt+Win").is_ok());
    }
}
