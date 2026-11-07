//! The CBOR codec of the member and friend protocols, with a bound on what a message may
//! become once decoded.
//!
//! A message is read whole (up to its size limit) before anyone knows who sent it, and
//! decoding happens before any check of the sender. Every CBOR data item, even an empty byte
//! string of one byte, becomes a value of 24 bytes or more, so 4 MiB of tiny items would take
//! a hundred megabytes and a tenth of a second of CPU to decode, from a peer that never
//! proved anything. Messages with more data items than one per 32 bytes of the largest
//! message (more than a whole group log, entry hash by entry hash, would need) are refused
//! before they are decoded.

use std::io;

use futures::{AsyncRead, AsyncReadExt as _, AsyncWrite};
use libp2p::{
    StreamProtocol,
    request_response::{self, cbor},
};
use serde::{Serialize, de::DeserializeOwned};

/// The CBOR codec of `libp2p`, refusing messages of too many data items.
pub(crate) struct Guarded<Req, Resp> {
    inner: cbor::codec::Codec<Req, Resp>,
    max_bytes: u64,
}

impl<Req, Resp> Guarded<Req, Resp> {
    /// Messages of up to `max_bytes` each way.
    pub(crate) fn new(max_bytes: u64) -> Self {
        Self {
            inner: cbor::codec::Codec::default()
                .set_request_size_maximum(max_bytes)
                .set_response_size_maximum(max_bytes),
            max_bytes,
        }
    }
}

/// Reads a whole message of up to `max_bytes`, refusing one of more data items than one per
/// 32 bytes of that, for the inner codec to decode.
async fn read_checked<T>(io: &mut T, max_bytes: u64) -> io::Result<futures::io::Cursor<Vec<u8>>>
where
    T: AsyncRead + Unpin + Send,
{
    let mut bytes = Vec::new();
    io.take(max_bytes).read_to_end(&mut bytes).await?;
    let max_items = usize::try_from(max_bytes / 32).unwrap_or(usize::MAX);
    if too_many_items(&bytes, max_items) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message of too many data items",
        ));
    }
    Ok(futures::io::Cursor::new(bytes))
}

impl<Req, Resp> Clone for Guarded<Req, Resp> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            max_bytes: self.max_bytes,
        }
    }
}

impl<Req, Resp> request_response::Codec for Guarded<Req, Resp>
where
    Req: Send + Serialize + DeserializeOwned,
    Resp: Send + Serialize + DeserializeOwned,
{
    type Protocol = StreamProtocol;
    type Request = Req;
    type Response = Resp;

    async fn read_request<T>(&mut self, protocol: &StreamProtocol, io: &mut T) -> io::Result<Req>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut checked = read_checked(io, self.max_bytes).await?;
        self.inner.read_request(protocol, &mut checked).await
    }

    async fn read_response<T>(&mut self, protocol: &StreamProtocol, io: &mut T) -> io::Result<Resp>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut checked = read_checked(io, self.max_bytes).await?;
        self.inner.read_response(protocol, &mut checked).await
    }

    async fn write_request<T>(
        &mut self,
        protocol: &StreamProtocol,
        io: &mut T,
        request: Req,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        self.inner.write_request(protocol, io, request).await
    }

    async fn write_response<T>(
        &mut self,
        protocol: &StreamProtocol,
        io: &mut T,
        response: Resp,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        self.inner.write_response(protocol, io, response).await
    }
}

/// Whether `bytes`, read as CBOR, hold more than `max` data items. Counts the head of every
/// item (a break code is none) and skips the contents of byte and text strings; it takes
/// one step per byte at most and never fails. Malformed CBOR is left for the decoder to
/// refuse.
pub(crate) fn too_many_items(bytes: &[u8], max: usize) -> bool {
    let mut at = 0usize;
    let mut items = 0usize;
    while let Some(&initial) = bytes.get(at) {
        at += 1;
        if initial == 0xff {
            continue;
        }
        items += 1;
        if items > max {
            return true;
        }
        let major = initial >> 5;
        let width = match initial & 0x1f {
            0..=23 => 0,
            24 => 1,
            25 => 2,
            26 => 4,
            27 => 8,
            // An indefinite length: its parts or items follow as items of their own.
            31 => continue,
            _ => return false,
        };
        let Some(argument) = bytes.get(at..at + width) else {
            return false;
        };
        let value = if width == 0 {
            u64::from(initial & 0x1f)
        } else {
            argument.iter().fold(0u64, |v, b| (v << 8) | u64::from(*b))
        };
        at += width;
        if matches!(major, 2 | 3) {
            match usize::try_from(value).ok().and_then(|n| at.checked_add(n)) {
                Some(next) => at = next,
                None => return false,
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Damaged copies of `good`, as a hostile peer could send: bits flipped, bytes and
    /// lengths changed, cut short, grown, and plain noise. The same ones every run.
    fn damaged(good: &[u8], seed: u64) -> Vec<Vec<u8>> {
        let mut x = seed | 1;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as usize
        };
        (0..2000)
            .map(|_| {
                let mut bytes = good.to_vec();
                let at = next() % (bytes.len() + 1);
                match next() % 6 {
                    0 if at < bytes.len() => bytes[at] ^= 1 << (next() % 8),
                    1 if at < bytes.len() => bytes[at] = next() as u8,
                    2 => bytes.truncate(at),
                    3 => {
                        let extra: Vec<u8> = (0..next() % 64).map(|_| next() as u8).collect();
                        bytes.splice(at..at, extra);
                    }
                    4 if at + 4 <= bytes.len() => {
                        let big = [u32::MAX, 0x8000_0000, 0x0100_0000][next() % 3];
                        bytes[at..at + 4].copy_from_slice(&big.to_be_bytes());
                    }
                    _ => bytes = (0..next() % 300).map(|_| next() as u8).collect(),
                }
                bytes
            })
            .collect()
    }

    #[test]
    fn items_are_counted_by_their_heads() {
        // [h'', h'0102', "ab", 7, -1, 1.5]
        let cbor = [
            0x86, 0x40, 0x42, 1, 2, 0x62, b'a', b'b', 0x07, 0x20, 0xf9, 0x3e, 0x00,
        ];
        assert!(!too_many_items(&cbor, 7));
        assert!(too_many_items(&cbor, 6));
        // An indefinite array of two empty byte strings, with its break code.
        let indefinite = [0x9f, 0x40, 0x40, 0xff];
        assert!(!too_many_items(&indefinite, 3));
        assert!(too_many_items(&indefinite, 2));
        // A string's contents are skipped, whatever they look like.
        let mut long = vec![0x59, 0x01, 0x00];
        long.extend([0x40; 256]);
        assert!(!too_many_items(&long, 1));
    }

    #[test]
    fn damaged_cbor_is_left_to_the_decoder() {
        // Cut short, a length beyond the end, reserved values: no panic, no refusal here.
        for bytes in [
            &[0x5a, 0xff, 0xff][..],
            &[0x5b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x40][..],
            &[0x1c, 0x40, 0x40][..],
            &[][..],
        ] {
            assert!(!too_many_items(bytes, 1));
        }
    }

    #[test]
    fn deeply_nested_or_damaged_messages_are_refused_without_a_panic() {
        use fogata_proto::{MemberRequest, MemberResponse};
        use libp2p::request_response::Codec as _;
        let read = |bytes: Vec<u8>| {
            futures::executor::block_on(async {
                let mut codec = Guarded::<MemberRequest, MemberResponse>::new(4 * 1024 * 1024);
                let protocol = StreamProtocol::new(fogata_proto::MEMBER_PROTOCOL);
                codec
                    .read_request(&protocol, &mut futures::io::Cursor::new(bytes))
                    .await
            })
        };
        // A field the message does not have, skipped whatever it holds: here arrays inside
        // arrays, as deep as the bytes allow.
        let mut nested = vec![0xa1, 0x66];
        nested.extend(b"Roster");
        nested.extend([0xa2, 0x68]);
        nested.extend(b"group_id");
        nested.extend([0x40, 0x61, b'x']);
        nested.extend(vec![0x81u8; 100_000]);
        nested.push(0x00);
        assert!(read(nested).is_err());
        let good = futures::executor::block_on(async {
            let mut codec = Guarded::<MemberRequest, MemberResponse>::new(4 * 1024 * 1024);
            let protocol = StreamProtocol::new(fogata_proto::MEMBER_PROTOCOL);
            let mut wire = Vec::new();
            let request = MemberRequest::Push {
                group_id: vec![1; 16],
                entries: vec![vec![2; 40].into(); 3],
            };
            codec
                .write_request(&protocol, &mut wire, request)
                .await
                .unwrap();
            wire
        });
        for bytes in damaged(&good, 1) {
            let _ = read(bytes);
        }
    }

    #[test]
    fn a_declined_leave_reads_with_older_devices_and_from_them() {
        use fogata_proto::{FRIEND_PROTOCOL, FriendRequest, FriendResponse};
        use libp2p::request_response::Codec as _;
        use serde::Deserialize;
        // The group request as older devices know it.
        #[derive(Debug, Deserialize, serde::Serialize)]
        enum Older {
            Group { entries: Vec<serde_bytes::ByteBuf> },
        }
        fn pass<A, B>(request: A) -> B
        where
            A: Serialize + DeserializeOwned + Send + 'static,
            B: Serialize + DeserializeOwned + Send + 'static,
        {
            futures::executor::block_on(async {
                let protocol = StreamProtocol::new(FRIEND_PROTOCOL);
                let mut wire = Vec::new();
                Guarded::<A, FriendResponse>::new(1024 * 1024)
                    .write_request(&protocol, &mut wire, request)
                    .await
                    .unwrap();
                Guarded::<B, FriendResponse>::new(1024 * 1024)
                    .read_request(&protocol, &mut futures::io::Cursor::new(wire))
                    .await
                    .unwrap()
            })
        }
        let entries = vec![serde_bytes::ByteBuf::from(vec![2; 40]); 2];
        let Older::Group { entries: read } = pass::<_, Older>(FriendRequest::Group {
            entries: entries.clone(),
            declined: Some(serde_bytes::ByteBuf::from(vec![9; 32])),
            part: None,
        });
        assert_eq!(read, entries);
        let FriendRequest::Group {
            entries: read,
            declined,
            part,
        } = pass::<_, FriendRequest>(Older::Group {
            entries: entries.clone(),
        })
        else {
            panic!("not a group");
        };
        assert_eq!((read, declined, part), (entries.clone(), None, None));
        let FriendRequest::Group { part, .. } = pass::<_, FriendRequest>(FriendRequest::Group {
            entries,
            declined: None,
            part: Some((2, 5)),
        }) else {
            panic!("not a group");
        };
        assert_eq!(part, Some((2, 5)));
    }

    #[test]
    fn away_reads_with_older_devices_and_from_them() {
        use fogata_proto::{MEMBER_PROTOCOL, MemberRequest, MemberResponse};
        use libp2p::request_response::Codec as _;
        use serde::Deserialize;
        // The presence request as older devices know it.
        #[derive(Debug, Deserialize, serde::Serialize)]
        enum Older {
            Presence { hidden: bool },
        }
        fn pass<A, B>(request: A) -> B
        where
            A: Serialize + DeserializeOwned + Send + 'static,
            B: Serialize + DeserializeOwned + Send + 'static,
        {
            futures::executor::block_on(async {
                let protocol = StreamProtocol::new(MEMBER_PROTOCOL);
                let mut wire = Vec::new();
                Guarded::<A, MemberResponse>::new(1024 * 1024)
                    .write_request(&protocol, &mut wire, request)
                    .await
                    .unwrap();
                Guarded::<B, MemberResponse>::new(1024 * 1024)
                    .read_request(&protocol, &mut futures::io::Cursor::new(wire))
                    .await
                    .unwrap()
            })
        }
        for (hidden, away) in [(false, false), (false, true), (true, false)] {
            let MemberRequest::Presence { hidden: h, away: a } =
                pass::<_, MemberRequest>(MemberRequest::Presence { hidden, away })
            else {
                panic!("not a presence");
            };
            assert_eq!((h, a), (hidden, away));
            let Older::Presence { hidden: h } =
                pass::<_, Older>(MemberRequest::Presence { hidden, away });
            assert_eq!(h, hidden);
        }
        let MemberRequest::Presence { hidden, away } =
            pass::<_, MemberRequest>(Older::Presence { hidden: true })
        else {
            panic!("not a presence");
        };
        assert_eq!((hidden, away), (true, false));
    }
}
