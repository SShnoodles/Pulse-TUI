use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Sparkline, Wrap},
    Frame,
};

use super::highlight::{detect_format, highlight_line};
use super::panel::Panel;
use super::style::{border_style, title_style};
use crate::core::{AppState, Message, SourceKind};

pub fn draw(frame: &mut Frame, state: &AppState, focus: Panel) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // status line (stats + error)
        Constraint::Min(0),    // body
        Constraint::Length(1), // hints line (keys / mode)
    ])
    .split(frame.area());

    draw_statusline(frame, chunks[0], state);
    draw_body(frame, chunks[1], state, focus);
    draw_hints(frame, chunks[2], state);

    if state.confirm_back {
        draw_confirm_back(frame);
    }
    if state.publish_mode {
        draw_publish_modal(frame, state);
    }
}

fn draw_publish_modal(frame: &mut Frame, state: &AppState) {
    let modal = centered_rect(62, 7, frame.area());
    frame.render_widget(Clear, modal);
    frame.render_widget(
        Block::new()
            .title(Span::styled(
                " Publish ",
                Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(Color::Magenta)),
        modal,
    );
    let inner = Rect {
        x: modal.x + 2,
        y: modal.y + 1,
        width: modal.width.saturating_sub(4),
        height: modal.height.saturating_sub(2),
    };
    let topic_label = state.selected_topic_name().unwrap_or("");
    // "Payload " = 8 cols, "█" = 1 col; show the tail of input that fits
    let text_cols = inner.width.saturating_sub(8 + 1) as usize;
    let visible_input: &str = {
        let s = state.publish_input.as_str();
        let char_count = s.chars().count();
        if char_count <= text_cols {
            s
        } else {
            let skip = char_count - text_cols;
            let byte_offset = s.char_indices().nth(skip).map(|(i, _)| i).unwrap_or(0);
            &s[byte_offset..]
        }
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Topic   ", Style::new().fg(Color::DarkGray)),
                Span::styled(topic_label, Style::new().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("Payload ", Style::new().fg(Color::DarkGray)),
                Span::styled(visible_input.to_string(), Style::new().fg(Color::White)),
                Span::styled("█", Style::new().fg(Color::White)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Enter ", Style::new().fg(Color::Green)),
                Span::styled("send   ", Style::new().fg(Color::DarkGray)),
                Span::styled("Esc ", Style::new().fg(Color::Cyan)),
                Span::styled("cancel", Style::new().fg(Color::DarkGray)),
            ]),
        ]),
        inner,
    );
}

fn draw_confirm_back(frame: &mut Frame) {
    let modal = centered_rect(50, 7, frame.area());
    frame.render_widget(Clear, modal);
    frame.render_widget(
        Block::new()
            .title(Span::styled(
                " Disconnect ",
                Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(Color::Yellow)),
        modal,
    );
    let inner = Rect {
        x: modal.x + 2,
        y: modal.y + 1,
        width: modal.width.saturating_sub(4),
        height: modal.height.saturating_sub(2),
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::raw("Disconnect and return to the connection form?")),
            Line::from(""),
            Line::from(vec![
                Span::styled("Enter / y ", Style::new().fg(Color::Green)),
                Span::styled("confirm   ", Style::new().fg(Color::DarkGray)),
                Span::styled("any other key ", Style::new().fg(Color::Cyan)),
                Span::styled("cancel", Style::new().fg(Color::DarkGray)),
            ]),
        ]),
        inner,
    );
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

fn draw_statusline(frame: &mut Frame, area: Rect, state: &AppState) {
    // ── Error mode: replace connection section with error ─────────────────────
    if let Some(ref err) = state.last_error {
        let max_len = area.width.saturating_sub(30) as usize;
        let msg = if err.len() > max_len {
            &err[..max_len]
        } else {
            err.as_str()
        };
        let spans = vec![
            Span::styled(
                " PULSE ",
                Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  │  ", Style::new().fg(Color::DarkGray)),
            Span::styled(
                " ✗ ",
                Style::new()
                    .fg(Color::White)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {}  ", msg), Style::new().fg(Color::Red)),
            Span::styled("  │  ", Style::new().fg(Color::DarkGray)),
            Span::raw(state.broker.clone()),
            Span::styled("   c ", Style::new().fg(Color::Cyan)),
            Span::styled("clear", Style::new().fg(Color::DarkGray)),
        ];
        frame.render_widget(
            Paragraph::new(Line::from(spans))
                .style(Style::new().bg(Color::DarkGray).fg(Color::White)),
            area,
        );
        return;
    }

    // ── Normal mode ───────────────────────────────────────────────────────────
    let (dot, conn_color, conn_label) = if state.connected {
        ("●", Color::Green, "Connected")
    } else {
        ("○", Color::Red, "Disconnected")
    };

    let mut spans = vec![
        Span::styled(
            " PULSE ",
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  │  ", Style::new().fg(Color::DarkGray)),
        Span::styled(dot, Style::new().fg(conn_color)),
        Span::styled(format!(" {} ", conn_label), Style::new().fg(conn_color)),
        Span::styled("  │  ", Style::new().fg(Color::DarkGray)),
        Span::raw(state.broker.clone()),
        Span::styled("  │  ", Style::new().fg(Color::DarkGray)),
        Span::styled(
            format!("{} topics", state.topics.len()),
            Style::new().fg(Color::White),
        ),
        Span::styled("  │  ", Style::new().fg(Color::DarkGray)),
        Span::styled(state.mqtt_version, Style::new().fg(Color::DarkGray)),
    ];

    if state.paused {
        spans.push(Span::styled("  │  ", Style::new().fg(Color::DarkGray)));
        spans.push(Span::styled(
            "⏸ PAUSED",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }

    if !state.search_query.is_empty() {
        spans.push(Span::styled(
            "  │  filter: ",
            Style::new().fg(Color::DarkGray),
        ));
        spans.push(Span::styled(
            state.search_query.clone(),
            Style::new().fg(Color::Cyan),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::new().bg(Color::DarkGray).fg(Color::White)),
        area,
    );
}

fn draw_hints(frame: &mut Frame, area: Rect, state: &AppState) {
    let line = if state.yank_mode {
        Line::from(vec![
            Span::styled(
                " YANK ",
                Style::new()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ←→ ", Style::new().fg(Color::Cyan)),
            Span::styled("select   ", Style::new().fg(Color::DarkGray)),
            Span::styled("↑↓ ", Style::new().fg(Color::Cyan)),
            Span::styled("switch msg   ", Style::new().fg(Color::DarkGray)),
            Span::styled("y ", Style::new().fg(Color::Cyan)),
            Span::styled("copy   ", Style::new().fg(Color::DarkGray)),
            Span::styled("Esc ", Style::new().fg(Color::Cyan)),
            Span::styled("cancel", Style::new().fg(Color::DarkGray)),
        ])
    } else if state.subscribe_mode {
        Line::from(vec![
            Span::styled(
                " SUB ",
                Style::new()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  topic: ", Style::new().fg(Color::DarkGray)),
            Span::styled(state.subscribe_input.clone(), Style::new().fg(Color::White)),
            Span::styled("█", Style::new().fg(Color::White)),
            Span::styled("  Enter ", Style::new().fg(Color::Cyan)),
            Span::styled("confirm   ", Style::new().fg(Color::DarkGray)),
            Span::styled("Esc ", Style::new().fg(Color::Cyan)),
            Span::styled("cancel", Style::new().fg(Color::DarkGray)),
        ])
    } else if state.search_mode {
        Line::from(vec![
            Span::styled(
                " SEARCH ",
                Style::new()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::new().fg(Color::DarkGray)),
            Span::styled(state.search_query.clone(), Style::new().fg(Color::White)),
            Span::styled("█", Style::new().fg(Color::White)),
            Span::styled("  Enter ", Style::new().fg(Color::Cyan)),
            Span::styled("confirm   ", Style::new().fg(Color::DarkGray)),
            Span::styled("Esc ", Style::new().fg(Color::Cyan)),
            Span::styled("cancel", Style::new().fg(Color::DarkGray)),
        ])
    } else {
        let mut spans = vec![
            Span::styled(" Tab ", Style::new().fg(Color::Cyan)),
            Span::styled("switch   ", Style::new().fg(Color::DarkGray)),
            Span::styled("s ", Style::new().fg(Color::Cyan)),
            Span::styled("subscribe   ", Style::new().fg(Color::DarkGray)),
            Span::styled("/ ", Style::new().fg(Color::Cyan)),
            Span::styled("search   ", Style::new().fg(Color::DarkGray)),
            Span::styled("Space ", Style::new().fg(Color::Cyan)),
            Span::styled(
                if state.paused {
                    "resume   "
                } else {
                    "pause   "
                },
                if state.paused {
                    Style::new().fg(Color::Yellow)
                } else {
                    Style::new().fg(Color::DarkGray)
                },
            ),
        ];
        if state.paused {
            spans.push(Span::styled("y ", Style::new().fg(Color::Cyan)));
            spans.push(Span::styled("yank   ", Style::new().fg(Color::DarkGray)));
        }
        if state.selected_topic_idx.is_some() && state.source_kind == SourceKind::Mqtt {
            spans.push(Span::styled("p ", Style::new().fg(Color::Magenta)));
            spans.push(Span::styled("publish   ", Style::new().fg(Color::DarkGray)));
        }
        spans.push(Span::styled("Esc ", Style::new().fg(Color::Cyan)));
        spans.push(Span::styled("back   ", Style::new().fg(Color::DarkGray)));
        spans.push(Span::styled("q ", Style::new().fg(Color::Cyan)));
        spans.push(Span::styled("quit", Style::new().fg(Color::DarkGray)));
        Line::from(spans)
    };

    frame.render_widget(Paragraph::new(line), area);
}

fn draw_body(frame: &mut Frame, area: Rect, state: &AppState, focus: Panel) {
    let chunks = Layout::horizontal([Constraint::Percentage(28), Constraint::Min(0)]).split(area);

    draw_topics(frame, chunks[0], state, focus == Panel::Topics);
    draw_messages(frame, chunks[1], state, focus == Panel::Messages);
}

fn draw_topics(frame: &mut Frame, area: Rect, state: &AppState, focused: bool) {
    let show_sparkline = area.height >= 8;
    let chunks = if show_sparkline {
        Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).split(area)
    } else {
        Layout::vertical([Constraint::Min(0)]).split(area)
    };

    let list_area = chunks[0];
    let block = Block::new()
        .title(Span::styled(
            format!(" Topics ({}) ", state.topics.len()),
            title_style(focused),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style(focused));

    let items: Vec<ListItem> = state
        .topics
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let is_selected = state.selected_topic_idx == Some(i);
            let prefix = if is_selected { "▶ " } else { "  " };
            let name_style = if is_selected {
                Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(Color::White)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{}{}", prefix, t.name), name_style),
                Span::styled(
                    format!("  {}  ", t.msg_count),
                    Style::new().fg(Color::DarkGray),
                ),
                Span::styled(
                    if t.tps > 0 {
                        format!("{}t/s", t.tps)
                    } else {
                        String::new()
                    },
                    Style::new().fg(Color::Yellow),
                ),
            ]))
        })
        .collect();

    let hint = if state.selected_topic_idx.is_some() {
        Span::styled(" Esc: all  d: delete ", Style::new().fg(Color::DarkGray))
    } else {
        Span::raw("")
    };

    frame.render_widget(List::new(items).block(block.title_bottom(hint)), list_area);

    if show_sparkline {
        let spark_area = chunks[1];
        let (title, points): (String, Vec<u64>) = state
            .selected_topic_idx
            .and_then(|i| state.topics.get(i))
            .map(|t| {
                (
                    format!(" TPS — {} ", t.name),
                    if t.tps_history.is_empty() {
                        vec![0]
                    } else {
                        t.tps_history.clone()
                    },
                )
            })
            .unwrap_or_else(|| (" TPS — select a topic ".to_string(), vec![0]));

        let (min_tps, max_tps, cur_tps) = points
            .iter()
            .copied()
            .fold((u64::MAX, 0u64, 0u64), |(min_v, max_v, _), v| {
                (min_v.min(v), max_v.max(v), v)
            });
        let min_tps = if min_tps == u64::MAX { 0 } else { min_tps };
        let legend = format!(" min {min_tps}  max {max_tps}  now {cur_tps} ");

        frame.render_widget(
            Sparkline::default()
                .block(
                    Block::new()
                        .title(Span::styled(title, Style::new().fg(Color::Yellow)))
                        .title_bottom(Span::styled(legend, Style::new().fg(Color::DarkGray)))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::new().fg(Color::DarkGray)),
                )
                .data(&points)
                .style(Style::new().fg(Color::Yellow)),
            spark_area,
        );
    }
}

fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if width == 0 || line.is_empty() {
        return vec![line.to_string()];
    }
    let chars: Vec<char> = line.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars.chunks(width).map(|c| c.iter().collect()).collect()
}

fn draw_messages(frame: &mut Frame, area: Rect, state: &AppState, focused: bool) {
    let yank_height: u16 = if state.yank_mode && area.height > 8 {
        (area.height / 3).max(4)
    } else {
        0
    };

    let (list_area, yank_area) = if yank_height > 0 {
        let chunks =
            Layout::vertical([Constraint::Min(3), Constraint::Length(yank_height)]).split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let msg_title = match state.selected_topic_idx.and_then(|i| state.topics.get(i)) {
        Some(t) => format!(" Messages — {} ", t.name),
        None => " Messages ".to_string(),
    };
    let block = Block::new()
        .title(Span::styled(msg_title, title_style(focused)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style(focused));

    let inner_width = list_area.width.saturating_sub(2) as usize;
    let visible_lines = list_area.height.saturating_sub(2) as usize;

    let all: Vec<&Message> = state.filtered_messages().collect();

    let mut msg_start_line: Vec<usize> = Vec::with_capacity(all.len());
    let mut all_lines: Vec<Line> = Vec::new();

    for (idx, msg) in all.iter().enumerate() {
        msg_start_line.push(all_lines.len());
        let is_sel = state.msg_cursor == Some(idx);
        let bg = if is_sel {
            Color::DarkGray
        } else {
            Color::Reset
        };

        let fmt = detect_format(&msg.payload);
        let search = state.search_query.as_str();

        // Timestamp + QoS + retained flag
        let qos_label = format!("QoS{}", msg.qos);
        let mut meta_spans = vec![
            Span::styled(
                format!("[{}]", msg.timestamp),
                Style::new().fg(Color::DarkGray),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(qos_label, Style::new().fg(Color::Cyan)),
        ];
        if msg.retained {
            meta_spans.push(Span::styled("  R", Style::new().fg(Color::Yellow)));
        }
        all_lines.push(Line::from(meta_spans).style(Style::new().bg(bg)));
        // Payload (multi-line + syntax highlight + word-wrap)
        if msg.payload.is_empty() {
            all_lines.push(Line::from("").style(Style::new().bg(bg)));
        } else {
            for pline in msg.payload.lines() {
                for w in wrap_line(pline, inner_width) {
                    let spans = highlight_line(&w, fmt, search);
                    all_lines.push(Line::from(spans).style(Style::new().bg(bg)));
                }
            }
        }
        // Separator
        all_lines.push(Line::from(Span::styled(
            "─".repeat(inner_width.min(60)),
            Style::new().fg(Color::DarkGray),
        )));
    }

    let total = all_lines.len();
    let scroll: u16 = if let Some(ci) = state.msg_cursor {
        let ci = ci.min(all.len().saturating_sub(1));
        if let Some(&sl) = msg_start_line.get(ci) {
            sl.saturating_sub(visible_lines / 3)
                .min(total.saturating_sub(visible_lines)) as u16
        } else {
            0
        }
    } else {
        total.saturating_sub(visible_lines) as u16
    };

    frame.render_widget(
        Paragraph::new(all_lines).block(block).scroll((scroll, 0)),
        list_area,
    );

    // ── Yank pane ────────────────────────────────────────────────────────────
    if let (Some(yr), Some(msg)) = (yank_area, state.selected_message()) {
        let len = msg.payload.len();
        let start = state.yank_start.min(state.yank_cursor).min(len);
        let end = state.yank_start.max(state.yank_cursor).min(len);

        let content = if start == end {
            vec![Line::from(vec![
                Span::raw(msg.payload[..start].to_string()),
                Span::styled("█", Style::new().fg(Color::Yellow)),
                Span::raw(msg.payload[start..].to_string()),
            ])]
        } else {
            vec![Line::from(vec![
                Span::raw(msg.payload[..start].to_string()),
                Span::styled(
                    msg.payload[start..end].to_string(),
                    Style::new().bg(Color::Blue).fg(Color::White),
                ),
                Span::raw(msg.payload[end..].to_string()),
            ])]
        };

        let yb = Block::new()
            .title(Span::styled(
                " YANK — ←→ select  ↑↓ switch msg  y copy  Esc cancel ",
                Style::new().fg(Color::Yellow),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(Color::Yellow));

        frame.render_widget(
            Paragraph::new(content).block(yb).wrap(Wrap { trim: false }),
            yr,
        );
    }
}
