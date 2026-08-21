# Pulse-TUI

![GitHub release](https://img.shields.io/github/v/release/sshnoodles/Pulse-TUI)
![Downloads](https://img.shields.io/github/downloads/sshnoodles/Pulse-TUI/total)

A real-time terminal monitor (TUI) built in Rust. Supports MQTT, Modbus TCP, IEC 104, OPC UA, and serial-port monitoring.

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

### OPC UA
- Connect to OPC UA servers using an `opc.tcp://` endpoint
- Poll one or more NodeIds at a configurable interval
- View each node's display name, value, data type, and source/server timestamps
- Add and remove monitored NodeIds without reconnecting
- Anonymous access or username/password authentication

### IEC 104
- Connect to an IEC 104 outstation by host, port, common address, and originator address
- Automatically activate data transfer with STARTDT
- Live bidirectional APDU trace with raw hex and decoded I/S/U frame details
- Decode common ASDU type, VSQ, cause of transmission, common address, IOA, and value fields
- Send station general interrogation commands
- Send complete raw APDUs in hexadecimal for protocol debugging
- Respond to STARTDT, STOPDT, and TESTFR activation frames and acknowledge received I-frames

### Serial
- Connect to any serial port with configurable baud rate, data bits, parity, and stop bits
- Timestamped RX / TX log (`hh:mm:ss RX <-` / `hh:mm:ss TX ->`)
- ASCII and Hex display modes (toggle with `x`)
- Send messages in ASCII or Hex format
- Real-time hex input validation (illegal characters, odd digit count)
- Pause / resume incoming data stream
- Status bar shows line count, total bytes received, and last message byte count
- Log capped at 2000 entries

### General
- Protocol selector on launch (MQTT / Modbus TCP / IEC 104 / OPC UA / Serial)
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

### Build from source

Rust 1.75 or later is required. On Linux, the `serialport` dependency may also
need your distribution's `libudev` development package.

```sh
git clone https://github.com/SShnoodles/Pulse-TUI.git
cd Pulse-TUI
cargo install --path .
```

## Usage

Just run `pulse` — no arguments needed. All settings are restored from `~/.pulse-tui.toml` on launch.

```bash
pulse
```

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

[opcua]
endpoint_url = "opc.tcp://localhost:4840"
node_ids = ["ns=2;s=Demo.Static.Scalar.Int32"]
poll_interval_ms = 1000
username = ""

[iec104]
host = "localhost"
port = 2404
common_address = 1
originator_address = 0

[serial]
port = "/dev/ttyUSB0"   # e.g. COM3 on Windows
baud_rate = 115200
data_bits = 8           # 5 / 6 / 7 / 8
parity = "None"         # None / Odd / Even
stop_bits = 1           # 1 / 2
```

Passwords are used only for the current connection and are not written to the
configuration file.

## Roadmap

- [x] MQTT publish from TUI
- [x] Modbus TCP source
- [x] OPC UA source
- [x] IEC 104 source
- [x] Serial source

## Tech Stack

| Crate | Purpose |
|-------|---------|
| [ratatui](https://github.com/ratatui/ratatui) | TUI framework |
| [crossterm](https://github.com/crossterm-rs/crossterm) | Terminal backend |
| [tokio](https://tokio.rs) | Async runtime |
| [rumqttc](https://github.com/bytebeamio/rumqtt) | MQTT client |
| [tokio-modbus](https://github.com/slowtec/tokio-modbus) | Modbus TCP client |
| [serialport](https://github.com/serialport/serialport-rs) | Serial port I/O |
| [async-opcua](https://github.com/locka99/opcua) | OPC UA client |
| [serde](https://serde.rs) + [toml](https://github.com/toml-rs/toml) | Config serialization |
| [arboard](https://github.com/1Password/arboard) | Clipboard access |
| [tracing](https://github.com/tokio-rs/tracing) | Logging |

## License

MIT
