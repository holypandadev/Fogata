//! Bios: a few lines a device shows on its card, under its name. Like the banner, a bio goes
//! to friends only, signed by the device key, apart from the profile: older versions refuse a
//! profile with anything after its signature, so a bio cannot ride inside one.

use fogata_crypto::{
    DeviceIdentity, DevicePublicKey, SIGNATURE_LEN,
    codec::{Reader, Writer},
    names,
};

use crate::FriendError;

const LABEL: &[u8] = b"fogata/v1/bio";
const ENCODING_VERSION: u8 = 1;
/// Longest bio, in characters, line breaks included.
pub const MAX_BIO_CHARS: usize = 190;
/// Most lines in a bio.
pub const MAX_BIO_LINES: usize = 4;
/// Most bytes a bio of [`MAX_BIO_CHARS`] characters can take in UTF-8.
const MAX_BIO_BYTES: usize = MAX_BIO_CHARS * 4;

/// What shows of a bio, checked: each line tidied as names are ([`names::tidy`]), blank
/// lines at either end left out and those in between kept as one, at most
/// [`MAX_BIO_LINES`] lines and [`MAX_BIO_CHARS`] characters. Line breaks are the only control
/// characters allowed; a bio with another one, or with a mark that hides in text or turns it
/// around, is refused. An empty bio is one: the person chose to show none.
pub fn clean_bio(text: &str) -> Result<String, FriendError> {
    if text
        .chars()
        .any(|c| (c.is_control() && c != '\n') || names::hiding_mark(c))
    {
        return Err(FriendError::BadBio);
    }
    let mut lines: Vec<String> = Vec::new();
    let mut gap = false;
    for line in text.split('\n') {
        let line = names::tidy(line);
        if line.is_empty() {
            gap = !lines.is_empty();
            continue;
        }
        if gap {
            lines.push(String::new());
            gap = false;
        }
        lines.push(line);
    }
    let bio = lines.join("\n");
    if lines.len() > MAX_BIO_LINES || bio.chars().count() > MAX_BIO_CHARS {
        return Err(FriendError::BadBio);
    }
    Ok(bio)
}

/// A device's bio, signed by its key. A newer `version_ms` replaces an older bio of the same
/// device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bio {
    pub device: DevicePublicKey,
    /// What shows, as [`clean_bio`] leaves it; empty when the person shows none.
    pub text: String,
    /// When the bio was last changed, in milliseconds since the Unix epoch.
    pub version_ms: u64,
    pub signature: [u8; SIGNATURE_LEN],
}

impl Bio {
    pub fn sign(
        identity: &DeviceIdentity,
        text: &str,
        version_ms: u64,
    ) -> Result<Self, FriendError> {
        let mut bio = Self {
            device: identity.public_key(),
            text: clean_bio(text)?,
            version_ms,
            signature: [0; SIGNATURE_LEN],
        };
        bio.signature = identity.sign(&bio.signed_bytes());
        Ok(bio)
    }

    fn write_body(&self, w: &mut Writer) {
        w.key(&self.device).str(&self.text).u64(self.version_ms);
    }

    fn signed_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.raw(LABEL);
        self.write_body(&mut w);
        w.into_bytes()
    }

    /// Checks the signature, and that the text is a bio as [`clean_bio`] leaves it.
    pub fn verify(&self) -> Result<(), FriendError> {
        if self.text.len() > MAX_BIO_BYTES || clean_bio(&self.text)? != self.text {
            return Err(FriendError::BadBio);
        }
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

    /// Decodes and verifies a bio.
    pub fn decode(bytes: &[u8]) -> Result<Self, FriendError> {
        let mut r = Reader::new(bytes);
        if r.u8()? != ENCODING_VERSION {
            return Err(FriendError::Malformed);
        }
        let bio = Self {
            device: r.key()?,
            text: r.str()?,
            version_ms: r.u64()?,
            signature: r.array()?,
        };
        r.finish()?;
        bio.verify()?;
        Ok(bio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bios_are_tidied_line_by_line() {
        assert_eq!(clean_bio(""), Ok(String::new()));
        assert_eq!(clean_bio("  \n \u{3164} \n"), Ok(String::new()));
        assert_eq!(
            clean_bio("  Hey there!  I'm on Fogata. "),
            Ok("Hey there! I'm on Fogata.".to_string())
        );
        // Blank lines at either end go, and those in between become one.
        assert_eq!(
            clean_bio("\n\nAna\n\n\n\nBea\n\n"),
            Ok("Ana\n\nBea".to_string())
        );
        assert_eq!(
            clean_bio("Ana\u{2800}\nBea\u{3000}\u{3164}"),
            Ok("Ana\nBea".to_string())
        );
        // Line and paragraph separators are blank space, not breaks.
        assert_eq!(
            clean_bio("Ana\u{2028}Bea\u{2029}Leo"),
            Ok("Ana Bea Leo".to_string())
        );
        for text in ["김민지\n👩🏽\u{200D}💻", "مريم", "क्\u{200D}षमा"] {
            assert_eq!(clean_bio(text), Ok(text.to_string()));
        }
    }

    #[test]
    fn bios_that_hide_marks_or_break_otherwise_are_refused() {
        for text in [
            "Ana\u{202E}exe.png",
            "A\u{200B}na",
            "\u{2067}Ana",
            "Ana\r\nBea",
            "Ana\tBea",
            "Ana\u{0}",
            "Ana\u{1B}[31m",
            "Ana\u{85}Bea",
            "Ana\u{FEFF}",
        ] {
            assert_eq!(clean_bio(text), Err(FriendError::BadBio), "{text:?}");
        }
    }

    #[test]
    fn bios_have_at_most_four_lines_and_190_characters() {
        assert!(clean_bio("1\n2\n3\n4").is_ok());
        assert_eq!(clean_bio("1\n2\n3\n4\n5"), Err(FriendError::BadBio));
        // A blank line in between counts as one.
        assert_eq!(clean_bio("1\n\n2\n3\n4"), Err(FriendError::BadBio));
        assert!(clean_bio("1\n\n\n\n2\n3").is_ok());
        assert!(clean_bio(&"ñ".repeat(MAX_BIO_CHARS)).is_ok());
        assert_eq!(
            clean_bio(&"ñ".repeat(MAX_BIO_CHARS + 1)),
            Err(FriendError::BadBio)
        );
        // Padding that does not show does not count.
        let padded = format!("{}{}", "ñ".repeat(MAX_BIO_CHARS), "\u{3164}".repeat(20));
        assert_eq!(clean_bio(&padded), Ok("ñ".repeat(MAX_BIO_CHARS)));
        // Line breaks do.
        let lines = format!("{}\n{}", "a".repeat(95), "b".repeat(95));
        assert_eq!(clean_bio(&lines), Err(FriendError::BadBio));
    }

    #[test]
    fn bios_round_trip_and_verify() {
        let identity = DeviceIdentity::generate().unwrap();
        let bio = Bio::sign(&identity, " Hey there!\n\n\nI'm on Fogata. ", 7).unwrap();
        assert_eq!(bio.text, "Hey there!\n\nI'm on Fogata.");
        assert_eq!(Bio::decode(&bio.encode()).unwrap(), bio);
        let empty = Bio::sign(&identity, "", 8).unwrap();
        assert_eq!(Bio::decode(&empty.encode()).unwrap(), empty);
    }

    #[test]
    fn tampering_and_untidy_bios_are_refused() {
        let identity = DeviceIdentity::generate().unwrap();
        let mut bio = Bio::sign(&identity, "Ana", 7).unwrap();
        bio.text = "Eva".into();
        assert_eq!(Bio::decode(&bio.encode()), Err(FriendError::NotProved));
        // Signed by its device, but not as a bio is cleaned: refused all the same.
        for text in ["Ana\u{202E}", "Ana\u{3164}", "\nAna", "1\n2\n3\n4\n5"] {
            bio.text = text.into();
            bio.signature = identity.sign(&bio.signed_bytes());
            assert_eq!(Bio::decode(&bio.encode()), Err(FriendError::BadBio));
        }
        bio.text = "a".repeat(MAX_BIO_BYTES + 1);
        bio.signature = identity.sign(&bio.signed_bytes());
        assert_eq!(Bio::decode(&bio.encode()), Err(FriendError::BadBio));
        // Nothing may follow the signature.
        let mut long = Bio::sign(&identity, "Ana", 8).unwrap().encode();
        long.push(0);
        assert_eq!(Bio::decode(&long), Err(FriendError::Malformed));
    }
}
