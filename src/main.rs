mod config;
mod core;
mod events;
mod modbus;
mod mqtt;
mod serial;
mod ui;

use std::{io::stdout, time::Duration};

use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    core::{
        AppEvent, AppMode, AppState, ConnectForm, ConnectStatus, ModbusForm, ModbusRow,
        SerialDisplayFormat, SerialEntry, SerialForm, SourceKind,
    },
    events::{new_event_channel, EventTx},
    modbus::{ModbusCommand, ModbusConfig, ModbusSource},
    mqtt::{MqttCommand, MqttConfig, MqttSource},
    serial::{SerialCommand, SerialConfig, SerialSource},
    ui::{
        draw, draw_connect, draw_modbus_connect, draw_modbus_monitor, draw_serial_connect,
        draw_serial_monitor, draw_source_select, Panel,
    },
};

fn random_hex_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .subsec_nanos();
    format!("{:04x}", (nanos & 0xFFFF) as u16)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableBracketedPaste
    )?;
    terminal.show_cursor()?;

    result
}

async fn run(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> anyhow::Result<()> {
    let (tx, mut rx) = new_event_channel();

    // Load saved config
    let saved = config::load();
    let mut form = ConnectForm::new(&saved.mqtt.host, saved.mqtt.port);
    form.values[2] = saved.mqtt.username.clone();
    form.mqtt_version = saved.mqtt_version();

    // Initial topics from saved config
    let initial_topics: Vec<String> = saved.mqtt.topics.clone();
    let mut mode = AppMode::SourceSelect;
    let mut source_select_idx: usize = 0;
    let mut state = AppState::default();
    let mut active_panel = Panel::default();
    let mut mqtt_cmd: Option<UnboundedSender<MqttCommand>> = None;
    let mut modbus_cmd: Option<UnboundedSender<ModbusCommand>> = None;
    let mut serial_cmd: Option<UnboundedSender<SerialCommand>> = None;
    let mut modbus_form = ModbusForm::new();
    modbus_form.values[0] = saved.modbus.host.clone();
    modbus_form.values[1] = saved.modbus.port.to_string();
    modbus_form.values[2] = saved.modbus.unit_id.to_string();
    modbus_form.values[3] = saved.modbus.poll_interval_ms.to_string();

    let mut serial_form = SerialForm::new();
    if !saved.serial.port.is_empty() {
        serial_form.select_port(&saved.serial.port);
    }
    serial_form.baud_idx = [
        1200u32, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600,
    ]
    .iter()
    .position(|&b| b == saved.serial.baud_rate)
    .unwrap_or(7);
    serial_form.parity_idx = match saved.serial.parity.as_str() {
        "Odd" => 1,
        "Even" => 2,
        _ => 0,
    };
    serial_form.stop_idx = if saved.serial.stop_bits == 2 { 1 } else { 0 };

    // Tick task
    let tick_tx = tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            interval.tick().await;
            if tick_tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    });

    // Keyboard / paste task
    let key_tx = tx.clone();
    tokio::task::spawn_blocking(move || loop {
        if key_tx.is_closed() {
            break;
        }

        match event::poll(Duration::from_millis(50)) {
            Ok(true) => match event::read() {
                Ok(Event::Key(k)) if k.kind != KeyEventKind::Release => {
                    if key_tx.send(AppEvent::Key(k)).is_err() {
                        break;
                    }
                }
                Ok(Event::Paste(s)) => {
                    if key_tx.send(AppEvent::Paste(s)).is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            },
            Ok(false) => {}
            Err(_) => break,
        }
    });

    'main: loop {
        match mode {
            AppMode::SourceSelect => {
                terminal.draw(|f| draw_source_select(f, source_select_idx))?;
            }
            AppMode::Connect | AppMode::Connecting => {
                terminal.draw(|f| draw_connect(f, &form))?;
            }
            AppMode::ModbusConnect | AppMode::ModbusConnecting => {
                terminal.draw(|f| draw_modbus_connect(f, &modbus_form))?;
            }
            AppMode::SerialConnect | AppMode::SerialConnecting => {
                terminal.draw(|f| draw_serial_connect(f, &serial_form))?;
            }
            AppMode::Monitor => {
                if state.source_kind == SourceKind::ModbusTcp {
                    terminal.draw(|f| draw_modbus_monitor(f, &state))?;
                } else if state.source_kind == SourceKind::Serial {
                    terminal.draw(|f| draw_serial_monitor(f, &state))?;
                } else {
                    terminal.draw(|f| draw(f, &state, active_panel))?;
                }
            }
        }

        // Wait for at least one event, then drain all pending ones before next redraw.
        // This prevents high-frequency MQTT messages from delaying key events.
        let Some(first) = rx.recv().await else {
            break 'main;
        };
        let mut batch = vec![first];
        while let Ok(e) = rx.try_recv() {
            batch.push(e);
        }

        for event in batch {
            match event {
                AppEvent::Tick => {
                    state.on_tick();
                }

                AppEvent::MqttMessage(msg) => {
                    if mode == AppMode::Monitor {
                        state.add_message(msg);
                    }
                }
                AppEvent::ModbusData { start, values } => {
                    if mode == AppMode::Monitor && state.source_kind == SourceKind::ModbusTcp {
                        state.modbus_rows = values
                            .into_iter()
                            .enumerate()
                            .map(|(i, v)| ModbusRow {
                                address: start.wrapping_add(i as u16),
                                value: v,
                            })
                            .collect();
                        state.update_modbus_trend_points();
                    }
                }
                AppEvent::SerialLine(line) => {
                    if mode == AppMode::Monitor
                        && state.source_kind == SourceKind::Serial
                        && !state.serial_paused
                    {
                        state.serial_lines.push(SerialEntry::rx(line));
                        if state.serial_lines.len() > 2000 {
                            state.serial_lines.remove(0);
                        }
                        state.serial_line_offset = state.serial_lines.len().saturating_sub(1);
                    }
                }
                AppEvent::Connected => {
                    state.connected = true;
                    state.last_error = None;
                    match mode {
                        AppMode::Connecting => {
                            mode = AppMode::Monitor;
                            form.status = ConnectStatus::Idle;
                        }
                        AppMode::ModbusConnecting => {
                            mode = AppMode::Monitor;
                            modbus_form.status = ConnectStatus::Idle;
                        }
                        AppMode::SerialConnecting => {
                            mode = AppMode::Monitor;
                            serial_form.status = ConnectStatus::Idle;
                        }
                        _ => {}
                    }
                }
                AppEvent::Disconnected => {
                    state.connected = false;
                    match mode {
                        AppMode::Connecting => {
                            mqtt_cmd = None;
                            mode = AppMode::Connect;
                        }
                        AppMode::ModbusConnecting => {
                            modbus_cmd = None;
                            mode = AppMode::ModbusConnect;
                        }
                        AppMode::SerialConnecting => {
                            mode = AppMode::SerialConnect;
                            serial_form.status = ConnectStatus::Idle;
                        }
                        AppMode::Monitor if state.source_kind == SourceKind::Serial => {
                            state.connected = false;
                        }
                        _ => {}
                    }
                }
                AppEvent::Error(e) => match mode {
                    AppMode::Connecting => form.status = ConnectStatus::Error(e),
                    AppMode::ModbusConnecting => modbus_form.status = ConnectStatus::Error(e),
                    AppMode::SerialConnecting => serial_form.status = ConnectStatus::Error(e),
                    _ => state.last_error = Some(e),
                },

                AppEvent::Paste(s) => {
                    if mode == AppMode::Connect {
                        form.paste(&s);
                    } else if mode == AppMode::ModbusConnect {
                        modbus_form.paste(&s);
                    } else if mode == AppMode::SerialConnect {
                        serial_form.paste(&s);
                    } else if state.serial_write_mode {
                        state.serial_write_input.push_str(&s);
                    } else if state.publish_mode {
                        state.publish_input.push_str(&s);
                    } else if state.subscribe_mode {
                        state.subscribe_input.push_str(&s);
                    } else if state.search_mode {
                        state.search_query.push_str(&s);
                    }
                }

                AppEvent::Key(key) => match mode {
                    AppMode::SourceSelect => match (key.modifiers, key.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('c'))
                        | (KeyModifiers::NONE, KeyCode::Char('q')) => break 'main,
                        (KeyModifiers::NONE, KeyCode::Up) => {
                            source_select_idx = source_select_idx.checked_sub(1).unwrap_or(2);
                        }
                        (KeyModifiers::NONE, KeyCode::Down) => {
                            source_select_idx = (source_select_idx + 1) % 3;
                        }
                        (KeyModifiers::NONE, KeyCode::Enter) => match source_select_idx {
                            0 => mode = AppMode::ModbusConnect,
                            1 => mode = AppMode::Connect,
                            _ => mode = AppMode::SerialConnect,
                        },
                        (KeyModifiers::NONE, KeyCode::Char('s'))
                        | (KeyModifiers::NONE, KeyCode::Char('1')) => {
                            mode = AppMode::SerialConnect;
                        }
                        (KeyModifiers::NONE, KeyCode::Char('m'))
                        | (KeyModifiers::NONE, KeyCode::Char('2')) => {
                            mode = AppMode::Connect;
                        }
                        (KeyModifiers::NONE, KeyCode::Char('b'))
                        | (KeyModifiers::NONE, KeyCode::Char('3')) => {
                            mode = AppMode::ModbusConnect;
                        }
                        _ => {}
                    },

                    AppMode::ModbusConnect => match (key.modifiers, key.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Tab)
                        | (KeyModifiers::NONE, KeyCode::Down) => modbus_form.next(),
                        (KeyModifiers::SHIFT, KeyCode::BackTab)
                        | (KeyModifiers::NONE, KeyCode::Up) => modbus_form.prev(),
                        (KeyModifiers::NONE, KeyCode::Backspace) => {
                            modbus_form.backspace();
                            modbus_form.status = ConnectStatus::Idle;
                        }
                        (KeyModifiers::NONE, KeyCode::Enter) => {
                            config::save(&config::SavedConfig {
                                mqtt: config::MqttConfig {
                                    host: saved.mqtt.host.clone(),
                                    port: saved.mqtt.port,
                                    username: saved.mqtt.username.clone(),
                                    version: saved.mqtt.version.clone(),
                                    topics: saved.mqtt.topics.clone(),
                                },
                                modbus: config::ModbusPersistedConfig {
                                    host: modbus_form.values[0].clone(),
                                    port: modbus_form.values[1].parse().unwrap_or(502),
                                    unit_id: modbus_form.values[2].parse().unwrap_or(1),
                                    poll_interval_ms: modbus_form.values[3].parse().unwrap_or(1000),
                                },
                                serial: saved.serial.clone(),
                            });
                            state.mqtt_version = "Modbus TCP";
                            state.source_kind = SourceKind::ModbusTcp;
                            modbus_form.status = ConnectStatus::Connecting;
                            modbus_cmd = Some(spawn_modbus(&modbus_form, &tx));
                            mode = AppMode::ModbusConnecting;
                        }
                        (KeyModifiers::NONE, KeyCode::Esc) => {
                            mode = AppMode::SourceSelect;
                        }
                        (KeyModifiers::NONE, KeyCode::Char(c)) => {
                            modbus_form.push(c);
                            modbus_form.status = ConnectStatus::Idle;
                        }
                        _ => {}
                    },

                    AppMode::ModbusConnecting => match (key.modifiers, key.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                        (KeyModifiers::NONE, KeyCode::Esc) => {
                            modbus_cmd = None;
                            mode = AppMode::ModbusConnect;
                            modbus_form.status = ConnectStatus::Idle;
                        }
                        _ => {}
                    },

                    AppMode::SerialConnect => match (key.modifiers, key.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Tab)
                        | (KeyModifiers::NONE, KeyCode::Down) => serial_form.next(),
                        (KeyModifiers::SHIFT, KeyCode::BackTab)
                        | (KeyModifiers::NONE, KeyCode::Up) => serial_form.prev(),
                        (KeyModifiers::NONE, KeyCode::Left) => serial_form.left(),
                        (KeyModifiers::NONE, KeyCode::Right) => serial_form.right(),
                        (KeyModifiers::NONE, KeyCode::Backspace) => {
                            serial_form.status = ConnectStatus::Idle;
                        }
                        (KeyModifiers::NONE, KeyCode::Char('r')) => {
                            serial_form.refresh_ports();
                        }
                        (KeyModifiers::NONE, KeyCode::Enter) => {
                            let port = serial_form.port().to_string();
                            if port.is_empty() {
                                serial_form.status = ConnectStatus::Error(
                                    "No ports detected — press r to refresh".into(),
                                );
                            } else {
                                config::save(&config::SavedConfig {
                                    mqtt: saved.mqtt.clone(),
                                    modbus: saved.modbus.clone(),
                                    serial: config::SerialPersistedConfig {
                                        port: port.clone(),
                                        baud_rate: serial_form.baud_rate(),
                                        data_bits: serial_form.data_bits_val(),
                                        parity: serial_form.parity_label().to_string(),
                                        stop_bits: serial_form.stop_bits_val(),
                                    },
                                });
                                state.source_kind = SourceKind::Serial;
                                state.serial_lines.clear();
                                state.serial_line_offset = 0;
                                serial_form.status = ConnectStatus::Connecting;
                                serial_cmd = Some(spawn_serial(&serial_form, &tx));
                                mode = AppMode::SerialConnecting;
                            }
                        }
                        (KeyModifiers::NONE, KeyCode::Esc) => {
                            mode = AppMode::SourceSelect;
                        }
                        _ => {}
                    },

                    AppMode::SerialConnecting => match (key.modifiers, key.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                        (KeyModifiers::NONE, KeyCode::Esc) => {
                            mode = AppMode::SerialConnect;
                            serial_form.status = ConnectStatus::Idle;
                        }
                        _ => {}
                    },

                    AppMode::Connect => match (key.modifiers, key.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,

                        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Tab)
                        | (KeyModifiers::NONE, KeyCode::Down) => form.next(),

                        (KeyModifiers::SHIFT, KeyCode::BackTab)
                        | (KeyModifiers::NONE, KeyCode::Up) => form.prev(),

                        (KeyModifiers::NONE, KeyCode::Backspace) => {
                            form.backspace();
                            form.status = ConnectStatus::Idle;
                        }

                        // Version selector toggle (active == 4)
                        (KeyModifiers::NONE, KeyCode::Left)
                        | (KeyModifiers::NONE, KeyCode::Right)
                        | (KeyModifiers::NONE, KeyCode::Char(' '))
                            if form.active == 4 =>
                        {
                            form.mqtt_version.toggle();
                        }

                        (KeyModifiers::NONE, KeyCode::Enter) => {
                            config::save(&config::SavedConfig {
                                mqtt: config::MqttConfig {
                                    host: form.values[0].clone(),
                                    port: form.values[1].parse().unwrap_or(1883),
                                    username: form.values[2].clone(),
                                    version: if form.mqtt_version == crate::core::MqttVersion::V5 {
                                        "v5".into()
                                    } else {
                                        "v311".into()
                                    },
                                    topics: initial_topics.clone(),
                                },
                                modbus: config::ModbusPersistedConfig {
                                    host: modbus_form.values[0].clone(),
                                    port: modbus_form.values[1].parse().unwrap_or(502),
                                    unit_id: modbus_form.values[2].parse().unwrap_or(1),
                                    poll_interval_ms: modbus_form.values[3].parse().unwrap_or(1000),
                                },
                                serial: saved.serial.clone(),
                            });
                            state.subscribed_topics = initial_topics.clone();
                            state.auto_select_first = !initial_topics.is_empty();
                            state.mqtt_version = form.mqtt_version.label();
                            form.status = ConnectStatus::Connecting;
                            mqtt_cmd = Some(spawn_mqtt(&form, &initial_topics, &tx));
                            mode = AppMode::Connecting;
                        }
                        (KeyModifiers::NONE, KeyCode::Esc) => {
                            mode = AppMode::SourceSelect;
                        }
                        (KeyModifiers::NONE, KeyCode::Char(c)) => {
                            form.push(c);
                            form.status = ConnectStatus::Idle;
                        }
                        _ => {}
                    },

                    AppMode::Connecting => match (key.modifiers, key.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                        (KeyModifiers::NONE, KeyCode::Esc) => {
                            mqtt_cmd = None; // drops sender → MQTT task stops
                            mode = AppMode::Connect;
                            form.status = ConnectStatus::Idle;
                        }
                        _ => {}
                    },

                    // ── Serial monitor: write mode ───────────────────────────────────
                    AppMode::Monitor
                        if state.source_kind == SourceKind::Serial && state.serial_write_mode =>
                    {
                        match (key.modifiers, key.code) {
                            (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                            (KeyModifiers::NONE, KeyCode::Esc) => {
                                state.serial_write_mode = false;
                                state.serial_write_input.clear();
                            }
                            (KeyModifiers::NONE, KeyCode::Enter) => {
                                let raw = state.serial_write_input.trim().to_string();
                                if !raw.is_empty() {
                                    let result = match state.serial_display_format {
                                        SerialDisplayFormat::Ascii => {
                                            let mut b = raw.as_bytes().to_vec();
                                            b.push(b'\n');
                                            Some((b.clone(), b[..b.len() - 1].to_vec()))
                                        }
                                        SerialDisplayFormat::Hex => match parse_hex_input(&raw) {
                                            Some(b) => {
                                                let mut send = b.clone();
                                                send.push(b'\n');
                                                Some((send, b))
                                            }
                                            None => {
                                                state.last_error =
                                                    Some(format!("Invalid hex input: {raw}"));
                                                None
                                            }
                                        },
                                    };
                                    if let Some((bytes_to_send, echo_bytes)) = result {
                                        if let Some(ref cmd_tx) = serial_cmd {
                                            let _ = cmd_tx.send(SerialCommand::Send(bytes_to_send));
                                        }
                                        state.serial_lines.push(SerialEntry::tx(echo_bytes));
                                        if state.serial_lines.len() > 2000 {
                                            state.serial_lines.remove(0);
                                        }
                                        state.serial_line_offset =
                                            state.serial_lines.len().saturating_sub(1);
                                    }
                                }
                                state.serial_write_mode = false;
                                state.serial_write_input.clear();
                            }
                            (KeyModifiers::NONE, KeyCode::Backspace) => {
                                state.serial_write_input.pop();
                            }
                            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                                state.serial_write_input.push(c);
                            }
                            _ => {}
                        }
                    }

                    // ── Serial monitor: normal navigation ───────────────────
                    AppMode::Monitor if state.source_kind == SourceKind::Serial => {
                        match (key.modifiers, key.code) {
                            (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                            (KeyModifiers::NONE, KeyCode::Char('q')) => {
                                drop(mqtt_cmd.take());
                                drop(modbus_cmd.take());
                                drop(serial_cmd.take());
                                break 'main;
                            }
                            (KeyModifiers::NONE, KeyCode::Char('w')) => {
                                state.serial_write_mode = true;
                                state.serial_write_input.clear();
                            }
                            (KeyModifiers::NONE, KeyCode::Char(' ')) => {
                                state.serial_paused = !state.serial_paused;
                            }
                            (KeyModifiers::NONE, KeyCode::Char('x')) => {
                                state.serial_display_format = match state.serial_display_format {
                                    SerialDisplayFormat::Ascii => SerialDisplayFormat::Hex,
                                    SerialDisplayFormat::Hex => SerialDisplayFormat::Ascii,
                                };
                            }
                            (KeyModifiers::NONE, KeyCode::Char('c')) => {
                                state.serial_lines.clear();
                                state.serial_line_offset = 0;
                            }
                            (KeyModifiers::NONE, KeyCode::Esc) => {
                                state.confirm_back = true;
                            }
                            _ => {}
                        }
                    }

                    // ── Modbus monitor: editing query form ────────────────
                    AppMode::Monitor
                        if state.source_kind == SourceKind::ModbusTcp
                            && state.modbus_query.editing =>
                    {
                        match (key.modifiers, key.code) {
                            (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                            (KeyModifiers::NONE, KeyCode::Tab)
                            | (KeyModifiers::NONE, KeyCode::Down) => {
                                state.modbus_query.next_field();
                            }
                            (KeyModifiers::SHIFT, KeyCode::BackTab)
                            | (KeyModifiers::NONE, KeyCode::Up) => {
                                state.modbus_query.prev_field();
                            }
                            (KeyModifiers::NONE, KeyCode::Left) => {
                                state.modbus_query.left();
                            }
                            (KeyModifiers::NONE, KeyCode::Right) => {
                                state.modbus_query.right();
                            }
                            (KeyModifiers::NONE, KeyCode::Backspace) => {
                                state.modbus_query.backspace();
                            }
                            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                                state.modbus_query.push(c);
                            }
                            (KeyModifiers::NONE, KeyCode::Enter) => {
                                let fc = state.modbus_query.fc();
                                let start = state.modbus_query.start();
                                let qty = state.modbus_query.qty();
                                state.modbus_query.editing = false;
                                state.modbus_table_offset = 0;
                                if let Some(ref cmd_tx) = modbus_cmd {
                                    let _ = cmd_tx.send(ModbusCommand::SetQuery {
                                        fc,
                                        start,
                                        quantity: qty,
                                    });
                                }
                            }
                            (KeyModifiers::NONE, KeyCode::Esc) => {
                                state.modbus_query.editing = false;
                            }
                            _ => {}
                        }
                    }

                    // ── Modbus monitor: add trend point mode ───────────────────
                    AppMode::Monitor
                        if state.source_kind == SourceKind::ModbusTcp
                            && state.modbus_add_point_mode =>
                    {
                        match (key.modifiers, key.code) {
                            (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                            (KeyModifiers::NONE, KeyCode::Esc) => {
                                state.modbus_add_point_mode = false;
                                state.modbus_add_point_input.clear();
                            }
                            (KeyModifiers::NONE, KeyCode::Enter) => {
                                if let Ok(addr) = state.modbus_add_point_input.parse::<u16>() {
                                    state.add_modbus_trend_point(addr);
                                }
                                state.modbus_add_point_mode = false;
                                state.modbus_add_point_input.clear();
                            }
                            (KeyModifiers::NONE, KeyCode::Backspace) => {
                                state.modbus_add_point_input.pop();
                            }
                            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c))
                                if c.is_ascii_digit() =>
                            {
                                state.modbus_add_point_input.push(c);
                            }
                            _ => {}
                        }
                    }

                    // ── Modbus monitor: normal navigation ─────────────────────
                    AppMode::Monitor if state.source_kind == SourceKind::ModbusTcp => {
                        match (key.modifiers, key.code) {
                            (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                            (KeyModifiers::NONE, KeyCode::Char('q')) => {
                                drop(mqtt_cmd.take());
                                drop(modbus_cmd.take());
                                drop(serial_cmd.take());
                                break 'main;
                            }
                            (KeyModifiers::NONE, KeyCode::Char('e')) => {
                                state.modbus_query.editing = true;
                            }
                            (KeyModifiers::NONE, KeyCode::Char('a')) => {
                                state.modbus_add_point_mode = true;
                                state.modbus_add_point_input.clear();
                            }
                            (KeyModifiers::NONE, KeyCode::Up) => {
                                state.modbus_table_offset =
                                    state.modbus_table_offset.saturating_sub(1);
                            }
                            (KeyModifiers::NONE, KeyCode::Down) => {
                                let max = state.modbus_rows.len().saturating_sub(1);
                                if state.modbus_table_offset < max {
                                    state.modbus_table_offset += 1;
                                }
                            }
                            (KeyModifiers::NONE, KeyCode::Char('c')) => {
                                state.last_error = None;
                            }
                            (KeyModifiers::NONE, KeyCode::Esc) => {
                                state.confirm_back = true;
                            }
                            _ => {}
                        }
                    }

                    AppMode::Monitor if state.confirm_back => match (key.modifiers, key.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                        (KeyModifiers::NONE, KeyCode::Char('q')) => {
                            drop(mqtt_cmd.take());
                            drop(modbus_cmd.take());
                            drop(serial_cmd.take());
                            break 'main;
                        }
                        (KeyModifiers::NONE, KeyCode::Char('y'))
                        | (KeyModifiers::NONE, KeyCode::Enter) => {
                            mqtt_cmd = None;
                            modbus_cmd = None;
                            serial_cmd = None;
                            state = AppState::default();
                            mode = AppMode::SourceSelect;
                            form.status = ConnectStatus::Idle;
                            modbus_form.status = ConnectStatus::Idle;
                            serial_form.status = ConnectStatus::Idle;
                        }
                        _ => {
                            state.confirm_back = false;
                        }
                    },

                    AppMode::Monitor if state.subscribe_mode => match (key.modifiers, key.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                        (KeyModifiers::NONE, KeyCode::Esc) => {
                            state.subscribe_mode = false;
                            state.subscribe_input.clear();
                        }
                        (KeyModifiers::NONE, KeyCode::Enter) => {
                            let topic = state.subscribe_input.trim().to_string();
                            if !topic.is_empty() && state.source_kind == SourceKind::Mqtt {
                                if let Some(ref cmd_tx) = mqtt_cmd {
                                    let _ = cmd_tx.send(MqttCommand::Subscribe(topic.clone()));
                                }
                                if !state.subscribed_topics.contains(&topic) {
                                    state.subscribed_topics.push(topic.clone());
                                    config::update_topics(&state.subscribed_topics);
                                }
                            }
                            state.subscribe_mode = false;
                            state.subscribe_input.clear();
                        }
                        (KeyModifiers::NONE, KeyCode::Backspace) => {
                            state.subscribe_input.pop();
                        }
                        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                            state.subscribe_input.push(c);
                        }
                        _ => {}
                    },

                    AppMode::Monitor if state.yank_mode => match (key.modifiers, key.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                        (KeyModifiers::NONE, KeyCode::Esc) => state.exit_yank_mode(),
                        (KeyModifiers::NONE, KeyCode::Left) => state.yank_left(),
                        (KeyModifiers::NONE, KeyCode::Right) => state.yank_right(),
                        (KeyModifiers::NONE, KeyCode::Up) => {
                            if let Some(i) = state.msg_cursor {
                                if i > 0 {
                                    state.msg_cursor = Some(i - 1);
                                }
                            }
                            state.yank_start = 0;
                            state.yank_cursor = 0;
                        }
                        (KeyModifiers::NONE, KeyCode::Down) => {
                            if let Some(i) = state.msg_cursor {
                                let count = state.filtered_messages().count();
                                if i + 1 < count {
                                    state.msg_cursor = Some(i + 1);
                                }
                            }
                            state.yank_start = 0;
                            state.yank_cursor = 0;
                        }
                        (KeyModifiers::NONE, KeyCode::Char('y')) => {
                            if let Some(text) = state.yank_text() {
                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                    let _ = cb.set_text(&text);
                                }
                            }
                            state.exit_yank_mode();
                        }
                        _ => {}
                    },

                    AppMode::Monitor if state.search_mode => match (key.modifiers, key.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                        (KeyModifiers::NONE, KeyCode::Esc) => state.exit_search(true),
                        (KeyModifiers::NONE, KeyCode::Enter) => state.exit_search(false),
                        (KeyModifiers::NONE, KeyCode::Backspace) => state.backspace_search(),
                        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                            state.push_search(c)
                        }
                        _ => {}
                    },

                    AppMode::Monitor if state.publish_mode => match (key.modifiers, key.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                        (KeyModifiers::NONE, KeyCode::Esc) => {
                            state.publish_mode = false;
                            state.publish_input.clear();
                        }
                        (KeyModifiers::NONE, KeyCode::Enter) => {
                            if let Some(topic) = state.selected_topic_name().map(str::to_string) {
                                let payload = state.publish_input.trim().to_string();
                                if let Some(ref cmd_tx) = mqtt_cmd {
                                    let _ = cmd_tx.send(MqttCommand::Publish { topic, payload });
                                }
                            }
                            state.publish_mode = false;
                            state.publish_input.clear();
                        }
                        (KeyModifiers::NONE, KeyCode::Backspace) => {
                            state.publish_input.pop();
                        }
                        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                            state.publish_input.push(c);
                        }
                        _ => {}
                    },

                    AppMode::Monitor => match (key.modifiers, key.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) => break 'main,
                        (KeyModifiers::NONE, KeyCode::Char('q')) => {
                            drop(mqtt_cmd.take());
                            drop(modbus_cmd.take());
                            drop(serial_cmd.take());
                            break 'main;
                        }
                        (KeyModifiers::NONE, KeyCode::Char(' '))
                            if state.source_kind == SourceKind::Mqtt =>
                        {
                            state.paused = !state.paused;
                        }

                        (KeyModifiers::NONE, KeyCode::Esc) => {
                            if state.selected_topic_idx.is_some() {
                                state.clear_topic_filter();
                            } else {
                                state.confirm_back = true;
                            }
                        }

                        (KeyModifiers::NONE, KeyCode::Tab) => active_panel = active_panel.toggle(),

                        (KeyModifiers::NONE, KeyCode::Up) => match active_panel {
                            Panel::Topics => state.select_topic_prev(),
                            Panel::Messages => state.select_msg_prev(),
                        },
                        (KeyModifiers::NONE, KeyCode::Down) => match active_panel {
                            Panel::Topics => state.select_topic_next(),
                            Panel::Messages => state.select_msg_next(),
                        },

                        (KeyModifiers::NONE, KeyCode::Char('/')) => state.enter_search(),
                        (KeyModifiers::NONE, KeyCode::Char('s'))
                            if state.source_kind == SourceKind::Mqtt =>
                        {
                            state.subscribe_mode = true;
                        }
                        (KeyModifiers::NONE, KeyCode::Char('p'))
                            if state.selected_topic_idx.is_some()
                                && state.source_kind == SourceKind::Mqtt =>
                        {
                            state.publish_mode = true;
                            state.publish_input.clear();
                        }
                        (KeyModifiers::NONE, KeyCode::Char('y')) => {
                            if state.paused {
                                state.enter_yank_mode();
                            }
                        }
                        (KeyModifiers::NONE, KeyCode::Char('d'))
                            if active_panel == Panel::Topics
                                && state.selected_topic_idx.is_some()
                                && state.source_kind == SourceKind::Mqtt =>
                        {
                            if let Some(topic) = state.delete_selected_topic() {
                                if let Some(ref cmd_tx) = mqtt_cmd {
                                    let _ = cmd_tx.send(MqttCommand::Unsubscribe(topic));
                                }
                                config::update_topics(&state.subscribed_topics);
                            }
                        }
                        (KeyModifiers::NONE, KeyCode::Char('c')) => {
                            state.last_error = None;
                        }
                        _ => {}
                    },
                },
            }
        }
    }

    Ok(())
}

fn spawn_mqtt(form: &ConnectForm, topics: &[String], tx: &EventTx) -> UnboundedSender<MqttCommand> {
    let client_id = format!("pulse-tui-{}", random_hex_suffix());

    let config = MqttConfig {
        host: form.host().to_string(),
        port: form.port(),
        client_id,
        topics: topics.to_vec(),
        keep_alive_secs: 5,
        username: if form.username().is_empty() {
            None
        } else {
            Some(form.username().to_string())
        },
        password: if form.password().is_empty() {
            None
        } else {
            Some(form.password().to_string())
        },
        version: form.mqtt_version,
    };

    let (source, cmd_tx) = MqttSource::new(config, tx.clone());
    tokio::spawn(source.run());
    cmd_tx
}

fn spawn_modbus(form: &ModbusForm, tx: &EventTx) -> UnboundedSender<ModbusCommand> {
    let config = ModbusConfig {
        host: form.host().to_string(),
        port: form.port(),
        unit_id: form.unit_id(),
        poll_interval_ms: form.poll_ms(),
    };
    let (source, cmd_tx) = ModbusSource::new(config, tx.clone());
    tokio::spawn(source.run());
    cmd_tx
}

// Parse space-separated hex tokens ("41 42 43") into bytes. Returns None on any invalid token.
fn parse_hex_input(s: &str) -> Option<Vec<u8>> {
    // Strip spaces, then parse consecutive pairs of hex digits as bytes.
    let digits: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if digits.len() % 2 != 0 {
        return None;
    }
    digits
        .as_bytes()
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect()
}

fn spawn_serial(form: &SerialForm, tx: &EventTx) -> UnboundedSender<SerialCommand> {
    use serialport::{DataBits, Parity, StopBits};
    let data_bits = match form.data_bits_val() {
        5 => DataBits::Five,
        6 => DataBits::Six,
        7 => DataBits::Seven,
        _ => DataBits::Eight,
    };
    let parity = match form.parity_label() {
        "Odd" => Parity::Odd,
        "Even" => Parity::Even,
        _ => Parity::None,
    };
    let stop_bits = if form.stop_bits_val() == 2 {
        StopBits::Two
    } else {
        StopBits::One
    };
    let config = SerialConfig {
        port: form.port().to_string(),
        baud_rate: form.baud_rate(),
        data_bits,
        parity,
        stop_bits,
    };
    let (source, cmd_tx) = SerialSource::new(config, tx.clone());
    tokio::spawn(source.run());
    cmd_tx
}
