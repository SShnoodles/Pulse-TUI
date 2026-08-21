use std::time::Duration;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};

use super::config::Iec104Config;
use crate::{
    core::{AppEvent, Iec104Direction},
    events::EventTx,
};

const START: u8 = 0x68;
const STARTDT_ACT: [u8; 6] = [START, 4, 0x07, 0, 0, 0];
const STARTDT_CON: [u8; 6] = [START, 4, 0x0b, 0, 0, 0];
const STOPDT_ACT: [u8; 6] = [START, 4, 0x13, 0, 0, 0];
const STOPDT_CON: [u8; 6] = [START, 4, 0x23, 0, 0, 0];
const TESTFR_CON: [u8; 6] = [START, 4, 0x83, 0, 0, 0];

#[derive(Debug)]
pub enum Iec104Command {
    GeneralInterrogation,
    SendRaw(Vec<u8>),
}

pub struct Iec104Source {
    config: Iec104Config,
    tx: EventTx,
    cmd_rx: mpsc::UnboundedReceiver<Iec104Command>,
}

impl Iec104Source {
    pub fn new(config: Iec104Config, tx: EventTx) -> (Self, mpsc::UnboundedSender<Iec104Command>) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        (Self { config, tx, cmd_rx }, cmd_tx)
    }

    pub async fn run(mut self) {
        let endpoint = format!("{}:{}", self.config.host, self.config.port);
        let connection =
            tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(&endpoint)).await;
        let mut stream = match connection {
            Ok(Ok(stream)) => stream,
            Ok(Err(error)) => {
                let _ = self
                    .tx
                    .send(AppEvent::Error(format!("IEC 104 connect: {error}")));
                let _ = self.tx.send(AppEvent::Disconnected);
                return;
            }
            Err(_) => {
                let _ = self
                    .tx
                    .send(AppEvent::Error("IEC 104 connection timed out".into()));
                let _ = self.tx.send(AppEvent::Disconnected);
                return;
            }
        };

        if let Err(error) = write_frame(&mut stream, &self.tx, &STARTDT_ACT, "U STARTDT act").await
        {
            let _ = self
                .tx
                .send(AppEvent::Error(format!("IEC 104 STARTDT: {error}")));
            let _ = self.tx.send(AppEvent::Disconnected);
            return;
        }
        let _ = self.tx.send(AppEvent::Connected);

        let mut send_sequence = 0u16;
        let mut receive_sequence = 0u16;
        let mut pending = Vec::<u8>::new();
        let mut read_buf = [0u8; 1024];

        loop {
            tokio::select! {
                read = stream.read(&mut read_buf) => match read {
                    Ok(0) => {
                        let _ = self.tx.send(AppEvent::Disconnected);
                        break;
                    }
                    Ok(count) => {
                        pending.extend_from_slice(&read_buf[..count]);
                        let frames = drain_frames(&mut pending);
                        for frame in frames {
                            let summary = describe_apdu(&frame);
                            emit_frame(&self.tx, Iec104Direction::Rx, &frame, summary);

                            if is_i_frame(&frame) {
                                let peer_sequence = control_sequence(frame[2], frame[3]);
                                receive_sequence = peer_sequence.wrapping_add(1) & 0x7fff;
                                let acknowledgement = s_frame(receive_sequence);
                                if let Err(error) = write_frame(
                                    &mut stream,
                                    &self.tx,
                                    &acknowledgement,
                                    &format!("S ACK R={receive_sequence}"),
                                ).await {
                                    let _ = self.tx.send(AppEvent::Error(format!("IEC 104 write: {error}")));
                                    let _ = self.tx.send(AppEvent::Disconnected);
                                    return;
                                }
                            } else if let Some(response) = u_frame_response(&frame) {
                                let label = describe_apdu(response);
                                if let Err(error) = write_frame(&mut stream, &self.tx, response, &label).await {
                                    let _ = self.tx.send(AppEvent::Error(format!("IEC 104 write: {error}")));
                                    let _ = self.tx.send(AppEvent::Disconnected);
                                    return;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        let _ = self.tx.send(AppEvent::Error(format!("IEC 104 read: {error}")));
                        let _ = self.tx.send(AppEvent::Disconnected);
                        break;
                    }
                },
                command = self.cmd_rx.recv() => match command {
                    None => {
                        let _ = write_frame(&mut stream, &self.tx, &STOPDT_ACT, "U STOPDT act").await;
                        break;
                    }
                    Some(command) => {
                        let (frame, label, next_send_sequence) = match command {
                            Iec104Command::GeneralInterrogation => (
                                build_i_frame(
                                    send_sequence,
                                    receive_sequence,
                                    &general_interrogation_asdu(&self.config),
                                ),
                                "I C_IC_NA_1 general interrogation".to_string(),
                                Some(send_sequence.wrapping_add(1) & 0x7fff),
                            ),
                            Iec104Command::SendRaw(frame) => {
                                if let Err(message) = validate_apdu(&frame) {
                                    let _ = self.tx.send(AppEvent::Error(message));
                                    continue;
                                }
                                let next_sequence = if is_i_frame(&frame) {
                                    Some(control_sequence(frame[2], frame[3]).wrapping_add(1) & 0x7fff)
                                } else {
                                    None
                                };
                                let label = format!("RAW {}", describe_apdu(&frame));
                                (frame, label, next_sequence)
                            }
                        };

                        if let Err(error) = write_frame(&mut stream, &self.tx, &frame, &label).await {
                            let _ = self.tx.send(AppEvent::Error(format!("IEC 104 write: {error}")));
                            let _ = self.tx.send(AppEvent::Disconnected);
                            break;
                        }
                        if let Some(next_sequence) = next_send_sequence {
                            send_sequence = next_sequence;
                        }
                    }
                },
            }
        }
    }
}

async fn write_frame(
    stream: &mut TcpStream,
    tx: &EventTx,
    frame: &[u8],
    summary: &str,
) -> std::io::Result<()> {
    stream.write_all(frame).await?;
    emit_frame(tx, Iec104Direction::Tx, frame, summary.to_string());
    Ok(())
}

fn emit_frame(tx: &EventTx, direction: Iec104Direction, raw: &[u8], summary: String) {
    let _ = tx.send(AppEvent::Iec104Frame {
        direction,
        raw: raw.to_vec(),
        summary,
    });
}

fn validate_apdu(frame: &[u8]) -> Result<(), String> {
    if frame.len() < 6 || frame[0] != START {
        return Err(
            "IEC 104 raw frame must start with 68 and include a 4-byte control field".into(),
        );
    }
    if frame[1] as usize + 2 != frame.len() {
        return Err(format!(
            "IEC 104 APDU length mismatch: header says {}, got {} bytes",
            frame[1],
            frame.len().saturating_sub(2)
        ));
    }
    Ok(())
}

fn drain_frames(buffer: &mut Vec<u8>) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    loop {
        let Some(start) = buffer.iter().position(|byte| *byte == START) else {
            buffer.clear();
            break;
        };
        if start > 0 {
            buffer.drain(..start);
        }
        if buffer.len() < 2 {
            break;
        }
        let frame_len = buffer[1] as usize + 2;
        if frame_len < 6 {
            buffer.remove(0);
            continue;
        }
        if buffer.len() < frame_len {
            break;
        }
        frames.push(buffer.drain(..frame_len).collect());
    }
    frames
}

fn is_i_frame(frame: &[u8]) -> bool {
    frame.len() >= 6 && frame[2] & 0x01 == 0
}

fn control_sequence(low: u8, high: u8) -> u16 {
    (u16::from_le_bytes([low, high]) >> 1) & 0x7fff
}

fn s_frame(receive_sequence: u16) -> [u8; 6] {
    let receive = (receive_sequence << 1).to_le_bytes();
    [START, 4, 0x01, 0, receive[0], receive[1]]
}

fn u_frame_response(frame: &[u8]) -> Option<&'static [u8; 6]> {
    if frame.len() < 6 || frame[2] & 0x03 != 0x03 {
        return None;
    }
    match frame[2] {
        0x07 => Some(&STARTDT_CON),
        0x13 => Some(&STOPDT_CON),
        0x43 => Some(&TESTFR_CON),
        _ => None,
    }
}

fn build_i_frame(send_sequence: u16, receive_sequence: u16, asdu: &[u8]) -> Vec<u8> {
    let send = ((send_sequence & 0x7fff) << 1).to_le_bytes();
    let receive = ((receive_sequence & 0x7fff) << 1).to_le_bytes();
    let mut frame = Vec::with_capacity(asdu.len() + 6);
    frame.extend_from_slice(&[
        START,
        (asdu.len() + 4) as u8,
        send[0],
        send[1],
        receive[0],
        receive[1],
    ]);
    frame.extend_from_slice(asdu);
    frame
}

fn asdu_header(type_id: u8, cause: u8, config: &Iec104Config) -> Vec<u8> {
    let common = config.common_address.to_le_bytes();
    vec![
        type_id,
        1,
        cause,
        config.originator_address,
        common[0],
        common[1],
    ]
}

fn general_interrogation_asdu(config: &Iec104Config) -> Vec<u8> {
    let mut asdu = asdu_header(100, 6, config);
    asdu.extend_from_slice(&[0, 0, 0, 20]); // IOA 0 + QOI station interrogation
    asdu
}

pub(crate) fn describe_apdu(frame: &[u8]) -> String {
    if frame.len() < 6 {
        return "invalid APDU".into();
    }
    let control = &frame[2..6];
    if control[0] & 0x01 == 0 {
        let send = control_sequence(control[0], control[1]);
        let receive = control_sequence(control[2], control[3]);
        if frame.len() < 12 {
            return format!("I S={send} R={receive} (missing ASDU header)");
        }
        let asdu = &frame[6..];
        let type_id = asdu[0];
        let count = asdu[1] & 0x7f;
        let sequence = asdu[1] & 0x80 != 0;
        let cause = asdu[2] & 0x3f;
        let negative = asdu[2] & 0x40 != 0;
        let test = asdu[2] & 0x80 != 0;
        let common = u16::from_le_bytes([asdu[4], asdu[5]]);
        let mut result = format!(
            "I S={send} R={receive} {}({type_id}) VSQ={}{} COT={}{}{} CA={common}",
            type_name(type_id),
            count,
            if sequence { " SQ" } else { "" },
            cause_name(cause),
            if negative { " NEG" } else { "" },
            if test { " TEST" } else { "" },
        );
        if asdu.len() >= 9 {
            let ioa = u32::from_le_bytes([asdu[6], asdu[7], asdu[8], 0]);
            result.push_str(&format!(" IOA={ioa}"));
            if let Some(value) = information_value(type_id, &asdu[9..]) {
                result.push_str(&format!(" value={value}"));
            }
        }
        result
    } else if control[0] & 0x03 == 0x01 {
        format!("S ACK R={}", control_sequence(control[2], control[3]))
    } else {
        match control[0] {
            0x07 => "U STARTDT act".into(),
            0x0b => "U STARTDT con".into(),
            0x13 => "U STOPDT act".into(),
            0x23 => "U STOPDT con".into(),
            0x43 => "U TESTFR act".into(),
            0x83 => "U TESTFR con".into(),
            value => format!("U control=0x{value:02X}"),
        }
    }
}

fn type_name(type_id: u8) -> &'static str {
    match type_id {
        1 => "M_SP_NA_1",
        3 => "M_DP_NA_1",
        9 => "M_ME_NA_1",
        11 => "M_ME_NB_1",
        13 => "M_ME_NC_1",
        30 => "M_SP_TB_1",
        31 => "M_DP_TB_1",
        34 => "M_ME_TD_1",
        35 => "M_ME_TE_1",
        36 => "M_ME_TF_1",
        45 => "C_SC_NA_1",
        46 => "C_DC_NA_1",
        100 => "C_IC_NA_1",
        101 => "C_CI_NA_1",
        103 => "C_CS_NA_1",
        _ => "ASDU",
    }
}

fn cause_name(cause: u8) -> String {
    match cause {
        1 => "periodic".into(),
        2 => "background".into(),
        3 => "spontaneous".into(),
        5 => "request".into(),
        6 => "activation".into(),
        7 => "activation-confirm".into(),
        10 => "activation-termination".into(),
        20 => "interrogated".into(),
        value => value.to_string(),
    }
}

fn information_value(type_id: u8, bytes: &[u8]) -> Option<String> {
    match type_id {
        1 | 30 => Some((bytes.first()? & 0x01 != 0).to_string()),
        3 | 31 => Some((bytes.first()? & 0x03).to_string()),
        9 | 11 | 34 | 35 => Some(i16::from_le_bytes([*bytes.first()?, *bytes.get(1)?]).to_string()),
        13 | 36 => Some(format!(
            "{:.6}",
            f32::from_le_bytes([
                *bytes.first()?,
                *bytes.get(1)?,
                *bytes.get(2)?,
                *bytes.get(3)?,
            ])
        )),
        100 => bytes.first().map(|qoi| format!("QOI={qoi}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drains_fragmented_and_multiple_frames() {
        let mut buffer = vec![0x00, 0x68, 0x04, 0x0b, 0, 0, 0, 0x68, 0x04];
        let frames = drain_frames(&mut buffer);
        assert_eq!(frames, vec![STARTDT_CON.to_vec()]);
        assert_eq!(buffer, vec![0x68, 0x04]);

        buffer.extend_from_slice(&[0x83, 0, 0, 0]);
        assert_eq!(drain_frames(&mut buffer), vec![TESTFR_CON.to_vec()]);
    }

    #[test]
    fn builds_general_interrogation_with_little_endian_common_address() {
        let config = Iec104Config {
            common_address: 0x1234,
            originator_address: 7,
            ..Iec104Config::default()
        };
        let frame = build_i_frame(2, 3, &general_interrogation_asdu(&config));
        assert_eq!(&frame[..6], &[0x68, 14, 4, 0, 6, 0]);
        assert_eq!(&frame[6..], &[100, 1, 6, 7, 0x34, 0x12, 0, 0, 0, 20]);
    }

    #[test]
    fn describes_measurement_frame() {
        let frame = [
            0x68, 18, 0, 0, 0, 0, 13, 1, 3, 0, 1, 0, 42, 0, 0, 0, 0, 0x80, 0x3f,
        ];
        let text = describe_apdu(&frame);
        assert!(text.contains("M_ME_NC_1(13)"));
        assert!(text.contains("IOA=42"));
        assert!(text.contains("value=1.000000"));
    }

    #[test]
    fn rejects_bad_raw_length() {
        assert!(validate_apdu(&[0x68, 4, 0x07]).is_err());
        assert!(validate_apdu(&[0x68, 5, 0x07, 0, 0, 0]).is_err());
    }
}
