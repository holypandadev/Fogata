//! Friend invite links: what a device shows as text or as a QR code so that someone can send
//! it a friend request. A link is valid for ten minutes and is accepted once.

use std::fmt;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use fogata_crypto::{
    DevicePublicKey,
    codec::{Reader, Writer},
    random_bytes,
};
use zeroize::Zeroizing;

use crate::{FriendError, clean_name, secret::derive};

pub const LINK_PREFIX: &str = "fogata:friend:";
/// How long a link stays valid.
/// How far the clock of a device that receives a link may be ahead of its maker's.
const CLOCK_SLACK_MS: u64 = 24 * 3600 * 1000;

pub const LINK_LIFETIME_MS: u64 = 10 * 60 * 1000;
/// Most addresses a link carries, to keep its QR code readable.
pub const MAX_LINK_ADDRS: usize = 4;
const LINK_VERSION: u8 = 1;
const MAX_PEER_LEN: usize = 64;
const SALT: &[u8] = b"fogata/v1/friend-link";

#[derive(Clone, PartialEq, Eq)]
pub struct FriendLink {
    /// The device that made the link.
    pub device: DevicePublicKey,
    pub name: String,
    /// Random token: whoever has the link can find the device and prove they saw the link.
    pub token: [u8; 16],
    pub expires_ms: u64,
    /// The device's transport identity when it made the link, in binary form.
    pub peer: Vec<u8>,
    /// Addresses where the device could be reached then, as binary multiaddrs without the
    /// transport identity.
    pub addrs: Vec<Vec<u8>>,
}

impl fmt::Debug for FriendLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FriendLink")
            .field("device", &self.device)
            .field("name", &self.name)
            .field("expires_ms", &self.expires_ms)
            .finish_non_exhaustive()
    }
}

impl FriendLink {
    pub fn new(
        device: DevicePublicKey,
        name: &str,
        peer: Vec<u8>,
        addrs: Vec<Vec<u8>>,
        now_ms: u64,
    ) -> Result<Self, FriendError> {
        if peer.len() > MAX_PEER_LEN {
            return Err(FriendError::Malformed);
        }
        let mut token = [0u8; 16];
        random_bytes(&mut token)?;
        Ok(Self {
            device,
            name: clean_name(name)?,
            token,
            expires_ms: now_ms + LINK_LIFETIME_MS,
            peer,
            addrs: addrs.into_iter().take(MAX_LINK_ADDRS).collect(),
        })
    }

    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms > self.expires_ms
    }

    /// Whether the link expired even for a clock a day ahead of its maker's. The maker drops
    /// the link once it expires, so a device that receives one only refuses those surely too
    /// old, and seeks the others for [`LINK_LIFETIME_MS`] by its own clock.
    pub fn is_surely_expired(&self, now_ms: u64) -> bool {
        now_ms > self.expires_ms.saturating_add(CLOCK_SLACK_MS)
    }

    /// The secret behind the rendezvous points the device announces itself under while the
    /// link is valid.
    pub fn rendezvous(&self) -> [u8; 32] {
        *derive(SALT, &self.token, b"rendezvous")
    }

    pub(crate) fn request_key(&self) -> Zeroizing<[u8; 32]> {
        derive(SALT, &self.token, b"request")
    }

    pub fn encode(&self) -> String {
        let mut w = Writer::default();
        w.u8(LINK_VERSION)
            .key(&self.device)
            .str(&self.name)
            .raw(&self.token)
            .u64(self.expires_ms)
            .bytes(&self.peer)
            .u32(self.addrs.len() as u32);
        for addr in &self.addrs {
            w.bytes(addr);
        }
        format!("{LINK_PREFIX}{}", URL_SAFE_NO_PAD.encode(w.into_bytes()))
    }

    pub fn decode(text: &str) -> Result<Self, FriendError> {
        let body = text
            .trim()
            .strip_prefix(LINK_PREFIX)
            .ok_or(FriendError::NotALink)?;
        let bytes = URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|_| FriendError::DamagedLink)?;
        let mut r = Reader::new(&bytes);
        match r.u8().map_err(|_| FriendError::DamagedLink)? {
            LINK_VERSION => {}
            _ => return Err(FriendError::NewerLink),
        }
        let damaged = |_| FriendError::DamagedLink;
        let device = r.key().map_err(damaged)?;
        let name = clean_name(&r.str().map_err(damaged)?).map_err(|_| FriendError::DamagedLink)?;
        let token = r.array().map_err(damaged)?;
        let expires_ms = r.u64().map_err(damaged)?;
        let peer = r.bytes().map_err(damaged)?.to_vec();
        if peer.len() > MAX_PEER_LEN {
            return Err(FriendError::DamagedLink);
        }
        let count = r.count(4).map_err(damaged)?;
        if count > MAX_LINK_ADDRS {
            return Err(FriendError::DamagedLink);
        }
        let addrs = (0..count)
            .map(|_| r.bytes().map(<[u8]>::to_vec))
            .collect::<Result<Vec<_>, _>>()
            .map_err(damaged)?;
        r.finish().map_err(damaged)?;
        Ok(Self {
            device,
            name,
            token,
            expires_ms,
            peer,
            addrs,
        })
    }
}

#[cfg(test)]
mod tests {
    use fogata_crypto::DeviceIdentity;

    use super::*;

    #[test]
    fn links_round_trip() {
        let device = DeviceIdentity::generate().unwrap().public_key();
        let link =
            FriendLink::new(device, "Ana", vec![9; 38], vec![vec![1, 2, 3]; 6], 1000).unwrap();
        assert_eq!(link.addrs.len(), MAX_LINK_ADDRS);
        assert_eq!(link.expires_ms, 1000 + LINK_LIFETIME_MS);
        let text = link.encode();
        assert!(text.starts_with(LINK_PREFIX));
        assert_eq!(FriendLink::decode(&format!("  {text}\n")).unwrap(), link);
        assert!(!link.is_expired(1000 + LINK_LIFETIME_MS));
        assert!(link.is_expired(1001 + LINK_LIFETIME_MS));
        // A clock some hours ahead still takes it.
        assert!(!link.is_surely_expired(1001 + LINK_LIFETIME_MS + 8 * 3600 * 1000));
        assert!(link.is_surely_expired(1001 + LINK_LIFETIME_MS + CLOCK_SLACK_MS));
    }

    #[test]
    fn bad_links_are_rejected() {
        assert_eq!(FriendLink::decode("hi"), Err(FriendError::NotALink));
        assert_eq!(
            FriendLink::decode("fogata:friend:%%"),
            Err(FriendError::DamagedLink)
        );
        let device = DeviceIdentity::generate().unwrap().public_key();
        let text = FriendLink::new(device, "Ana", vec![], vec![], 0)
            .unwrap()
            .encode();
        assert_eq!(
            FriendLink::decode(&text[..text.len() - 4]),
            Err(FriendError::DamagedLink)
        );
        let newer = format!("{LINK_PREFIX}{}", URL_SAFE_NO_PAD.encode([2u8, 0, 0]));
        assert_eq!(FriendLink::decode(&newer), Err(FriendError::NewerLink));
    }

    #[test]
    fn link_secrets_are_tied_to_the_token() {
        let device = DeviceIdentity::generate().unwrap().public_key();
        let a = FriendLink::new(device, "Ana", vec![], vec![], 0).unwrap();
        let b = FriendLink::new(device, "Ana", vec![], vec![], 0).unwrap();
        assert_ne!(a.rendezvous(), b.rendezvous());
        assert_ne!(a.rendezvous(), *a.request_key());
        assert!(!format!("{a:?}").contains("token"));
    }
}
