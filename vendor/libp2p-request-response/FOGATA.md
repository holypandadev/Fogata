# libp2p-request-response in Fogata

A copy of libp2p-request-response 0.30.0 (MIT, see LICENSE: rust-libp2p's, which the
published crate does not carry) from crates.io, put in place of the published crate by
`[patch.crates-io]` in the workspace's Cargo.toml and fuzz/Cargo.toml. Its integration tests
(`tests/`), its dev-dependencies and its changelog are left out. The unit tests of its codecs
stay in `src/cbor.rs` and `src/json.rs` as published, but are not run: they need the
dev-dependencies left out, and Fogata uses neither codec but in its tests.
