# libp2p-kad in Fogata

A copy of libp2p-kad 0.49.0 (MIT, see LICENSE: rust-libp2p's, which the published crate does
not carry) from crates.io, put in place of the published crate by `[patch.crates-io]` in the
workspace's Cargo.toml and fuzz/Cargo.toml. Its integration test (`tests/client_mode.rs`), its
dev-dependencies and its changelog are left out. Its unit tests stay in `src/` as published,
but they cannot be built from the published crate (they need `quickcheck` and
`libp2p-swarm-test` from the rust-libp2p tree, which the published crate does not name): they
are run in rust-libp2p at the commit the crate was published from
(`7171dce2f90c05ba7892d4ba926abb1881db27c7`, `protocols/kad`), with Fogata's changes applied.

No changes yet.
