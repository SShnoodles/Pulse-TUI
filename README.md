# Pulse-TUI

![GitHub release](https://img.shields.io/github/v/release/sshnoodles/Pulse-TUI)
![Downloads](https://img.shields.io/github/downloads/sshnoodles/Pulse-TUI/total)

A real-time terminal monitor (TUI) built in Rust. Supports MQTT and Modbus TCP, with planned support for Serial and more.

![tui](assets/tui.png)

## Features

### MQTT
- Live message stream with per-topic filtering
- JSON syntax highlighting
- Message search with inline match highlighting
- Yank mode — copy message payload to clipboard (when paused)
- Subscribe / unsubscribe to topics at runtime
- Publish messages to selected topic
- Per-topic message count and TPS (messages/sec) stats
- MQTT 3.1.1 and MQTT v5 support
- Username / password authentication
- Auto-reconnect on disconnect

### Modbus TCP
- Connect to any Modbus TCP device by host, port, and unit ID
- Query registers via Function Code selector (FC01 Coil, FC02 Discrete, FC03 Holding, FC04 Input)
- Configurable start address and quantity
- Live tabular view: Address, Hex, Binary, and interpreted Display columns
- Multiple display formats: Unsigned, Signed, Hex, Binary, Long, Long Inverse, Float, Float Inverse, Double, Double Inverse
- Auto-reconnect on disconnect

### General
- Protocol selector on launch (MQTT / Modbus TCP)
- Config persisted to `~/.pulse-tui.toml` (all connection settings restored on next launch)

## Install

### Install prebuilt binaries via shell script

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/SShnoodles/Pulse-TUI/releases/latest/download/pulse-installer.sh | sh
```

### Install prebuilt binaries via powershell script

```sh
powershell -ExecutionPolicy Bypass -c "irm https://github.com/SShnoodles/Pulse-TUI/releases/latest/download/pulse-installer.ps1 | iex"
```

### Install prebuilt binaries via Homebrew

```sh
brew install sshnoodles/tap/pulse
```

## Usage

Just run `pulse` — no arguments needed. All settings are restored from `~/.pulse-tui.toml` on launch.

```bash
pulse
```

## Key Bindings

### Protocol select

| Key | Action |
|-----|--------|
| `↑` / `↓` | Move selection |
| `Enter` | Confirm |
| `M` | Go to MQTT connect form |
| `B` | Go to Modbus TCP connect form |
| `q` / `Ctrl+C` | Quit |

### Connect form (MQTT & Modbus TCP)

| Key | Action |
|-----|--------|
| `Tab` / `↓` | Next field |
| `Shift+Tab` / `↑` | Previous field |
| `←` / `→` / `Space` | Toggle MQTT version (on version field) |
| `Enter` | Connect |
| `Esc` | Back to protocol select |
| `Ctrl+C` | Quit |

### MQTT Monitor — normal mode

| Key | Action |
|-----|--------|
| `Tab` | Switch focus between Topics and Messages panels |
| `↑` / `↓` | Navigate topics or messages |
| `Space` | Pause / resume message stream |
| `/` | Enter search mode |
| `s` | Enter subscribe mode |
| `d` | Delete selected topic (Topics panel) |
| `p` | Publish to selected topic |
| `y` | Enter yank (copy) mode — only when paused |
| `Esc` | Clear topic filter / open disconnect dialog |
| `c` | Clear error bar |
| `q` / `Ctrl+C` | Quit |

### MQTT Monitor — search mode

| Key | Action |
|-----|--------|
| _type_ | Filter messages by keyword |
| `Enter` | Confirm and keep filter |
| `Esc` | Cancel and clear filter |

### MQTT Monitor — subscribe mode

| Key | Action |
|-----|--------|
| _type_ | Enter topic pattern (wildcards supported) |
| `Enter` | Subscribe |
| `Esc` | Cancel |

### MQTT Monitor — yank mode (active when paused)

| Key | Action |
|-----|--------|
| `←` / `→` | Move selection cursor |
| `y` | Copy selected text to clipboard |
| `↑` / `↓` | Move to adjacent message |
| `Esc` | Exit yank mode |

### Modbus TCP Monitor — normal mode

| Key | Action |
|-----|--------|
| `e` | Open query edit form |
| `↑` / `↓` | Scroll data table |
| `c` | Clear error bar |
| `Esc` | Open disconnect dialog |
| `q` / `Ctrl+C` | Quit |

### Modbus TCP Monitor — query edit mode

| Key | Action |
|-----|--------|
| `Tab` / `↓` | Next field |
| `Shift+Tab` / `↑` | Previous field |
| `←` / `→` | Change Function Code or Display Format |
| `0–9` | Type start address or quantity |
| `Backspace` | Delete last digit |
| `Enter` | Send query |
| `Esc` | Cancel |

## Configuration

Settings are saved automatically to `~/.pulse-tui.toml` on connect:

```toml
[mqtt]
host = "localhost"
port = 1883
username = ""
version = "v311"   # or "v5"
topics = ["sensors/#", "plc/status"]

[modbus]
host = "localhost"
port = 502
unit_id = 1
poll_interval_ms = 1000
```

## Roadmap

- [x] MQTT publish from TUI
- [x] Modbus TCP source
- [ ] Serial source
- [ ] `pulse mqtt` / `pulse modbus` subcommand model

## Tech Stack

| Crate | Purpose |
|-------|---------|
| [ratatui](https://github.com/ratatui/ratatui) | TUI framework |
| [crossterm](https://github.com/crossterm-rs/crossterm) | Terminal backend |
| [tokio](https://tokio.rs) | Async runtime |
| [rumqttc](https://github.com/bytebeamio/rumqtt) | MQTT client |
| [tokio-modbus](https://github.com/slowtec/tokio-modbus) | Modbus TCP client |
| [serde](https://serde.rs) + [toml](https://github.com/toml-rs/toml) | Config serialization |
| [arboard](https://github.com/1Password/arboard) | Clipboard access |
| [tracing](https://github.com/tokio-rs/tracing) | Logging |

## License

MIT
