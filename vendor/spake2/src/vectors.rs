//! Fogata: known answers of whole runs, recorded with the published crate (spake2 0.4.0 on
//! curve25519-dalek 4, sha2 0.10 and hkdf 0.12) in vectors.txt. Friend codes and move codes
//! run between devices that may hold different versions of this crate, so what a run sends
//! and the key it ends with must never change: these runs use the passwords and identities
//! Fogata's runs use, a fixed stream in place of the system's generator, and second messages
//! a peer could send that are not what an honest run sends.

use crate::*;
use alloc::string::String;
use core::fmt::Write;

/// SplitMix64: a fixed stream of bytes for a seed, so that each side's random scalar (drawn as
/// 64 bytes) is known.
struct Stream(u64);

impl Stream {
    fn word(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn fill(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let word = self.word().to_le_bytes();
            chunk.copy_from_slice(&word[..chunk.len()]);
        }
    }
}

impl RngCore for Stream {
    fn next_u32(&mut self) -> u32 {
        self.word() as u32
    }

    fn next_u64(&mut self) -> u64 {
        self.word()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.fill(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> core::result::Result<(), rand_core::Error> {
        self.fill(dest);
        Ok(())
    }
}

impl CryptoRng for Stream {}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn outcome(result: Result<Vec<u8>>) -> String {
    match result {
        Ok(key) => hex(&key),
        Err(e) => format!("{e:?}"),
    }
}

/// Every line vectors.txt records, worked out by this build.
fn computed() -> String {
    let passwords: [(&str, Vec<u8>); 4] = [
        ("empty", Vec::new()),
        ("password", b"password".to_vec()),
        // Fogata's passwords are 32 bytes derived from the code.
        ("32-bytes", (0u8..32).collect()),
        (
            "200-bytes",
            (0u32..200).map(|i| (i * 7 + 3) as u8).collect(),
        ),
    ];
    let pairs: [(&str, &[u8], &[u8]); 3] = [
        ("friend-code", b"fogata/v1/requester", b"fogata/v1/inviter"),
        (
            "transfer",
            b"fogata/v1/transfer-sender",
            b"fogata/v1/transfer-receiver",
        ),
        ("python", b"idA", b"idB"),
    ];
    let mut out = String::new();
    for (name, password) in &passwords {
        let scalar = Ed25519Group::hash_to_scalar(password);
        writeln!(out, "password-scalar {name} {}", hex(scalar.as_bytes())).unwrap();
    }
    let mut seed = 1;
    for (pair, id_a, id_b) in pairs {
        for (name, password) in &passwords {
            let password = Password::new(password);
            let (id_a, id_b) = (Identity::new(id_a), Identity::new(id_b));
            let (a, msg_a) =
                Spake2::<Ed25519Group>::start_a_with_rng(&password, &id_a, &id_b, Stream(seed));
            let (b, msg_b) =
                Spake2::<Ed25519Group>::start_b_with_rng(&password, &id_a, &id_b, Stream(seed + 1));
            seed += 2;
            let key_a = a.finish(&msg_b).unwrap();
            let key_b = b.finish(&msg_a).unwrap();
            assert_eq!(key_a, key_b);
            writeln!(out, "run {pair} {name} a {}", hex(&msg_a)).unwrap();
            writeln!(out, "run {pair} {name} b {}", hex(&msg_b)).unwrap();
            writeln!(out, "run {pair} {name} key {}", hex(&key_a)).unwrap();
        }
    }
    // Two codes that differ: each side ends with a key of its own.
    let (id_a, id_b) = (
        Identity::new(b"fogata/v1/requester"),
        Identity::new(b"fogata/v1/inviter"),
    );
    let (a, msg_a) = Spake2::<Ed25519Group>::start_a_with_rng(
        &Password::new([1u8; 32]),
        &id_a,
        &id_b,
        Stream(100),
    );
    let (b, msg_b) = Spake2::<Ed25519Group>::start_b_with_rng(
        &Password::new([2u8; 32]),
        &id_a,
        &id_b,
        Stream(101),
    );
    writeln!(out, "mismatch a {}", hex(&msg_a)).unwrap();
    writeln!(out, "mismatch b {}", hex(&msg_b)).unwrap();
    writeln!(out, "mismatch key-a {}", outcome(a.finish(&msg_b))).unwrap();
    writeln!(out, "mismatch key-b {}", outcome(b.finish(&msg_a))).unwrap();
    // The symmetric run (not used by Fogata, but part of the crate).
    let id_s = Identity::new(b"idS");
    let password = Password::new(b"password");
    let (s1, msg_1) =
        Spake2::<Ed25519Group>::start_symmetric_with_rng(&password, &id_s, Stream(200));
    let (s2, msg_2) =
        Spake2::<Ed25519Group>::start_symmetric_with_rng(&password, &id_s, Stream(201));
    let (key_1, key_2) = (s1.finish(&msg_2).unwrap(), s2.finish(&msg_1).unwrap());
    assert_eq!(key_1, key_2);
    writeln!(out, "symmetric 1 {}", hex(&msg_1)).unwrap();
    writeln!(out, "symmetric 2 {}", hex(&msg_2)).unwrap();
    writeln!(out, "symmetric key {}", hex(&key_1)).unwrap();
    // Second messages a peer could send that an honest run does not: what each one ends in
    // (a key, or the error) must stay the same, as it decides whether a run goes on.
    let hostile: [(&str, &str); 13] = [
        ("short", "42"),
        (
            "long",
            "42 5866666666666666666666666666666666666666666666666666666666666666 00",
        ),
        (
            "side-a",
            "41 5866666666666666666666666666666666666666666666666666666666666666",
        ),
        (
            "side-s",
            "53 5866666666666666666666666666666666666666666666666666666666666666",
        ),
        (
            "basepoint",
            "42 5866666666666666666666666666666666666666666666666666666666666666",
        ),
        (
            "not-on-curve",
            "42 0200000000000000000000000000000000000000000000000000000000000000",
        ),
        (
            "identity",
            "42 0100000000000000000000000000000000000000000000000000000000000000",
        ),
        (
            "identity-negative-x",
            "42 0100000000000000000000000000000000000000000000000000000000000080",
        ),
        (
            "identity-y-plus-p",
            "42 eeffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
        ),
        (
            "y-zero",
            "42 0000000000000000000000000000000000000000000000000000000000000000",
        ),
        (
            "y-is-p",
            "42 edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
        ),
        (
            "all-ones",
            "42 ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        ),
        (
            "order-8",
            "42 c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac037a",
        ),
    ];
    for (name, message) in hostile {
        let message = unhex(&message.replace(' ', ""));
        let (a, _) = Spake2::<Ed25519Group>::start_a_with_rng(
            &Password::new([7u8; 32]),
            &Identity::new(b"fogata/v1/requester"),
            &Identity::new(b"fogata/v1/inviter"),
            Stream(300),
        );
        writeln!(out, "hostile {name} {}", outcome(a.finish(&message))).unwrap();
    }
    out
}

#[test]
fn runs_end_in_the_recorded_messages_and_keys() {
    let recorded: String = include_str!("vectors.txt")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| format!("{line}\n"))
        .collect();
    let got = computed();
    if got != recorded {
        println!("{got}");
    }
    assert!(got == recorded, "a run differs from vectors.txt");
}
