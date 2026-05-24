//! In-TUI key-generation modal.
//!
//! Stays inside an alt-screen / raw-mode session — unlike the old
//! `run_generate_key_flow` which left the terminal to drive `inquire` and
//! confused users into thinking nothing had happened.

use std::io::stdout;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};

use crate::tui::ssh::keygen_form_state::{KeygenFormState, KEY_TYPES};
use crate::tui::ssh::modal::centered_rect;
use crate::tui::theme;

pub fn draw_keygen_form(f: &mut Frame, state: &KeygenFormState) {
    let size = f.area();
    let area = centered_rect(70, 60, size);
    let theme = theme::load();
    let bg = theme.bg;
    let fg = theme.fg;
    let accent = theme.accent;

    let mode_style = if state.vim_mode.is_normal() {
        Style::default().fg(bg).bg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };
    let title = Line::from(vec![
        Span::styled(
            " Generate SSH key  ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {} ", state.vim_mode.label()), mode_style),
    ]);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(bg).fg(fg));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // key type
            Constraint::Length(1), // path
            Constraint::Length(1), // comment
            Constraint::Length(1), // passphrase
            Constraint::Length(1), // spacer
            Constraint::Length(1), // actions
        ])
        .split(inner);

    // Key type row — Space or ←/→ cycles between types.
    let kt_selected = state.selected_field == KeygenFormState::KEY_TYPE_FIELD;
    let kt_value_span = if kt_selected {
        Span::styled(
            format!("‹ {} ›", state.key_type()),
            Style::default().bg(accent).fg(bg).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw(format!("[{}]", state.key_type()))
    };
    let kt_hint = if kt_selected {
        format!("  Space/←/→ to cycle ({}/{})", state.key_type_idx + 1, KEY_TYPES.len())
    } else {
        String::new()
    };
    let kt_para = Paragraph::new(Line::from(vec![
        Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
        kt_value_span,
        Span::styled(kt_hint, Style::default().fg(theme.muted)),
    ]));
    f.render_widget(kt_para, chunks[0]);

    let mk_text = |label: &str, value: &str, selected: bool| -> Paragraph<'static> {
        let value_span = if selected {
            Span::styled(
                format!("[{}]", value),
                Style::default().bg(accent).fg(bg).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::raw(format!("[{}]", value))
        };
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{label}: "), Style::default().add_modifier(Modifier::BOLD)),
            value_span,
        ]))
    };

    f.render_widget(
        mk_text("Path", &state.path, state.selected_field == KeygenFormState::PATH_FIELD),
        chunks[1],
    );
    f.render_widget(
        mk_text("Comment", &state.comment, state.selected_field == KeygenFormState::COMMENT_FIELD),
        chunks[2],
    );

    // Passphrase row — mask but hint at empty.
    let pp_selected = state.selected_field == KeygenFormState::PASSPHRASE_FIELD;
    let masked: String = "•".repeat(state.passphrase.chars().count());
    let pp_value_span = if pp_selected {
        Span::styled(
            format!("[{}]", masked),
            Style::default().bg(accent).fg(bg).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw(format!("[{}]", masked))
    };
    let pp_hint = if state.passphrase.is_empty() {
        "  (empty = no passphrase)"
    } else {
        ""
    };
    let pp_para = Paragraph::new(Line::from(vec![
        Span::styled("Passphrase: ", Style::default().add_modifier(Modifier::BOLD)),
        pp_value_span,
        Span::styled(pp_hint.to_string(), Style::default().fg(theme.muted)),
    ]));
    f.render_widget(pp_para, chunks[3]);

    // Save / Cancel actions
    let save_selected = state.selected_field == KeygenFormState::fields_count();
    let save_style = if save_selected {
        Style::default().bg(accent).fg(bg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(accent)
    };
    let actions = Paragraph::new(Line::from(vec![
        Span::styled("[ Generate ]", save_style),
        Span::raw("  "),
        Span::styled("[ Esc = Cancel ]", Style::default().fg(theme.muted)),
    ]));
    f.render_widget(actions, chunks[5]);

    // Footer: error if any, else field-specific hint.
    let footer_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    if let Some(ref err) = state.error {
        let err_para = Paragraph::new(Line::from(vec![
            Span::styled("✗ ", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
            Span::styled(err.clone(), Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
        ]));
        f.render_widget(err_para, footer_area);
    } else {
        let hint = match state.selected_field {
            x if x == KeygenFormState::KEY_TYPE_FIELD =>
                "ed25519 is the modern default. *-sk variants require a FIDO2 hardware token.",
            x if x == KeygenFormState::PATH_FIELD =>
                "Where to write the private key (the matching .pub is generated alongside).",
            x if x == KeygenFormState::COMMENT_FIELD =>
                "Stored in the public key — visible in authorized_keys; identifies the key.",
            x if x == KeygenFormState::PASSPHRASE_FIELD =>
                "Empty = no passphrase. With one, ssh will prompt (or load via the agent).",
            _ => "Tab/↑↓ to move • Enter on [ Generate ] to create • Esc to cancel",
        };
        let para = Paragraph::new(Line::from(Span::raw(hint)))
            .style(Style::default().fg(theme.muted));
        f.render_widget(para, footer_area);
    }
}

/// Validate the form and shell out to `ssh-keygen`. Returns the path of the
/// freshly written private key on success.
fn try_generate(state: &KeygenFormState) -> Result<PathBuf, String> {
    let raw_path = state.path.trim();
    if raw_path.is_empty() {
        return Err("Path cannot be empty".to_string());
    }
    let expanded = shellexpand::tilde(raw_path).to_string();
    let path = PathBuf::from(&expanded);
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
        }
    }
    crate::ssh::keys::generate_key(state.key_type(), &path, state.comment.trim(), &state.passphrase)
        .map_err(|e| format!("ssh-keygen failed: {e}"))?;
    Ok(path)
}

/// Drive the key-gen modal in its own alt-screen/raw-mode terminal. Returns
/// the freshly created private-key path, or `None` if the user pressed Esc.
///
/// Caller is expected to have already left the parent alt screen (mirrors
/// `run_host_form` — see `run_tui` for the call pattern).
pub fn run_keygen_form() -> Option<PathBuf> {
    let mut stdout = stdout();
    let _ = enable_raw_mode();
    let _ = execute!(stdout, EnterAlternateScreen);
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).ok()?;

    let mut state = KeygenFormState::new();
    let mut result: Option<PathBuf> = None;

    loop {
        let _ = terminal.draw(|f| draw_keygen_form(f, &state));

        if !event::poll(Duration::from_millis(150)).unwrap_or(false) {
            continue;
        }
        let Ok(Event::Key(k)) = event::read() else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }

        use crate::tui::vim_mode::{classify_modal_key, ModalIntent, VimMode};
        let intent = classify_modal_key(state.vim_mode, k.code, &mut state.pending_g);
        let mut consumed = true;
        match intent {
            ModalIntent::EnterNormal => state.vim_mode = VimMode::Normal,
            ModalIntent::EnterInsert => {
                if matches!(k.code, KeyCode::Enter)
                    && state.selected_field == KeygenFormState::fields_count()
                {
                    match try_generate(&state) {
                        Ok(path) => {
                            result = Some(path);
                            break;
                        }
                        Err(e) => state.error = Some(e),
                    }
                } else {
                    state.vim_mode = VimMode::Insert;
                }
            }
            ModalIntent::NavDown => state.next_field(),
            ModalIntent::NavUp => state.prev_field(),
            ModalIntent::NavTop | ModalIntent::NavHome => state.selected_field = 0,
            ModalIntent::NavBottom => state.selected_field = KeygenFormState::fields_count(),
            ModalIntent::LeaveForm => break,
            ModalIntent::Swallow => {}
            ModalIntent::Passthrough => consumed = false,
        }
        if consumed { continue; }

        match k.code {
            KeyCode::Esc => break,
            KeyCode::Tab | KeyCode::Down => state.next_field(),
            KeyCode::BackTab | KeyCode::Up => state.prev_field(),
            KeyCode::Left if state.selected_field == KeygenFormState::KEY_TYPE_FIELD => {
                state.cycle_key_type(false);
            }
            KeyCode::Right if state.selected_field == KeygenFormState::KEY_TYPE_FIELD => {
                state.cycle_key_type(true);
            }
            KeyCode::Enter => {
                if state.selected_field == KeygenFormState::fields_count() {
                    match try_generate(&state) {
                        Ok(path) => {
                            result = Some(path);
                            break;
                        }
                        Err(e) => state.error = Some(e),
                    }
                } else {
                    state.next_field();
                }
            }
            KeyCode::Char(c) => state.push_char(c),
            KeyCode::Backspace => state.pop_char(),
            _ => {}
        }
    }

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    result
}
