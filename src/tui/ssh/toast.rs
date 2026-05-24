use std::time::Instant;
use ratatui::layout::Rect;
use ratatui::prelude::{Line, Modifier, Span, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use crate::tui::theme::Theme;

#[derive(Clone, Copy, PartialEq)]
pub enum ToastKind {
    Success,
    Error,
}

pub struct Toast {
    pub message: String,
    pub created: Instant,
    pub kind: ToastKind,
}

impl Toast {
    pub fn success(message: impl Into<String>) -> Self {
        Toast { message: message.into(), created: Instant::now(), kind: ToastKind::Success }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Toast { message: message.into(), created: Instant::now(), kind: ToastKind::Error }
    }

    pub fn is_expired(&self) -> bool {
        self.created.elapsed().as_secs() >= 5
    }
}

pub fn render_toast(f: &mut ratatui::Frame, screen: Rect, toast: &Toast, theme: &Theme) {
    // Layout constants
    const MIN_WIDTH: u16 = 14;
    const MAX_WIDTH: u16 = 70;
    const PADDING: u16 = 2; // 1 char on each side of the border
    const BORDERS: u16 = 2;
    const ICON_WIDTH: u16 = 2; // "✓ " / "✗ "

    let border_color = match toast.kind {
        ToastKind::Success => theme.success,
        ToastKind::Error => theme.error,
    };
    let icon = match toast.kind {
        ToastKind::Success => "✓ ",
        ToastKind::Error => "✗ ",
    };

    // Width: as wide as needed for a single line, but never wider than the
    // screen (minus a small margin) and capped at MAX_WIDTH.
    let screen_cap = screen.width.saturating_sub(PADDING);
    let msg_chars = toast.message.chars().count() as u16;
    let ideal_width = msg_chars.saturating_add(ICON_WIDTH).saturating_add(BORDERS);
    let width = ideal_width.clamp(MIN_WIDTH, MAX_WIDTH.min(screen_cap.max(MIN_WIDTH)));

    // Height: enough lines to fit the wrapped message. We approximate by
    // dividing character count by the inner text width (good enough for
    // ASCII/Cyrillic; CJK would slightly over- or under-shoot).
    let inner_text_width = width
        .saturating_sub(BORDERS)
        .saturating_sub(ICON_WIDTH)
        .max(1);
    let line_count = msg_chars
        .saturating_add(inner_text_width - 1)
        .saturating_div(inner_text_width)
        .max(1);
    let max_height = screen.height.saturating_sub(PADDING).max(3);
    let toast_height = (line_count + BORDERS).clamp(3, max_height);

    let x = screen.width.saturating_sub(width + 1);
    let y = screen.height.saturating_sub(toast_height + 2);
    let area = Rect { x, y, width, height: toast_height };

    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let text = Paragraph::new(Line::from(vec![
        Span::styled(icon.to_string(), Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
        Span::styled(toast.message.clone(), Style::default().fg(theme.fg)),
    ]))
    .wrap(Wrap { trim: false });
    f.render_widget(text, inner);
}
