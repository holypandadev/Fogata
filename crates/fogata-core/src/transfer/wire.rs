//! What the two computers say over `/fogata/transfer/1`: the code run in the clear (it
//! reveals nothing of the code), then frames encrypted with the run's keys, each a control
//! message or a piece of a file.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use fogata_friends::{FrameOpener, FrameSealer, RunKeys, RunReply, RunStart};
use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// The protocol a new computer takes streams of while it shows a code.
pub(crate) const PROTOCOL: fogata_net::StreamProtocol =
    fogata_net::StreamProtocol::new("/fogata/transfer/1");

/// Bytes of a file in one frame.
pub(crate) const CHUNK: usize = 256 * 1024;
/// Longest frame read: a chunk and what encryption and the frame's kind add, or a control
/// message (a profile with its picture is the largest).
const MAX_FRAME: usize = CHUNK + 1024;
const MAX_RUN_MESSAGE: usize = 1024;

/// A control message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub(crate) enum Msg {
    /// The new computer: what it calls itself, and the name of the profile it has now, which
    /// the move replaces.
    Hello {
        computer: String,
        replaces: Option<String>,
    },
    /// The computer with the account: what moves.
    Offer {
        transfer: String,
        computer: String,
        device: String,
        profile: String,
        friends: u32,
        groups: u32,
        chats: u32,
        files: u32,
        bytes: u64,
    },
    /// The person said yes: on each computer, to the move both show.
    Go,
    File {
        path: String,
    },
    FileEnd {
        blake3: String,
    },
    /// Every file went.
    Sent {
        files: u32,
    },
    /// The new computer holds it all, checked, and waits to start using it.
    Staged,
    /// The computer with the account stopped using it: the new one starts.
    Commit,
    /// The new computer uses the account now, and proves it is the computer the move went
    /// to: a keyed hash of the move's id and of this run under the secret of the run that
    /// sent the account ([`Channel::proof`]), which only the computer that got it can make,
    /// in this run.
    Active {
        proof: String,
    },
    /// The computer with the account took the proof and stopped for good: the new one
    /// forgets the move's secret, as it did.
    Moved,
    /// The computer with the account, finishing a move whose end was not confirmed.
    Resume {
        transfer: String,
    },
    /// The new computer knows no such move.
    Unknown,
    Fail {
        reason: String,
    },
}

/// A frame as it arrives.
pub(crate) enum Frame {
    Msg(Msg),
    Data(Zeroizing<Vec<u8>>),
}

async fn write_raw<S: AsyncWrite + Unpin>(io: &mut S, bytes: &[u8]) -> Result<()> {
    io.write_all(&(bytes.len() as u32).to_be_bytes()).await?;
    io.write_all(bytes).await?;
    io.flush().await?;
    Ok(())
}

async fn read_raw<S: AsyncRead + Unpin>(io: &mut S, max: usize) -> Result<Vec<u8>> {
    let mut len = [0u8; 4];
    io.read_exact(&mut len).await?;
    let len = u32::from_be_bytes(len) as usize;
    if len > max {
        bail!("a frame of {len} bytes is too long");
    }
    let mut bytes = vec![0u8; len];
    io.read_exact(&mut bytes).await?;
    Ok(bytes)
}

/// Waits at most `wait` for `what`.
pub(crate) async fn within<T>(
    wait: Duration,
    what: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    tokio::time::timeout(wait, what)
        .await
        .context("the other computer stopped answering")?
}

pub(crate) async fn send_start<S: AsyncWrite + Unpin>(io: &mut S, start: &RunStart) -> Result<()> {
    write_raw(io, &[start.nameplate.as_slice(), &start.spake].concat()).await
}

pub(crate) async fn read_start<S: AsyncRead + Unpin>(io: &mut S) -> Result<RunStart> {
    let bytes = read_raw(io, MAX_RUN_MESSAGE).await?;
    let (nameplate, spake) = bytes.split_at_checked(32).context("a short start")?;
    Ok(RunStart {
        nameplate: nameplate.try_into().expect("32 bytes"),
        spake: spake.to_vec(),
    })
}

/// The answer to a start: `None` when the other computer shows another code.
pub(crate) async fn send_reply<S: AsyncWrite + Unpin>(
    io: &mut S,
    reply: Option<&RunReply>,
) -> Result<()> {
    match reply {
        None => write_raw(io, &[0]).await,
        Some(reply) => {
            write_raw(io, &[&[1], reply.confirm.as_slice(), &reply.spake].concat()).await
        }
    }
}

pub(crate) async fn read_reply<S: AsyncRead + Unpin>(io: &mut S) -> Result<Option<RunReply>> {
    let bytes = read_raw(io, MAX_RUN_MESSAGE).await?;
    match bytes.split_first() {
        Some((0, [])) => Ok(None),
        Some((1, rest)) if rest.len() > 32 => Ok(Some(RunReply {
            confirm: rest[..32].try_into().expect("32 bytes"),
            spake: rest[32..].to_vec(),
        })),
        _ => bail!("a malformed answer"),
    }
}

pub(crate) async fn send_confirm<S: AsyncWrite + Unpin>(
    io: &mut S,
    confirm: &[u8; 32],
) -> Result<()> {
    write_raw(io, confirm).await
}

pub(crate) async fn read_confirm<S: AsyncRead + Unpin>(io: &mut S) -> Result<Vec<u8>> {
    read_raw(io, MAX_RUN_MESSAGE).await
}

/// The encrypted channel after a run.
pub(crate) struct Channel<S> {
    io: S,
    seal: FrameSealer,
    open: FrameOpener,
    /// This run's own secret: the move's secret ([`Self::move_key`]) for the run that sends
    /// the account, and what binds each proof to its run.
    proof: Zeroizing<[u8; 32]>,
}

impl<S> Channel<S> {
    /// The secret of the move this run sends, in hex, which both computers keep: a later run
    /// that finishes the move proves with it that the computer at its other end is the one
    /// the account went to, which knowing the move's id (sent in that run) does not prove.
    pub(crate) fn move_key(&self) -> Zeroizing<String> {
        Zeroizing::new(hex::encode(&self.proof[..]))
    }

    /// What proves, in this run, that the computer at the other end holds `key`, the secret
    /// of move `transfer` ([`Self::move_key`] of the run that sent it).
    pub(crate) fn proof(&self, transfer: &str, key: &str) -> String {
        self.proof_of(transfer, key)
            .map(|hash| hash.to_hex().to_string())
            .unwrap_or_default()
    }

    fn proof_of(&self, transfer: &str, key: &str) -> Option<blake3::Hash> {
        let key: Zeroizing<Vec<u8>> = Zeroizing::new(hex::decode(key).ok()?);
        let key: &[u8; 32] = key.as_slice().try_into().ok()?;
        let mut hasher = blake3::Hasher::new_keyed(key);
        hasher.update(&self.proof[..]).update(transfer.as_bytes());
        Some(hasher.finalize())
    }

    /// Whether `proof` is [`Self::proof`] of `transfer` and `key`, compared in a time that
    /// does not depend on where they differ.
    pub(crate) fn proves(&self, transfer: &str, key: &str, proof: &str) -> bool {
        match (blake3::Hash::from_hex(proof), self.proof_of(transfer, key)) {
            (Ok(given), Some(expected)) => given == expected,
            _ => false,
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> Channel<S> {
    /// The channel of the computer with the account (`sender`) or of the new one.
    pub(crate) fn new(io: S, keys: &RunKeys, sender: bool) -> Self {
        let (out, inn) = if sender {
            (&keys.to_receiver, &keys.to_sender)
        } else {
            (&keys.to_sender, &keys.to_receiver)
        };
        let mut material = Zeroizing::new([0u8; 64]);
        material[..32].copy_from_slice(&keys.to_receiver[..]);
        material[32..].copy_from_slice(&keys.to_sender[..]);
        Self {
            io,
            seal: FrameSealer::new(out),
            open: FrameOpener::new(inn),
            proof: Zeroizing::new(blake3::derive_key(
                "fogata v1 transfer active proof",
                &material[..],
            )),
        }
    }

    pub(crate) async fn send(&mut self, msg: &Msg) -> Result<()> {
        let mut plain = Zeroizing::new(vec![0u8]);
        plain.extend_from_slice(&serde_json::to_vec(msg)?);
        let sealed = self.seal.seal(&plain).map_err(anyhow::Error::msg)?;
        write_raw(&mut self.io, &sealed).await
    }

    pub(crate) async fn send_data(&mut self, data: &[u8]) -> Result<()> {
        let mut plain = Zeroizing::new(Vec::with_capacity(data.len() + 1));
        plain.push(1);
        plain.extend_from_slice(data);
        let sealed = self.seal.seal(&plain).map_err(anyhow::Error::msg)?;
        write_raw(&mut self.io, &sealed).await
    }

    pub(crate) async fn recv(&mut self) -> Result<Frame> {
        let sealed = read_raw(&mut self.io, MAX_FRAME).await?;
        let plain = self
            .open
            .open(&sealed)
            .map_err(|_| anyhow::anyhow!("a frame did not open: changed on the way"))?;
        match plain.split_first() {
            Some((0, json)) => Ok(Frame::Msg(
                serde_json::from_slice(json).context("a malformed message")?,
            )),
            Some((1, data)) => Ok(Frame::Data(Zeroizing::new(data.to_vec()))),
            _ => bail!("a malformed frame"),
        }
    }

    /// The next control message; a piece of a file here is out of place.
    pub(crate) async fn recv_msg(&mut self) -> Result<Msg> {
        match self.recv().await? {
            Frame::Msg(Msg::Fail { reason }) => bail!(Failed(reason)),
            Frame::Msg(msg) => Ok(msg),
            Frame::Data(_) => bail!("a piece of a file out of place"),
        }
    }

    #[cfg(test)]
    pub(crate) fn into_inner(self) -> S {
        self.io
    }

    pub(crate) async fn close(&mut self) {
        let _ = self.io.close().await;
    }

    /// Waits a moment for the other end to close, so that what was sent last reaches it
    /// before this end's connection goes.
    pub(crate) async fn linger(&mut self) {
        let _ = self.io.close().await;
        let mut rest = [0u8; 64];
        let _ = tokio::time::timeout(Duration::from_secs(5), async {
            while matches!(self.io.read(&mut rest).await, Ok(n) if n > 0) {}
        })
        .await;
    }
}

/// The other computer said why it stopped.
#[derive(Debug)]
pub(crate) struct Failed(pub(crate) String);

impl std::fmt::Display for Failed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Failed {}

#[cfg(test)]
mod tests {
    use fogata_friends::{TransferCode, run_reply, run_start};
    use futures::io::Cursor;

    use super::*;

    /// Both ends' keys of a run of one code.
    fn keys() -> (RunKeys, RunKeys) {
        let code = TransferCode::generate().unwrap();
        let (sender, start) = run_start(&code, b"a", b"b");
        let (receiver, reply) = run_reply(&code, b"b", b"a", &start).unwrap();
        let (sender_keys, confirm) = sender.finish(&reply).unwrap();
        (sender_keys, receiver.finish(&confirm).unwrap())
    }

    /// What the sender's channel writes for `frames`.
    fn written(keys: &RunKeys, frames: &[Msg]) -> Vec<u8> {
        futures::executor::block_on(async {
            let mut channel = Channel::new(Cursor::new(Vec::new()), keys, true);
            for msg in frames {
                channel.send(msg).await.unwrap();
            }
            channel.send_data(b"a piece of a file").await.unwrap();
            channel.io.into_inner()
        })
    }

    fn read_all(keys: &RunKeys, bytes: Vec<u8>) -> Vec<Result<Frame>> {
        read_as(keys, bytes, false)
    }

    fn read_as(keys: &RunKeys, bytes: Vec<u8>, sender: bool) -> Vec<Result<Frame>> {
        futures::executor::block_on(async {
            let mut channel = Channel::new(Cursor::new(bytes), keys, sender);
            let mut out = Vec::new();
            loop {
                let frame = channel.recv().await;
                let end = frame.is_err();
                out.push(frame);
                if end {
                    return out;
                }
            }
        })
    }

    #[test]
    fn frames_arrive_as_sent_and_nothing_else_opens() {
        let (sender, receiver) = keys();
        let frames = [Msg::Go, Msg::Sent { files: 3 }];
        let bytes = written(&sender, &frames);
        let read = read_all(&receiver, bytes.clone());
        assert!(matches!(read[0], Ok(Frame::Msg(Msg::Go))));
        assert!(matches!(read[1], Ok(Frame::Msg(Msg::Sent { files: 3 }))));
        assert!(matches!(&read[2], Ok(Frame::Data(d)) if &d[..] == b"a piece of a file"));
        assert_eq!(read.len(), 4, "then the end of the stream");

        // Any byte changed on the way: that frame and the rest do not open.
        for at in [5, bytes.len() / 2, bytes.len() - 1] {
            let mut changed = bytes.clone();
            changed[at] ^= 1;
            let read = read_all(&receiver, changed);
            assert!(read.iter().any(|f| f.is_err()));
            assert!(read.iter().filter(|f| f.is_ok()).count() < 3, "byte {at}");
        }
        // A frame sent again, or the frames in another order: refused.
        let first_len = 4 + u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        let mut replayed = bytes[..first_len].to_vec();
        replayed.extend_from_slice(&bytes);
        let read = read_all(&receiver, replayed);
        assert!(read[0].is_ok() && read[1].is_err());
        let mut swapped = bytes[first_len..].to_vec();
        swapped.extend_from_slice(&bytes[..first_len]);
        assert!(read_all(&receiver, swapped)[0].is_err());
        // Frames of another run (another code), or sent the other way: refused.
        let (_, stranger) = keys();
        assert!(read_all(&stranger, bytes.clone())[0].is_err());
        assert!(read_as(&sender, bytes, true)[0].is_err());
    }

    #[test]
    fn only_the_computer_the_move_went_to_proves_it() {
        let (sender, receiver) = keys();
        let (_, stranger) = keys();
        let at = |keys: &RunKeys, sending| Channel::new(Cursor::new(Vec::new()), keys, sending);
        let old = at(&sender, true);
        let new = at(&receiver, false);
        let id = "ab".repeat(16);
        // Both ends of the run that sends the account keep the same secret for it.
        let key = old.move_key();
        assert_eq!(*new.move_key(), *key);
        assert_ne!(*at(&stranger, false).move_key(), *key);
        let proof = new.proof(&id, &key);
        assert!(old.proves(&id, &key, &proof));
        // Another move's id, another secret, a run of another code, or something made up: no.
        assert!(!old.proves(&"cd".repeat(16), &key, &proof));
        assert!(!old.proves(&id, &at(&stranger, false).move_key(), &proof));
        assert!(!old.proves(&id, &key, &at(&stranger, false).proof(&id, &key)));
        assert!(!old.proves(&id, &key, "not a proof"));
        assert!(!old.proves(&id, &key, ""));
        assert!(!old.proves(&id, "", &new.proof(&id, "")));
        // A later run to finish the move tells the other end the move's id: that end, a
        // computer that saw the new code, is not the one the account went to, and what it
        // makes with this run's own secret proves nothing.
        let (later, thief) = keys();
        let (old_again, thief) = (at(&later, true), at(&thief, false));
        assert!(!old_again.proves(&id, &key, &thief.proof(&id, &thief.move_key())));
        // The computer it went to proves it in that run with the secret it kept; a proof of
        // one run is not one of another.
        let (later, back) = keys();
        let (old_again, back) = (at(&later, true), at(&back, false));
        assert!(old_again.proves(&id, &key, &back.proof(&id, &key)));
        assert!(!old_again.proves(&id, &key, &proof));
    }

    #[test]
    fn an_overlong_frame_is_refused_before_it_is_read() {
        let (_, receiver) = keys();
        let mut bytes = ((MAX_FRAME + 1) as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(&[0; 64]);
        let read = read_all(&receiver, bytes);
        assert!(format!("{:#}", read[0].as_ref().err().unwrap()).contains("too long"));
    }
}
