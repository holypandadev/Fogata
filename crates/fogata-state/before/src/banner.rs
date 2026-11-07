//! A group's banner: a picture, still or moving, at the top of its column. A manager signs it
//! apart from the log: versions from before banners cannot read a kind of entry they do not
//! know, and every entry signed on one would wait for ever there. Only the newest banner is
//! kept, so changing it often does not grow anything; it goes to the members that say they
//! take banners (see `fogata_proto::MemberRequest::Roster`).

use crate::record::{GroupRecord, RecordKind};

/// Largest banner picture, in bytes (512 KiB in earlier versions, which refuse a larger
/// one and keep the banner they have).
pub const MAX_BANNER_BYTES: usize = 550 * 1024;

/// Whether `bytes` start like a picture a banner may be: a PNG, a JPEG, a GIF or a WebP.
pub fn is_banner_picture(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"\xff\xd8\xff")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || (bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP")
}

/// The kind of [`GroupBanner`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BannerKind;

impl RecordKind for BannerKind {
    const LABEL: &'static [u8] = b"fogata/v1/group-banner";
    const MAX_BYTES: usize = MAX_BANNER_BYTES;

    fn takes(bytes: &[u8]) -> bool {
        is_banner_picture(bytes)
    }
}

/// A group's banner, signed by the manager who chose it.
pub type GroupBanner = GroupRecord<BannerKind>;

#[cfg(test)]
mod tests {
    use super::*;
    use fogata_crypto::{DeviceIdentity, GroupId};

    #[test]
    fn banners_round_trip_and_verify() {
        let identity = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        let mut gif = b"GIF89a".to_vec();
        gif.resize(300, 1);
        let banner = GroupBanner::sign(&identity, group, gif, 5).unwrap();
        assert_eq!(GroupBanner::decode(&banner.encode()).unwrap(), banner);
        let none = GroupBanner::sign(&identity, group, Vec::new(), 6).unwrap();
        assert_eq!(GroupBanner::decode(&none.encode()).unwrap(), none);
        assert!(none.replaces(&banner));
        assert!(!banner.replaces(&none));
        assert_ne!(none.digest(), banner.digest());

        let mut changed = banner.clone();
        changed.time_ms = 9;
        assert!(GroupBanner::decode(&changed.encode()).is_err());
    }

    #[test]
    fn only_small_pictures_are_banners() {
        let identity = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        assert!(GroupBanner::sign(&identity, group, b"%PDF-1.7".to_vec(), 1).is_err());
        let mut big = b"\xff\xd8\xff".to_vec();
        big.resize(MAX_BANNER_BYTES + 1, 0);
        assert!(GroupBanner::sign(&identity, group, big.clone(), 1).is_err());
        // 550 KiB goes and is taken.
        big.truncate(550 * 1024);
        let banner = GroupBanner::sign(&identity, group, big, 1).unwrap();
        assert_eq!(GroupBanner::decode(&banner.encode()).unwrap(), banner);
        for kind in [
            b"\x89PNG\r\n\x1a\n".as_slice(),
            b"\xff\xd8\xff",
            b"GIF87a",
            b"RIFF\0\0\0\0WEBP",
        ] {
            assert!(GroupBanner::sign(&identity, group, kind.to_vec(), 1).is_ok());
        }
    }
}
