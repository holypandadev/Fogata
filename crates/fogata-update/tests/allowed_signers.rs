//! The keys testers check a pre-release's first installs with are the keys every build
//! trusts. The README of each pre-release tells testers to take `tools/allowed_signers` from
//! the source code and verify `SHA256SUMS.sig` with its `fogata-prerelease` lines; copies take
//! releases signed by `RELEASE_KEYS` only. A key in one and not the other makes that check
//! fail for a genuine download, or pass for a list signed by a key no copy trusts.

use std::collections::BTreeSet;

use fogata_update::{RELEASE_KEYS, sums};

#[test]
fn allowed_signers_names_exactly_the_compiled_in_keys() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/allowed_signers");
    let file = std::fs::read_to_string(path).expect("tools/allowed_signers");
    let listed: BTreeSet<String> = file
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("fogata-prerelease "))
        .map(|line| {
            line.split_whitespace()
                .take(4)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    let compiled: BTreeSet<String> = RELEASE_KEYS
        .iter()
        .map(|key| sums::allowed_signers_line(&key.key))
        .collect();
    assert_eq!(listed, compiled);
}
