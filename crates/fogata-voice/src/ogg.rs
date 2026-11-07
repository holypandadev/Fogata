//! Ogg Opus files: pages as RFC 3533 lays them out, holding Opus as RFC 7845 says, so any
//! player that reads `.ogg` or `.opus` plays a saved message. One logical stream, mono or
//! stereo, mapping family 0.
//!
//! [`read`] takes a file that came from a friend: every page's checksum, its place in the
//! stream and its flags are checked, packets are bounded in size and their length is read
//! from each one's first byte (RFC 6716, 3.1), and the whole is capped in bytes
//! ([`MAX_FILE_BYTES`]) and in length ([`MAX_READ_MS`]) before anything is decoded.
//! Anything else is an [`Error`]; it never panics.

use crate::{Error, MAX_FILE_BYTES, MAX_READ_MS, RATE};

const CAPTURE: &[u8; 4] = b"OggS";

/// Header type flags of a page.
const CONTINUED: u8 = 0x01;
const FIRST: u8 = 0x02;
const LAST: u8 = 0x04;

/// Bytes of a page's header before its segment table.
const HEADER: usize = 27;

/// Longest audio packet read. Opus allows longer ones (120 ms of 2.5 ms frames at their
/// largest), which no voice needs; a packet written here is at most 1275 bytes.
pub const MAX_PACKET: usize = 8 * 1024;

/// Longest comment packet (`OpusTags`) read.
const MAX_TAGS: usize = 64 * 1024;

/// Packets written to a page: a second of 20 ms packets.
const PACKETS_PER_PAGE: usize = 50;

/// Most samples (at 48 kHz) a message read may decode to, its pre-skip included.
const MAX_SAMPLES: u64 = (MAX_READ_MS + 1000) * RATE as u64 / 1000;

/// What the `OpusTags` packet names as the encoder.
const VENDOR: &[u8] = b"Fogata";

/// A file's audio, as read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opus {
    /// Every audio packet, one after another; [`Opus::packet`] gives each.
    data: Vec<u8>,
    /// Where each packet ends in `data`.
    ends: Vec<usize>,
    /// Channels the packets hold: 1 or 2.
    pub channels: u8,
    /// Samples the decoder gives before the sound starts, to drop.
    pub pre_skip: u16,
    /// Samples of sound after the pre-skip, at 48 kHz: the file's length.
    pub samples: u64,
}

impl Opus {
    pub fn packets(&self) -> usize {
        self.ends.len()
    }

    pub fn packet(&self, i: usize) -> &[u8] {
        let start = if i == 0 { 0 } else { self.ends[i - 1] };
        &self.data[start..self.ends[i]]
    }

    /// The length in milliseconds, rounded down.
    pub fn duration_ms(&self) -> u64 {
        self.samples * 1000 / RATE as u64
    }
}

/// Samples (at 48 kHz) an Opus packet decodes to, from its table of contents byte, or why
/// it is not a packet: empty, or a frame count that is missing, zero or over 120 ms.
pub fn packet_samples(packet: &[u8]) -> Result<usize, Error> {
    let toc = *packet.first().ok_or(Error("an empty packet"))?;
    let config = toc >> 3;
    let frame = match config {
        // SILK: 10, 20, 40, 60 ms.
        0..=11 => [480, 960, 1920, 2880][usize::from(config % 4)],
        // Hybrid: 10, 20 ms.
        12..=15 => [480, 960][usize::from(config % 2)],
        // CELT: 2.5, 5, 10, 20 ms.
        _ => [120, 240, 480, 960][usize::from(config % 4)],
    };
    let frames = match toc & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => {
            let count = usize::from(
                packet
                    .get(1)
                    .ok_or(Error("a packet without its frame count"))?
                    & 0x3f,
            );
            if count == 0 {
                return Err(Error("a packet of no frames"));
            }
            count
        }
    };
    let samples = frame * frames;
    if samples > 5760 {
        return Err(Error("a packet over 120 ms"));
    }
    Ok(samples)
}

/// The Ogg checksum: CRC-32 with polynomial 0x04c11db7, no reflection, starting at 0.
pub(crate) fn crc(bytes: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut table = [0u32; 256];
        for (i, slot) in table.iter_mut().enumerate() {
            let mut r = (i as u32) << 24;
            for _ in 0..8 {
                r = if r & 0x8000_0000 != 0 {
                    (r << 1) ^ 0x04c1_1db7
                } else {
                    r << 1
                };
            }
            *slot = r;
        }
        table
    });
    bytes.iter().fold(0u32, |c, &b| {
        (c << 8) ^ table[usize::from(((c >> 24) as u8) ^ b)]
    })
}

/// Writes one page holding `packets` whole, the last of them ending on it.
fn page(out: &mut Vec<u8>, flags: u8, granule: u64, serial: u32, seq: u32, packets: &[&[u8]]) {
    let start = out.len();
    out.extend_from_slice(CAPTURE);
    out.push(0);
    out.push(flags);
    out.extend_from_slice(&granule.to_le_bytes());
    out.extend_from_slice(&serial.to_le_bytes());
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    let lacing: Vec<u8> = packets
        .iter()
        .flat_map(|p| {
            let full = p.len() / 255;
            std::iter::repeat_n(255u8, full).chain([(p.len() % 255) as u8])
        })
        .collect();
    debug_assert!(lacing.len() <= 255);
    out.push(lacing.len() as u8);
    out.extend_from_slice(&lacing);
    for p in packets {
        out.extend_from_slice(p);
    }
    let sum = crc(&out[start..]);
    out[start + 22..start + 26].copy_from_slice(&sum.to_le_bytes());
}

/// Lacing values a packet takes in a page's segment table.
fn lacing(packet: &[u8]) -> usize {
    packet.len() / 255 + 1
}

/// `packets` cut into pages: at most [`PACKETS_PER_PAGE`] each, and never more than a page's
/// 255 lacing values (a packet of at most [`MAX_PACKET`] bytes takes 33).
fn pages_of(packets: &[Vec<u8>]) -> Vec<&[Vec<u8>]> {
    let mut out = Vec::new();
    let (mut start, mut used) = (0, 0);
    for (i, p) in packets.iter().enumerate() {
        let n = lacing(p);
        if i > start && (i - start == PACKETS_PER_PAGE || used + n > 255) {
            out.push(&packets[start..i]);
            (start, used) = (i, 0);
        }
        used += n;
    }
    if start < packets.len() {
        out.push(&packets[start..]);
    }
    out
}

/// An Ogg Opus file of mono `packets` (at least one, each at most [`MAX_PACKET`] bytes)
/// whose sound is `samples` long after `pre_skip` samples, in a stream numbered `serial`.
pub fn write(packets: &[Vec<u8>], pre_skip: u16, samples: u64, serial: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut head = Vec::with_capacity(19);
    head.extend_from_slice(b"OpusHead");
    head.push(1);
    head.push(1);
    head.extend_from_slice(&pre_skip.to_le_bytes());
    head.extend_from_slice(&RATE.to_le_bytes());
    head.extend_from_slice(&0i16.to_le_bytes());
    head.push(0);
    page(&mut out, FIRST, 0, serial, 0, &[&head]);
    let mut tags = Vec::new();
    tags.extend_from_slice(b"OpusTags");
    tags.extend_from_slice(&(VENDOR.len() as u32).to_le_bytes());
    tags.extend_from_slice(VENDOR);
    tags.extend_from_slice(&0u32.to_le_bytes());
    let mut seq = 1;
    page(&mut out, 0, 0, serial, seq, &[&tags]);
    let end = u64::from(pre_skip) + samples;
    let mut decoded = 0u64;
    let chunks = pages_of(packets);
    for (i, chunk) in chunks.iter().enumerate() {
        seq += 1;
        decoded += chunk
            .iter()
            .map(|p| packet_samples(p).unwrap_or(0) as u64)
            .sum::<u64>();
        let last = i + 1 == chunks.len();
        // No page ends after the end: its position would be past the last page's, which
        // no reader takes.
        let granule = end.min(decoded);
        let refs: Vec<&[u8]> = chunk.iter().map(Vec::as_slice).collect();
        page(
            &mut out,
            if last { LAST } else { 0 },
            granule,
            serial,
            seq,
            &refs,
        );
    }
    debug_assert!(!chunks.is_empty());
    out
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// The `OpusHead` packet: (channels, pre-skip).
fn head(p: &[u8]) -> Result<(u8, u16), Error> {
    if p.len() != 19 || &p[..8] != b"OpusHead" {
        return Err(Error("not an Opus file"));
    }
    if p[8] >> 4 != 0 {
        return Err(Error("an Opus file of a version not known"));
    }
    let channels = p[9];
    if !(1..=2).contains(&channels) || p[18] != 0 {
        return Err(Error("not a mono or stereo Opus file"));
    }
    Ok((channels, u16_at(p, 10)))
}

/// Checks the `OpusTags` packet: its vendor and comments fit in it.
fn tags(p: &[u8]) -> Result<(), Error> {
    const BAD: Error = Error("damaged Opus comments");
    if p.get(..8) != Some(b"OpusTags".as_slice()) {
        return Err(Error("an Opus file without its comments"));
    }
    let mut at = 8usize;
    let vendor = u32_at(p, at).ok_or(BAD)? as usize;
    // Lengths up to 4 GiB: added with care, as a size is 32 bits on some computers.
    at = at
        .checked_add(4)
        .and_then(|a| a.checked_add(vendor))
        .filter(|&a| a <= p.len())
        .ok_or(BAD)?;
    let count = u32_at(p, at).ok_or(BAD)? as usize;
    at += 4;
    if count > (p.len() - at) / 4 {
        return Err(BAD);
    }
    for _ in 0..count {
        let len = u32_at(p, at).ok_or(BAD)? as usize;
        at = at
            .checked_add(4)
            .and_then(|a| a.checked_add(len))
            .filter(|&a| a <= p.len())
            .ok_or(BAD)?;
    }
    Ok(())
}

/// Reads an Ogg Opus file of one stream, checking everything (see the module's text).
pub fn read(bytes: &[u8]) -> Result<Opus, Error> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(Error("too big for a voice message"));
    }
    let mut at = 0usize;
    let mut seq = 0u32;
    let mut serial = 0u32;
    // Packets completed so far: 0 head, 1 comments, then audio.
    let mut count = 0usize;
    let mut partial: Vec<u8> = Vec::new();
    let mut open = false;
    let mut channels = 0;
    let mut pre_skip = 0;
    let mut data = Vec::new();
    let mut ends = Vec::new();
    let mut decoded = 0u64;
    let mut last_granule = 0u64;
    let mut end = None;
    while at < bytes.len() {
        if end.is_some() {
            return Err(Error("data after the last page"));
        }
        let header = bytes
            .get(at..at + HEADER)
            .ok_or(Error("a page cut short"))?;
        if &header[..4] != CAPTURE || header[4] != 0 {
            return Err(Error("not an Ogg page"));
        }
        let flags = header[5];
        let granule = u64::from_le_bytes(header[6..14].try_into().map_err(|_| Error("?"))?);
        let page_serial = u32_at(header, 14).ok_or(Error("?"))?;
        let page_seq = u32_at(header, 18).ok_or(Error("?"))?;
        let sum = u32_at(header, 22).ok_or(Error("?"))?;
        let segments = usize::from(header[26]);
        if flags & !(CONTINUED | FIRST | LAST) != 0 {
            return Err(Error("a page with unknown flags"));
        }
        if seq == 0 {
            serial = page_serial;
        } else if page_serial != serial {
            return Err(Error("more than one stream"));
        }
        if page_seq != seq {
            return Err(Error("pages missing or out of order"));
        }
        if (flags & FIRST != 0) != (seq == 0) {
            return Err(Error("a stream that does not start on its first page"));
        }
        if (flags & CONTINUED != 0) != open {
            return Err(Error("a packet broken across pages"));
        }
        let table = bytes
            .get(at + HEADER..at + HEADER + segments)
            .ok_or(Error("a page cut short"))?;
        let body_len: usize = table.iter().map(|&l| usize::from(l)).sum();
        let page_end = at + HEADER + segments + body_len;
        let page_bytes = bytes.get(at..page_end).ok_or(Error("a page cut short"))?;
        let mut zeroed = page_bytes.to_vec();
        zeroed[22..26].fill(0);
        if crc(&zeroed) != sum {
            return Err(Error("a damaged page"));
        }
        if segments == 0 {
            return Err(Error("an empty page"));
        }
        let mut body = at + HEADER + segments;
        let mut completed = 0usize;
        for (i, &lacing) in table.iter().enumerate() {
            let len = usize::from(lacing);
            let limit = if count < 2 { MAX_TAGS } else { MAX_PACKET };
            if partial.len() + len > limit {
                return Err(Error("a packet too long"));
            }
            partial.extend_from_slice(&bytes[body..body + len]);
            body += len;
            open = lacing == 255;
            if open {
                continue;
            }
            let packet = std::mem::take(&mut partial);
            completed += 1;
            match count {
                0 => (channels, pre_skip) = head(&packet)?,
                1 => {
                    tags(&packet)?;
                    // Audio starts on a page of its own.
                    if i + 1 != table.len() {
                        return Err(Error("audio on the comments' page"));
                    }
                }
                _ => {
                    decoded += packet_samples(&packet)? as u64;
                    if decoded > MAX_SAMPLES {
                        return Err(Error("longer than a voice message"));
                    }
                    data.extend_from_slice(&packet);
                    ends.push(data.len());
                }
            }
            count += 1;
        }
        if seq == 0 && (completed != 1 || open) {
            return Err(Error("a first page with more than the Opus header"));
        }
        // A page ends no later than the sound decoded so far: 0 on the header pages, so a
        // file with no audio packets ends at 0 and has no sound.
        if granule != u64::MAX {
            if granule < last_granule || granule > decoded {
                return Err(Error("pages whose positions do not add up"));
            }
            last_granule = granule;
        }
        if flags & LAST != 0 {
            if open || granule == u64::MAX {
                return Err(Error("a last page that ends nothing"));
            }
            end = Some(granule);
        }
        seq = seq.checked_add(1).ok_or(Error("too many pages"))?;
        at = page_end;
    }
    let end = end.ok_or(Error("a file without its last page"))?;
    if count < 2 {
        return Err(Error("an Opus file without its headers"));
    }
    let samples = end
        .checked_sub(u64::from(pre_skip))
        .filter(|&s| s > 0)
        .ok_or(Error("a file with no sound"))?;
    // Compared in samples: no product of an untrusted number.
    if samples > MAX_READ_MS * u64::from(RATE) / 1000 {
        return Err(Error("longer than a voice message"));
    }
    Ok(Opus {
        data,
        ends,
        channels,
        pre_skip,
        samples,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A CELT 20 ms mono packet's first byte (config 31, code 0), then `len - 1` bytes.
    fn packet(len: usize, fill: u8) -> Vec<u8> {
        let mut p = vec![fill; len];
        p[0] = 31 << 3;
        p
    }

    fn sample_file(packets: usize, samples: u64) -> (Vec<Vec<u8>>, Vec<u8>) {
        let list: Vec<Vec<u8>> = (0..packets)
            .map(|i| packet(40 + i % 300, i as u8))
            .collect();
        let file = write(&list, 312, samples, 0x1234_5678);
        (list, file)
    }

    #[test]
    fn the_checksum_is_ogg_s() {
        // A known value: the CRC-32/MPEG-2 polynomial with no reflection and no final
        // inversion, starting from 0 (CRC-32/CKSUM without its length and inversion).
        assert_eq!(crc(b"123456789"), 0x89a1_897f);
    }

    #[test]
    fn round_trip() {
        for (packets, samples) in [
            (1, 600),
            (50, 50 * 960 - 312),
            (51, 51 * 960 - 800),
            (777, 777 * 960 - 400),
        ] {
            let (list, file) = sample_file(packets, samples);
            let read = read(&file).unwrap();
            assert_eq!(read.packets(), packets);
            for (i, p) in list.iter().enumerate() {
                assert_eq!(read.packet(i), p.as_slice());
            }
            assert_eq!(
                (read.channels, read.pre_skip, read.samples),
                (1, 312, samples)
            );
        }
    }

    /// Comments whose lengths say up to 4 GiB are refused, never wrapping around.
    #[test]
    fn comments_of_huge_lengths_are_refused() {
        let mut p = b"OpusTags".to_vec();
        p.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(tags(&p).is_err());
        let mut p = b"OpusTags".to_vec();
        p.extend_from_slice(&0u32.to_le_bytes());
        p.extend_from_slice(&1u32.to_le_bytes());
        p.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(tags(&p).is_err());
        let mut p = b"OpusTags".to_vec();
        p.extend_from_slice(&0u32.to_le_bytes());
        p.extend_from_slice(&0u32.to_le_bytes());
        assert!(tags(&p).is_ok());
    }

    /// Packets that decode to more than the sound (its end trimmed by more than a page)
    /// still make a file that reads, with that length.
    #[test]
    fn a_file_trimmed_by_more_than_a_page_reads() {
        let (_, file) = sample_file(120, 1000);
        let read = read(&file).unwrap();
        assert_eq!((read.packets(), read.samples), (120, 1000));
    }

    #[test]
    fn packets_of_255_bytes_and_more_take_several_segments() {
        let list = vec![
            packet(255, 1),
            packet(510, 2),
            packet(1275, 3),
            packet(1, 4),
        ];
        let file = write(&list, 0, 4 * 960, 7);
        let read = read(&file).unwrap();
        for (i, p) in list.iter().enumerate() {
            assert_eq!(read.packet(i), p.as_slice());
        }
    }

    #[test]
    fn pages_never_take_more_than_255_lacing_values() {
        // Packets as large as the encoder makes, and as large as are read: several pages.
        for len in [1275, MAX_PACKET, 254, 255, 510] {
            let list: Vec<Vec<u8>> = (0..120).map(|i| packet(len, i as u8)).collect();
            let file = write(&list, 312, 120 * 960 - 400, 3);
            for at in pages(&file) {
                let segments = usize::from(file[at + 26]);
                let body: usize = file[at + HEADER..at + HEADER + segments]
                    .iter()
                    .map(|&l| usize::from(l))
                    .sum();
                assert!(body <= 255 * 255);
            }
            let read = read(&file).unwrap();
            assert_eq!(read.packets(), list.len(), "{len}");
            for (i, p) in list.iter().enumerate() {
                assert_eq!(read.packet(i), p.as_slice());
            }
        }
    }

    #[test]
    fn packet_lengths_come_from_their_first_byte() {
        assert_eq!(packet_samples(&[31 << 3]), Ok(960));
        assert_eq!(packet_samples(&[(1 << 3) | 1, 0, 0]), Ok(1920));
        assert_eq!(packet_samples(&[(16 << 3) | 3, 48]), Ok(5760));
        assert!(packet_samples(&[(16 << 3) | 3, 49]).is_err());
        assert!(packet_samples(&[(3 << 3) | 3, 3]).is_err());
        assert!(packet_samples(&[(31 << 3) | 3, 0]).is_err());
        assert!(packet_samples(&[(31 << 3) | 3]).is_err());
        assert!(packet_samples(&[]).is_err());
    }

    /// Where each page starts.
    fn pages(file: &[u8]) -> Vec<usize> {
        let mut out = vec![];
        let mut at = 0;
        while at < file.len() {
            out.push(at);
            let segments = usize::from(file[at + 26]);
            let body: usize = file[at + HEADER..at + HEADER + segments]
                .iter()
                .map(|&l| usize::from(l))
                .sum();
            at += HEADER + segments + body;
        }
        out
    }

    #[test]
    fn hostile_files_are_refused() {
        let (_, good) = sample_file(120, 100_000);
        assert!(read(&good).is_ok());
        let starts = pages(&good);
        let mut cases: Vec<(&str, Vec<u8>)> = vec![];
        // A flipped byte anywhere fails the checksum (or the capture pattern).
        for at in [0, 5, 30, 60, good.len() / 2, good.len() - 1] {
            let mut f = good.clone();
            f[at] ^= 0x40;
            cases.push(("flipped", f));
        }
        // Cut short, at every page boundary and inside pages.
        for &cut in starts.iter().skip(1).chain([good.len() - 3, 20, 0].iter()) {
            cases.push(("cut", good[..cut].to_vec()));
        }
        // Data after the end.
        let mut f = good.clone();
        f.extend_from_slice(&good[..100]);
        cases.push(("trailing", f));
        // A page dropped, or two swapped.
        let mut f = good[..starts[3]].to_vec();
        f.extend_from_slice(&good[starts[4]..]);
        cases.push(("dropped", f));
        // Fields changed, checksums made right again.
        let (tags_page, audio_page, last_page) = (starts[1], starts[2], *starts.last().unwrap());
        type Edit = Box<dyn Fn(&mut Vec<u8>)>;
        let edits: Vec<(&str, Edit)> = vec![
            ("version", Box::new(|f| f[4] = 1)),
            ("flags", Box::new(|f| f[5] |= 0x08)),
            ("no first flag", Box::new(|f| f[5] = 0)),
            ("serial", Box::new(move |f| f[audio_page + 14] ^= 1)),
            ("sequence", Box::new(move |f| f[audio_page + 18] = 9)),
            (
                "continued",
                Box::new(move |f| f[audio_page + 5] |= CONTINUED),
            ),
            ("head magic", Box::new(|f| f[HEADER + 1] = b'X')),
            ("channels", Box::new(|f| f[HEADER + 1 + 9] = 3)),
            ("mapping", Box::new(|f| f[HEADER + 1 + 18] = 1)),
            ("head version", Box::new(|f| f[HEADER + 1 + 8] = 0x10)),
            (
                "granule past what decodes",
                Box::new(move |f| {
                    f[audio_page + 6..audio_page + 14]
                        .copy_from_slice(&u64::MAX.wrapping_sub(5).to_le_bytes())
                }),
            ),
            (
                "no last flag",
                Box::new(move |f| {
                    f[last_page + 5] &= !LAST;
                }),
            ),
            (
                "vendor too long",
                Box::new(move |f| {
                    let tags = tags_page + HEADER + 1 + 8;
                    f[tags..tags + 4].copy_from_slice(&1000u32.to_le_bytes());
                }),
            ),
            (
                "comment count",
                Box::new(move |f| {
                    let tags = tags_page + HEADER + 1 + 8 + 4 + VENDOR.len();
                    f[tags..tags + 4].copy_from_slice(&u32::MAX.to_le_bytes());
                }),
            ),
            (
                "empty audio packet",
                Box::new(move |f| {
                    // The first audio packet's lacing value 0: an empty packet.
                    f[audio_page + HEADER] = 0;
                }),
            ),
        ];
        for (name, edit) in edits {
            let mut f = good.clone();
            edit(&mut f);
            reseal_lenient(&mut f);
            cases.push((name, f));
        }
        for (name, f) in cases {
            assert!(read(&f).is_err(), "{name} was read");
        }
    }

    /// The two header pages alone, the comments' page marked last with `granule`.
    fn headers_only(granule: u64) -> Vec<u8> {
        let good = write(&[packet(30, 1)], 312, 600, 9);
        let starts = pages(&good);
        let mut out = good[..starts[2]].to_vec();
        out.truncate(starts[1]);
        let tags = &good[starts[1]..starts[2]];
        let segments = usize::from(tags[26]);
        let body = tags[HEADER + segments..].to_vec();
        page(&mut out, LAST, granule, 9, 1, &[&body]);
        out
    }

    #[test]
    fn a_file_with_no_audio_is_refused_whatever_its_end_says() {
        for granule in [0, 312, 48_000 * 900 + 312, 1 << 60, 1 << 63, u64::MAX - 1] {
            let file = headers_only(granule);
            let expected = if granule == 0 {
                "a file with no sound"
            } else {
                "pages whose positions do not add up"
            };
            assert_eq!(read(&file), Err(Error(expected)), "{granule}");
        }
        // A header page that claims a position is refused too.
        let mut file = write(&[packet(30, 1)], 312, 600, 9);
        file[6] = 1;
        reseal_lenient(&mut file);
        assert!(read(&file).is_err());
    }

    #[test]
    fn caps_hold() {
        // 15 minutes and a few seconds of small packets: refused for its length.
        let small = |n: usize| (0..n).map(|i| packet(20, i as u8)).collect::<Vec<_>>();
        let packets = (MAX_READ_MS as usize + 3000) / 20;
        let file = write(&small(packets), 312, packets as u64 * 960 - 312, 1);
        assert!(file.len() < MAX_FILE_BYTES);
        assert_eq!(
            read(&file).unwrap_err(),
            Error("longer than a voice message")
        );
        // 15 minutes reads.
        let packets = crate::MAX_MS as usize / 20;
        let file = write(&small(packets), 312, packets as u64 * 960 - 312, 1);
        assert!(read(&file).is_ok());
        // A pre-skip past the end leaves no sound.
        let mut file = write(&[packet(30, 1)], 312, 600, 1);
        file[HEADER + 1 + 10..HEADER + 1 + 12].copy_from_slice(&2000u16.to_le_bytes());
        reseal_lenient(&mut file);
        assert_eq!(read(&file).unwrap_err(), Error("a file with no sound"));
        // A file over the size cap is refused before anything is read.
        assert!(read(&vec![0; MAX_FILE_BYTES + 1]).is_err());
        // A packet over the cap, in segments of 255.
        let file = write(&[packet(MAX_PACKET + 1, 1)], 0, 960, 1);
        assert_eq!(read(&file).unwrap_err(), Error("a packet too long"));
    }

    /// Random changes to a good file never panic, and random bytes are never read.
    #[test]
    fn noise_never_panics() {
        let (_, good) = sample_file(80, 70_000);
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for round in 0..4000 {
            let mut f = good.clone();
            for _ in 0..1 + next() % 8 {
                let at = (next() as usize) % f.len();
                match next() % 4 {
                    0 => f[at] = next() as u8,
                    1 => f.truncate(at),
                    2 => f.insert(at, next() as u8),
                    _ => {
                        f.remove(at);
                    }
                }
                if f.is_empty() {
                    break;
                }
            }
            if round % 2 == 0 {
                reseal_lenient(&mut f);
            }
            let _ = read(&f);
        }
        for len in [0, 1, 26, 27, 28, 100, 5000] {
            let noise: Vec<u8> = (0..len).map(|_| next() as u8).collect();
            assert!(read(&noise).is_err());
        }
    }

    /// Fixes checksums where the page structure still holds, so changes reach past them.
    pub(crate) fn reseal_lenient(file: &mut [u8]) {
        let mut at = 0;
        while at + HEADER <= file.len() {
            let segments = usize::from(file[at + 26]);
            let Some(table) = file.get(at + HEADER..at + HEADER + segments) else {
                return;
            };
            let body: usize = table.iter().map(|&l| usize::from(l)).sum();
            let end = at + HEADER + segments + body;
            if end > file.len() {
                return;
            }
            file[at + 22..at + 26].fill(0);
            let sum = crc(&file[at..end]);
            file[at + 22..at + 26].copy_from_slice(&sum.to_le_bytes());
            at = end;
        }
    }
}
