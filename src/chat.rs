//! Routing and formatting of FSD text messages and `.msg` commands.

use crate::{FsdDecoder, FsdMessage, Result};
use std::collections::BTreeMap;

const MAX_MESSAGES: usize = 500;
const MAX_THREAD_MESSAGES: usize = 200;

/// Destination of a text message received from or sent to FSD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatChannel {
    /// Private conversation identified by the other station's callsign.
    Private(String),
    /// Text radio frequency in kilohertz.
    Radio(u32),
    /// Message sent by the network server to the connected pilot.
    Server,
}

/// One visible text message with its network callsign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    /// Network callsign of the sender.
    pub sender: String,
    /// User-visible message text.
    pub body: String,
    /// Conversation or radio channel containing the message.
    pub channel: ChatChannel,
    /// Whether the message arrived from the network.
    pub incoming: bool,
}

/// A checked outbound FSD text packet and its conversation destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingText {
    /// Channel to display after the network accepts the packet.
    pub channel: ChatChannel,
    /// Complete CRLF-terminated FSD packet.
    pub packet: String,
    /// Human-readable text without the `.msg` command prefix.
    pub body: String,
}

/// Keeps bounded text history grouped by private callsign and radio channel.
#[derive(Debug, Default)]
pub struct ChatInbox {
    own_callsign: String,
    messages: Vec<ChatMessage>,
    private_threads: BTreeMap<String, Vec<ChatMessage>>,
}

impl ChatInbox {
    /// Creates a text inbox for one connected pilot callsign.
    pub fn new(own_callsign: impl Into<String>) -> Self {
        Self {
            own_callsign: own_callsign.into().to_ascii_uppercase(),
            ..Self::default()
        }
    }

    /// Returns all recent visible messages in arrival order.
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Returns private conversations keyed by the other callsign.
    pub fn private_threads(&self) -> &BTreeMap<String, Vec<ChatMessage>> {
        &self.private_threads
    }

    /// Adds a text packet addressed to this pilot or a radio frequency.
    ///
    /// Internal FSD text addressed to other services is ignored.
    pub fn receive(&mut self, packet: &FsdMessage) -> Option<&ChatMessage> {
        let FsdMessage::Text {
            sender,
            receiver,
            body,
        } = packet
        else {
            return None;
        };
        let channel = if let Some(frequency) = parse_radio_address(receiver) {
            ChatChannel::Radio(frequency)
        } else if receiver.eq_ignore_ascii_case(&self.own_callsign) {
            if sender.eq_ignore_ascii_case("SERVER") {
                ChatChannel::Server
            } else {
                ChatChannel::Private(sender.to_ascii_uppercase())
            }
        } else {
            return None;
        };
        let message = ChatMessage {
            sender: sender.to_ascii_uppercase(),
            body: body.clone(),
            channel,
            incoming: true,
        };
        self.insert(message);
        self.messages.last()
    }

    /// Records a sent message only after the network confirms it was written.
    pub fn record_sent(&mut self, outgoing: &OutgoingText) {
        let message = ChatMessage {
            sender: self.own_callsign.clone(),
            body: outgoing.body.clone(),
            channel: outgoing.channel.clone(),
            incoming: false,
        };
        self.insert(message);
    }

    /// Parses `.msg CALLSIGN text` or radio text for the tuned frequency.
    pub fn prepare(&self, input: &str, radio_frequency_khz: Option<u32>) -> Result<OutgoingText> {
        let trimmed = input.trim();
        let (channel, receiver, body) = if trimmed
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(".msg"))
            && trimmed
                .as_bytes()
                .get(4)
                .is_some_and(u8::is_ascii_whitespace)
        {
            let rest = trimmed[5..].trim_start();
            let mut fields = rest.splitn(2, char::is_whitespace);
            let recipient = fields.next().unwrap_or("").to_ascii_uppercase();
            let body = fields.next().unwrap_or("").trim();
            if recipient.len() < 2
                || recipient.len() > 12
                || !recipient
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
            {
                return Err(".msg requires a valid recipient callsign".into());
            }
            (ChatChannel::Private(recipient.clone()), recipient, body)
        } else if trimmed.starts_with('.') {
            return Err("unknown text command".into());
        } else {
            let frequency = radio_frequency_khz.ok_or("no text radio frequency is tuned")?;
            if !(100_000..200_000).contains(&frequency) {
                return Err("text radio frequency is out of range".into());
            }
            (
                ChatChannel::Radio(frequency),
                format!("@{:05}", frequency - 100_000),
                trimmed,
            )
        };
        if body.is_empty() {
            return Err("message text cannot be empty".into());
        }
        let packet = FsdDecoder::text(&self.own_callsign, &receiver, body)?;
        Ok(OutgoingText {
            channel,
            packet,
            body: body.into(),
        })
    }

    fn insert(&mut self, message: ChatMessage) {
        if let ChatChannel::Private(callsign) = &message.channel {
            let thread = self.private_threads.entry(callsign.clone()).or_default();
            thread.push(message.clone());
            trim_front(thread, MAX_THREAD_MESSAGES);
        }
        self.messages.push(message);
        trim_front(&mut self.messages, MAX_MESSAGES);
    }
}

fn trim_front<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        items.drain(..items.len() - limit);
    }
}

fn parse_radio_address(receiver: &str) -> Option<u32> {
    let digits = receiver.strip_prefix('@')?;
    if digits.len() != 5 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok()?.checked_add(100_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_message_opens_thread_by_sender_callsign() {
        let mut inbox = ChatInbox::new("CCA1220");
        let message = FsdMessage::Text {
            sender: "egll_app".into(),
            receiver: "cca1220".into(),
            body: "Contact tower".into(),
        };
        assert_eq!(
            inbox.receive(&message).unwrap().channel,
            ChatChannel::Private("EGLL_APP".into())
        );
        assert_eq!(inbox.private_threads()["EGLL_APP"][0].sender, "EGLL_APP");
    }

    #[test]
    fn radio_and_server_text_do_not_open_private_threads() {
        let mut inbox = ChatInbox::new("CCA1220");
        inbox.receive(&FsdMessage::Text {
            sender: "EGLL_TWR".into(),
            receiver: "@19350".into(),
            body: "Line up".into(),
        });
        inbox.receive(&FsdMessage::Text {
            sender: "SERVER".into(),
            receiver: "CCA1220".into(),
            body: "Welcome".into(),
        });
        assert_eq!(inbox.messages()[0].channel, ChatChannel::Radio(119_350));
        assert_eq!(inbox.messages()[1].channel, ChatChannel::Server);
        assert!(inbox.private_threads().is_empty());
        assert!(inbox
            .receive(&FsdMessage::Text {
                sender: "SERVER".into(),
                receiver: "FP".into(),
                body: "internal".into(),
            })
            .is_none());
    }

    #[test]
    fn msg_command_generates_private_packet_and_rejects_injection() {
        let inbox = ChatInbox::new("CCA1220");
        let outgoing = inbox.prepare(".msg egll_app Hello: there", None).unwrap();
        assert_eq!(outgoing.channel, ChatChannel::Private("EGLL_APP".into()));
        assert_eq!(outgoing.packet, "#TMCCA1220:EGLL_APP:Hello: there\r\n");
        assert!(inbox.prepare(".msg EGLL_APP hi\r\n#APbad", None).is_err());
        assert!(inbox.prepare(".msg EGLL_APP", None).is_err());
        assert!(inbox.prepare(".msg BAD:NAME hi", None).is_err());
    }

    #[test]
    fn plain_text_uses_tuned_frequency() {
        let inbox = ChatInbox::new("CCA1220");
        let outgoing = inbox.prepare("Ready for departure", Some(119_350)).unwrap();
        assert_eq!(outgoing.packet, "#TMCCA1220:@19350:Ready for departure\r\n");
        assert!(inbox.prepare("Ready", None).is_err());
    }
}
