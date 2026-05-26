use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

use crate::core::{AppState, DisplayFormat, FunctionCode};

// ── Public entry point ────────────────────────────────────────────────────────

pub fn draw_modbus_monitor(frame: &mut Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_modbus_status(frame, chunks[0], state);
    draw_modbus_body(frame, chunks[1], state);
    draw_modbus_hints(frame, chunks[2], state);

    if state.confirm_back {
        draw_confirm_back(frame);
    }
}

// ── Status bar ────────────────────────────────────────────────────────────────

fn draw_modbus_status(frame: &mut Frame, area: Rect, state: &AppState) {
    let (conn_label, conn_color) = if state.connected {
        ("CONNECTED", Color::Green)
    } else {
        ("DISCONNECTED", Color::Red)
    };

    let mut spans = vec![
        Span::raw(" "),
        Span::styled(
            conn_label,
            Style::default().fg(conn_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  {}", state.broker)),
    ];

    if let Some(err) = &state.last_error {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("ERR: {}", err),
            Style::default().fg(Color::Yellow),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray)),
        area,
    );
}

// ── Body (query panel + data table) ──────────────────────────────────────────

fn draw_modbus_body(frame: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(34), Constraint::Min(0)])
        .split(area);

    draw_query_panel(frame, chunks[0], state);
    draw_data_table(frame, chunks[1], state);
}

// ── Query panel ───────────────────────────────────────────────────────────────

fn field_style(active: usize, field: usize, editing: bool) -> Style {
    if active == field {
        if editing {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        }
    } else {
        Style::default().fg(Color::White)
    }
}

fn draw_query_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let q = &state.modbus_query;

    let block = Block::default()
        .title(" Query ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if q.editing {
            Color::Cyan
        } else {
            Color::DarkGray
        }));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build lines
    let mut lines: Vec<Line> = Vec::new();

    // FC selector
    lines.push(Line::from(vec![
        Span::raw(" "),
        Span::styled("Function Code", Style::default().fg(Color::Gray)),
    ]));
    let fc_label = format!(" ◀ {} ▶", q.fc().label());
    lines.push(Line::from(Span::styled(
        format!(" {:<30}", fc_label),
        field_style(q.active, 0, q.editing),
    )));
    lines.push(Line::from(""));

    // Start address
    lines.push(Line::from(vec![
        Span::raw(" "),
        Span::styled("Start Address", Style::default().fg(Color::Gray)),
    ]));
    let start_display = if q.active == 1 && q.editing {
        format!(" {}_", q.start_input)
    } else {
        format!(" {}", q.start_input)
    };
    lines.push(Line::from(Span::styled(
        format!(" {:<30}", start_display),
        field_style(q.active, 1, q.editing),
    )));
    lines.push(Line::from(""));

    // Quantity
    lines.push(Line::from(vec![
        Span::raw(" "),
        Span::styled("Quantity", Style::default().fg(Color::Gray)),
    ]));
    let qty_display = if q.active == 2 && q.editing {
        format!(" {}_", q.qty_input)
    } else {
        format!(" {}", q.qty_input)
    };
    lines.push(Line::from(Span::styled(
        format!(" {:<30}", qty_display),
        field_style(q.active, 2, q.editing),
    )));
    lines.push(Line::from(""));

    // Display format selector
    lines.push(Line::from(vec![
        Span::raw(" "),
        Span::styled("Display Format", Style::default().fg(Color::Gray)),
    ]));
    let fmt_label = format!(" ◀ {} ▶", q.format().label());
    lines.push(Line::from(Span::styled(
        format!(" {:<30}", fmt_label),
        field_style(q.active, 3, q.editing),
    )));

    frame.render_widget(Paragraph::new(lines), inner);
}

// ── Data table ────────────────────────────────────────────────────────────────

fn draw_data_table(frame: &mut Frame, area: Rect, state: &AppState) {
    let q = &state.modbus_query;
    let fmt = q.format();
    let fc = q.fc();

    let header_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let header = Row::new(vec![
        Cell::from("Address").style(header_style),
        Cell::from("Hex").style(header_style),
        Cell::from("Binary").style(header_style),
        Cell::from("Display").style(header_style),
    ]);

    let rows: Vec<Row> = state
        .modbus_rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let display = format_display(&state.modbus_rows, i, fmt, fc);
            Row::new(vec![
                Cell::from(format!("{:5}", row.address)),
                Cell::from(format!("{:#06X}", row.value)),
                Cell::from(format!("{:016b}", row.value)),
                Cell::from(display),
            ])
        })
        .collect();

    let offset = state.modbus_table_offset;
    let visible_rows: Vec<Row> = rows.into_iter().skip(offset).collect();

    let table = Table::new(
        visible_rows,
        [
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(18),
            Constraint::Min(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(" Data ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)),
    )
    .row_highlight_style(Style::default().fg(Color::Cyan));

    frame.render_widget(table, area);
}

// ── Display value formatter ───────────────────────────────────────────────────

fn format_display(
    rows: &[crate::core::ModbusRow],
    idx: usize,
    fmt: DisplayFormat,
    fc: FunctionCode,
) -> String {
    if fc.is_bit() {
        return if rows[idx].value != 0 {
            "ON".into()
        } else {
            "OFF".into()
        };
    }

    let reg_count = fmt.register_count();

    // For multi-register formats, only the first register in each group shows a value
    if reg_count > 1 && idx % reg_count != 0 {
        return String::new();
    }

    match fmt {
        DisplayFormat::Unsigned => rows[idx].value.to_string(),
        DisplayFormat::Signed => (rows[idx].value as i16).to_string(),
        DisplayFormat::Hex => format!("{:#06X}", rows[idx].value),
        DisplayFormat::Binary => format!("{:016b}", rows[idx].value),
        DisplayFormat::Long => {
            if idx + 1 < rows.len() {
                let hi = rows[idx].value as u32;
                let lo = rows[idx + 1].value as u32;
                ((hi << 16 | lo) as i32).to_string()
            } else {
                String::new()
            }
        }
        DisplayFormat::LongInverse => {
            if idx + 1 < rows.len() {
                let lo = rows[idx].value as u32;
                let hi = rows[idx + 1].value as u32;
                ((hi << 16 | lo) as i32).to_string()
            } else {
                String::new()
            }
        }
        DisplayFormat::Float => {
            if idx + 1 < rows.len() {
                let bytes = u32::from(rows[idx].value) << 16 | u32::from(rows[idx + 1].value);
                format!("{:.4}", f32::from_bits(bytes))
            } else {
                String::new()
            }
        }
        DisplayFormat::FloatInverse => {
            if idx + 1 < rows.len() {
                let bytes = u32::from(rows[idx + 1].value) << 16 | u32::from(rows[idx].value);
                format!("{:.4}", f32::from_bits(bytes))
            } else {
                String::new()
            }
        }
        DisplayFormat::Double => {
            if idx + 3 < rows.len() {
                let b: u64 = (u64::from(rows[idx].value) << 48)
                    | (u64::from(rows[idx + 1].value) << 32)
                    | (u64::from(rows[idx + 2].value) << 16)
                    | u64::from(rows[idx + 3].value);
                format!("{:.6}", f64::from_bits(b))
            } else {
                String::new()
            }
        }
        DisplayFormat::DoubleInverse => {
            if idx + 3 < rows.len() {
                let b: u64 = (u64::from(rows[idx + 3].value) << 48)
                    | (u64::from(rows[idx + 2].value) << 32)
                    | (u64::from(rows[idx + 1].value) << 16)
                    | u64::from(rows[idx].value);
                format!("{:.6}", f64::from_bits(b))
            } else {
                String::new()
            }
        }
    }
}

// ── Hints bar ─────────────────────────────────────────────────────────────────

fn draw_modbus_hints(frame: &mut Frame, area: Rect, state: &AppState) {
    let q = &state.modbus_query;

    let line = if q.editing {
        if q.active == 0 || q.active == 3 {
            Line::from(vec![
                Span::styled(
                    " EDIT ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ←→ ", Style::default().fg(Color::Cyan)),
                Span::styled("change   ", Style::default().fg(Color::DarkGray)),
                Span::styled("Tab ", Style::default().fg(Color::Cyan)),
                Span::styled("next field   ", Style::default().fg(Color::DarkGray)),
                Span::styled("Enter ", Style::default().fg(Color::Cyan)),
                Span::styled("send query   ", Style::default().fg(Color::DarkGray)),
                Span::styled("Esc ", Style::default().fg(Color::Cyan)),
                Span::styled("cancel", Style::default().fg(Color::DarkGray)),
            ])
        } else {
            Line::from(vec![
                Span::styled(
                    " EDIT ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  0-9 ", Style::default().fg(Color::Cyan)),
                Span::styled("type   ", Style::default().fg(Color::DarkGray)),
                Span::styled("BS ", Style::default().fg(Color::Cyan)),
                Span::styled("delete   ", Style::default().fg(Color::DarkGray)),
                Span::styled("Tab ", Style::default().fg(Color::Cyan)),
                Span::styled("next field   ", Style::default().fg(Color::DarkGray)),
                Span::styled("Enter ", Style::default().fg(Color::Cyan)),
                Span::styled("send query   ", Style::default().fg(Color::DarkGray)),
                Span::styled("Esc ", Style::default().fg(Color::Cyan)),
                Span::styled("cancel", Style::default().fg(Color::DarkGray)),
            ])
        }
    } else {
        Line::from(vec![
            Span::styled("  e ", Style::default().fg(Color::Cyan)),
            Span::styled("edit query   ", Style::default().fg(Color::DarkGray)),
            Span::styled("↑↓ ", Style::default().fg(Color::Cyan)),
            Span::styled("scroll   ", Style::default().fg(Color::DarkGray)),
            Span::styled("c ", Style::default().fg(Color::Cyan)),
            Span::styled("clear error   ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc ", Style::default().fg(Color::Cyan)),
            Span::styled("back   ", Style::default().fg(Color::DarkGray)),
            Span::styled("q ", Style::default().fg(Color::Cyan)),
            Span::styled("quit", Style::default().fg(Color::DarkGray)),
        ])
    };

    frame.render_widget(
        Paragraph::new(line).style(Style::default().fg(Color::White)),
        area,
    );
}

// ── Confirm back dialog ───────────────────────────────────────────────────────

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
