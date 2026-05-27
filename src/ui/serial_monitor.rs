use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::core::{AppState, SerialDirection, SerialDisplayFormat};

pub fn draw_serial_monitor(frame: &mut Frame, state: &AppState) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // status line
        Constraint::Min(0),    // scrollable body
        Constraint::Length(3), // write input (always visible)
        Constraint::Length(1), // validation error line
        Constraint::Length(1), // hints line
    ])
    .split(frame.area());

    // ── Status line ──────────────────────────────────────────────────────────
    let conn_span = if state.connected {
        Span::styled(
            "● CONNECTED",
            Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("○ DISCONNECTED", Style::new().fg(Color::Red))
    };
    let line_count = Span::styled(
        format!("  {} lines", state.serial_lines.len()),
        Style::new().fg(Color::DarkGray),
    );
    let total_bytes: usize = state.serial_lines.iter().map(|e| e.raw.len()).sum();
    let current_bytes = state.serial_lines.last().map_or(0, |e| e.raw.len());
    let bytes_label = Span::styled(
        format!("  {total_bytes} B total  {current_bytes} B last"),
        Style::new().fg(Color::DarkGray),
    );
    let fmt_label = match state.serial_display_format {
        SerialDisplayFormat::Ascii => Span::styled("  [ASCII]", Style::new().fg(Color::Cyan)),
        SerialDisplayFormat::Hex => Span::styled("  [HEX]", Style::new().fg(Color::Magenta)),
    };
    let pause_label = if state.serial_paused {
        Span::styled(
            "  ⏸ PAUSED",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("")
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            conn_span,
            line_count,
            bytes_label,
            fmt_label,
            pause_label,
        ])),
        chunks[0],
    );

    // ── Body ─────────────────────────────────────────────────────────────────
    let body_height = chunks[1].height as usize;
    let total = state.serial_lines.len();
    let max_offset = if total > body_height {
        total - body_height
    } else {
        0
    };
    let offset = state.serial_line_offset.min(max_offset);

    let visible: Vec<Line> = state
        .serial_lines
        .iter()
        .skip(offset)
        .take(body_height)
        .map(|entry| {
            let text = entry.render(state.serial_display_format);
            let color = match entry.direction {
                SerialDirection::Rx => Color::Green,
                SerialDirection::Tx => Color::Yellow,
            };
            Line::from(Span::styled(text, Style::new().fg(color)))
        })
        .collect();

    frame.render_widget(
        Paragraph::new(visible)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(Color::Cyan))
                    .title(Span::styled(
                        " Serial Monitor ",
                        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    )),
            )
            .wrap(Wrap { trim: false }),
        chunks[1],
    );

    // ── Write input bar ───────────────────────────────────────────────────────
    let is_hex_mode = state.serial_display_format == SerialDisplayFormat::Hex;

    // Validate hex input: illegal chars and odd digit count.
    let hex_error: Option<&str> = if is_hex_mode && !state.serial_write_input.is_empty() {
        let input = &state.serial_write_input;
        let has_illegal = input.chars().any(|c| !c.is_ascii_hexdigit() && c != ' ');
        let digit_count = input.chars().filter(|c| !c.is_whitespace()).count();
        if has_illegal {
            Some("Illegal character — only 0-9 A-F a-f and spaces allowed")
        } else if digit_count % 2 != 0 {
            Some("Odd number of hex digits — each byte needs 2 digits")
        } else {
            None
        }
    } else {
        None
    };

    let has_error = hex_error.is_some();
    let (border_style, title_style) = if !state.serial_write_mode {
        (
            Style::new().fg(Color::DarkGray),
            Style::new().fg(Color::DarkGray),
        )
    } else if has_error {
        (
            Style::new().fg(Color::Red),
            Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else {
        (
            Style::new().fg(Color::Yellow),
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )
    };

    let send_title = match state.serial_display_format {
        SerialDisplayFormat::Ascii => " Send [ASCII] ",
        SerialDisplayFormat::Hex => " Send [HEX] ",
    };
    let input_text = format!(
        "{}{}",
        state.serial_write_input,
        if state.serial_write_mode { "▌" } else { "" }
    );
    frame.render_widget(
        Paragraph::new(Span::raw(input_text)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(border_style)
                .title(Span::styled(send_title, title_style)),
        ),
        chunks[2],
    );

    // ── Validation error line ─────────────────────────────────────────────────
    let error_line = if let Some(msg) = hex_error {
        Line::from(Span::styled(
            format!(" ✗ {msg}"),
            Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
        ))
    } else {
        Line::from("")
    };
    frame.render_widget(Paragraph::new(error_line), chunks[3]);

    // ── Hints line ───────────────────────────────────────────────────────────
    let hints: Vec<Span> = if state.serial_write_mode {
        let fmt_hint = match state.serial_display_format {
            SerialDisplayFormat::Ascii => " Type text",
            SerialDisplayFormat::Hex => " Type hex (e.g. 4142 or 41 42)",
        };
        vec![
            Span::styled(fmt_hint, Style::new().fg(Color::DarkGray)),
            Span::styled("  Enter", Style::new().fg(Color::Yellow)),
            Span::styled(" send", Style::new().fg(Color::DarkGray)),
            Span::styled("  Esc", Style::new().fg(Color::Yellow)),
            Span::styled(" cancel", Style::new().fg(Color::DarkGray)),
        ]
    } else {
        vec![
            Span::styled(" w", Style::new().fg(Color::Yellow)),
            Span::styled(" write", Style::new().fg(Color::DarkGray)),
            Span::styled("  x", Style::new().fg(Color::Yellow)),
            Span::styled(" hex/ascii", Style::new().fg(Color::DarkGray)),
            Span::styled("  Space", Style::new().fg(Color::Yellow)),
            Span::styled(" pause", Style::new().fg(Color::DarkGray)),
            Span::styled("  c", Style::new().fg(Color::Yellow)),
            Span::styled(" clear", Style::new().fg(Color::DarkGray)),
            Span::styled("  Esc", Style::new().fg(Color::Yellow)),
            Span::styled(" back", Style::new().fg(Color::DarkGray)),
            Span::styled("  q", Style::new().fg(Color::Yellow)),
            Span::styled(" quit", Style::new().fg(Color::DarkGray)),
        ]
    };
    frame.render_widget(Paragraph::new(Line::from(hints)), chunks[4]);
}
