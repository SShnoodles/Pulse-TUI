use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::core::{AppState, Iec104Direction};

pub fn draw_iec104_monitor(frame: &mut Frame, state: &AppState) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(frame.area());

    let connection = if state.connected {
        Span::styled(
            "● CONNECTED / STARTDT SENT",
            Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("○ DISCONNECTED", Style::new().fg(Color::Red))
    };
    let rx = state
        .iec104_entries
        .iter()
        .filter(|entry| entry.direction == Iec104Direction::Rx)
        .count();
    let tx = state.iec104_entries.len().saturating_sub(rx);
    let paused = if state.iec104_paused {
        Span::styled("  ⏸ PAUSED", Style::new().fg(Color::Yellow))
    } else {
        Span::raw("")
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            connection,
            Span::styled(
                format!("  RX {rx}  TX {tx}"),
                Style::new().fg(Color::DarkGray),
            ),
            paused,
        ])),
        chunks[0],
    );

    let height = chunks[1].height.saturating_sub(2) as usize;
    let entry_capacity = (height / 2).max(1);
    let offset = trace_start(
        state.iec104_entries.len(),
        entry_capacity,
        state.iec104_offset,
    );
    let mut lines = Vec::new();
    for entry in state
        .iec104_entries
        .iter()
        .skip(offset)
        .take(entry_capacity)
    {
        let (arrow, color) = match entry.direction {
            Iec104Direction::Rx => ("RX ←", Color::Green),
            Iec104Direction::Tx => ("TX →", Color::Yellow),
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} {arrow} ", entry.timestamp),
                Style::new().fg(color),
            ),
            Span::styled(entry.summary.clone(), Style::new().fg(Color::White)),
        ]));
        lines.push(Line::from(Span::styled(
            format!("             {}", entry.hex()),
            Style::new().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::new()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(Color::Cyan))
                    .title(Span::styled(
                        " IEC 104 APDU Trace ",
                        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    )),
            )
            .wrap(Wrap { trim: false }),
        chunks[1],
    );

    let validation_error = raw_input_error(&state.iec104_write_input);
    let border = if state.iec104_write_mode {
        if validation_error.is_some() {
            Color::Red
        } else {
            Color::Yellow
        }
    } else {
        Color::DarkGray
    };
    let cursor = if state.iec104_write_mode { "▌" } else { "" };
    frame.render_widget(
        Paragraph::new(format!("{}{cursor}", state.iec104_write_input)).block(
            Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(border))
                .title(" Raw APDU (hex, including 68 + length) "),
        ),
        chunks[2],
    );
    let error = validation_error
        .or_else(|| state.last_error.clone())
        .unwrap_or_default();
    frame.render_widget(
        Paragraph::new(error).style(Style::new().fg(Color::Red)),
        chunks[3],
    );

    let hints = if state.iec104_write_mode {
        vec![
            Span::styled("Enter", Style::new().fg(Color::Yellow)),
            Span::styled(" send   ", Style::new().fg(Color::DarkGray)),
            Span::styled("Esc", Style::new().fg(Color::Yellow)),
            Span::styled(" cancel", Style::new().fg(Color::DarkGray)),
        ]
    } else {
        let mut hints = vec![
            Span::styled("g", Style::new().fg(Color::Yellow)),
            Span::styled(
                " general interrogation   ",
                Style::new().fg(Color::DarkGray),
            ),
            Span::styled("w", Style::new().fg(Color::Yellow)),
            Span::styled(" raw APDU   ", Style::new().fg(Color::DarkGray)),
            Span::styled("Space", Style::new().fg(Color::Yellow)),
            Span::styled(" pause   ", Style::new().fg(Color::DarkGray)),
        ];
        if state.iec104_paused || !state.connected {
            hints.extend([
                Span::styled("↑↓", Style::new().fg(Color::Yellow)),
                Span::styled(" scroll   ", Style::new().fg(Color::DarkGray)),
            ]);
        }
        hints.extend([
            Span::styled("c", Style::new().fg(Color::Yellow)),
            Span::styled(" clear   ", Style::new().fg(Color::DarkGray)),
            Span::styled("Esc", Style::new().fg(Color::Yellow)),
            Span::styled(" back", Style::new().fg(Color::DarkGray)),
        ]);
        hints
    };
    frame.render_widget(Paragraph::new(Line::from(hints)), chunks[4]);

    if state.confirm_back {
        draw_confirm_back(frame);
    }
}

fn draw_confirm_back(frame: &mut Frame) {
    let area = frame.area();
    let width = 40u16.min(area.width);
    let height = 5u16.min(area.height);
    let popup = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from("  Disconnect and go back? (y/n)"),
        ])
        .block(
            Block::new()
                .title(" Confirm ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::new().fg(Color::Yellow)),
        ),
        popup,
    );
}

fn raw_input_error(input: &str) -> Option<String> {
    if input.is_empty() {
        return None;
    }
    if input
        .chars()
        .any(|character| !character.is_ascii_hexdigit() && !character.is_whitespace())
    {
        return Some("Only hexadecimal bytes and spaces are allowed".into());
    }
    let digits = input
        .chars()
        .filter(|character| !character.is_whitespace())
        .count();
    if digits % 2 != 0 {
        return Some("Each APDU byte needs two hexadecimal digits".into());
    }
    None
}

fn trace_start(total: usize, capacity: usize, scrollback: usize) -> usize {
    let newest_page = total.saturating_sub(capacity);
    newest_page.saturating_sub(scrollback.min(newest_page))
}

#[cfg(test)]
mod tests {
    use super::trace_start;

    #[test]
    fn trace_follows_latest_and_scrolls_back_immediately() {
        assert_eq!(trace_start(100, 10, 0), 90);
        assert_eq!(trace_start(100, 10, 1), 89);
        assert_eq!(trace_start(100, 10, 500), 0);
        assert_eq!(trace_start(5, 10, 3), 0);
    }
}
