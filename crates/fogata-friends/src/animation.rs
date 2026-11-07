//! Animations: a GIF or WebP that moves in place of a device's picture or banner. Older
//! versions refuse a profile or banner whose picture is not a PNG or JPEG, so the profile and
//! the banner keep a still picture (the animation's first frame) that every version shows,
//! and the animation goes apart, signed by the device key and naming that still picture: it
//! shows only while the device's picture or banner is the one it names.

use fogata_crypto::{
    DeviceIdentity, DevicePublicKey, SIGNATURE_LEN,
    codec::{Reader, Writer},
};
use sha2::{Digest, Sha256};

use crate::FriendError;

const LABEL: &[u8] = b"fogata/v1/animation";
const ENCODING_VERSION: u8 = 1;
/// Largest animation of a profile picture, in bytes: it goes to the members of every group
/// at each connection. Earlier versions took 256 KiB, and refuse a larger one: they show
/// the still picture.
pub const MAX_PHOTO_ANIMATION_BYTES: usize = 550 * 1024;
/// Largest animation of a banner, in bytes (512 KiB in earlier versions).
pub const MAX_BANNER_ANIMATION_BYTES: usize = 550 * 1024;
/// Most pixels of an animation of a profile picture. Each frame is decoded at the
/// animation's whole size however small it is drawn, again each time it comes round, so
/// it is held to about what the largest avatar needs. Earlier versions took any size.
pub const MAX_PHOTO_ANIMATION_PIXELS: u64 = 512 * 512;
/// Most pixels of an animation of a banner (three times as wide as tall).
pub const MAX_BANNER_ANIMATION_PIXELS: u64 = 1024 * 384;
/// Most frames of an animation; earlier versions took any number.
pub const MAX_ANIMATION_FRAMES: u32 = 500;

/// What an animation moves in place of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Animated {
    /// The profile's picture.
    Photo,
    /// The banner's picture.
    Banner,
}

impl Animated {
    /// The largest animation of this kind, in bytes.
    pub fn max_bytes(self) -> usize {
        match self {
            Animated::Photo => MAX_PHOTO_ANIMATION_BYTES,
            Animated::Banner => MAX_BANNER_ANIMATION_BYTES,
        }
    }

    /// The most pixels of an animation of this kind.
    pub fn max_pixels(self) -> u64 {
        match self {
            Animated::Photo => MAX_PHOTO_ANIMATION_PIXELS,
            Animated::Banner => MAX_BANNER_ANIMATION_PIXELS,
        }
    }

    fn code(self) -> u8 {
        match self {
            Animated::Photo => 0,
            Animated::Banner => 1,
        }
    }

    fn from_code(code: u8) -> Result<Self, FriendError> {
        match code {
            0 => Ok(Animated::Photo),
            1 => Ok(Animated::Banner),
            _ => Err(FriendError::Malformed),
        }
    }
}

/// Whether `bytes` start like a GIF or a WebP, the kinds of animation the application plays.
pub fn is_animation(bytes: &[u8]) -> bool {
    bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || (bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP")
}

/// What an animation names its still picture by.
pub fn still_digest(still: &[u8]) -> [u8; 32] {
    Sha256::new()
        .chain_update(LABEL)
        .chain_update(still)
        .finalize()
        .into()
}

/// A device's animation, signed by its key. A newer `version_ms` replaces an older one of
/// the same device and kind; one with no picture says the device shows none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Animation {
    pub device: DevicePublicKey,
    pub of: Animated,
    /// [`still_digest`] of the still picture this one moves in place of.
    pub still: [u8; 32],
    /// The GIF or WebP; empty for none.
    pub picture: Vec<u8>,
    /// When the animation was last changed, in milliseconds since the Unix epoch.
    pub version_ms: u64,
    pub signature: [u8; SIGNATURE_LEN],
}

impl Animation {
    pub fn sign(
        identity: &DeviceIdentity,
        of: Animated,
        still: &[u8],
        picture: Vec<u8>,
        version_ms: u64,
    ) -> Result<Self, FriendError> {
        let mut animation = Self {
            device: identity.public_key(),
            of,
            still: still_digest(still),
            picture,
            version_ms,
            signature: [0; SIGNATURE_LEN],
        };
        animation.check()?;
        animation.signature = identity.sign(&animation.signed_bytes());
        Ok(animation)
    }

    /// Empty, or a GIF or WebP within the limits of its kind in bytes, pixels and frames.
    fn check(&self) -> Result<(), FriendError> {
        let fits = |m: Measure| {
            (1..=self.of.max_pixels()).contains(&m.pixels())
                && (1..=MAX_ANIMATION_FRAMES).contains(&m.frames)
        };
        if self.picture.is_empty()
            || (self.picture.len() <= self.of.max_bytes()
                && measure(&self.picture).is_some_and(fits))
        {
            Ok(())
        } else {
            Err(FriendError::BadAnimation)
        }
    }

    /// Whether this moves in place of `still`, the device's picture or banner now.
    pub fn animates(&self, still: &[u8]) -> bool {
        !self.picture.is_empty() && self.still == still_digest(still)
    }

    fn write_body(&self, w: &mut Writer) {
        w.key(&self.device)
            .u8(self.of.code())
            .raw(&self.still)
            .bytes(&self.picture)
            .u64(self.version_ms);
    }

    fn signed_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.raw(LABEL);
        self.write_body(&mut w);
        w.into_bytes()
    }

    /// Checks the signature and the limits on the picture.
    pub fn verify(&self) -> Result<(), FriendError> {
        self.check()?;
        self.device
            .verify(&self.signed_bytes(), &self.signature)
            .map_err(|_| FriendError::NotProved)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u8(ENCODING_VERSION);
        self.write_body(&mut w);
        w.raw(&self.signature);
        w.into_bytes()
    }

    /// Decodes and verifies an animation.
    pub fn decode(bytes: &[u8]) -> Result<Self, FriendError> {
        let mut r = Reader::new(bytes);
        if r.u8()? != ENCODING_VERSION {
            return Err(FriendError::Malformed);
        }
        let animation = Self {
            device: r.key()?,
            of: Animated::from_code(r.u8()?)?,
            still: r.array()?,
            picture: r.bytes()?.to_vec(),
            version_ms: r.u64()?,
            signature: r.array()?,
        };
        r.finish()?;
        animation.verify()?;
        Ok(animation)
    }
}

/// The size and the number of frames of a GIF or WebP, read from its headers and the sizes
/// of its blocks: nothing is decoded. Every frame is decoded at the picture's whole size
/// (the canvas every frame lies within), so that is the size given. None when the bytes are
/// neither, are cut short or are damaged.
fn measure(bytes: &[u8]) -> Option<Measure> {
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        measure_gif(bytes)
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        measure_webp(bytes)
    } else {
        None
    }
}

/// What [`measure`] reads of an animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Measure {
    width: u32,
    height: u32,
    frames: u32,
}

impl Measure {
    fn pixels(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// Grows the canvas to hold a frame at (`x`, `y`) of `width` by `height`.
    fn hold(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.width = self.width.max(x.saturating_add(width));
        self.height = self.height.max(y.saturating_add(height));
    }
}

/// Bytes read one after the other; every read past the end gives None.
struct Bytes<'a> {
    rest: &'a [u8],
}

impl<'a> Bytes<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let (taken, rest) = self.rest.split_at_checked(n)?;
        self.rest = rest;
        Some(taken)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn u16(&mut self) -> Option<u32> {
        let b = self.take(2)?;
        Some(u32::from(u16::from_le_bytes([b[0], b[1]])))
    }

    /// GIF data sub-blocks: each a length and that many bytes, until a length of 0.
    fn skip_sub_blocks(&mut self) -> Option<()> {
        loop {
            let len = self.u8()?;
            if len == 0 {
                return Some(());
            }
            self.take(usize::from(len))?;
        }
    }
}

fn measure_gif(bytes: &[u8]) -> Option<Measure> {
    let mut r = Bytes { rest: &bytes[6..] };
    let mut size = Measure {
        width: r.u16()?,
        height: r.u16()?,
        frames: 0,
    };
    let flags = r.u8()?;
    r.take(2)?;
    if flags & 0x80 != 0 {
        r.take(3 << ((flags & 7) + 1))?;
    }
    // Up to the trailer; a file that ends between two blocks without one still shows the
    // frames it has.
    while let Some(block) = r.u8() {
        match block {
            0x3b => break,
            0x21 => {
                r.u8()?;
                r.skip_sub_blocks()?;
            }
            0x2c => {
                let (x, y, width, height) = (r.u16()?, r.u16()?, r.u16()?, r.u16()?);
                let flags = r.u8()?;
                if flags & 0x80 != 0 {
                    r.take(3 << ((flags & 7) + 1))?;
                }
                // The LZW code size, then the picture's data.
                r.u8()?;
                r.skip_sub_blocks()?;
                size.hold(x, y, width, height);
                size.frames += 1;
            }
            _ => return None,
        }
    }
    Some(size)
}

fn measure_webp(bytes: &[u8]) -> Option<Measure> {
    let u24 = |b: &[u8]| u32::from_le_bytes([b[0], b[1], b[2], 0]);
    let mut r = Bytes { rest: &bytes[12..] };
    let mut size = Measure {
        width: 0,
        height: 0,
        frames: 0,
    };
    let mut still = false;
    // Chunks: a name, a length and that many bytes, padded to an even length. Fewer than a
    // chunk's header left is the end.
    while r.rest.len() >= 8 {
        let name = r.take(4)?;
        let len = u32::from_le_bytes(r.take(4)?.try_into().ok()?) as usize;
        let data = r.take(len)?;
        if len % 2 == 1 {
            r.take(1.min(r.rest.len()))?;
        }
        match name {
            b"VP8X" => {
                let d = data.get(..10)?;
                size.hold(0, 0, u24(&d[4..7]) + 1, u24(&d[7..10]) + 1);
            }
            b"ANMF" => {
                let d = data.get(..12)?;
                size.hold(
                    u24(&d[0..3]) * 2,
                    u24(&d[3..6]) * 2,
                    u24(&d[6..9]) + 1,
                    u24(&d[9..12]) + 1,
                );
                size.frames += 1;
            }
            b"VP8 " => {
                let d = data.get(..10)?;
                if d[3..6] != [0x9d, 0x01, 0x2a] {
                    return None;
                }
                let width = u32::from(u16::from_le_bytes([d[6], d[7]]) & 0x3fff);
                let height = u32::from(u16::from_le_bytes([d[8], d[9]]) & 0x3fff);
                size.hold(0, 0, width, height);
                still = true;
            }
            b"VP8L" => {
                let d = data.get(..5)?;
                if d[0] != 0x2f {
                    return None;
                }
                let bits = u32::from_le_bytes([d[1], d[2], d[3], d[4]]);
                size.hold(0, 0, (bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1);
                still = true;
            }
            _ => {}
        }
    }
    // A picture with no frames of an animation is one still frame.
    if size.frames == 0 && still {
        size.frames = 1;
    }
    Some(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STILL: &[u8] = b"\x89PNG\r\n\x1a\nstill";

    /// A GIF `width` by `height` pixels with `frames` frames of one pixel each, shown one
    /// after the other with no wait: tens of bytes that ask for a full-size frame for each.
    fn frames(width: u16, height: u16, frames: u32) -> Vec<u8> {
        let mut gif = b"GIF89a".to_vec();
        gif.extend(width.to_le_bytes());
        gif.extend(height.to_le_bytes());
        // A global table of two colours.
        gif.extend([0x80, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff]);
        for _ in 0..frames {
            // No wait before the next frame; one pixel at the top left: clear, colour 0, end.
            gif.extend([0x21, 0xf9, 4, 0, 0, 0, 0, 0]);
            gif.extend([0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0]);
            gif.extend([2, 2, 0x44, 0x01, 0]);
        }
        gif.push(0x3b);
        gif
    }

    /// A GIF of two frames of 8 by 8 pixels padded with a comment to `len` bytes.
    fn gif(len: usize) -> Vec<u8> {
        let mut gif = frames(8, 8, 2);
        gif.pop();
        let mut left = len - gif.len() - 4;
        gif.extend([0x21, 0xfe]);
        while left > 0 {
            let mut block = left.min(256);
            if left - block == 1 {
                block -= 1;
            }
            gif.push((block - 1) as u8);
            gif.resize(gif.len() + block - 1, b'A');
            left -= block;
        }
        gif.extend([0, 0x3b]);
        assert_eq!(gif.len(), len);
        gif
    }

    /// Three frames of 16 by 16 pixels in WebP, 80 ms each.
    const WEBP: &str = "UklGRhgBAABXRUJQVlA4WAoAAAACAAAADwAADwAAQU5JTQYAAAAAAAAAAABBTk1GSgAAAAAAAAAAAA8AAA8AAFAA\
        AAJWUDggMgAAADABAJ0BKhAAEAABQCYloAADcAD+8ut///mwP/bz/wR6Af//0uD//pcH//S4P/SkAAAAQU5NRkwA\
        AAAAAAAAAAAPAAAPAABQAAAAVlA4IDQAAAA0AQCdASoQABAAAAAmJaAAA3AA/ukiH//3nz//ufP/+58/6M///yn7\
        //I4//8jj/5QIAAAQU5NRkYAAAAAAAAAAAAPAAAPAABQAAAAVlA4IC4AAAA0AQCdASoQABAAAAAmJaAAA3AA/vtV\
        4///S4P/+lwf/9Lg/9Lg//rV5Vesq6AA";

    fn webp() -> Vec<u8> {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(WEBP.replace(' ', ""))
            .unwrap()
    }

    #[test]
    fn animations_round_trip_and_name_their_still_picture() {
        let identity = DeviceIdentity::generate().unwrap();
        let animation = Animation::sign(&identity, Animated::Photo, STILL, gif(100), 7).unwrap();
        let back = Animation::decode(&animation.encode()).unwrap();
        assert_eq!(back, animation);
        assert!(back.animates(STILL));
        assert!(!back.animates(b"\x89PNG\r\n\x1a\nanother"));

        let banner = Animation::sign(&identity, Animated::Banner, STILL, webp(), 8).unwrap();
        assert_eq!(Animation::decode(&banner.encode()).unwrap(), banner);

        // None: the device shows its still picture again.
        let none = Animation::sign(&identity, Animated::Photo, STILL, Vec::new(), 9).unwrap();
        assert!(!Animation::decode(&none.encode()).unwrap().animates(STILL));
    }

    #[test]
    fn only_small_gifs_and_webps_are_signed_or_taken() {
        let identity = DeviceIdentity::generate().unwrap();
        for bad in [
            b"\x89PNG\r\n\x1a\n".to_vec(),
            b"\xff\xd8\xff".to_vec(),
            gif(MAX_PHOTO_ANIMATION_BYTES + 1),
        ] {
            assert_eq!(
                Animation::sign(&identity, Animated::Photo, STILL, bad, 1),
                Err(FriendError::BadAnimation)
            );
        }
        assert_eq!(
            Animation::sign(
                &identity,
                Animated::Banner,
                STILL,
                gif(MAX_BANNER_ANIMATION_BYTES + 1),
                1
            ),
            Err(FriendError::BadAnimation)
        );
        // 550 KiB of either goes and is taken.
        for of in [Animated::Photo, Animated::Banner] {
            assert_eq!(of.max_bytes(), 550 * 1024);
            let big = Animation::sign(&identity, of, STILL, gif(550 * 1024), 1).unwrap();
            assert_eq!(Animation::decode(&big.encode()).unwrap(), big);
        }
        let mut animation =
            Animation::sign(&identity, Animated::Photo, STILL, gif(100), 1).unwrap();
        animation.picture = gif(101);
        assert_eq!(
            Animation::decode(&animation.encode()),
            Err(FriendError::NotProved)
        );
        let mut bytes = animation.encode();
        bytes[33] = 7;
        assert_eq!(Animation::decode(&bytes), Err(FriendError::Malformed));
    }

    #[test]
    fn the_size_and_frames_are_read_without_decoding() {
        let size = |bytes: &[u8]| measure(bytes).map(|m| (m.width, m.height, m.frames));
        assert_eq!(size(&frames(600, 400, 3)), Some((600, 400, 3)));
        assert_eq!(size(&gif(10_000)), Some((8, 8, 2)));
        assert_eq!(size(&webp()), Some((16, 16, 3)));
        // A frame past the picture's size counts: decoders grow the picture to hold it.
        let mut outside = frames(8, 8, 1);
        outside[28..30].copy_from_slice(&1000u16.to_le_bytes());
        assert_eq!(size(&outside), Some((1001, 8, 1)));
        // Without its trailer it shows the frames it has; cut inside a block, nothing.
        let mut cut = frames(8, 8, 2);
        cut.pop();
        assert_eq!(size(&cut), Some((8, 8, 2)));
        cut.pop();
        assert_eq!(size(&cut), None);
        // Damaged: a block of no kind, a WebP chunk longer than the file.
        let mut damaged = frames(8, 8, 1);
        damaged.insert(damaged.len() - 1, 0x99);
        assert_eq!(size(&damaged), None);
        let mut long = webp();
        long.truncate(100);
        assert_eq!(size(&long), None);
        assert_eq!(size(b"\x89PNG\r\n\x1a\n"), None);
    }

    #[test]
    fn only_animations_within_the_pixels_and_frames_of_their_kind_are_taken() {
        let identity = DeviceIdentity::generate().unwrap();
        let sign = |of, picture| Animation::sign(&identity, of, STILL, picture, 1);
        // A frame of 512 by 512 for a picture, as many pixels in another shape for a banner.
        assert!(sign(Animated::Photo, frames(512, 512, 2)).is_ok());
        assert!(sign(Animated::Banner, frames(1024, 384, 2)).is_ok());
        assert!(sign(Animated::Banner, frames(600, 600, 2)).is_ok());
        for (of, picture) in [
            (Animated::Photo, frames(513, 512, 2)),
            (Animated::Photo, frames(2048, 2048, 2)),
            (Animated::Banner, frames(1025, 384, 2)),
            // A flat colour of hundreds of frames, each decoded whole.
            (Animated::Photo, frames(64, 64, MAX_ANIMATION_FRAMES + 1)),
            (Animated::Photo, frames(64, 64, 0)),
        ] {
            assert_eq!(sign(of, picture), Err(FriendError::BadAnimation));
        }
        assert!(sign(Animated::Photo, frames(64, 64, MAX_ANIMATION_FRAMES)).is_ok());
        // A record signed by a copy that took more is refused when it arrives.
        let mut big = sign(Animated::Photo, frames(64, 64, 2)).unwrap();
        big.picture = frames(2048, 2048, 2);
        big.signature = identity.sign(&big.signed_bytes());
        assert_eq!(
            Animation::decode(&big.encode()),
            Err(FriendError::BadAnimation)
        );
    }
}
