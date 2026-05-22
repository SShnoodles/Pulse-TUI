use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::core::{ConnectForm, ConnectStatus};

pub fn draw_connect(frame: &mut Frame, form: &ConnectForm) {
    let modal = centered_rect(56, 21, frame.area());

    frame.render_widget(
        Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(Color::Cyan)),
        modal,
    );

    // Inner area (inset from border)
    let inner = Rect {
        x: modal.x + 2,
        y: modal.y + 1,
        width: modal.width.saturating_sub(4),
        height: modal.height.saturating_sub(2),
    };

    let chunks = Layout::vertical([
        Constraint::Length(2), // title
        Constraint::Length(3), // broker
        Constraint::Length(3), // port
        Constraint::Length(3), // username
        Constraint::Length(3), // password
        Constraint::Length(3), // version selector
        Constraint::Min(0),    // spacer
        Constraint::Length(2), // hint / status
    ])
    .split(inner);

    // Title
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "● PULSE TUI",
                Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
            Line::from(Span::styled(
                "MQTT Connection",
                Style::new().fg(Color::DarkGray),
            ))
            .alignment(Alignment::Center),
        ]),
        chunks[0],
    );

    let is_editing = matches!(form.status, ConnectStatus::Idle | ConnectStatus::Error(_));

    // 4 input fields (dim when connecting)
    for (i, chunk) in chunks[1..=4].iter().enumerate() {
        let is_password = i == 3;
        draw_field(
            frame,
            *chunk,
            ConnectForm::LABELS[i],
            &form.values[i],
            i == form.active && is_editing,
            is_password,
        );
    }

    // Version selector row
    draw_version(frame, chunks[5], form, is_editing);

    // Status / hint bar
    let status_line = match &form.status {
        ConnectStatus::Connecting => Line::from(vec![
            Span::styled("⠿ ", Style::new().fg(Color::Yellow)),
            Span::styled("Connecting…  ", Style::new().fg(Color::Yellow)),
            Span::styled("Esc ", Style::new().fg(Color::Cyan)),
            Span::styled("cancel", Style::new().fg(Color::DarkGray)),
        ]),
        ConnectStatus::Error(e) => Line::from(vec![
            Span::styled("✗ ", Style::new().fg(Color::Red)),
            Span::styled(e.clone(), Style::new().fg(Color::Red)),
        ]),
        ConnectStatus::Idle => Line::from(vec![
            Span::styled("Tab", Style::new().fg(Color::Cyan)),
            Span::styled("/", Style::new().fg(Color::DarkGray)),
            Span::styled("↑↓", Style::new().fg(Color::Cyan)),
            Span::styled(" navigate   ", Style::new().fg(Color::DarkGray)),
            Span::styled("Enter", Style::new().fg(Color::Cyan)),
            Span::styled(" connect   ", Style::new().fg(Color::DarkGray)),
            Span::styled("Esc", Style::new().fg(Color::Cyan)),
            Span::styled(" skip auth", Style::new().fg(Color::DarkGray)),
        ]),
    };

    frame.render_widget(Paragraph::new(vec![status_line]), chunks[7]);

    // Overlay: "Connecting" dimmed backdrop
    if matches!(form.status, ConnectStatus::Connecting) {
        let overlay = Rect {
            x: modal.x + 1,
            y: modal.y + 3,
            width: modal.width.saturating_sub(2),
            height: 12,
        };
        frame.render_widget(
            Block::new().style(Style::new().fg(Color::DarkGray)),
            overlay,
        );
    }
}

fn draw_field(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    active: bool,
    secret: bool,
) {
    let cols = Layout::horizontal([
        Constraint::Length(11), // label column
        Constraint::Min(0),     // input column
    ])
    .split(area);

    // Vertically center the label in the 3-row slot
    let label_rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(cols[0]);

    let label_style = if active {
        Style::new().fg(Color::Cyan)
    } else {
        Style::new().fg(Color::Gray)
    };

    frame.render_widget(
        Paragraph::new(Span::styled(format!("{:>10}", label), label_style)),
        label_rows[1],
    );

    // Mask password, then append fake cursor on active field
    let text = if secret { "*".repeat(value.len()) } else { value.to_string() };
    let display = if active { format!("{text}▌") } else { text };

    frame.render_widget(
        Paragraph::new(display).block(
            Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(if active {
                    Style::new().fg(Color::Cyan)
                } else {
                    Style::new().fg(Color::DarkGray)
                }),
        ),
        cols[1],
    );
}

fn draw_version(frame: &mut Frame, area: Rect, form: &ConnectForm, is_editing: bool) {
    let active = form.active == 4 && is_editing;
    let label_style = if active { Style::new().fg(Color::Cyan) } else { Style::new().fg(Color::Gray) };
    let border_style = if active { Style::new().fg(Color::Cyan) } else { Style::new().fg(Color::DarkGray) };
    let arrow_style  = if active { Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::new().fg(Color::DarkGray) };
    let value_style  = if active { Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::new().fg(Color::White) };

    // area is Length(3): split into label col (11) and value col (rest)
    let cols = Layout::horizontal([Constraint::Length(11), Constraint::Min(0)]).split(area);

    // Vertically center label in 3-row slot (same pattern as draw_field)
    let label_rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ]).split(cols[0]);
    frame.render_widget(
        Paragraph::new(Span::styled("   Version", label_style)),
        label_rows[1],
    );

    // Bordered selector uses all 3 rows of cols[1]
    let mut line_spans = vec![
        Span::styled("◀ ", arrow_style),
        Span::styled(form.mqtt_version.label(), value_style),
        Span::styled(" ▶", arrow_style),
    ];
    if active {
        line_spans.push(Span::styled("  ←/→/Space", Style::new().fg(Color::DarkGray)));
    }
    frame.render_widget(
        Paragraph::new(Line::from(line_spans)).block(
            Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(border_style),
        ),
        cols[1],
    );
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect { x, y, width: width.min(area.width), height: height.min(area.height) }
}
