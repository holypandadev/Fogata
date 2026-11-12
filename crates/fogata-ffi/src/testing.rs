//! What the crate's own tests (`tests/`) set and read that the screens have otherwise: the
//! network a core starts on, the keychain, and the events the screens would be given. Not
//! part of the interface: flutter_rust_bridge reads `api` only, and the application never
//! calls these.

use fogata_core::{NetOptions, vault::Keychain};

use crate::api::types::UiEvent;

/// The network the next cores started with [`crate::api::app::start`] use in place of the
/// computer's: `options` as given (the port, the peers to dial, the DHT and mDNS), with the
/// settings of the data folder (bridge, appearing offline) as the application reads them;
/// the releases page is never asked.
pub fn use_network(options: NetOptions) {
    *crate::api::app::TEST_NETWORK
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(options);
}

/// The keychain the data folders of this process use from now on, in place of the
/// system's that the application chooses on its first call.
pub fn use_keychain(keychain: Keychain) {
    crate::api::app::use_keychain();
    fogata_core::vault::use_keychain(keychain);
}

/// The events given to the screens since the last call, oldest first, taken as the screens
/// take them: as many more may come.
pub fn take_events() -> Vec<UiEvent> {
    crate::api::app::take_kept_events()
}
