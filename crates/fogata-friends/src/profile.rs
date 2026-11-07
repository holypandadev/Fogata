//! Profiles: the name and picture a device shows to its friends and to the members of its
//! groups, signed by the device key.

use fogata_crypto::{
    DeviceIdentity, DevicePublicKey, SIGNATURE_LEN,
    codec::{Reader, Writer},
    names,
};

use crate::FriendError;

const LABEL: &[u8] = b"fogata/v1/profile";
const ENCODING_VERSION: u8 = 1;
/// Longest name, in characters.
pub const MAX_NAME_CHARS: usize = 40;
/// Largest picture, in bytes.
pub const MAX_PHOTO_BYTES: usize = 64 * 1024;
/// Number of preset avatars (pictures shipped with the application). Profiles signed by
/// a newer version may use more; an application shows those as one of its own.
pub const PRESET_AVATARS: u8 = 5;

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG_MAGIC: &[u8] = b"\xff\xd8\xff";

/// Whether `bytes` start like a PNG or a JPEG.
pub fn is_picture(bytes: &[u8]) -> bool {
    bytes.starts_with(PNG_MAGIC) || bytes.starts_with(JPEG_MAGIC)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Avatar {
    /// One of the preset avatars, `0..PRESET_AVATARS` when signed here.
    Preset(u8),
    /// A small PNG or JPEG picture.
    Photo(Vec<u8>),
}

impl Avatar {
    fn check(&self) -> Result<(), FriendError> {
        match self {
            Avatar::Preset(_) => Ok(()),
            Avatar::Photo(bytes) if bytes.len() <= MAX_PHOTO_BYTES && is_picture(bytes) => Ok(()),
            Avatar::Photo(_) => Err(FriendError::BadPhoto),
        }
    }

    fn write(&self, w: &mut Writer) {
        match self {
            Avatar::Preset(n) => w.u8(0).u8(*n),
            Avatar::Photo(bytes) => w.u8(1).bytes(bytes),
        };
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, FriendError> {
        match r.u8()? {
            0 => Ok(Avatar::Preset(r.u8()?)),
            1 => Ok(Avatar::Photo(r.bytes()?.to_vec())),
            _ => Err(FriendError::Malformed),
        }
    }
}

/// A device's name and picture, signed by its key. A newer `version_ms` replaces an older
/// profile of the same device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub device: DevicePublicKey,
    pub name: String,
    pub avatar: Avatar,
    /// When the profile was last changed, in milliseconds since the Unix epoch.
    pub version_ms: u64,
    pub signature: [u8; SIGNATURE_LEN],
}

/// What shows of a name ([`fogata_crypto::names::tidy`]), checked: 1 to [`MAX_NAME_CHARS`]
/// characters, none of them control characters or hiding marks. A name of nothing but
/// blank characters (fillers, spaces, marks with nothing to sit on) is refused like an empty
/// one.
pub fn clean_name(name: &str) -> Result<String, FriendError> {
    if name.chars().any(hides) {
        return Err(FriendError::BadName);
    }
    let name = names::tidy(name);
    let count = name.chars().count();
    if count == 0 || count > MAX_NAME_CHARS {
        return Err(FriendError::BadName);
    }
    Ok(name)
}

/// Control characters, and marks that hide in text or turn it around: those let a name pass
/// for another one, or reverse what is shown after it. The same as group names refuse.
fn hides(c: char) -> bool {
    c.is_control() || names::hiding_mark(c)
}

impl Profile {
    pub fn sign(
        identity: &DeviceIdentity,
        name: &str,
        avatar: Avatar,
        version_ms: u64,
    ) -> Result<Self, FriendError> {
        if matches!(avatar, Avatar::Preset(n) if n >= PRESET_AVATARS) {
            return Err(FriendError::BadAvatar);
        }
        avatar.check()?;
        let mut profile = Self {
            device: identity.public_key(),
            name: clean_name(name)?,
            avatar,
            version_ms,
            signature: [0; SIGNATURE_LEN],
        };
        profile.signature = identity.sign(&profile.signed_bytes());
        Ok(profile)
    }

    fn signed_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.raw(LABEL).key(&self.device).str(&self.name);
        self.avatar.write(&mut w);
        w.u64(self.version_ms);
        w.into_bytes()
    }

    /// Checks the signature and the limits on name and picture.
    pub fn verify(&self) -> Result<(), FriendError> {
        if clean_name(&self.name)? != self.name {
            return Err(FriendError::BadName);
        }
        self.avatar.check()?;
        self.device
            .verify(&self.signed_bytes(), &self.signature)
            .map_err(|_| FriendError::NotProved)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u8(ENCODING_VERSION).key(&self.device).str(&self.name);
        self.avatar.write(&mut w);
        w.u64(self.version_ms).raw(&self.signature);
        w.into_bytes()
    }

    /// Decodes and verifies a profile.
    pub fn decode(bytes: &[u8]) -> Result<Self, FriendError> {
        let mut r = Reader::new(bytes);
        if r.u8()? != ENCODING_VERSION {
            return Err(FriendError::Malformed);
        }
        let profile = Self {
            device: r.key()?,
            name: r.str()?,
            avatar: Avatar::read(&mut r)?,
            version_ms: r.u64()?,
            signature: r.array()?,
        };
        r.finish()?;
        profile.verify()?;
        Ok(profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_that_hide_marks_are_refused() {
        assert_eq!(clean_name("  Ana "), Ok("Ana".to_string()));
        assert!(clean_name("Ana\u{202E}exe.png").is_err());
        assert!(clean_name("A\u{200B}na").is_err());
        assert!(clean_name("\u{2067}Ana").is_err());
        assert!(clean_name("Ana\nBea").is_err());
        assert!(clean_name("👨\u{200D}👩\u{200D}👧").is_ok());
        assert!(clean_name("Ana\u{FEFF}").is_err());
    }

    #[test]
    fn names_that_show_nothing_are_refused_and_padding_goes() {
        for blank in [
            "\u{3164}",
            "\u{115F}\u{1160}",
            "\u{FFA0}",
            "\u{2800}\u{2800}",
            "\u{180E}",
            "\u{2003}\u{205F}\u{3000}\u{00A0}",
            "\u{0301}\u{0308}",
            "\u{FE0F}\u{E0100}",
            "\u{E0041}\u{E007F}",
            "\u{200D}",
            "\u{00AD}",
        ] {
            assert_eq!(clean_name(blank), Err(FriendError::BadName), "{blank:?}");
        }
        assert_eq!(clean_name("Ana\u{3164}\u{3164}"), Ok("Ana".to_string()));
        assert_eq!(
            clean_name("\u{2800}Ana  \u{3000}Bea"),
            Ok("Ana Bea".to_string())
        );
        // Forty characters that show, whatever padding came with them.
        let padded = format!("{}{}", "ñ".repeat(40), "\u{3164}".repeat(10));
        assert_eq!(clean_name(&padded), Ok("ñ".repeat(40)));
        for name in ["김민지", "مريم", "क्\u{200D}षमा", "👩🏽\u{200D}💻"] {
            assert_eq!(clean_name(name), Ok(name.to_string()));
        }
    }

    #[test]
    fn profiles_with_padded_or_blank_names_are_refused() {
        let identity = DeviceIdentity::generate().unwrap();
        let mut profile = Profile::sign(&identity, "Ana", Avatar::Preset(0), 7).unwrap();
        for name in ["Ana\u{3164}", "\u{3164}", "Ana  Bea"] {
            profile.name = name.into();
            profile.signature = identity.sign(&profile.signed_bytes());
            assert_eq!(
                Profile::decode(&profile.encode()),
                Err(FriendError::BadName)
            );
        }
        assert_eq!(
            Profile::sign(&identity, "\u{2800}\u{3164}", Avatar::Preset(0), 8),
            Err(FriendError::BadName)
        );
    }

    #[test]
    fn profiles_round_trip_and_verify() {
        let identity = DeviceIdentity::generate().unwrap();
        let profile = Profile::sign(&identity, "  Ana  ", Avatar::Preset(3), 7).unwrap();
        assert_eq!(profile.name, "Ana");
        assert_eq!(Profile::decode(&profile.encode()).unwrap(), profile);

        let mut photo = PNG_MAGIC.to_vec();
        photo.extend_from_slice(&[0; 100]);
        let with_photo = Profile::sign(&identity, "Ana", Avatar::Photo(photo), 8).unwrap();
        assert_eq!(Profile::decode(&with_photo.encode()).unwrap(), with_photo);
    }

    #[test]
    fn presets_from_newer_versions_are_accepted() {
        let identity = DeviceIdentity::generate().unwrap();
        let mut profile = Profile::sign(&identity, "Ana", Avatar::Preset(0), 7).unwrap();
        profile.avatar = Avatar::Preset(PRESET_AVATARS + 3);
        profile.signature = identity.sign(&profile.signed_bytes());
        assert_eq!(Profile::decode(&profile.encode()).unwrap(), profile);
    }

    #[test]
    fn tampering_is_detected() {
        let identity = DeviceIdentity::generate().unwrap();
        let mut profile = Profile::sign(&identity, "Ana", Avatar::Preset(0), 7).unwrap();
        profile.name = "Eva".into();
        assert_eq!(profile.verify(), Err(FriendError::NotProved));
        assert_eq!(
            Profile::decode(&profile.encode()),
            Err(FriendError::NotProved)
        );
    }

    #[test]
    fn limits_are_enforced() {
        let identity = DeviceIdentity::generate().unwrap();
        let sign = |name: &str, avatar| Profile::sign(&identity, name, avatar, 1);
        assert_eq!(sign("", Avatar::Preset(0)), Err(FriendError::BadName));
        assert_eq!(sign("a\nb", Avatar::Preset(0)), Err(FriendError::BadName));
        assert_eq!(
            sign(&"ñ".repeat(41), Avatar::Preset(0)),
            Err(FriendError::BadName)
        );
        assert!(sign(&"ñ".repeat(40), Avatar::Preset(0)).is_ok());
        assert_eq!(
            sign("Ana", Avatar::Preset(PRESET_AVATARS)),
            Err(FriendError::BadAvatar)
        );
        assert_eq!(
            sign("Ana", Avatar::Photo(b"GIF89a".to_vec())),
            Err(FriendError::BadPhoto)
        );
        let mut big = JPEG_MAGIC.to_vec();
        big.resize(MAX_PHOTO_BYTES + 1, 0);
        assert_eq!(sign("Ana", Avatar::Photo(big)), Err(FriendError::BadPhoto));
    }
}
