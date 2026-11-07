//! Banners: the colour, and the picture over it, a device shows at the top of its card. Unlike
//! the profile, a banner goes to friends only, signed by the device key.

use fogata_crypto::{
    DeviceIdentity, DevicePublicKey, SIGNATURE_LEN,
    codec::{Reader, Writer},
};

use crate::{FriendError, profile::is_picture};

const LABEL: &[u8] = b"fogata/v1/banner";
const ENCODING_VERSION: u8 = 1;
/// Largest banner picture, in bytes.
pub const MAX_BANNER_BYTES: usize = 64 * 1024;

/// A device's banner, signed by its key. A newer `version_ms` replaces an older banner of the
/// same device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Banner {
    pub device: DevicePublicKey,
    /// The colour, as 0xAARRGGBB.
    pub color: u32,
    /// A small PNG or JPEG picture shown over the colour.
    pub picture: Option<Vec<u8>>,
    /// When the banner was last changed, in milliseconds since the Unix epoch.
    pub version_ms: u64,
    pub signature: [u8; SIGNATURE_LEN],
}

fn check_picture(picture: &Option<Vec<u8>>) -> Result<(), FriendError> {
    match picture {
        Some(bytes) if bytes.len() > MAX_BANNER_BYTES || !is_picture(bytes) => {
            Err(FriendError::BadPhoto)
        }
        _ => Ok(()),
    }
}

impl Banner {
    pub fn sign(
        identity: &DeviceIdentity,
        color: u32,
        picture: Option<Vec<u8>>,
        version_ms: u64,
    ) -> Result<Self, FriendError> {
        check_picture(&picture)?;
        let mut banner = Self {
            device: identity.public_key(),
            color,
            picture,
            version_ms,
            signature: [0; SIGNATURE_LEN],
        };
        banner.signature = identity.sign(&banner.signed_bytes());
        Ok(banner)
    }

    fn write_body(&self, w: &mut Writer) {
        w.key(&self.device).u32(self.color);
        match &self.picture {
            Some(bytes) => w.u8(1).bytes(bytes),
            None => w.u8(0),
        };
        w.u64(self.version_ms);
    }

    fn signed_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.raw(LABEL);
        self.write_body(&mut w);
        w.into_bytes()
    }

    /// Checks the signature and the limit on the picture.
    pub fn verify(&self) -> Result<(), FriendError> {
        check_picture(&self.picture)?;
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

    /// Decodes and verifies a banner.
    pub fn decode(bytes: &[u8]) -> Result<Self, FriendError> {
        let mut r = Reader::new(bytes);
        if r.u8()? != ENCODING_VERSION {
            return Err(FriendError::Malformed);
        }
        let device = r.key()?;
        let color = r.u32()?;
        let picture = match r.u8()? {
            0 => None,
            1 => Some(r.bytes()?.to_vec()),
            _ => return Err(FriendError::Malformed),
        };
        let banner = Self {
            device,
            color,
            picture,
            version_ms: r.u64()?,
            signature: r.array()?,
        };
        r.finish()?;
        banner.verify()?;
        Ok(banner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

    #[test]
    fn banners_round_trip_and_verify() {
        let identity = DeviceIdentity::generate().unwrap();
        let plain = Banner::sign(&identity, 0xFF3D21AD, None, 7).unwrap();
        assert_eq!(Banner::decode(&plain.encode()).unwrap(), plain);

        let mut picture = PNG.to_vec();
        picture.extend_from_slice(&[0; 100]);
        let with_picture = Banner::sign(&identity, 0xFF2563EB, Some(picture), 8).unwrap();
        assert_eq!(
            Banner::decode(&with_picture.encode()).unwrap(),
            with_picture
        );
    }

    #[test]
    fn tampering_is_detected() {
        let identity = DeviceIdentity::generate().unwrap();
        let mut banner = Banner::sign(&identity, 0xFF3D21AD, None, 7).unwrap();
        banner.color = 0xFF000000;
        assert_eq!(banner.verify(), Err(FriendError::NotProved));
        assert_eq!(
            Banner::decode(&banner.encode()),
            Err(FriendError::NotProved)
        );
    }

    #[test]
    fn pictures_are_small_pngs_or_jpegs() {
        let identity = DeviceIdentity::generate().unwrap();
        assert_eq!(
            Banner::sign(&identity, 0, Some(b"GIF89a".to_vec()), 1),
            Err(FriendError::BadPhoto)
        );
        let mut big = PNG.to_vec();
        big.resize(MAX_BANNER_BYTES + 1, 0);
        assert_eq!(
            Banner::sign(&identity, 0, Some(big), 1),
            Err(FriendError::BadPhoto)
        );
    }
}
