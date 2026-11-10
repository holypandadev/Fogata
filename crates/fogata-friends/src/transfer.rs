//! Codes that move an account to another computer, and the run that checks one.
//!
//! The new computer shows a code of ten Crockford base32 characters, `XXXXX-XXXXX`; the
//! person types it on the computer that has the account. Nine characters are random; the
//! last is a check character, so a mistyped code is told at once instead of after a search.
//!
//! The first four characters name where the two meet (the *nameplate*): the DHT rendezvous
//! point comes from them alone, so a DHT node that sees it learns those four characters and
//! nothing about the other five, which is not much to learn. All nine are the password of a
//! SPAKE2 run between the two computers: whoever does not know the code gets one guess per
//! run, and nothing seen on the network (the rendezvous point, the run's messages, the
//! encrypted transfer) lets anyone test guesses offline.
//!
//! The computer where the code is typed proves first that it knows the code
//! ([`SenderRun::prove`]): the answer of the computer showing it is a SPAKE2 message alone,
//! which tells nothing of the code to whoever sent the start, so a run that stops there (a
//! stranger that learnt the nameplate and stalls, or goes) is not a guess and costs the
//! code nothing. Only a wrong proof is one; the computer showing the code stops answering it
//! after [`MAX_FAILED_TRANSFER_RUNS`] of those. Its own proof comes only after that, so a
//! computer pretending to show the code learns from the one typing it whether its own guess
//! was right, and no more: the computer where the code is typed makes one run per code
//! typed. A guesser gets a handful of tries at 2^25 possible codes for the part it does not
//! know.
//!
//! That is the second way of running the code (the move's protocol `/fogata/transfer/2`); the
//! first had the computer showing the code prove first, and counted every run that did not
//! finish. The two do not run with each other: [`old_protocol_probe`] tells a computer of the
//! first way showing this code apart without spending anything on either side.
//!
//! The run is bound to the transport identities of both ends and ends with a key for each
//! direction, which the transfer encrypts every frame with ([`FrameSealer`]).

use std::fmt;

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use fogata_crypto::{CryptoError, random_bytes};
use sha2::{Digest, Sha256};
use spake2::{Ed25519Group, Identity, Password, Spake2};
use zeroize::{Zeroize, Zeroizing};

use crate::secret::derive;

/// How long a code shown on the new computer stays valid.
pub const TRANSFER_CODE_LIFETIME_MS: u64 = 10 * 60 * 1000;
/// Runs whose proof of the code was wrong after which the new computer stops answering a
/// code. A run that stops before its proof is not counted: it learnt nothing.
pub const MAX_FAILED_TRANSFER_RUNS: u32 = 3;

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const LEN: usize = 10;
const NAMEPLATE_LEN: usize = 4;
const RENDEZVOUS: &[u8] = b"fogata/v1/transfer-rendezvous";
const NAMEPLATE: &[u8] = b"fogata/v1/transfer-nameplate";
const PASSWORD: &[u8] = b"fogata/v1/transfer-password";
/// The transcript of a run of the second way (the computer typing the code proves first):
/// another label than the first way's, so that no proof of one passes in the other.
const TRANSCRIPT: &[u8] = b"fogata/v2/transfer-transcript";
const KEYS: &[u8] = b"fogata/v1/transfer-keys";
const SPAKE_SENDER: &[u8] = b"fogata/v1/transfer-sender";
const LAN: &[u8] = b"fogata/v1/transfer-lan";
const SPAKE_RECEIVER: &[u8] = b"fogata/v1/transfer-receiver";

/// Why typed text is not a code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CodeProblem {
    #[error("a code has ten letters and digits, like XXXXX-XXXXX")]
    NotACode,
    #[error("that isn't a whole code: check the letters and digits")]
    Mistyped,
}

/// Why a run did not end with a shared key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    /// The other end showed a code with other first characters: not the computer sought.
    #[error("the other computer shows another code")]
    NotMine,
    /// The two ends did not use the same code.
    #[error("the code does not match the one shown on the other computer")]
    WrongCode,
    #[error("malformed message")]
    Malformed,
}

/// A code that moves an account: nine random characters and a check character. Wiped from
/// memory when dropped.
#[derive(Clone, PartialEq, Eq)]
pub struct TransferCode(Zeroizing<[u8; LEN]>);

impl TransferCode {
    pub fn generate() -> Result<Self, CryptoError> {
        let mut bytes = [0u8; 8];
        random_bytes(&mut bytes)?;
        let bits = u64::from_le_bytes(bytes);
        let mut chars = [0u8; LEN];
        for (i, c) in chars.iter_mut().take(LEN - 1).enumerate() {
            *c = CROCKFORD[((bits >> (5 * i)) & 31) as usize];
        }
        chars[LEN - 1] = check_char(&chars[..LEN - 1]);
        let code = Self(Zeroizing::new(chars));
        chars.zeroize();
        Ok(code)
    }

    /// Reads a code as people type it: dashes and spaces anywhere, any case, and O, I and L
    /// read as 0, 1 and 1.
    pub fn parse(text: &str) -> Result<Self, CodeProblem> {
        // Wiped when dropped; never grown, so no copy is left behind either.
        let mut chars = Zeroizing::new(Vec::with_capacity(LEN));
        for c in text.chars() {
            if c == '-' || c.is_whitespace() {
                continue;
            }
            if chars.len() == LEN {
                return Err(CodeProblem::NotACode);
            }
            let c = match c.to_ascii_uppercase() {
                'O' => b'0',
                'I' | 'L' => b'1',
                c if c.is_ascii() && CROCKFORD.contains(&(c as u8)) => c as u8,
                _ => return Err(CodeProblem::NotACode),
            };
            chars.push(c);
        }
        let chars: Zeroizing<[u8; LEN]> = Zeroizing::new(
            chars
                .as_slice()
                .try_into()
                .map_err(|_| CodeProblem::NotACode)?,
        );
        if check_char(&chars[..LEN - 1]) != chars[LEN - 1] {
            return Err(CodeProblem::Mistyped);
        }
        Ok(Self(chars))
    }

    /// The secret behind the rendezvous points where the two computers meet: from the
    /// nameplate alone, so it says nothing of the rest of the code.
    pub fn rendezvous(&self) -> [u8; 32] {
        *derive(RENDEZVOUS, &self.0[..NAMEPLATE_LEN], b"rendezvous")
    }

    /// What the computer typing the code says first, so that a computer showing another code
    /// (on the same network, say) tells it so before the run counts against either code.
    fn nameplate(&self) -> [u8; 32] {
        *derive(NAMEPLATE, &self.0[..NAMEPLATE_LEN], b"nameplate")
    }

    /// Whether `start` is for this code, as far as its nameplate says: anyone who learnt the
    /// rendezvous point may know it, so it proves nothing of the code.
    pub fn named_by(&self, start: &RunStart) -> bool {
        equal(&self.nameplate(), &start.nameplate)
    }

    /// The secret the two computers seal their local addresses under in their DHT
    /// announcements: from the nameplate alone, like the rendezvous point. Whoever watches
    /// that point can open them; anything made from the rest of the code would let it test
    /// guesses of the code offline against them, however slow to make.
    pub fn lan_secret(&self) -> Zeroizing<[u8; 32]> {
        derive(LAN, &self.0[..NAMEPLATE_LEN], b"lan")
    }

    fn password(&self) -> Password {
        Password::new(derive(PASSWORD, &self.0[..LEN - 1], b"password").as_slice())
    }
}

impl fmt::Display for TransferCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = std::str::from_utf8(&self.0[..]).map_err(|_| fmt::Error)?;
        write!(f, "{}-{}", &text[..LEN / 2], &text[LEN / 2..])
    }
}

impl fmt::Debug for TransferCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TransferCode(..)")
    }
}

/// The check character of nine code characters: their values weighted 1, 3, 5, ... 17,
/// summed modulo 32. Odd weights catch every single wrong character, and most pairs of
/// neighbours swapped.
fn check_char(chars: &[u8]) -> u8 {
    let sum: usize = chars
        .iter()
        .enumerate()
        .map(|(i, c)| (2 * i + 1) * CROCKFORD.iter().position(|x| x == c).unwrap_or(0))
        .sum();
    CROCKFORD[sum % 32]
}

/// The first message of a run, from the computer where the code was typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunStart {
    pub nameplate: [u8; 32],
    pub spake: Vec<u8>,
}

/// The answer of the computer showing the code: its SPAKE2 message alone, which tells
/// nothing of the code to whoever sent the start; the proofs come after.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunReply {
    pub spake: Vec<u8>,
}

/// The keys of a finished run, one for each direction.
pub struct RunKeys {
    /// What the computer with the account sends.
    pub to_receiver: Zeroizing<[u8; 32]>,
    /// What the new computer sends.
    pub to_sender: Zeroizing<[u8; 32]>,
}

impl fmt::Debug for RunKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RunKeys(..)")
    }
}

/// The computer where the code was typed, waiting for the answer.
pub struct SenderRun {
    spake: Spake2<Ed25519Group>,
    outbound: Vec<u8>,
    local: Vec<u8>,
    remote: Vec<u8>,
}

impl fmt::Debug for SenderRun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SenderRun(..)")
    }
}

/// The computer showing the code, waiting for the sender's proof.
pub struct ReceiverRun {
    keys: RunKeys,
    /// The proof the sender must send.
    expected: Zeroizing<[u8; 32]>,
    /// This computer's own, sent once the sender's checked out.
    confirm: Zeroizing<[u8; 32]>,
}

/// The computer where the code was typed, having sent its proof: waiting for the other's.
pub struct SenderProven {
    keys: RunKeys,
    expected: Zeroizing<[u8; 32]>,
}

impl fmt::Debug for SenderProven {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SenderProven(..)")
    }
}

impl fmt::Debug for ReceiverRun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReceiverRun(..)")
    }
}

/// Both ends' keys and proofs from the run's shared key and what was said.
struct Derived {
    keys: RunKeys,
    receiver_confirm: Zeroizing<[u8; 32]>,
    sender_confirm: Zeroizing<[u8; 32]>,
}

fn derived(
    key: &[u8],
    sender_peer: &[u8],
    receiver_peer: &[u8],
    start: &[u8],
    reply: &[u8],
) -> Derived {
    let mut hash = Sha256::new();
    for part in [TRANSCRIPT, sender_peer, receiver_peer, start, reply] {
        hash.update((part.len() as u32).to_be_bytes());
        hash.update(part);
    }
    let transcript: [u8; 32] = hash.finalize().into();
    let with = |label: &[u8]| derive(KEYS, key, &[label, &transcript].concat());
    Derived {
        keys: RunKeys {
            to_receiver: with(b"to-receiver"),
            to_sender: with(b"to-sender"),
        },
        receiver_confirm: with(b"receiver-confirm"),
        sender_confirm: with(b"sender-confirm"),
    }
}

/// Starts a run of `code` towards `remote`, a computer that may be showing it.
pub fn run_start(code: &TransferCode, local: &[u8], remote: &[u8]) -> (SenderRun, RunStart) {
    let (spake, outbound) = Spake2::<Ed25519Group>::start_a(
        &code.password(),
        &Identity::new(SPAKE_SENDER),
        &Identity::new(SPAKE_RECEIVER),
    );
    let start = RunStart {
        nameplate: code.nameplate(),
        spake: outbound.clone(),
    };
    (
        SenderRun {
            spake,
            outbound,
            local: local.to_vec(),
            remote: remote.to_vec(),
        },
        start,
    )
}

/// The computer showing `code`: answers a run `remote` started. [`RunError::NotMine`] when
/// the run is for another code, which does not count as a failed run.
pub fn run_reply(
    code: &TransferCode,
    local: &[u8],
    remote: &[u8],
    start: &RunStart,
) -> Result<(ReceiverRun, RunReply), RunError> {
    if !equal(&code.nameplate(), &start.nameplate) {
        return Err(RunError::NotMine);
    }
    let (spake, outbound) = Spake2::<Ed25519Group>::start_b(
        &code.password(),
        &Identity::new(SPAKE_SENDER),
        &Identity::new(SPAKE_RECEIVER),
    );
    let key = Zeroizing::new(
        spake
            .finish(&start.spake)
            .map_err(|_| RunError::Malformed)?,
    );
    let d = derived(&key, remote, local, &start.spake, &outbound);
    Ok((
        ReceiverRun {
            keys: d.keys,
            expected: d.sender_confirm,
            confirm: d.receiver_confirm,
        },
        RunReply { spake: outbound },
    ))
}

/// A start of the first way of running a code (`/fogata/transfer/1`, versions up to
/// 0.1.0-beta.7) for `code`, which no computer can run: the nameplate of the code and a
/// SPAKE2 message that is none. A computer of that way showing another code answers that
/// it does; one showing this code drops the stream at once, without counting a run or
/// telling anything, since the message is not one. So the computer where the code was
/// typed can tell the person to update both computers, and neither side spends anything.
pub fn old_protocol_probe(code: &TransferCode) -> RunStart {
    RunStart {
        nameplate: code.nameplate(),
        spake: vec![0xff],
    }
}

impl SenderRun {
    /// Takes the answer and proves this computer knows the code: the proof to send, and the
    /// run waiting for the other computer's. [`RunError::Malformed`] for an answer that is no
    /// SPAKE2 message; nothing here tells whether the other computer used the same code.
    pub fn prove(self, reply: &RunReply) -> Result<(SenderProven, [u8; 32]), RunError> {
        let key = Zeroizing::new(
            self.spake
                .finish(&reply.spake)
                .map_err(|_| RunError::Malformed)?,
        );
        let d = derived(
            &key,
            &self.local,
            &self.remote,
            &self.outbound,
            &reply.spake,
        );
        Ok((
            SenderProven {
                keys: d.keys,
                expected: d.receiver_confirm,
            },
            *d.sender_confirm,
        ))
    }
}

impl SenderProven {
    /// Checks the other computer's proof: the run's keys, or [`RunError::WrongCode`] when it
    /// used another code (or is not the computer this one connected to).
    pub fn finish(self, confirm: &[u8]) -> Result<RunKeys, RunError> {
        if !equal(&self.expected, confirm) {
            return Err(RunError::WrongCode);
        }
        Ok(self.keys)
    }
}

impl ReceiverRun {
    /// Checks the sender's proof: the run's keys and this computer's own proof to send back,
    /// or [`RunError::WrongCode`], a guess that missed.
    pub fn finish(self, confirm: &[u8]) -> Result<(RunKeys, [u8; 32]), RunError> {
        if !equal(&self.expected, confirm) {
            return Err(RunError::WrongCode);
        }
        Ok((self.keys, *self.confirm))
    }
}

/// Compares in a time that does not depend on where the two differ.
fn equal(a: &[u8; 32], b: &[u8]) -> bool {
    b.len() == 32 && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Encrypts the frames one end sends, each with the next number as its nonce, so that none
/// can be dropped, repeated or put in another order without the next one failing to open.
pub struct FrameSealer {
    cipher: XChaCha20Poly1305,
    next: u64,
}

impl fmt::Debug for FrameSealer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FrameSealer(..)")
    }
}

/// Opens the frames the other end sends, in order.
pub struct FrameOpener {
    cipher: XChaCha20Poly1305,
    next: u64,
}

impl fmt::Debug for FrameOpener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FrameOpener(..)")
    }
}

/// What every frame is bound to besides its number.
const FRAME_AAD: &[u8] = b"fogata/v1/transfer-frame";

fn frame_nonce(n: u64) -> XNonce {
    let mut nonce = [0u8; 24];
    nonce[16..].copy_from_slice(&n.to_be_bytes());
    XNonce::from(nonce)
}

impl FrameSealer {
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: XChaCha20Poly1305::new(&(*key).into()),
            next: 0,
        }
    }

    pub fn seal(&mut self, plain: &[u8]) -> Result<Vec<u8>, RunError> {
        let nonce = frame_nonce(self.next);
        self.next += 1;
        self.cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plain,
                    aad: FRAME_AAD,
                },
            )
            .map_err(|_| RunError::Malformed)
    }
}

impl FrameOpener {
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: XChaCha20Poly1305::new(&(*key).into()),
            next: 0,
        }
    }

    pub fn open(&mut self, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, RunError> {
        let nonce = frame_nonce(self.next);
        let plain = self
            .cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: sealed,
                    aad: FRAME_AAD,
                },
            )
            .map_err(|_| RunError::Malformed)?;
        self.next += 1;
        Ok(Zeroizing::new(plain))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENDER: &[u8] = b"sender peer";
    const RECEIVER: &[u8] = b"receiver peer";

    fn run(typed: &TransferCode, shown: &TransferCode) -> Result<(RunKeys, RunKeys), RunError> {
        let (sender, start) = run_start(typed, SENDER, RECEIVER);
        let (receiver, reply) = run_reply(shown, RECEIVER, SENDER, &start)?;
        let (sender, proof) = sender.prove(&reply)?;
        let (receiver_keys, answer) = receiver.finish(&proof)?;
        let sender_keys = sender.finish(&answer)?;
        Ok((sender_keys, receiver_keys))
    }

    #[test]
    fn codes_print_parse_and_catch_typos() {
        for _ in 0..200 {
            let code = TransferCode::generate().unwrap();
            let text = code.to_string();
            assert_eq!(text.len(), 11);
            assert_eq!(&text[5..6], "-");
            assert_eq!(TransferCode::parse(&text).unwrap(), code);
            assert_eq!(
                TransferCode::parse(&text.to_lowercase().replace('-', " ")).unwrap(),
                code
            );
            // Every single wrong character is caught.
            let chars: Vec<u8> = text.bytes().filter(|c| *c != b'-').collect();
            for i in 0..chars.len() {
                for other in CROCKFORD.iter().filter(|c| **c != chars[i]) {
                    let mut typo = chars.clone();
                    typo[i] = *other;
                    let typo = String::from_utf8(typo).unwrap();
                    assert_eq!(
                        TransferCode::parse(&typo),
                        Err(CodeProblem::Mistyped),
                        "{typo}"
                    );
                }
            }
        }
        assert_eq!(TransferCode::parse("ABCD-EFGH"), Err(CodeProblem::NotACode));
        assert_eq!(
            TransferCode::parse("ABCDE-FGHJU"),
            Err(CodeProblem::NotACode)
        );
        assert_eq!(
            TransferCode::parse("FGT-ABCDE-FGHJK"),
            Err(CodeProblem::NotACode)
        );
        // O, I and L read as 0, 1 and 1.
        for _ in 0..50 {
            let code = TransferCode::generate().unwrap();
            let typed = code.to_string().replace('0', "o").replace('1', "l");
            assert_eq!(TransferCode::parse(&typed).unwrap(), code);
        }
        assert_eq!(
            format!("{:?}", TransferCode::generate().unwrap()),
            "TransferCode(..)"
        );
    }

    #[test]
    fn the_same_code_ends_with_the_same_keys_both_ways() {
        let code = TransferCode::generate().unwrap();
        let (sender, receiver) = run(&code, &code).unwrap();
        assert_eq!(*sender.to_receiver, *receiver.to_receiver);
        assert_eq!(*sender.to_sender, *receiver.to_sender);
        assert_ne!(*sender.to_receiver, *sender.to_sender);

        let mut seal = FrameSealer::new(&sender.to_receiver);
        let mut open = FrameOpener::new(&receiver.to_receiver);
        let one = seal.seal(b"one").unwrap();
        let two = seal.seal(b"two").unwrap();
        // Out of order, or the other direction's key: nothing opens.
        assert!(open.open(&two).is_err());
        assert!(FrameOpener::new(&receiver.to_sender).open(&one).is_err());
        assert_eq!(&open.open(&one).unwrap()[..], b"one");
        assert_eq!(&open.open(&two).unwrap()[..], b"two");
        assert!(open.open(&two).is_err());
    }

    #[test]
    fn another_code_fails_and_says_which_way() {
        let code = TransferCode::generate().unwrap();
        // Another code altogether: another nameplate, which does not count as a run.
        let other = loop {
            let other = TransferCode::generate().unwrap();
            if other.0[..NAMEPLATE_LEN] != code.0[..NAMEPLATE_LEN] {
                break other;
            }
        };
        assert_eq!(run(&other, &code).unwrap_err(), RunError::NotMine);
        // The same nameplate and another rest: the run fails at the computer showing the
        // code, on the sender's proof, before it proves anything itself.
        let mut same_place = code.0.clone();
        same_place[6] = if same_place[6] == b'0' { b'1' } else { b'0' };
        same_place[LEN - 1] = check_char(&same_place[..LEN - 1]);
        let guess = TransferCode(same_place);
        assert_eq!(run(&guess, &code).unwrap_err(), RunError::WrongCode);
        assert_eq!(code.rendezvous(), guess.rendezvous());
        let (sender, start) = run_start(&guess, SENDER, RECEIVER);
        let (receiver, reply) = run_reply(&code, RECEIVER, SENDER, &start).unwrap();
        let (_, proof) = sender.prove(&reply).unwrap();
        assert_eq!(receiver.finish(&proof).unwrap_err(), RunError::WrongCode);
        // A computer pretending to show the code, with a guess, cannot prove it to the one
        // where the code was typed.
        let (sender, start) = run_start(&code, SENDER, RECEIVER);
        let (pretender, reply) = run_reply(&guess, RECEIVER, SENDER, &start).unwrap();
        let (sender, proof) = sender.prove(&reply).unwrap();
        assert_eq!(pretender.finish(&proof).unwrap_err(), RunError::WrongCode);
        assert_eq!(sender.finish(&[0u8; 32]).unwrap_err(), RunError::WrongCode);
    }

    /// The answer of the computer showing the code is its SPAKE2 message and nothing else:
    /// whoever sent the start has nothing to check a guess against until it proves one, so
    /// a run that stops there is not a guess. And the first way's proofs pass in no run of
    /// this way.
    #[test]
    fn the_answer_to_a_start_tells_nothing_of_the_code() {
        let code = TransferCode::generate().unwrap();
        let (_, start) = run_start(&code, SENDER, RECEIVER);
        let (_, reply) = run_reply(&code, RECEIVER, SENDER, &start).unwrap();
        // A SPAKE2 message: one byte for its side and a point, nothing derived from the
        // run's key.
        assert_eq!(reply.spake.len(), 33);
        // The probe of the first way is no SPAKE2 message: a computer of that way showing
        // this code cannot run it, and one showing another tells it apart by the nameplate.
        let probe = old_protocol_probe(&code);
        assert!(code.named_by(&probe));
        assert_eq!(
            run_reply(&code, RECEIVER, SENDER, &probe).unwrap_err(),
            RunError::Malformed
        );
    }

    #[test]
    fn local_addresses_are_sealed_under_the_nameplate_alone() {
        let code = TransferCode::generate().unwrap();
        let secret = code.lan_secret();
        // The same nameplate with another rest: the same key, so the sealed addresses say
        // nothing of the rest.
        let mut other = code.0.clone();
        other[6] = if other[6] == b'0' { b'1' } else { b'0' };
        other[LEN - 1] = check_char(&other[..LEN - 1]);
        assert_eq!(*secret, *TransferCode(other).lan_secret());
        assert_ne!(*secret, code.rendezvous());
        // Another nameplate: another key.
        let mut other = code.0.clone();
        other[1] = if other[1] == b'0' { b'1' } else { b'0' };
        other[LEN - 1] = check_char(&other[..LEN - 1]);
        assert_ne!(*secret, *TransferCode(other).lan_secret());
    }

    #[test]
    fn a_run_is_bound_to_the_two_transport_identities() {
        let code = TransferCode::generate().unwrap();
        let (sender, start) = run_start(&code, SENDER, RECEIVER);
        // Answered by a computer that is not the one the sender connected to.
        let (receiver, reply) = run_reply(&code, b"someone else", SENDER, &start).unwrap();
        let (_, proof) = sender.prove(&reply).unwrap();
        assert_eq!(receiver.finish(&proof).unwrap_err(), RunError::WrongCode);
    }
}
