# Pulse-TUI

A real-time terminal monitor (TUI) built in Rust. Currently focused on MQTT, with planned support for Modbus, Serial, and more.

## Features

- Live MQTT message stream with per-topic filtering
- JSON syntax highlighting
- Message search with inline match highlighting
- Yank mode — copy message payload to clipboard (when paused)
- Subscribe / unsubscribe to topics at runtime
- Per-topic message count and TPS (messages/sec) stats
- MQTT 3.1.1 and MQTT v5 support
- Username / password authentication
- Auto-reconnect on disconnect
- Config persisted to `~/.pulse-tui.toml` (broker, port, topics, version)

## Install

```bash
cargo build --release
# binary at ./target/release/pulse
```

Or install directly into `~/.cargo/bin`:

```bash
cargo install --path .
```

## Usage

```
pulse [OPTIONS]

Options:
  -b, --broker <HOST>       Broker host [default: localhost]
  -p, --port <PORT>         Broker port [default: 1883]
  -t, --topics <TOPIC>...   Topics to subscribe (repeat for multiple)
      --client-id <ID>      MQTT client ID [default: pulse-tui]
  -h, --help                Print help
  -V, --version             Print version
```

Examples:

```bash
# Connect to local broker and subscribe to all topics
pulse

# Connect to a remote broker and subscribe to specific topics
pulse -b mqtt.example.com -t sensors/# -t plc/status

# Use MQTT v5 (selectable in the connect form)
pulse -b 192.168.1.10 -p 1883
```

## Key Bindings

### Connect form

| Key | Action |
|-----|--------|
| `Tab` / `↓` | Next field |
| `Shift+Tab` / `↑` | Previous field |
| `←` / `→` / `Space` | Toggle MQTT version (on version field) |
| `Enter` | Connect |
| `Esc` | Connect without credentials |
| `Ctrl+C` | Quit |

### Monitor — normal mode

| Key | Action |
|-----|--------|
| `Tab` | Switch focus between Topics and Messages panels |
| `↑` / `↓` | Navigate topics or messages |
| `Space` | Pause / resume message stream |
| `/` | Enter search mode |
| `s` | Enter subscribe mode (add a new topic) |
| `d` | Delete selected topic (Topics panel) |
| `y` | Enter yank (copy) mode — only when paused |
| `Esc` | Clear topic filter / open disconnect dialog |
| `c` | Clear error bar |
| `q` / `Ctrl+C` | Quit |

### Search mode

| Key | Action |
|-----|--------|
| _type_ | Filter messages by keyword |
| `Enter` | Confirm and keep filter |
| `Esc` | Cancel and clear filter |

### Subscribe mode

| Key | Action |
|-----|--------|
| _type_ | Enter topic pattern (wildcards supported) |
| `Enter` | Subscribe |
| `Esc` | Cancel |

### Yank mode (active when paused)

| Key | Action |
|-----|--------|
| `←` / `→` | Move selection cursor |
| `y` | Copy selected text to clipboard |
| `↑` / `↓` | Move to adjacent message |
| `Esc` | Exit yank mode |

## Configuration

Settings are saved automatically to `~/.pulse-tui.toml` on connect:

```toml
[mqtt]
host = "localhost"
port = 1883
username = ""
version = "v311"   # or "v5"
topics = ["sensors/#", "plc/status"]
```

## Roadmap

- [ ] MQTT publish from TUI
- [ ] Modbus source
- [ ] Serial source
- [ ] `pulse mqtt` / `pulse modbus` subcommand model

## Tech Stack

| Crate | Purpose |
|-------|---------|
| [ratatui](https://github.com/ratatui/ratatui) | TUI framework |
| [crossterm](https://github.com/crossterm-rs/crossterm) | Terminal backend |
| [tokio](https://tokio.rs) | Async runtime |
| [rumqttc](https://github.com/bytebeamio/rumqtt) | MQTT client |
| [clap](https://github.com/clap-rs/clap) | CLI argument parsing |
| [serde](https://serde.rs) + [toml](https://github.com/toml-rs/toml) | Config serialization |
| [arboard](https://github.com/1Password/arboard) | Clipboard access |
| [tracing](https://github.com/tokio-rs/tracing) | Logging |

## License

MIT
