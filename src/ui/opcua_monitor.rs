use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

use crate::core::AppState;

pub fn draw_opcua_monitor(frame: &mut Frame, state: &AppState) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // status line
        Constraint::Min(0),    // body
        Constraint::Length(1), // hints
    ])
    .split(frame.area());

    // ── Status bar ────────────────────────────────────────────────────────────
    let conn_span = if state.connected {
        Span::styled(
            "● CONNECTED",
            Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("○ DISCONNECTED", Style::new().fg(Color::Red))
    };

    let node_count = state.opcua_rows.len();
    let refresh_str = match &state.opcua_last_refresh {
        Some(t) => format!("  ↻ {}", t.format("%H:%M:%S")),
        None => "  ↻ --:--:--".to_string(),
    };

    let mut status_spans = vec![
        conn_span,
        Span::styled(
            format!("  {}", state.broker),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("  nodes: {node_count}"),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(refresh_str, Style::new().fg(Color::DarkGray)),
    ];

    if let Some(ref err) = state.last_error {
        status_spans.push(Span::styled(
            format!("  ✗ {err}"),
            Style::new().fg(Color::Yellow),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(status_spans)), chunks[0]);

    // ── Body ──────────────────────────────────────────────────────────────────
    let body_height = chunks[1].height.saturating_sub(3) as usize;
    let total = state.opcua_rows.len();
    let max_offset = total.saturating_sub(body_height);
    let offset = state.opcua_offset.min(max_offset);

    let rows: Vec<Row> = state
        .opcua_rows
        .iter()
        .enumerate()
        .skip(offset)
        .take(body_height)
        .map(|(idx, r)| {
            let mut row = Row::new(vec![
                Cell::from(r.node_id.clone()),
                Cell::from(r.display_name.clone()),
                Cell::from(r.value.clone()),
                Cell::from(r.data_type.clone()),
                Cell::from(r.source_timestamp.clone()),
                Cell::from(r.server_timestamp.clone()),
            ]);
            if idx == state.opcua_offset {
                row = row.style(Style::new().bg(Color::Rgb(32, 44, 66)));
            }
            row
        })
        .collect();

    let header = Row::new(vec![
        Cell::from("NodeId"),
        Cell::from("DisplayName"),
        Cell::from("Value"),
        Cell::from("DataType"),
        Cell::from("SourceTimestamp"),
        Cell::from("ServerTimestamp"),
    ])
    .style(
        Style::new()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_widget(
        Table::new(rows, opcua_table_constraints(chunks[1].width))
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(Color::Cyan))
                    .title(Span::styled(
                        " OPC UA Monitor ",
                        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    )),
            ),
        chunks[1],
    );

    // ── Hints ─────────────────────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("↑↓", Style::new().fg(Color::Cyan)),
            Span::styled(" select   ", Style::new().fg(Color::DarkGray)),
            Span::styled("a", Style::new().fg(Color::Cyan)),
            Span::styled(" add node   ", Style::new().fg(Color::DarkGray)),
            Span::styled("d", Style::new().fg(Color::Cyan)),
            Span::styled(" delete node   ", Style::new().fg(Color::DarkGray)),
            Span::styled("Esc", Style::new().fg(Color::Cyan)),
            Span::styled(" back   ", Style::new().fg(Color::DarkGray)),
            Span::styled("q", Style::new().fg(Color::Cyan)),
            Span::styled(" quit", Style::new().fg(Color::DarkGray)),
        ])),
        chunks[2],
    );

    if state.confirm_back {
        draw_confirm_back(frame);
    }

    if state.opcua_add_node_mode {
        draw_add_node_modal(frame, &state.opcua_add_node_input);
    }

    if state.opcua_delete_node_mode {
        draw_delete_node_modal(frame, &state.opcua_delete_node_input);
    }
}

fn opcua_table_constraints(width: u16) -> [Constraint; 6] {
    if width < 90 {
        [
            Constraint::Percentage(20),
            Constraint::Percentage(18),
            Constraint::Percentage(16),
            Constraint::Percentage(12),
            Constraint::Percentage(17),
            Constraint::Percentage(17),
        ]
    } else {
        [
            Constraint::Percentage(22),
            Constraint::Percentage(18),
            Constraint::Percentage(14),
            Constraint::Percentage(12),
            Constraint::Percentage(17),
            Constraint::Percentage(17),
        ]
    }
}

fn draw_confirm_back(frame: &mut Frame) {
    let area = frame.area();
    let w = 40u16;
    let h = 5u16;
    let x = area.width.saturating_sub(w) / 2;
    let y = area.height.saturating_sub(h) / 2;
    let popup = Rect::new(x, y, w.min(area.width), h.min(area.height));

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from("  Disconnect and go back? (y/n)"),
        ])
        .block(
            Block::default()
                .title(" Confirm ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        popup,
    );
}

fn draw_add_node_modal(frame: &mut Frame, input: &str) {
    let area = frame.area();
    let w = 64u16;
    let h = 7u16;
    let x = area.width.saturating_sub(w) / 2;
    let y = area.height.saturating_sub(h) / 2;
    let popup = Rect::new(x, y, w.min(area.width), h.min(area.height));

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                format!(" NodeId: {input}_"),
                Style::new().fg(Color::White),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Enter", Style::new().fg(Color::Cyan)),
                Span::styled(" add   ", Style::new().fg(Color::DarkGray)),
                Span::styled("Esc", Style::new().fg(Color::Cyan)),
                Span::styled(" cancel", Style::new().fg(Color::DarkGray)),
            ]),
        ])
        .block(
            Block::default()
                .title(" Add OPC UA Node ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        popup,
    );
}

fn draw_delete_node_modal(frame: &mut Frame, input: &str) {
    let area = frame.area();
    let w = 64u16;
    let h = 7u16;
    let x = area.width.saturating_sub(w) / 2;
    let y = area.height.saturating_sub(h) / 2;
    let popup = Rect::new(x, y, w.min(area.width), h.min(area.height));

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                format!(" NodeId: {input}_"),
                Style::new().fg(Color::White),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Enter", Style::new().fg(Color::Cyan)),
                Span::styled(" delete   ", Style::new().fg(Color::DarkGray)),
                Span::styled("Esc", Style::new().fg(Color::Cyan)),
                Span::styled(" cancel", Style::new().fg(Color::DarkGray)),
            ]),
        ])
        .block(
            Block::default()
                .title(" Delete OPC UA Node ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        popup,
    );
}
