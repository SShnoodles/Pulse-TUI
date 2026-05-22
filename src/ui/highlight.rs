use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

#[derive(Clone, Copy)]
pub enum PayloadFormat {
    Json,
    Plain,
}

pub fn detect_format(s: &str) -> PayloadFormat {
    let t = s.trim();
    if t.starts_with('{') || t.starts_with('[') {
        PayloadFormat::Json
    } else {
        PayloadFormat::Plain
    }
}

/// Build highlighted spans for one (already-wrapped) line.
/// Background is applied at the `Line` level by the caller.
pub fn highlight_line(line: &str, format: PayloadFormat, search: &str) -> Vec<Span<'static>> {
    if !search.is_empty() {
        return highlight_search(line, search);
    }
    match format {
        PayloadFormat::Json  => highlight_json(line),
        PayloadFormat::Plain => vec![Span::raw(line.to_string())],
    }
}

// ── Search highlighting ──────────────────────────────────────────────────────

fn highlight_search(line: &str, query: &str) -> Vec<Span<'static>> {
    let lower_line  = line.to_lowercase();
    let lower_query = query.to_lowercase();
    let mut spans   = Vec::new();
    let mut pos     = 0usize;

    loop {
        match lower_line[pos..].find(lower_query.as_str()) {
            None => {
                spans.push(Span::raw(line[pos..].to_string()));
                break;
            }
            Some(offset) => {
                let s = pos + offset;
                let e = s + lower_query.len();
                if s > pos {
                    spans.push(Span::raw(line[pos..s].to_string()));
                }
                spans.push(Span::styled(
                    line[s..e].to_string(),
                    Style::new()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
                pos = e;
            }
        }
    }

    if spans.is_empty() {
        spans.push(Span::raw(line.to_string()));
    }
    spans
}

// ── JSON highlighting ────────────────────────────────────────────────────────

fn highlight_json(line: &str) -> Vec<Span<'static>> {
    let bytes = line.as_bytes();
    let len   = bytes.len();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut i = 0usize; // byte position

    while i < len {
        if bytes[i] == b'"' {
            // ── String ──────────────────────────────────────────────────────
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' { i += 2; continue; }
                if bytes[i] == b'"'  { i += 1; break; }
                i += 1;
            }
            let s = line[start..i].to_string();
            // Key if immediately followed by `:` (ignore whitespace)
            let color = if line[i..].trim_start_matches(|c: char| c == ' ' || c == '\t').starts_with(':') {
                Color::Cyan
            } else {
                Color::Green
            };
            spans.push(Span::styled(s, Style::new().fg(color)));

        } else if bytes[i].is_ascii_digit()
            || (bytes[i] == b'-' && i + 1 < len && bytes[i + 1].is_ascii_digit())
        {
            // ── Number ──────────────────────────────────────────────────────
            let start = i;
            i += 1;
            while i < len
                && (bytes[i].is_ascii_digit()
                    || bytes[i] == b'.'
                    || bytes[i] == b'e'
                    || bytes[i] == b'E'
                    || bytes[i] == b'+')
            {
                i += 1;
            }
            spans.push(Span::styled(
                line[start..i].to_string(),
                Style::new().fg(Color::Yellow),
            ));

        } else if line[i..].starts_with("true") {
            spans.push(Span::styled(
                "true",
                Style::new().fg(Color::LightGreen).add_modifier(Modifier::BOLD),
            ));
            i += 4;

        } else if line[i..].starts_with("false") {
            spans.push(Span::styled("false", Style::new().fg(Color::LightRed)));
            i += 5;

        } else if line[i..].starts_with("null") {
            spans.push(Span::styled("null", Style::new().fg(Color::DarkGray)));
            i += 4;

        } else if matches!(bytes[i], b'{' | b'}' | b'[' | b']') {
            // ── Bracket ─────────────────────────────────────────────────────
            spans.push(Span::styled(
                line[i..i + 1].to_string(),
                Style::new().fg(Color::White).add_modifier(Modifier::BOLD),
            ));
            i += 1;

        } else if bytes[i] == b':' {
            spans.push(Span::styled(":", Style::new().fg(Color::DarkGray)));
            i += 1;

        } else if bytes[i] == b',' {
            spans.push(Span::styled(",", Style::new().fg(Color::DarkGray)));
            i += 1;

        } else {
            // Whitespace / UTF-8 continuation / any other char
            let start = i;
            i += 1;
            // Advance through UTF-8 continuation bytes
            while i < len && (bytes[i] & 0xC0) == 0x80 { i += 1; }
            spans.push(Span::raw(line[start..i].to_string()));
        }
    }

    if spans.is_empty() {
        spans.push(Span::raw(line.to_string()));
    }
    spans
}
