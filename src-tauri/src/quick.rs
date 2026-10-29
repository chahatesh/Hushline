// SPDX-License-Identifier: MIT
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickCommand {
    Paste,
    Copy,
    ImprovePrompt,
    History,
    Settings,
    HideHour,
    ShowWidget,
    Cancel,
    Quit,
}
impl QuickCommand {
    pub const ALL: [Self; 9] = [
        Self::Paste,
        Self::Copy,
        Self::ImprovePrompt,
        Self::History,
        Self::Settings,
        Self::HideHour,
        Self::ShowWidget,
        Self::Cancel,
        Self::Quit,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Paste => "paste",
            Self::Copy => "copy",
            Self::ImprovePrompt => "improve_prompt",
            Self::History => "history",
            Self::Settings => "settings",
            Self::HideHour => "hide_hour",
            Self::ShowWidget => "show_widget",
            Self::Cancel => "cancel",
            Self::Quit => "quit",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Paste => "Paste last transcript",
            Self::Copy => "Copy last transcript",
            Self::ImprovePrompt => "Improve prompt",
            Self::History => "Transcription history",
            Self::Settings => "Settings",
            Self::HideHour => "Hide widget for 1 hour",
            Self::ShowWidget => "Show widget",
            Self::Cancel => "Cancel dictation",
            Self::Quit => "Quit Hush",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|command| command.id() == id)
    }
    pub fn enabled(self, has_text: bool, busy: bool) -> bool {
        match self {
            Self::Paste | Self::Copy => has_text,
            Self::ImprovePrompt => !busy,
            Self::Cancel => busy,
            _ => true,
        }
    }
    pub fn needs_target(self) -> bool {
        matches!(self, Self::Paste | Self::ImprovePrompt)
    }
}
/// Only window identity crosses the native menu; text is read after an explicit action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuTarget {
    pub hwnd: isize,
    pub pid: u32,
}
impl MenuTarget {
    pub fn matches(self, hwnd: isize, pid: u32) -> bool {
        self.hwnd != 0 && self.pid != 0 && self.hwnd == hwnd && self.pid == pid
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickSelection {
    pub command: QuickCommand,
    pub target: Option<MenuTarget>,
}

pub struct WidgetSnooze {
    until: Option<Instant>,
}
pub fn widget_visible(active: bool, dock_hidden: bool, snoozed: bool) -> bool {
    active || !(dock_hidden || snoozed)
}
impl WidgetSnooze {
    pub const fn new() -> Self {
        Self { until: None }
    }
    pub fn hide_hour(&mut self, now: Instant) {
        self.until = Some(now + Duration::from_secs(3600));
    }
    pub fn show(&mut self) {
        self.until = None;
    }
    pub fn remaining(&mut self, now: Instant) -> Option<Duration> {
        let remaining = self
            .until
            .and_then(|until| until.checked_duration_since(now))
            .filter(|d| !d.is_zero());
        if remaining.is_none() {
            self.until = None;
        }
        remaining
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hide_is_temporary_and_can_be_reversed_without_changing_preferences() {
        let now = Instant::now();
        let mut snooze = WidgetSnooze::new();
        snooze.hide_hour(now);
        assert_eq!(
            snooze.remaining(now + Duration::from_secs(3599)),
            Some(Duration::from_secs(1))
        );
        assert_eq!(snooze.remaining(now + Duration::from_secs(3600)), None);
        snooze.hide_hour(now);
        snooze.show();
        assert_eq!(snooze.remaining(now), None);
        for hidden in [false, true] {
            for snoozed in [false, true] {
                assert!(
                    widget_visible(true, hidden, snoozed),
                    "Recording feedback must stay visible"
                );
                assert_eq!(widget_visible(false, hidden, snoozed), !hidden && !snoozed);
            }
        }
    }
    #[test]
    fn menu_commands_are_distinct_and_never_read_an_unselected_window() {
        for command in QuickCommand::ALL {
            assert_eq!(QuickCommand::from_id(command.id()), Some(command));
        }
        assert_eq!(QuickCommand::from_id("unknown"), None);
        assert!(!QuickCommand::Paste.enabled(false, false));
        assert!(!QuickCommand::ImprovePrompt.enabled(true, true));
        assert!(QuickCommand::History.enabled(false, true));
        assert!(QuickCommand::Paste.needs_target());
        assert!(!QuickCommand::History.needs_target());
        let target = MenuTarget { hwnd: 10, pid: 20 };
        assert!(target.matches(10, 20));
        assert!(!target.matches(11, 20));
        assert!(!target.matches(10, 21));
        assert!(!MenuTarget { hwnd: 0, pid: 0 }.matches(0, 0));
    }
}
