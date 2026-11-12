# spake2 in Fogata

This is spake2 0.4.0 from crates.io (MIT or Apache-2.0, see LICENSE-MIT and LICENSE-APACHE),
the password-authenticated key exchange that friend codes and move codes run
(`crates/fogata-friends`), put in place of the published crate by `[patch.crates-io]` in the
workspace's Cargo.toml, without its benchmark and its `tests/` folder, its README.md a short
note pointing here. Every change from the published crate is listed here.

## Secrets wiped from memory

The published crate kept the run's password (`Password`, a `Vec<u8>`) and its random scalar
and the password's scalar (in `Spake2`) in types that are not wiped when dropped. Now, with
the `zeroize` crate (already in the workspace) and curve25519-dalek's `zeroize` feature:

- `src/lib.rs`: `Password` and `Spake2` wipe their secrets when dropped (`ZeroizeOnDrop`;
  `Spake2` is `Zeroize` too). `finish` borrows the identities instead of moving them out,
  as a type that is wiped when dropped cannot be taken apart, and wipes the points and bytes
  of the shared element it works out once the key is hashed from them.
- `src/group.rs`: a group's `Scalar` and `Element` are `Zeroize`.
- `src/ed25519.rs`: the bytes the password's scalar is made from, and the transcripts the
  key is hashed from, are wiped once used.

Copies the compiler makes on the stack or in registers are not reached; nor is what the
callers hand in (Fogata passes the password from a `Zeroizing` buffer and wipes the key the
run returns).

## On the crates the rest of Fogata uses

The published crate is built on curve25519-dalek 4, sha2 0.10, hkdf 0.12 and rand_core 0.6,
the generation before the one the rest of Fogata uses; it alone kept that generation in the
programs Fogata ships (snow, libp2p's other user of it, takes it only for WebAssembly). It is
moved to curve25519-dalek 5, sha2 0.11, hkdf 0.13, rand_core 0.10 and getrandom 0.4, computing
the same: `src/vectors.rs` (below), recorded before the move, passes unchanged after it.

- `Cargo.toml`: the new versions; `getrandom` (the `getrandom` feature, on by default) is now
  getrandom 0.4 with `sys_rng`, as rand_core 0.10 has no `OsRng`; `rust-version` is the one
  those crates need.
- `src/lib.rs`: the `*_with_rng` constructors take a rand_core 0.10 `CryptoRng`; `start_a`,
  `start_b` and `start_symmetric` draw from `UnwrapErr(SysRng)`, the same system generator,
  panicking when it fails as `OsRng` did.
- `src/group.rs`: `random_scalar` takes a `CryptoRng`.
- `src/ed25519.rs`: `random_scalar` draws 64 bytes and reduces them modulo the group's order
  itself, which is all `Scalar::random` does, so curve25519-dalek's `rand_core` feature stays
  off; the bytes are wiped once used.

## Tests

The crate is not a workspace member: `tools/check.sh` runs its tests with
`cargo test -p spake2 --lib`. A crate outside the workspace cannot be tested with
dev-dependencies or features, so its tests module runs under `cfg(test)` alone (it needed the
`std` feature) and two small helpers stand in for the dev-dependencies `hex` and
`num-bigint`. The known-answer tests of the published crate pass unchanged, so the exchange is
the same; `the_secrets_of_a_run_are_wiped` checks the wiping.

`src/vectors.rs` (added) runs whole exchanges with a fixed stream of random bytes and checks
each message sent and each key against `src/vectors.txt` (added), recorded with the published
crate on curve25519-dalek 4: Fogata's own identities and passwords of the length Fogata uses,
a run whose two codes differ, the symmetric run, and second messages no honest run sends
(wrong length or side, bytes that are no point, the identity and small-order points, encodings
that are not canonical). Friend codes and move codes run between devices that may hold
different versions of this crate, so no line of it may change.
