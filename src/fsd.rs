//! Bounded decoder for line-oriented FSD packets observed in pilot clients.

use crate::Result;

const MAX_LINE_BYTES: usize = 16 * 1024;

/// An inbound FSD packet currently understood by the client core.
#[derive(Debug, Clone, PartialEq)]
pub enum FsdMessage {
    /// Private or radio text addressed to a callsign or frequency.
    Text {
        sender: String,
        receiver: String,
        body: String,
    },
    /// Server latency request, which must be answered with a pong.
    Ping {
        sender: String,
        receiver: String,
        timestamp: String,
    },
    /// Server latency response.
    Pong {
        sender: String,
        receiver: String,
        timestamp: String,
    },
    /// A controller disconnected from the network.
    ControllerOffline { callsign: String },
    /// Network controller position, frequency, and advertised range.
    ControllerUpdate {
        callsign: String,
        frequency_khz: u32,
        facility: u8,
        visible_range_nm: u16,
        latitude: f64,
        longitude: f64,
    },
}

/// Incrementally turns an arbitrary TCP byte stream into FSD messages.
#[derive(Debug, Default)]
pub struct FsdDecoder {
    pending: Vec<u8>,
}

impl FsdDecoder {
    /// Creates an empty decoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Consumes bytes, returning complete supported packets in arrival order.
    ///
    /// Unsupported packet types are ignored. An oversized or non-UTF-8 line
    /// clears the pending buffer and returns an error.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<FsdMessage>> {
        self.pending.extend_from_slice(bytes);
        let mut messages = Vec::new();
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            if end > MAX_LINE_BYTES {
                self.pending.clear();
                return Err("FSD line exceeds the maximum supported length".into());
            }
            let line = self.pending.drain(..=end).collect::<Vec<_>>();
            let line = line.strip_suffix(b"\n").unwrap_or(&line);
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            let line = std::str::from_utf8(line)?;
            if let Some(message) = parse_line(line) {
                messages.push(message);
            }
        }
        if self.pending.len() > MAX_LINE_BYTES {
            self.pending.clear();
            return Err("FSD line exceeds the maximum supported length".into());
        }
        Ok(messages)
    }

    /// Encodes a text message without allowing packet delimiter injection.
    pub fn text(sender: &str, receiver: &str, body: &str) -> Result<String> {
        if sender.is_empty()
            || receiver.is_empty()
            || sender.contains([':', '\r', '\n'])
            || receiver.contains([':', '\r', '\n'])
            || body.contains(['\r', '\n'])
        {
            return Err("invalid FSD text packet field".into());
        }
        Ok(format!("#TM{sender}:{receiver}:{body}\r\n"))
    }

    /// Encodes a pong corresponding to an inbound ping.
    pub fn pong(sender: &str, receiver: &str, timestamp: &str) -> Result<String> {
        if sender.is_empty()
            || receiver.is_empty()
            || timestamp.is_empty()
            || [sender, receiver, timestamp]
                .iter()
                .any(|part| part.contains([':', '\r', '\n']))
        {
            return Err("invalid FSD pong packet field".into());
        }
        Ok(format!("$PO{sender}:{receiver}:{timestamp}\r\n"))
    }
}

fn parse_line(line: &str) -> Option<FsdMessage> {
    if let Some(rest) = line.strip_prefix("#TM") {
        let (sender, rest) = rest.split_once(':')?;
        let (receiver, body) = rest.split_once(':')?;
        Some(FsdMessage::Text {
            sender: sender.into(),
            receiver: receiver.into(),
            body: body.into(),
        })
    } else if let Some(rest) = line.strip_prefix("$PI") {
        parse_triplet(rest).map(|(sender, receiver, timestamp)| FsdMessage::Ping {
            sender,
            receiver,
            timestamp,
        })
    } else if let Some(rest) = line.strip_prefix("$PO") {
        parse_triplet(rest).map(|(sender, receiver, timestamp)| FsdMessage::Pong {
            sender,
            receiver,
            timestamp,
        })
    } else if let Some(rest) = line.strip_prefix("#DA") {
        let callsign = rest.split(':').next()?;
        (!callsign.is_empty()).then(|| FsdMessage::ControllerOffline {
            callsign: callsign.into(),
        })
    } else if let Some(rest) = line.strip_prefix('%') {
        let fields = rest.split(':').collect::<Vec<_>>();
        if fields.len() < 8 {
            return None;
        }
        Some(FsdMessage::ControllerUpdate {
            callsign: fields[0].into(),
            frequency_khz: fields[1].parse::<u32>().ok()?.checked_add(100_000)?,
            facility: fields[2].parse().ok()?,
            visible_range_nm: fields[3].parse().ok()?,
            latitude: fields[5].parse().ok()?,
            longitude: fields[6].parse().ok()?,
        })
    } else {
        None
    }
}

fn parse_triplet(rest: &str) -> Option<(String, String, String)> {
    let (sender, rest) = rest.split_once(':')?;
    let (receiver, timestamp) = rest.split_once(':')?;
    if sender.is_empty() || receiver.is_empty() || timestamp.is_empty() {
        return None;
    }
    Some((sender.into(), receiver.into(), timestamp.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_fragmented_text_and_ping_without_losing_colons() {
        let mut decoder = FsdDecoder::new();
        assert!(decoder.push(b"#TMATC:PILOT:Cleared").unwrap().is_empty());
        let messages = decoder
            .push(b" route: direct\r\n$PISERVER:PILOT:123\r\n")
            .unwrap();
        assert_eq!(
            messages,
            [
                FsdMessage::Text {
                    sender: "ATC".into(),
                    receiver: "PILOT".into(),
                    body: "Cleared route: direct".into()
                },
                FsdMessage::Ping {
                    sender: "SERVER".into(),
                    receiver: "PILOT".into(),
                    timestamp: "123".into()
                },
            ]
        );
    }

    #[test]
    fn rejects_oversized_lines_and_packet_injection() {
        let mut decoder = FsdDecoder::new();
        assert!(decoder.push(&vec![b'A'; MAX_LINE_BYTES + 1]).is_err());
        assert!(FsdDecoder::text("PILOT", "ATC", "hello\r\n#APbad").is_err());
        assert_eq!(
            FsdDecoder::pong("PILOT", "SERVER", "123").unwrap(),
            "$POPILOT:SERVER:123\r\n"
        );
    }

    #[test]
    fn decodes_atc_position_and_frequency() {
        let mut decoder = FsdDecoder::new();
        let messages = decoder
            .push(b"%EGLL_APP:19350:5:80:1:51.47000:-0.45430:83\r\n")
            .unwrap();
        assert_eq!(
            messages,
            [FsdMessage::ControllerUpdate {
                callsign: "EGLL_APP".into(),
                frequency_khz: 119_350,
                facility: 5,
                visible_range_nm: 80,
                latitude: 51.47,
                longitude: -0.4543,
            }]
        );
    }
}
