//! Shared modal-editing primitive for forms (Settings, Theme, Host editor,
//! key-gen, cluster form, …).
//!
//! Forms open in [`VimMode::Insert`] (their original behaviour: every char
//! lands in the active field). Pressing `Esc` switches to [`VimMode::Normal`]
//! where `j`/`k`/`g`/`G` navigate fields and `i`/`a`/`Enter` returns to
//! `Insert`. A second `Esc` (from `Normal`) is interpreted by the form
//! itself — most modals close on it, tab views bounce back to `Insert`.
//!
//! The helper only *classifies* the keystroke. The form does the actual
//! field movement so it can keep using its existing `next_field` /
//! `prev_field` logic (which already knows how to skip separators, wrap,
//! etc.).

use crossterm::event::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VimMode {
    /// Characters land in the active text field. This is the default.
    #[default]
    Insert,
    /// Vim-style navigation: `j`/`k` move between fields, `i`/`a`/`Enter`
    /// re-enter `Insert`, `Esc` bubbles up as "leave form".
    Normal,
}

impl VimMode {
    /// Short tag shown in form title bars so the user can see which mode
    /// they're in. Coloured by the caller.
    pub fn label(self) -> &'static str {
        match self {
            VimMode::Insert => "INSERT",
            VimMode::Normal => "NORMAL",
        }
    }

    pub fn is_insert(self) -> bool {
        matches!(self, VimMode::Insert)
    }

    pub fn is_normal(self) -> bool {
        matches!(self, VimMode::Normal)
    }
}

/// What a keystroke means under the modal layer. The form handler picks
/// each variant up and applies its own movement / save logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalIntent {
    /// `Esc` while in `Insert` — switch to `Normal`.
    EnterNormal,
    /// `i` / `a` / `I` / `A` / `Enter` while in `Normal` — switch to `Insert`.
    EnterInsert,
    /// `j` or `Down` while in `Normal`.
    NavDown,
    /// `k` or `Up` while in `Normal`.
    NavUp,
    /// `gg` chord completed while in `Normal`.
    NavTop,
    /// `G` or `End` while in `Normal`.
    NavBottom,
    /// `Home` while in `Normal` — same as `gg` but no chord.
    NavHome,
    /// `Esc` (or `q`) while already in `Normal`. Modals close, tab views
    /// usually bounce back to `Insert`.
    LeaveForm,
    /// `Insert` mode — let the form's own handler process the key.
    Passthrough,
    /// `Normal` mode but the key isn't bound. Swallow it so stray letters
    /// don't fall through into text-field handlers.
    Swallow,
}

/// Classify one keystroke under the modal layer.
///
/// `pending_g` tracks the first half of a `gg` chord across calls. The
/// helper resets it on any non-`g` keypress.
pub fn classify_modal_key(
    mode: VimMode,
    key: KeyCode,
    pending_g: &mut bool,
) -> ModalIntent {
    let was_pending_g = *pending_g;
    if !matches!(key, KeyCode::Char('g')) {
        *pending_g = false;
    }

    match mode {
        VimMode::Insert => match key {
            KeyCode::Esc => ModalIntent::EnterNormal,
            _ => ModalIntent::Passthrough,
        },
        VimMode::Normal => match key {
            KeyCode::Esc | KeyCode::Char('q') => ModalIntent::LeaveForm,
            KeyCode::Char('i')
            | KeyCode::Char('a')
            | KeyCode::Char('I')
            | KeyCode::Char('A')
            | KeyCode::Enter => ModalIntent::EnterInsert,
            KeyCode::Char('j') | KeyCode::Down => ModalIntent::NavDown,
            KeyCode::Char('k') | KeyCode::Up => ModalIntent::NavUp,
            KeyCode::Char('g') => {
                if was_pending_g {
                    ModalIntent::NavTop
                } else {
                    *pending_g = true;
                    ModalIntent::Swallow
                }
            }
            KeyCode::Char('G') | KeyCode::End => ModalIntent::NavBottom,
            KeyCode::Home => ModalIntent::NavHome,
            _ => ModalIntent::Swallow,
        },
    }
}
