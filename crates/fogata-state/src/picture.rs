//! A group's picture, shown instead of its icon. A manager signs it apart from the log, like
//! the banner: only the newest is kept, so a picture changed or taken away is gone from every
//! device that gets the change, and a member who joins later never gets the ones before.
//!
//! Earlier versions set it with a `SetPicture` log entry, which stays in the log for good.
//! Those entries still count: the picture shown is the newer of the last one that applied
//! and the record ([`crate::GroupLog::picture`]). Versions from before records never get one
//! and keep showing the last picture set with an entry.

use crate::{
    banner::is_banner_picture,
    record::{GroupRecord, RecordKind},
};

/// Largest picture in a record, in bytes: a GIF or WebP that moves, as large as a banner.
/// Versions that took at most 48 KiB, a PNG or a JPEG, refuse a larger one, or one that
/// moves, and keep the picture they have.
pub const MAX_RECORD_PICTURE_BYTES: usize = 550 * 1024;

/// Whether `bytes` start like a picture a `SetPicture` entry may hold: a PNG or a JPEG.
pub fn is_group_picture(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.starts_with(b"\xff\xd8\xff")
}

/// Whether `bytes` start like a picture a record may hold: a PNG, a JPEG, a GIF or a WebP.
pub fn is_record_picture(bytes: &[u8]) -> bool {
    is_banner_picture(bytes)
}

/// The kind of [`GroupPicture`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PictureKind;

impl RecordKind for PictureKind {
    const LABEL: &'static [u8] = b"fogata/v1/group-picture";
    const MAX_BYTES: usize = MAX_RECORD_PICTURE_BYTES;

    fn takes(bytes: &[u8]) -> bool {
        is_record_picture(bytes)
    }
}

/// A group's picture, signed by the manager who chose it.
pub type GroupPicture = GroupRecord<PictureKind>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GroupBanner;
    use fogata_crypto::{DeviceIdentity, GroupId};

    fn png(len: usize) -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.resize(len, 3);
        png
    }

    #[test]
    fn pictures_round_trip_and_verify() {
        let identity = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        let picture = GroupPicture::sign(&identity, group, png(300), 5).unwrap();
        assert_eq!(GroupPicture::decode(&picture.encode()).unwrap(), picture);
        let none = GroupPicture::sign(&identity, group, Vec::new(), 6).unwrap();
        assert_eq!(GroupPicture::decode(&none.encode()).unwrap(), none);
        assert!(none.replaces(&picture));
        assert_ne!(none.digest(), picture.digest());

        let mut changed = picture.clone();
        changed.time_ms = 9;
        assert!(GroupPicture::decode(&changed.encode()).is_err());
    }

    #[test]
    fn a_picture_is_never_read_as_a_banner_or_the_other_way() {
        let identity = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        let picture = GroupPicture::sign(&identity, group, png(300), 5).unwrap();
        assert!(GroupBanner::decode(&picture.encode()).is_err());
        let banner = GroupBanner::sign(&identity, group, png(300), 5).unwrap();
        assert!(GroupPicture::decode(&banner.encode()).is_err());
    }

    #[test]
    fn pictures_are_pngs_jpegs_gifs_or_webps_of_at_most_550_kib() {
        let identity = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        for not in [b"BM6\0\0".as_slice(), b"%PDF-1.7"] {
            assert!(GroupPicture::sign(&identity, group, not.to_vec(), 1).is_err());
        }
        let most = MAX_RECORD_PICTURE_BYTES;
        assert!(GroupPicture::sign(&identity, group, png(most + 1), 1).is_err());
        let full = GroupPicture::sign(&identity, group, png(most), 1).unwrap();
        assert_eq!(GroupPicture::decode(&full.encode()).unwrap(), full);
        for start in [
            b"\xff\xd8\xff".as_slice(),
            b"GIF89a",
            b"GIF87a",
            b"RIFF\0\0\0\0WEBP",
        ] {
            let mut picture = start.to_vec();
            picture.resize(1000, 0);
            assert!(GroupPicture::sign(&identity, group, picture, 1).is_ok());
        }
    }
}
