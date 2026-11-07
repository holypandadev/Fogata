//! Media packets larger than one QUIC datagram can carry, sent in parts.
//!
//! QUIC packets shrink to 1200 bytes on a small path, and quinn also falls back to that size
//! for a minute at a time when it takes jitter or bursts of loss for one. That leaves less
//! room than a full WebRTC video packet. Such a packet then goes as two (or a few) datagrams,
//! each with the packet's addresses, a number and its place in it; the receiving bridge puts
//! them back together and forgets the ones whose parts do not all come soon. Like a whole
//! datagram, a part is never sent again and holds up nothing behind it: a lost part loses
//! the packet, which WebRTC asks for again or does without, as with any lost packet.

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{Duration, Instant},
};

use bytes::{BufMut, Bytes, BytesMut};
use fogata_net::PeerId;

use crate::bridge::{TAG, put_addr};

/// Marks a part, where a whole packet has the bridge's `VERSION`.
pub(crate) const PART: u8 = 2;
/// Tag, mark, both addresses, the packet's number, and the part's place and count.
pub(crate) const PART_HEADER: usize = 2 + 6 + 6 + 2 + 1 + 1;
/// No packet goes in more parts than this; the smallest QUIC datagram carries about 1150
/// bytes, so a WebRTC packet takes two.
const MOST_PARTS: usize = 4;
/// A packet whose parts did not all come within this long is dropped: WebRTC's jitter
/// buffer has moved on, or a part was lost.
const PART_WAIT: Duration = Duration::from_millis(300);
/// Packets waiting for their other parts, from every member together, and from one member:
/// one sending parts it never completes cannot crowd out the others.
const WAITING: usize = 256;
const WAITING_PER_MEMBER: usize = 64;

/// One part of a packet, as received.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Part {
    pub src: SocketAddr,
    pub dst: SocketAddr,
    id: u16,
    index: u8,
    count: u8,
    chunk: Bytes,
}

/// Splits a packet from `src` to `dst` into datagrams of at most `max` bytes, as evenly as
/// possible; `None` when it would take more than [`MOST_PARTS`].
pub(crate) fn split(
    src: SocketAddr,
    dst: SocketAddr,
    payload: &[u8],
    max: usize,
    id: u16,
) -> Option<Vec<Bytes>> {
    let room = max.checked_sub(PART_HEADER).filter(|room| *room > 0)?;
    let count = payload.len().div_ceil(room).max(1);
    if count > MOST_PARTS {
        return None;
    }
    let size = payload.len().div_ceil(count);
    let mut parts = Vec::with_capacity(count);
    for (index, chunk) in payload.chunks(size.max(1)).enumerate() {
        let mut out = BytesMut::with_capacity(PART_HEADER + chunk.len());
        out.put_u8(TAG);
        out.put_u8(PART);
        put_addr(&mut out, src)?;
        put_addr(&mut out, dst)?;
        out.put_u16(id);
        out.put_u8(index as u8);
        out.put_u8(count as u8);
        out.put_slice(chunk);
        parts.push(out.freeze());
    }
    Some(parts)
}

/// Reads a part; `None` for anything else, a whole packet included.
pub(crate) fn decode(packet: &Bytes) -> Option<Part> {
    if packet.len() <= PART_HEADER || packet[0] != TAG || packet[1] != PART {
        return None;
    }
    let addr = |at: usize| {
        let ip = Ipv4Addr::new(packet[at], packet[at + 1], packet[at + 2], packet[at + 3]);
        SocketAddr::new(
            IpAddr::V4(ip),
            u16::from_be_bytes([packet[at + 4], packet[at + 5]]),
        )
    };
    let (index, count) = (packet[16], packet[17]);
    if count < 2 || usize::from(count) > MOST_PARTS || index >= count {
        return None;
    }
    Some(Part {
        src: addr(2),
        dst: addr(8),
        id: u16::from_be_bytes([packet[14], packet[15]]),
        index,
        count,
        chunk: packet.slice(PART_HEADER..),
    })
}

struct Waiting {
    since: Instant,
    src: SocketAddr,
    dst: SocketAddr,
    chunks: Vec<Option<Bytes>>,
}

/// Packets whose parts are coming in, by member and number.
#[derive(Default)]
pub(crate) struct Reassembly {
    waiting: HashMap<(PeerId, u16), Waiting>,
}

impl Reassembly {
    /// Takes a part from `peer`; once every part of its packet is in, returns the packet's
    /// addresses and the packet.
    pub(crate) fn add(
        &mut self,
        peer: PeerId,
        part: Part,
        now: Instant,
    ) -> Option<(SocketAddr, SocketAddr, Bytes)> {
        let key = (peer, part.id);
        let fits = self.waiting.get(&key).is_some_and(|w| {
            now.duration_since(w.since) < PART_WAIT
                && w.src == part.src
                && w.dst == part.dst
                && w.chunks.len() == usize::from(part.count)
        });
        if !fits {
            // A number used again (it wraps), or a packet whose parts stopped coming.
            self.waiting.remove(&key);
            let full = |waiting: &HashMap<(PeerId, u16), Waiting>| {
                waiting.len() >= WAITING
                    || waiting.keys().filter(|(p, _)| *p == peer).count() >= WAITING_PER_MEMBER
            };
            if full(&self.waiting) {
                self.waiting
                    .retain(|_, w| now.duration_since(w.since) < PART_WAIT);
                if full(&self.waiting) {
                    return None;
                }
            }
            self.waiting.insert(
                key,
                Waiting {
                    since: now,
                    src: part.src,
                    dst: part.dst,
                    chunks: vec![None; usize::from(part.count)],
                },
            );
        }
        let waiting = self.waiting.get_mut(&key)?;
        waiting.chunks[usize::from(part.index)] = Some(part.chunk);
        if waiting.chunks.iter().any(Option::is_none) {
            return None;
        }
        let waiting = self.waiting.remove(&key)?;
        let mut packet = BytesMut::new();
        for chunk in waiting.chunks.into_iter().flatten() {
            packet.put_slice(&chunk);
        }
        Some((waiting.src, waiting.dst, packet.freeze()))
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.waiting.len()
    }
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

    fn addrs() -> (SocketAddr, SocketAddr) {
        (
            "10.1.2.1:40000".parse().unwrap(),
            "10.3.4.1:40007".parse().unwrap(),
        )
    }

    #[test]
    fn a_full_video_packet_fits_the_smallest_quic_datagram_in_two_parts() {
        let (src, dst) = addrs();
        // 1162 bytes is what a 1200-byte QUIC packet carries; a full SRTP video packet from
        // WebRTC is about 1210.
        let payload: Vec<u8> = (0..1210u32).map(|i| i as u8).collect();
        let parts = split(src, dst, &payload, 1162, 7).unwrap();
        assert_eq!(parts.len(), 2);
        assert!(parts.iter().all(|p| p.len() <= 1162));
        let peer = PeerId::random();
        let mut joined = Reassembly::default();
        let now = Instant::now();
        // In any order.
        assert!(joined.add(peer, decode(&parts[1]).unwrap(), now).is_none());
        let (s, d, packet) = joined.add(peer, decode(&parts[0]).unwrap(), now).unwrap();
        assert_eq!((s, d, &packet[..]), (src, dst, &payload[..]));
        assert_eq!(joined.len(), 0);
    }

    #[test]
    fn parts_of_different_packets_and_members_do_not_mix() {
        let (src, dst) = addrs();
        let (a, b) = (PeerId::random(), PeerId::random());
        let one = split(src, dst, &[1; 1500], 1000, 1).unwrap();
        let two = split(src, dst, &[2; 1500], 1000, 2).unwrap();
        let mut joined = Reassembly::default();
        let now = Instant::now();
        assert!(joined.add(a, decode(&one[0]).unwrap(), now).is_none());
        assert!(joined.add(a, decode(&two[1]).unwrap(), now).is_none());
        // The same number from another member is another packet.
        assert!(joined.add(b, decode(&one[1]).unwrap(), now).is_none());
        let (_, _, packet) = joined.add(a, decode(&one[1]).unwrap(), now).unwrap();
        assert_eq!(&packet[..], &[1; 1500][..]);
        let (_, _, packet) = joined.add(a, decode(&two[0]).unwrap(), now).unwrap();
        assert_eq!(&packet[..], &[2; 1500][..]);
    }

    #[test]
    fn a_part_that_comes_too_late_starts_over() {
        let (src, dst) = addrs();
        let peer = PeerId::random();
        let parts = split(src, dst, &[3; 1500], 1000, 9).unwrap();
        let mut joined = Reassembly::default();
        let start = Instant::now();
        assert!(
            joined
                .add(peer, decode(&parts[0]).unwrap(), start)
                .is_none()
        );
        let late = start + PART_WAIT;
        assert!(joined.add(peer, decode(&parts[1]).unwrap(), late).is_none());
        assert_eq!(joined.len(), 1);
    }

    #[test]
    fn waiting_packets_are_bounded() {
        let (src, dst) = addrs();
        let mut joined = Reassembly::default();
        let start = Instant::now();
        // One member alone fills only its own share.
        let greedy = PeerId::random();
        for id in 0..(WAITING_PER_MEMBER as u16 + 20) {
            let parts = split(src, dst, &[4; 1500], 1000, id).unwrap();
            joined.add(greedy, decode(&parts[0]).unwrap(), start);
        }
        assert_eq!(joined.len(), WAITING_PER_MEMBER);
        let peer = PeerId::random();
        let parts = split(src, dst, &[4; 1500], 1000, 0).unwrap();
        joined.add(peer, decode(&parts[0]).unwrap(), start);
        assert_eq!(
            joined
                .add(peer, decode(&parts[1]).unwrap(), start)
                .map(|(_, _, p)| p.len()),
            Some(1500)
        );
        // Several together fill the whole.
        for _ in 0..(WAITING / WAITING_PER_MEMBER) {
            let peer = PeerId::random();
            for id in 0..(WAITING_PER_MEMBER as u16) {
                let parts = split(src, dst, &[4; 1500], 1000, id).unwrap();
                joined.add(peer, decode(&parts[0]).unwrap(), start);
            }
        }
        assert_eq!(joined.len(), WAITING);
        let peer = PeerId::random();
        // Once the oldest are past waiting for, new ones take their place.
        let parts = split(src, dst, &[5; 1500], 1000, 60000).unwrap();
        let later = start + PART_WAIT;
        assert!(
            joined
                .add(peer, decode(&parts[0]).unwrap(), later)
                .is_none()
        );
        assert_eq!(joined.len(), 1);
    }

    #[test]
    fn odd_parts_are_refused() {
        let (src, dst) = addrs();
        assert!(split(src, dst, &[0; 5000], 1000, 1).is_none());
        assert!(split(src, dst, &[0; 100], PART_HEADER, 1).is_none());
        let mut part = BytesMut::from(&split(src, dst, &[0; 1500], 1000, 1).unwrap()[0][..]);
        part[16] = 2; // place 2 of 2
        assert!(decode(&part.freeze()).is_none());
        let whole =
            Bytes::from_static(&[TAG, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9]);
        assert!(decode(&whole).is_none());
    }

    #[test]
    fn damaged_parts_and_packets_are_refused_without_a_panic() {
        let (src, dst) = addrs();
        let parts = split(src, dst, &[5; 3000], 1162, 7).unwrap();
        let whole = crate::bridge::encode(src, dst, &[6; 200]).unwrap();
        let mut reassembly = Reassembly::default();
        let peer = fogata_net::PeerId::random();
        let now = std::time::Instant::now();
        for (i, good) in parts.iter().chain([&whole]).enumerate() {
            for bytes in damaged(good, i as u64 + 1) {
                let bytes = Bytes::from(bytes);
                let _ = crate::bridge::decode(&bytes);
                if let Some(part) = decode(&bytes) {
                    let _ = reassembly.add(peer, part, now);
                }
            }
        }
    }
}
