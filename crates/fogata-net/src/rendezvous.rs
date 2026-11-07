//! Where group members meet on the DHT.

use fogata_crypto::{rendezvous_point, window_at};
use libp2p::kad::RecordKey;

/// Default bootstrap peers of the public IPFS "Amino" DHT. Only needed when no DHT node is
/// known from an earlier session.
pub const AMINO_BOOTSTRAP: &[&str] = &[
    "/dnsaddr/bootstrap.libp2p.io/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    "/dnsaddr/bootstrap.libp2p.io/p2p/QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
    "/dnsaddr/bootstrap.libp2p.io/p2p/QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
    "/dnsaddr/bootstrap.libp2p.io/p2p/QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt",
    "/ip4/104.131.131.82/tcp/4001/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ",
];

/// Kademlia protocol of the Amino DHT.
pub const AMINO_PROTOCOL: &str = "/ipfs/kad/1.0.0";

/// Multihash code and length prefix for SHA-256. Amino nodes only accept provider records
/// whose key is a valid multihash.
const SHA2_256_MULTIHASH: [u8; 2] = [0x12, 0x20];

/// DHT key under which members announce themselves during `window`.
pub(crate) fn key_for_window(secret: &[u8; 32], window: u64) -> RecordKey {
    let mut bytes = Vec::with_capacity(34);
    bytes.extend_from_slice(&SHA2_256_MULTIHASH);
    bytes.extend_from_slice(&rendezvous_point(secret, window));
    RecordKey::new(&bytes)
}

/// Keys to announce at `unix_secs`: one per rendezvous secret, for the current window.
pub(crate) fn announce_keys(secrets: &[[u8; 32]], unix_secs: u64) -> Vec<RecordKey> {
    let window = window_at(unix_secs);
    secrets.iter().map(|s| key_for_window(s, window)).collect()
}

/// Keys to look up at `unix_secs`: for each secret, the current window and the previous one,
/// so members that announced just before a window began still meet, and so do two devices
/// whose clocks are hours apart (as after booting another system): the one ahead finds the
/// other.
pub(crate) fn lookup_keys(secrets: &[[u8; 32]], unix_secs: u64) -> Vec<RecordKey> {
    let window = window_at(unix_secs);
    secrets
        .iter()
        .flat_map(|s| {
            [
                key_for_window(s, window),
                key_for_window(s, window.saturating_sub(1)),
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use fogata_crypto::WINDOW_SECS;

    use super::*;

    #[test]
    fn keys_are_multihashes_that_rotate() {
        let secret = [3u8; 32];
        let a = key_for_window(&secret, 1);
        assert_eq!(a.as_ref().len(), 34);
        assert_eq!(&a.as_ref()[..2], &SHA2_256_MULTIHASH);
        assert_ne!(a, key_for_window(&secret, 2));
        assert_ne!(a, key_for_window(&[4u8; 32], 1));
        let keys = lookup_keys(&[secret], WINDOW_SECS * 5);
        assert_eq!(
            keys,
            vec![key_for_window(&secret, 5), key_for_window(&secret, 4)]
        );
        // A device five hours ahead looks up the window the other announces in.
        let behind = WINDOW_SECS * 5 + 3 * 3600;
        let ahead = behind + 5 * 3600;
        assert!(lookup_keys(&[secret], ahead).contains(&announce_keys(&[secret], behind)[0]));
    }
}
