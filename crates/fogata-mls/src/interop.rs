//! This crate's provider against OpenMLS's own (`openmls_rust_crypto`, on `hpke-rs`): what a
//! device on that provider sent, recorded, must read here exactly as it reads there. The
//! recording holds the stored secrets of a KeyPackage made here, the Welcome a device on
//! the other provider made for it, three Commits by that device (each with new path
//! secrets sealed to this one), and the secret that device exported in each epoch.

use fogata_crypto::codec::Reader;
use openmls::prelude::{
    MlsGroup, MlsGroupCreateConfig, MlsMessageBodyIn, MlsMessageIn,
    PURE_PLAINTEXT_WIRE_FORMAT_POLICY, ProcessedMessageContent, StagedWelcome,
    tls_codec::Deserialize as _,
};
use openmls_traits::OpenMlsProvider as _;

use crate::{CIPHERSUITE, storage::Provider};

const RECORDED: &[u8] = include_bytes!("testdata/openmls-rust-crypto.bin");

#[test]
fn what_the_reference_provider_sent_reads_the_same_here() {
    let mut r = Reader::new(RECORDED);
    let provider = Provider::from_encoded(r.bytes().unwrap()).unwrap();
    let welcome = r.bytes().unwrap();
    let commits: Vec<&[u8]> = (0..r.u32().unwrap()).map(|_| r.bytes().unwrap()).collect();
    let exports: Vec<[u8; 32]> = (0..r.u32().unwrap()).map(|_| r.array().unwrap()).collect();
    r.finish().unwrap();
    assert_eq!((commits.len(), exports.len()), (3, 4));
    let config = MlsGroupCreateConfig::builder()
        .ciphersuite(CIPHERSUITE)
        .use_ratchet_tree_extension(true)
        .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
        .max_past_epochs(0)
        .build()
        .join_config()
        .clone();
    let MlsMessageBodyIn::Welcome(welcome) = MlsMessageIn::tls_deserialize_exact(welcome)
        .unwrap()
        .extract()
    else {
        panic!("a welcome")
    };
    let mut group = StagedWelcome::new_from_welcome(&provider, &config, welcome, None)
        .unwrap()
        .into_group(&provider)
        .unwrap();
    let export = |g: &MlsGroup| {
        g.export_secret(provider.crypto(), "fogata/v3/epoch", b"ctx", 32)
            .unwrap()
    };
    assert_eq!(export(&group), exports[0]);
    for (commit, expected) in commits.iter().zip(&exports[1..]) {
        let message = MlsMessageIn::tls_deserialize_exact(*commit)
            .unwrap()
            .try_into_protocol_message()
            .unwrap();
        let ProcessedMessageContent::StagedCommitMessage(staged) = group
            .process_message(&provider, message)
            .unwrap()
            .into_content()
        else {
            panic!("a commit")
        };
        group.merge_staged_commit(&provider, *staged).unwrap();
        assert_eq!(export(&group), *expected);
    }
}
