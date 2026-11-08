//! What travels in a direct chat between two friends, inside messages sealed with the key of
//! the chat's two-person group, as group messages are.

use fogata_crypto::CryptoError;
use serde::{Deserialize, Serialize};

use crate::WireVideo;

/// Longest payload read: a message of 2000 characters with ten attachments, each with a
/// video's first picture, fits well inside it.
pub const MAX_DIRECT_PAYLOAD_BYTES: usize = 1 << 20;

/// Most sealed messages in one [`crate::MemberRequest::Direct`].
pub const MAX_DIRECT_BATCH: usize = 64;

/// Plaintext of a message in a direct chat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DirectPayload {
    /// Something written: text, attachments or both. `id` is the same on both devices, and
    /// a copy sent again is kept once.
    Message {
        id: [u8; 16],
        /// When it was written, by the author's clock.
        sent_ms: u64,
        #[serde(default)]
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        files: Vec<DirectFile>,
        /// How long it lasts on each device once there, in milliseconds; 0 for ever.
        #[serde(default, skip_serializing_if = "is_zero")]
        ttl_ms: u64,
    },
    /// The chat's disappearing timer changed, for both people, to `ms` (0 is off).
    Timer { id: [u8; 16], sent_ms: u64, ms: u64 },
    /// These items reached the device that sends this and are kept there; the other one
    /// stops sending them.
    Receipt { ids: Vec<[u8; 16]> },
    /// Message `target` was pinned (or unpinned, `pinned` false) for both people: while
    /// pinned it does not disappear. A device that does not know this kind skips it and
    /// sends no receipt, so it keeps coming until that device reads it.
    Pin {
        id: [u8; 16],
        sent_ms: u64,
        target: [u8; 16],
        pinned: bool,
    },
    /// The sender deleted its own message `target` for both people: the device that gets
    /// this deletes it too, with its files. Sent only to devices that take it, as pins are.
    Unsend {
        id: [u8; 16],
        sent_ms: u64,
        target: [u8; 16],
    },
}

/// What kind of attachment a file is, as the sender tells it: pictures may come by
/// themselves, videos play in the chat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectFileKind {
    Picture,
    Video,
    File,
}

/// An attachment: fetched over the file protocol from the friend, by its BLAKE3 hash.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectFile {
    pub id: [u8; 32],
    pub name: String,
    pub size: u64,
    pub kind: DirectFileKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<WireVideo>,
    /// A voice message: an Ogg Opus file of kind `File`, which copies from before voice
    /// messages show as a file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<WireVoice>,
}

/// Longest voice message: 15 minutes.
pub const MAX_VOICE_MS: u64 = 15 * 60 * 1000;

/// Bars of a voice message's levels, from its start to its end.
pub const VOICE_BARS: usize = 40;

/// Biggest voice message file.
pub const MAX_VOICE_BYTES: u64 = 4 << 20;

/// What a voice message shows before it is fetched: its length and its levels.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireVoice {
    pub duration_ms: u64,
    /// [`VOICE_BARS`] levels, 0 to 255.
    #[serde(with = "crate::base64_text")]
    pub bars: Vec<u8>,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

impl DirectPayload {
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("plain data always serialises")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() > MAX_DIRECT_PAYLOAD_BYTES {
            return Err(CryptoError::Malformed);
        }
        serde_json::from_slice(bytes).map_err(|_| CryptoError::Malformed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payloads_round_trip_and_leave_out_what_is_empty() {
        let message = DirectPayload::Message {
            id: [1; 16],
            sent_ms: 7,
            text: "hi".into(),
            files: vec![DirectFile {
                id: [2; 32],
                name: "weekend.mp4".into(),
                size: 48_000_000,
                kind: DirectFileKind::Video,
                video: Some(WireVideo {
                    thumb: vec![9, 9],
                    width: 1280,
                    height: 720,
                    duration_ms: 72_000,
                }),
                voice: None,
            }],
            ttl_ms: 0,
        };
        assert_eq!(DirectPayload::decode(&message.encode()).unwrap(), message);
        let voice = DirectPayload::Message {
            id: [1; 16],
            sent_ms: 7,
            text: String::new(),
            files: vec![DirectFile {
                id: [2; 32],
                name: "Voice message.ogg".into(),
                size: 60_000,
                kind: DirectFileKind::File,
                video: None,
                voice: Some(WireVoice {
                    duration_ms: 18_000,
                    bars: (0..VOICE_BARS as u8).collect(),
                }),
            }],
            ttl_ms: 0,
        };
        assert_eq!(DirectPayload::decode(&voice.encode()).unwrap(), voice);
        let text = String::from_utf8(message.encode()).unwrap();
        assert!(!text.contains("voice"));
        let plain = DirectPayload::Message {
            id: [1; 16],
            sent_ms: 7,
            text: "hi".into(),
            files: vec![],
            ttl_ms: 0,
        };
        let text = String::from_utf8(plain.encode()).unwrap();
        assert!(!text.contains("files") && !text.contains("ttl_ms"));
        for payload in [
            DirectPayload::Timer {
                id: [3; 16],
                sent_ms: 8,
                ms: 86_400_000,
            },
            DirectPayload::Receipt {
                ids: vec![[4; 16], [5; 16]],
            },
            DirectPayload::Pin {
                id: [6; 16],
                sent_ms: 9,
                target: [1; 16],
                pinned: true,
            },
            DirectPayload::Unsend {
                id: [7; 16],
                sent_ms: 10,
                target: [1; 16],
            },
        ] {
            assert_eq!(DirectPayload::decode(&payload.encode()).unwrap(), payload);
        }
    }

    /// The payloads as copies made before pins read them: a pin does not decode there, and
    /// such a copy skips a payload that does not decode, so it neither keeps nor confirms
    /// it. What those copies send still reads the same here.
    #[test]
    fn copies_before_pins_skip_a_pin_and_still_read_the_rest() {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        #[allow(dead_code)]
        enum Before {
            Message {
                id: [u8; 16],
                sent_ms: u64,
                #[serde(default)]
                text: String,
                #[serde(default)]
                files: Vec<DirectFile>,
                #[serde(default)]
                ttl_ms: u64,
            },
            Timer {
                id: [u8; 16],
                sent_ms: u64,
                ms: u64,
            },
            Receipt {
                ids: Vec<[u8; 16]>,
            },
        }
        let pin = DirectPayload::Pin {
            id: [6; 16],
            sent_ms: 9,
            target: [1; 16],
            pinned: true,
        };
        assert!(serde_json::from_slice::<Before>(&pin.encode()).is_err());
        let message = DirectPayload::Message {
            id: [1; 16],
            sent_ms: 7,
            text: "hi".into(),
            files: vec![],
            ttl_ms: 0,
        };
        assert!(serde_json::from_slice::<Before>(&message.encode()).is_ok());
        // A voice message reads there as a file: the field it does not know is skipped.
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct FileBefore {
            id: [u8; 32],
            name: String,
            size: u64,
            kind: DirectFileKind,
        }
        let file = DirectFile {
            id: [2; 32],
            name: "Voice message.ogg".into(),
            size: 9,
            kind: DirectFileKind::File,
            video: None,
            voice: Some(WireVoice {
                duration_ms: 1000,
                bars: vec![0; VOICE_BARS],
            }),
        };
        let before: FileBefore =
            serde_json::from_slice(&serde_json::to_vec(&file).unwrap()).unwrap();
        assert_eq!(before.name, "Voice message.ogg");
        // What a copy before pins sends, byte for byte.
        let old = br#"{"kind":"receipt","ids":[[6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6]]}"#;
        assert_eq!(
            DirectPayload::decode(old).unwrap(),
            DirectPayload::Receipt { ids: vec![[6; 16]] }
        );
    }

    #[test]
    fn oversized_or_unknown_payloads_are_refused() {
        assert!(DirectPayload::decode(br#"{"kind":"shout","id":[1]}"#).is_err());
        let big = format!(
            r#"{{"kind":"message","id":[{}],"sent_ms":1,"text":"{}"}}"#,
            ["0"; 16].join(","),
            "x".repeat(MAX_DIRECT_PAYLOAD_BYTES)
        );
        assert!(DirectPayload::decode(big.as_bytes()).is_err());
    }
}
