use std::fmt;

use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::{CryptoError, random_bytes};

/// Length of a group secret in bytes.
pub const GROUP_SECRET_LEN: usize = 32;

/// Length of a rendezvous window in seconds. The DHT key that members meet under changes
/// once per window.
pub const WINDOW_SECS: u64 = 6 * 60 * 60;

const SALT: &[u8] = b"fogata/v2/group";

/// Returns the rendezvous window that contains `unix_secs`.
pub fn window_at(unix_secs: u64) -> u64 {
    unix_secs / WINDOW_SECS
}

/// The secret of one key epoch of a group, shared by its members at that time. A new random
/// secret starts every epoch (when a member is removed or leaves).
pub struct GroupSecret(Zeroizing<[u8; GROUP_SECRET_LEN]>);

impl GroupSecret {
    pub fn generate() -> Result<Self, CryptoError> {
        let mut bytes = Zeroizing::new([0u8; GROUP_SECRET_LEN]);
        random_bytes(bytes.as_mut())?;
        Ok(Self(bytes))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let bytes: [u8; GROUP_SECRET_LEN] = bytes.try_into().map_err(|_| CryptoError::Malformed)?;
        Ok(Self(Zeroizing::new(bytes)))
    }

    pub fn as_bytes(&self) -> &[u8; GROUP_SECRET_LEN] {
        &self.0
    }
}

impl fmt::Debug for GroupSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GroupSecret(..)")
    }
}

/// Identifier of a group, random and fixed for the group's whole life. Shared with members,
/// never published on the DHT.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupId([u8; 16]);

impl GroupId {
    pub fn random() -> Result<Self, CryptoError> {
        let mut bytes = [0u8; 16];
        random_bytes(&mut bytes)?;
        Ok(Self(bytes))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        Ok(Self(bytes.try_into().map_err(|_| CryptoError::Malformed)?))
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Debug for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GroupId(")?;
        for b in &self.0[..6] {
            write!(f, "{b:02x}")?;
        }
        write!(f, ")")
    }
}

/// Keys of one epoch, derived from its [`GroupSecret`] and the group id with HKDF-SHA256.
/// Each purpose has its own key so that, for example, knowing a rendezvous key reveals
/// nothing about message keys.
pub struct GroupKeys {
    group_id: GroupId,
    epoch: u32,
    auth_key: Zeroizing<[u8; 32]>,
    message_key: Zeroizing<[u8; 32]>,
    rendezvous_key: Zeroizing<[u8; 32]>,
}

impl GroupKeys {
    pub fn derive(group_id: GroupId, epoch: u32, secret: &GroupSecret) -> Self {
        let mut salt = SALT.to_vec();
        salt.extend_from_slice(group_id.as_bytes());
        salt.extend_from_slice(&epoch.to_be_bytes());
        let hkdf = Hkdf::<Sha256>::new(Some(&salt), secret.as_bytes());
        let expand = |label: &[u8], out: &mut [u8]| {
            hkdf.expand(label, out)
                .expect("HKDF output length is far below the limit");
        };
        let mut auth_key = Zeroizing::new([0u8; 32]);
        expand(b"auth", auth_key.as_mut());
        let mut message_key = Zeroizing::new([0u8; 32]);
        expand(b"message", message_key.as_mut());
        let mut rendezvous_key = Zeroizing::new([0u8; 32]);
        expand(b"rendezvous", rendezvous_key.as_mut());
        Self {
            group_id,
            epoch,
            auth_key,
            message_key,
            rendezvous_key,
        }
    }

    pub fn group_id(&self) -> GroupId {
        self.group_id
    }

    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    pub(crate) fn auth_key(&self) -> &[u8; 32] {
        &self.auth_key
    }

    pub(crate) fn message_key(&self) -> &[u8; 32] {
        &self.message_key
    }

    /// The secret from which this epoch's rendezvous points derive, where members find each
    /// other; it reveals nothing about the other keys. Invites do not carry it (see the
    /// invite's own meeting token).
    pub fn rendezvous_secret(&self) -> &[u8; 32] {
        &self.rendezvous_key
    }

    /// The opaque 32-byte value that members announce on the DHT during `window`.
    pub fn rendezvous_point(&self, window: u64) -> [u8; 32] {
        rendezvous_point(&self.rendezvous_key, window)
    }
}

/// A 32-byte secret derived from `ikm` for one purpose, named by `salt` and `info`. For
/// example, the rendezvous secret of an invite from its token.
pub fn derive_secret(salt: &[u8], ikm: &[u8], info: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    Hkdf::<Sha256>::new(Some(salt), ikm)
        .expand(info, &mut out)
        .expect("HKDF output length is far below the limit");
    out
}

/// The rendezvous point of `window` for a rendezvous secret. Without the secret it cannot be
/// linked to the group or to other windows.
pub fn rendezvous_point(rendezvous_secret: &[u8; 32], window: u64) -> [u8; 32] {
    let hkdf = Hkdf::<Sha256>::new(Some(b"fogata/v1/rendezvous"), rendezvous_secret);
    let mut out = [0u8; 32];
    hkdf.expand(&window.to_be_bytes(), &mut out)
        .expect("HKDF output length is far below the limit");
    out
}

impl fmt::Debug for GroupKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GroupKeys")
            .field("group_id", &self.group_id)
            .field("epoch", &self.epoch)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(byte: u8) -> GroupSecret {
        GroupSecret::from_bytes(&[byte; 32]).unwrap()
    }

    #[test]
    fn derivation_is_deterministic_and_separated() {
        let id = GroupId::from_bytes(&[9u8; 16]).unwrap();
        let a = GroupKeys::derive(id, 0, &secret(7));
        let b = GroupKeys::derive(id, 0, &secret(7));
        assert_eq!(a.rendezvous_point(10), b.rendezvous_point(10));
        assert_ne!(a.auth_key(), a.message_key());
        assert_ne!(a.rendezvous_point(10), a.rendezvous_point(11));
    }

    #[test]
    fn epochs_and_groups_have_different_keys() {
        let id = GroupId::from_bytes(&[9u8; 16]).unwrap();
        let other = GroupId::from_bytes(&[8u8; 16]).unwrap();
        let base = GroupKeys::derive(id, 0, &secret(1));
        assert_ne!(
            base.message_key(),
            GroupKeys::derive(id, 1, &secret(1)).message_key()
        );
        assert_ne!(
            base.message_key(),
            GroupKeys::derive(other, 0, &secret(1)).message_key()
        );
        assert_ne!(
            base.message_key(),
            GroupKeys::derive(id, 0, &secret(2)).message_key()
        );
    }

    #[test]
    fn windows_are_six_hours_long() {
        assert_eq!(window_at(0), 0);
        assert_eq!(window_at(WINDOW_SECS - 1), 0);
        assert_eq!(window_at(WINDOW_SECS), 1);
    }
}
