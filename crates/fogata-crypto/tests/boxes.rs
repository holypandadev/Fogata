//! Every box this crate seals, through its public calls only, as the crates above it use
//! them: what was sealed opens with the right key and context, and with nothing else; and
//! any single bit changed anywhere in what was sealed, or any end cut off, is refused,
//! never opened to something else and never a panic. Each format's unit tests try a few
//! such changes; these try every one, on small boxes.

use fogata_crypto::{
    DeviceIdentity, DevicePublicKey, GroupId, GroupKeys, GroupSecret, MembershipProof,
    MessageHeader, NONCE_LEN, SIGNATURE_LEN, Sealed, SealedMessage, SealedProof,
    at_rest::{self, DataKey, Form},
    new_challenge, open_short,
    recovery::{CopyOpener, CopyProblem, CopySealer, RecoveryKey, copy_proof, proves},
    seal_short,
};

/// Checks that `opens` refuses every change of one bit of `sealed` and every cut of its
/// end, and takes `sealed` itself. `opens` says whether a copy opened to what was sealed;
/// opening to anything else is a failure of the box, told apart by its panic.
fn every_change_refused(what: &str, sealed: &[u8], opens: impl Fn(&[u8]) -> bool) {
    every_bit_and_cut_refused(what, sealed, &opens);
    let mut longer = sealed.to_vec();
    longer.push(0);
    assert!(!opens(&longer), "{what}: opens with a byte added");
}

/// [`every_change_refused`] but for a byte added: for what is read from the start of a
/// file, whose next bytes are the rest of the file.
fn every_bit_and_cut_refused(what: &str, sealed: &[u8], opens: impl Fn(&[u8]) -> bool) {
    assert!(opens(sealed), "{what}: does not open as sealed");
    let mut changed = sealed.to_vec();
    for byte in 0..sealed.len() {
        for bit in 0..8 {
            changed[byte] ^= 1 << bit;
            assert!(
                !opens(&changed),
                "{what}: opens with bit {bit} of byte {byte} changed"
            );
            changed[byte] ^= 1 << bit;
        }
    }
    for len in 0..sealed.len() {
        assert!(!opens(&sealed[..len]), "{what}: opens cut to {len} bytes");
    }
}

const PLAIN: &[u8] = b"the tent is behind the north ridge";

#[test]
fn a_box_sealed_to_a_device_opens_for_it_alone() {
    let bea = DeviceIdentity::generate().unwrap();
    let cai = DeviceIdentity::generate().unwrap();
    let sealed = Sealed::seal(&bea.public_key(), b"friend-card", PLAIN).unwrap();
    assert_eq!(sealed.open(&bea, b"friend-card").unwrap(), PLAIN);
    assert!(sealed.open(&cai, b"friend-card").is_err());
    assert!(sealed.open(&bea, b"friend-cards").is_err());
    // As it travels: the sender's half of the agreement, the nonce, the box.
    let mut flat = sealed.ephemeral.to_vec();
    flat.extend_from_slice(&sealed.nonce);
    flat.extend_from_slice(&sealed.ciphertext);
    every_change_refused("a sealed box", &flat, |bytes| {
        if bytes.len() < 32 + NONCE_LEN {
            return false;
        }
        let copy = Sealed {
            ephemeral: bytes[..32].try_into().unwrap(),
            nonce: bytes[32..32 + NONCE_LEN].try_into().unwrap(),
            ciphertext: bytes[32 + NONCE_LEN..].to_vec(),
        };
        match copy.open(&bea, b"friend-card") {
            Ok(plain) => {
                assert_eq!(plain, PLAIN, "opened to something else");
                true
            }
            Err(_) => false,
        }
    });
}

#[test]
fn a_short_box_opens_with_its_secret_and_context_alone() {
    let secret = [7u8; 32];
    let sealed = seal_short(&secret, b"lan", PLAIN).unwrap();
    assert!(open_short(&[8u8; 32], b"lan", &sealed).is_err());
    assert!(open_short(&secret, b"wan", &sealed).is_err());
    every_change_refused("a short box", &sealed, |bytes| {
        match open_short(&secret, b"lan", bytes) {
            Ok(plain) => {
                assert_eq!(plain, PLAIN, "opened to something else");
                true
            }
            Err(_) => false,
        }
    });
}

fn group_keys(epoch: u32) -> (GroupKeys, GroupSecret) {
    let secret = GroupSecret::generate().unwrap();
    let keys = GroupKeys::derive(GroupId::random().unwrap(), epoch, &secret);
    (keys, secret)
}

#[test]
fn a_group_message_opens_for_its_group_and_epoch_alone() {
    let ana = DeviceIdentity::generate().unwrap();
    let (keys, secret) = group_keys(3);
    let sealed = SealedMessage::seal(&keys, &ana, 1_700_000_000_000, PLAIN).unwrap();
    assert_eq!(sealed.open(&keys).unwrap(), PLAIN);
    let next = GroupKeys::derive(keys.group_id(), 4, &secret);
    assert!(sealed.open(&next).is_err());
    let (other, _) = group_keys(3);
    assert!(sealed.open(&other).is_err());

    let h = &sealed.header;
    let mut flat = vec![h.version];
    flat.extend_from_slice(h.group_id.as_bytes());
    flat.extend_from_slice(&h.epoch.to_be_bytes());
    flat.extend_from_slice(h.sender.as_bytes());
    flat.extend_from_slice(&h.timestamp_ms.to_be_bytes());
    flat.extend_from_slice(&h.message_id);
    flat.extend_from_slice(&sealed.nonce);
    flat.extend_from_slice(&sealed.signature);
    flat.extend_from_slice(&sealed.ciphertext);
    let fixed = 1 + 16 + 4 + 32 + 8 + 16 + NONCE_LEN + SIGNATURE_LEN;
    every_change_refused("a group message", &flat, |b| {
        if b.len() < fixed {
            return false;
        }
        let Ok(sender) = DevicePublicKey::from_bytes(&b[21..53]) else {
            return false;
        };
        let copy = SealedMessage {
            header: MessageHeader {
                version: b[0],
                group_id: GroupId::from_bytes(&b[1..17]).unwrap(),
                epoch: u32::from_be_bytes(b[17..21].try_into().unwrap()),
                sender,
                timestamp_ms: u64::from_be_bytes(b[53..61].try_into().unwrap()),
                message_id: b[61..77].try_into().unwrap(),
            },
            nonce: b[77..77 + NONCE_LEN].try_into().unwrap(),
            signature: b[77 + NONCE_LEN..fixed].try_into().unwrap(),
            ciphertext: b[fixed..].to_vec(),
        };
        match copy.open(&keys) {
            Ok(plain) => {
                assert_eq!(plain, PLAIN, "opened to something else");
                true
            }
            Err(_) => false,
        }
    });
}

#[test]
fn a_membership_proof_opens_for_its_group_its_connection_and_its_challenge_alone() {
    let bea = DeviceIdentity::generate().unwrap();
    let (keys, _) = group_keys(1);
    let (prover, verifier) = (b"peer-bea".as_slice(), b"peer-ana".as_slice());
    let challenge = new_challenge().unwrap();
    let proof = MembershipProof::create(&keys, &bea, prover, verifier, &challenge);
    let sealed = proof.seal(&keys, prover, verifier, &challenge).unwrap();
    let opened = MembershipProof::open(&sealed, &keys, prover, verifier, &challenge).unwrap();
    assert_eq!(opened, bea.public_key());
    // Another connection, another challenge, another group: refused.
    assert!(MembershipProof::open(&sealed, &keys, verifier, prover, &challenge).is_err());
    let other = new_challenge().unwrap();
    assert!(MembershipProof::open(&sealed, &keys, prover, verifier, &other).is_err());
    let (strangers, _) = group_keys(1);
    assert!(MembershipProof::open(&sealed, &strangers, prover, verifier, &challenge).is_err());
    // A decoy is refused like any box not sealed with the key.
    let decoy = SealedProof::decoy(1).unwrap();
    assert!(MembershipProof::open(&decoy, &keys, prover, verifier, &challenge).is_err());

    let mut flat = sealed.epoch.to_be_bytes().to_vec();
    flat.extend_from_slice(&sealed.nonce);
    flat.extend_from_slice(&sealed.ciphertext);
    every_change_refused("a sealed proof", &flat, |b| {
        if b.len() < 4 + NONCE_LEN {
            return false;
        }
        let copy = SealedProof {
            epoch: u32::from_be_bytes(b[..4].try_into().unwrap()),
            nonce: b[4..4 + NONCE_LEN].try_into().unwrap(),
            ciphertext: b[4 + NONCE_LEN..].to_vec(),
        };
        match MembershipProof::open(&copy, &keys, prover, verifier, &challenge) {
            Ok(device) => {
                assert_eq!(device, bea.public_key(), "opened to another device");
                true
            }
            Err(_) => false,
        }
    });
}

#[test]
fn a_signature_holds_for_its_message_and_key_alone() {
    let ana = DeviceIdentity::generate().unwrap();
    let signature = ana.sign(PLAIN);
    every_change_refused("a signature", &signature, |s| {
        ana.public_key().verify(PLAIN, s).is_ok()
    });
    every_change_refused("a signed message", PLAIN, |m| {
        ana.public_key().verify(m, &signature).is_ok()
    });
    let bea = DeviceIdentity::generate().unwrap();
    assert!(bea.public_key().verify(PLAIN, &signature).is_err());
    // A key kept as its secret bytes is the same key.
    let again = DeviceIdentity::from_secret_bytes(&ana.secret_bytes());
    assert_eq!(again.public_key(), ana.public_key());
    assert!(again.public_key().verify(PLAIN, &signature).is_ok());
}

/// A key the system keychain would keep, made up here: what is sealed on the disk opens
/// with it and its context alone, in each form of box the data folder holds.
#[test]
fn what_is_sealed_on_the_disk_opens_with_its_key_alone() {
    let key = DataKey::from_bytes(&[42; 32]);
    let other = DataKey::from_bytes(&[43; 32]);
    for form in [Form::Whole, Form::Records, Form::Stream, Form::Named] {
        let sealed = key.seal(form, "friends.json", PLAIN).unwrap();
        assert!(other.open(form, "friends.json", &sealed).is_err());
        assert!(key.open(form, "groups.json", &sealed).is_err());
        every_change_refused(&format!("a box of form {form:?}"), &sealed, |b| {
            match key.open(form, "friends.json", b) {
                Ok(plain) => {
                    assert_eq!(*plain, PLAIN, "opened to something else");
                    true
                }
                Err(_) => false,
            }
        });
        // A box of one form is not one of another.
        for another in [Form::Whole, Form::Records, Form::Stream, Form::Named] {
            if another != form {
                assert!(key.open(another, "friends.json", &sealed).is_err());
            }
        }
    }

    // A whole file, plain and after its key's name.
    let file = key.seal_whole("profile.json", PLAIN).unwrap();
    assert_eq!(at_rest::sealed_form(&file), Some(Form::Whole));
    every_change_refused("a whole sealed file", &file, |b| {
        key.open_whole("profile.json", b).is_ok_and(|plain| {
            assert_eq!(*plain, PLAIN, "opened to something else");
            true
        })
    });
    let named = DataKey::from_bytes(&[42; 32]).named("data-key-2").unwrap();
    let file = named.seal_whole("profile.json", PLAIN).unwrap();
    assert_eq!(at_rest::key_name(&file), Some("data-key-2"));
    assert_eq!(*key.open_whole("profile.json", &file).unwrap(), PLAIN);
    assert!(other.open_whole("profile.json", &file).is_err());
    // The name only says which key to try: nothing vouches for it, so a changed name may
    // still open with the key. Every other byte counts.
    let name_at = at_rest::HEADER_LEN + 1.."data-key-2".len() + at_rest::HEADER_LEN + 1;
    let mut changed = file.clone();
    for byte in (0..file.len()).filter(|b| !name_at.contains(b)) {
        for bit in 0..8 {
            changed[byte] ^= 1 << bit;
            assert!(
                key.open_whole("profile.json", &changed).is_err(),
                "a named file opens with bit {bit} of byte {byte} changed"
            );
            changed[byte] ^= 1 << bit;
        }
    }
    for len in 0..file.len() {
        assert!(key.open_whole("profile.json", &file[..len]).is_err());
    }

    // The records of a chain open only in their place, after the record before them.
    let first = key
        .seal_link("log", &at_rest::CHAIN_START, b"first")
        .unwrap();
    let link = at_rest::chain_link(&first).unwrap();
    let second = key.seal_link("log", &link, b"second").unwrap();
    assert_eq!(*key.open_link("log", &link, &second).unwrap(), b"second");
    assert!(
        key.open_link("log", &at_rest::CHAIN_START, &second)
            .is_err()
    );
    every_change_refused("a record of a chain", &second, |b| {
        key.open_link("log", &link, b).is_ok()
    });
}

#[test]
fn a_copy_of_an_account_opens_with_its_recovery_key_alone() {
    let key = RecoveryKey::generate().unwrap();
    // As the person writes it down and types it again, in small letters with dashes.
    let typed = key.text().to_lowercase().replace(' ', "-");
    assert_eq!(RecoveryKey::parse(&typed).unwrap(), key);

    let mut sealer = CopySealer::new(&key.public(), 20_400).unwrap();
    let header = *sealer.header();
    let frames: Vec<Vec<u8>> = [
        b"profile".as_slice(),
        b"friends".as_slice(),
        b"groups".as_slice(),
    ]
    .iter()
    .map(|part| sealer.seal(part).unwrap())
    .collect();
    let proof = copy_proof(&key.auth_key(), &header);
    assert!(proves(&key.auth_key(), &header, &proof));

    let open_all = |header: &[u8], frames: &[Vec<u8>]| -> Result<Vec<Vec<u8>>, CopyProblem> {
        let mut opener = CopyOpener::new(header, &key)?;
        frames
            .iter()
            .map(|f| opener.open(f).map(|p| p.to_vec()))
            .collect()
    };
    assert_eq!(
        open_all(&header, &frames).unwrap(),
        [b"profile".to_vec(), b"friends".to_vec(), b"groups".to_vec()]
    );
    let stranger = RecoveryKey::generate().unwrap();
    assert_eq!(
        CopyOpener::new(&header, &stranger).unwrap_err(),
        CopyProblem::OtherKey
    );
    // Frames dropped, repeated or put in another order do not open.
    let reordered = [frames[1].clone(), frames[0].clone(), frames[2].clone()];
    assert!(open_all(&header, &reordered).is_err());
    let repeated = [frames[0].clone(), frames[0].clone()];
    assert!(open_all(&header, &repeated).is_err());
    assert!(open_all(&header, &frames[1..]).is_err());

    // The header is read from the start of the copy's file, which goes on after it.
    every_bit_and_cut_refused("a copy's header", &header, |h| open_all(h, &frames).is_ok());
    for (n, frame) in frames.iter().enumerate() {
        every_change_refused(&format!("frame {n} of a copy"), frame, |f| {
            let mut changed = frames.clone();
            changed[n] = f.to_vec();
            open_all(&header, &changed).is_ok()
        });
    }
    every_change_refused("a copy's proof", &proof, |p| {
        proves(&key.auth_key(), &header, p)
    });
}
