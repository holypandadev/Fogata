//! A group's picture, shown instead of its icon. A manager signs it apart from the log, like
//! the banner: only the newest is kept, so a picture changed or taken away is gone from every
//! device that gets the change, and a member who joins later never gets the ones before.
//!
//! Earlier versions set it with a `SetPicture` log entry, which stays in the log for good.
//! Those entries still count: the picture shown is the newer of the last one that applied
//! and the record ([`crate::GroupLog::picture`]). Versions from before records never get one
//! and keep showing the last picture set with an entry.

use crate::{
    record::{GroupRecord, RecordKind},
    state::MAX_PICTURE_BYTES,
};

/// Whether `bytes` start like a picture a group's picture may be: a PNG or a JPEG.
pub fn is_group_picture(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.starts_with(b"\xff\xd8\xff")
}

/// The kind of [`GroupPicture`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PictureKind;

impl RecordKind for PictureKind {
    const LABEL: &'static [u8] = b"fogata/v1/group-picture";
    const MAX_BYTES: usize = MAX_PICTURE_BYTES;

    fn takes(bytes: &[u8]) -> bool {
        is_group_picture(bytes)
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
    fn only_small_pngs_and_jpegs_are_pictures() {
        let identity = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        for not in [b"GIF89a".as_slice(), b"RIFF\0\0\0\0WEBP", b"%PDF-1.7"] {
            assert!(GroupPicture::sign(&identity, group, not.to_vec(), 1).is_err());
        }
        assert!(GroupPicture::sign(&identity, group, png(MAX_PICTURE_BYTES + 1), 1).is_err());
        let full = GroupPicture::sign(&identity, group, png(MAX_PICTURE_BYTES), 1).unwrap();
        assert_eq!(GroupPicture::decode(&full.encode()).unwrap(), full);
        let mut jpeg = b"\xff\xd8\xff".to_vec();
        jpeg.resize(1000, 0);
        assert!(GroupPicture::sign(&identity, group, jpeg, 1).is_ok());
    }
}
