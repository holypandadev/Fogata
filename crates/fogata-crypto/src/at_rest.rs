//! What a device keeps on its own disk, sealed with a data key that the operating system's
//! keychain holds, so a copy of the data folder (a backup, a synced folder, a stolen disk)
//! reads as nothing without it.
//!
//! Every sealed file starts with [`MAGIC`] and a byte for its form:
//! - [`Form::Whole`]: one box, for a file that is read and written whole (the identity, the
//!   friends file); [`Form::Named`] is the same box after the name, in the clear, of the
//!   key in the keychain that seals it, so a folder that lost the file naming its key
//!   (`sealed.json`) finds it again in a keychain that cannot list what it holds;
//! - [`Form::Chain`]: a list of boxes, each after its length, for a file that grows at its
//!   end (a chat's journal), each record sealed on its own and bound to the one before it;
//! - [`Form::Resumable`]: a large file in chunks of [`CHUNK`] bytes (an attachment), so it
//!   is sealed and read as it goes and any part can be read without the rest; [`Form::Stream`]
//!   is the earlier form of such files, still read.
//!
//! A box is XChaCha20-Poly1305 under the data key with a random 24-byte nonce. Its
//! associated data is the form and a context naming what the file is, so a box cannot be
//! taken for another kind of file. A record of a chain also binds the tag of the record
//! before it ([`CHAIN_START`] for the first), so records cannot be dropped from its start or
//! middle, repeated or put in another order without the rest failing to open; dropping
//! records from the end, or putting back an older copy of the whole file, is not noticed.
//! [`Form::Records`] is the earlier form of such files, each record on its own: it is still
//! read, and written again as a chain. A stream's chunk nonces are a random prefix of the
//! file, the generation of the writer that sealed the chunk and the chunk's number: a stream
//! cut short is taken up by a writer of the next generation, which keeps the chunks there as
//! they are (see [`SealWriter::resume`] for why no nonce ever seals twice, and how a stream
//! moves to a new file when its chunks have to be sealed again). The last chunk
//! says it is the last, so a stream cut short at a chunk's end does not read as whole. A
//! [`Form::Stream`] has no generation: its nonces are its prefix and the chunk's number.
//!
//! [`MAGIC`] starts with byte 32, which copies of Fogata from before sealing read in the
//! friends file as a version written by a later Fogata: they refuse the folder with that
//! message instead of taking it as damaged.

use std::io::{self, Read, Seek, SeekFrom, Write};

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{AeadInOut, KeyInit},
};
use zeroize::Zeroizing;

use crate::{CryptoError, random_bytes};

/// The start of every sealed file.
pub const MAGIC: [u8; 7] = *b" fogata";
/// [`MAGIC`] and the form byte.
pub const HEADER_LEN: usize = MAGIC.len() + 1;
/// Bytes of plaintext in each chunk of a stream but the last.
pub const CHUNK: usize = 64 * 1024;
const NONCE_LEN: usize = 24;
/// The bytes of a box's tag, which ends it.
pub const TAG_LEN: usize = 16;
/// What the first record of a chain is bound to, in place of a record before it.
pub const CHAIN_START: [u8; TAG_LEN] = [0; TAG_LEN];
/// What a box adds to its plaintext: the nonce and the tag.
pub const BOX_OVERHEAD: usize = NONCE_LEN + TAG_LEN;
/// The bytes after a stream's form byte: the nonce prefix, whose last [`GENERATION_LEN`]
/// bytes are the generation in a [`Form::Resumable`].
const PREFIX_LEN: usize = 16;
const GENERATION_LEN: usize = 4;
/// The header of a stream: [`HEADER_LEN`] and the nonce prefix.
pub const STREAM_HEADER_LEN: usize = HEADER_LEN + PREFIX_LEN;
/// Generations of writers one random prefix of a [`Form::Resumable`] serves; the writer
/// that would be the next one seals the kept chunks again under a new prefix instead. It
/// bounds what a reader tries to open a chunk with.
const GENERATIONS: u32 = 32;
const SEALED_CHUNK: usize = CHUNK + TAG_LEN;
const LABEL: &[u8] = b"fogata/v1/at-rest/";

/// How a sealed file is laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Whole = 1,
    /// Records sealed each on its own, as files of records were written before chains.
    Records = 2,
    Stream = 3,
    Chain = 4,
    /// [`Form::Whole`] after the name of its key: a byte for its length, then the name.
    Named = 5,
    /// A stream whose nonce prefix ends with the generation of the writer that took it up
    /// last; written since streams are taken up without sealing their chunks again.
    Resumable = 6,
}

impl Form {
    fn header(self) -> [u8; HEADER_LEN] {
        let mut header = [0u8; HEADER_LEN];
        header[..MAGIC.len()].copy_from_slice(&MAGIC);
        header[MAGIC.len()] = self as u8;
        header
    }
}

/// The form of a sealed file from its first bytes, or `None` for a file that is not sealed.
pub fn sealed_form(bytes: &[u8]) -> Option<Form> {
    if bytes.len() < HEADER_LEN || bytes[..MAGIC.len()] != MAGIC {
        return None;
    }
    match bytes[MAGIC.len()] {
        1 => Some(Form::Whole),
        2 => Some(Form::Records),
        3 => Some(Form::Stream),
        4 => Some(Form::Chain),
        5 => Some(Form::Named),
        6 => Some(Form::Resumable),
        _ => None,
    }
}

/// The name of the key that sealed a whole file, if the file says it ([`Form::Named`]).
/// Nothing vouches for it: the key it names must still open the file.
pub fn key_name(file: &[u8]) -> Option<&str> {
    named_box(file).map(|(name, _)| name)
}

/// The key's name and the box of a [`Form::Named`] file.
fn named_box(file: &[u8]) -> Option<(&str, &[u8])> {
    if sealed_form(file) != Some(Form::Named) {
        return None;
    }
    let (&len, rest) = file[HEADER_LEN..].split_first()?;
    let (name, sealed) = rest.split_at_checked(usize::from(len))?;
    Some((std::str::from_utf8(name).ok()?, sealed))
}

/// The header that starts a file of sealed records, as [`Form::Records`] was written.
pub fn records_header() -> [u8; HEADER_LEN] {
    Form::Records.header()
}

/// The header that starts a chain of sealed records.
pub fn chain_header() -> [u8; HEADER_LEN] {
    Form::Chain.header()
}

/// What a box of a chain binds the record after it to: its tag, its last bytes.
pub fn chain_link(sealed: &[u8]) -> Option<[u8; TAG_LEN]> {
    sealed.last_chunk().copied()
}

/// The key that seals what this device keeps on its disk. It is wiped from memory when
/// dropped. A key given its name in the keychain ([`Self::named`]) writes it at the start
/// of the whole files it seals.
#[derive(Clone)]
pub struct DataKey {
    key: Zeroizing<[u8; 32]>,
    name: Option<String>,
}

impl std::fmt::Debug for DataKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataKey(..)")
    }
}

impl DataKey {
    /// A new random key.
    pub fn generate() -> Result<Self, CryptoError> {
        let mut key = Zeroizing::new([0u8; 32]);
        random_bytes(key.as_mut())?;
        Ok(Self { key, name: None })
    }

    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self {
            key: Zeroizing::new(*bytes),
            name: None,
        }
    }

    /// The key, known in the keychain as `name` (at most 255 bytes).
    pub fn named(mut self, name: &str) -> Result<Self, CryptoError> {
        if name.is_empty() || name.len() > usize::from(u8::MAX) {
            return Err(CryptoError::Malformed);
        }
        self.name = Some(name.to_string());
        Ok(self)
    }

    /// Its name in the keychain, if it was given one.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new(&(*self.key).into())
    }

    /// Seals `plain` as one box for `context`: the nonce, the ciphertext and the tag.
    pub fn seal(&self, form: Form, context: &str, plain: &[u8]) -> Result<Vec<u8>, CryptoError> {
        self.seal_bound(form, context, &[], plain)
    }

    /// Seals `plain` as the record of a chain for `context` that comes after the record
    /// whose [`chain_link`] is `before`.
    pub fn seal_link(
        &self,
        context: &str,
        before: &[u8; TAG_LEN],
        plain: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        self.seal_bound(Form::Chain, context, before, plain)
    }

    /// Opens a record of a chain sealed by [`Self::seal_link`] after `before`.
    pub fn open_link(
        &self,
        context: &str,
        before: &[u8; TAG_LEN],
        sealed: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        self.open_bound(Form::Chain, context, before, sealed)
    }

    fn seal_bound(
        &self,
        form: Form,
        context: &str,
        extra: &[u8],
        plain: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let mut nonce = [0u8; NONCE_LEN];
        random_bytes(&mut nonce)?;
        let mut out = Vec::with_capacity(BOX_OVERHEAD + plain.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(plain);
        let tag = self
            .cipher()
            .encrypt_inout_detached(
                &XNonce::from(nonce),
                &aad(form, context, extra),
                (&mut out[NONCE_LEN..]).into(),
            )
            .map_err(|_| CryptoError::Malformed)?;
        out.extend_from_slice(tag.as_slice());
        Ok(out)
    }

    /// Opens a box made by [`Self::seal`] with the same form and context.
    pub fn open(
        &self,
        form: Form,
        context: &str,
        sealed: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        self.open_bound(form, context, &[], sealed)
    }

    fn open_bound(
        &self,
        form: Form,
        context: &str,
        extra: &[u8],
        sealed: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        if sealed.len() < BOX_OVERHEAD {
            return Err(CryptoError::Decrypt);
        }
        let (nonce, rest) = sealed.split_at(NONCE_LEN);
        let (body, tag) = rest.split_at(rest.len() - TAG_LEN);
        let mut plain = Zeroizing::new(body.to_vec());
        self.cipher()
            .decrypt_inout_detached(
                &XNonce::try_from(nonce).expect("24 bytes"),
                &aad(form, context, extra),
                plain.as_mut_slice().into(),
                tag.try_into().expect("16 bytes"),
            )
            .map_err(|_| CryptoError::Decrypt)?;
        Ok(plain)
    }

    /// A whole sealed file holding `plain`, after the key's name when it has one.
    pub fn seal_whole(&self, context: &str, plain: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut out = match &self.name {
            Some(name) => {
                let mut out = Form::Named.header().to_vec();
                out.push(u8::try_from(name.len()).map_err(|_| CryptoError::Malformed)?);
                out.extend_from_slice(name.as_bytes());
                out
            }
            None => Form::Whole.header().to_vec(),
        };
        out.extend_from_slice(&self.seal(Form::Whole, context, plain)?);
        Ok(out)
    }

    /// What a whole sealed file holds, whether it names its key or not.
    pub fn open_whole(
        &self,
        context: &str,
        file: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        let sealed = match sealed_form(file) {
            Some(Form::Whole) => &file[HEADER_LEN..],
            Some(Form::Named) => named_box(file).ok_or(CryptoError::Malformed)?.1,
            _ => return Err(CryptoError::Malformed),
        };
        self.open(Form::Whole, context, sealed)
    }

    fn seal_chunk(
        &self,
        context: &str,
        prefix: &[u8; PREFIX_LEN],
        index: u64,
        last: bool,
        chunk: &mut Vec<u8>,
    ) -> Result<(), CryptoError> {
        let nonce = chunk_nonce(prefix, index);
        #[cfg(test)]
        tests::SEALED.with(|n| n.borrow_mut().push(nonce.into()));
        let tag = self
            .cipher()
            .encrypt_inout_detached(
                &nonce,
                &aad(Form::Stream, context, &[u8::from(last)]),
                chunk.as_mut_slice().into(),
            )
            .map_err(|_| CryptoError::Malformed)?;
        chunk.extend_from_slice(tag.as_slice());
        Ok(())
    }

    /// Opens chunk `index` in place, leaving its plaintext in `chunk`. One that does not
    /// open is left as it was (the tag is checked before anything is decrypted), to be tried
    /// under another prefix.
    fn open_chunk(
        &self,
        context: &str,
        prefix: &[u8; PREFIX_LEN],
        index: u64,
        last: bool,
        chunk: &mut Vec<u8>,
    ) -> Result<(), CryptoError> {
        let body = chunk
            .len()
            .checked_sub(TAG_LEN)
            .ok_or(CryptoError::Decrypt)?;
        let tag: [u8; TAG_LEN] = chunk[body..].try_into().expect("16 bytes");
        self.cipher()
            .decrypt_inout_detached(
                &chunk_nonce(prefix, index),
                &aad(Form::Stream, context, &[u8::from(last)]),
                chunk[..body].as_mut().into(),
                &tag.into(),
            )
            .map_err(|_| CryptoError::Decrypt)?;
        chunk.truncate(body);
        Ok(())
    }
}

fn aad(form: Form, context: &str, extra: &[u8]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(LABEL.len() + context.len() + 2 + extra.len());
    aad.extend_from_slice(LABEL);
    aad.extend_from_slice(context.as_bytes());
    aad.push(0);
    aad.push(form as u8);
    aad.extend_from_slice(extra);
    aad
}

fn chunk_nonce(prefix: &[u8; PREFIX_LEN], index: u64) -> XNonce {
    let mut nonce = [0u8; NONCE_LEN];
    nonce[..PREFIX_LEN].copy_from_slice(prefix);
    nonce[PREFIX_LEN..].copy_from_slice(&index.to_be_bytes());
    XNonce::from(nonce)
}

/// The generation a [`Form::Resumable`]'s `prefix` ends with.
fn generation(prefix: &[u8; PREFIX_LEN]) -> u32 {
    u32::from_be_bytes(
        prefix[PREFIX_LEN - GENERATION_LEN..]
            .try_into()
            .expect("4 bytes"),
    )
}

/// `prefix` ending with generation `g` instead.
fn with_generation(prefix: &[u8; PREFIX_LEN], g: u32) -> [u8; PREFIX_LEN] {
    let mut out = *prefix;
    out[PREFIX_LEN - GENERATION_LEN..].copy_from_slice(&g.to_be_bytes());
    out
}

/// A new random prefix of generation 0.
fn fresh_prefix() -> io::Result<[u8; PREFIX_LEN]> {
    let mut prefix = [0u8; PREFIX_LEN];
    random_bytes(&mut prefix[..PREFIX_LEN - GENERATION_LEN]).map_err(io::Error::other)?;
    Ok(prefix)
}

/// The prefixes a chunk of a stream with header `form` and `prefix` may be sealed under,
/// the one of generation `near` first, then those after it and then those before when the
/// chunk comes `after` the one opened with `near`, the other way round otherwise: a writer
/// of a later generation only ever seals chunks after the kept ones.
fn prefixes(
    form: Form,
    prefix: &[u8; PREFIX_LEN],
    near: u32,
    after: bool,
) -> impl Iterator<Item = [u8; PREFIX_LEN]> {
    let last = match form {
        Form::Resumable => generation(prefix),
        _ => 0,
    };
    let near = near.min(last);
    let (up, down) = (near + 1..=last, (0..near).rev());
    let rest: Box<dyn Iterator<Item = u32>> = if after {
        Box::new(up.chain(down))
    } else {
        Box::new(down.chain(up))
    };
    let prefix = *prefix;
    std::iter::once(near).chain(rest).map(move |g| match form {
        Form::Resumable => with_generation(&prefix, g),
        _ => prefix,
    })
}

/// The form of the stream that `header` starts, if it starts one any writer could have
/// written.
fn stream_form(header: &[u8; STREAM_HEADER_LEN]) -> Option<Form> {
    let prefix: &[u8; PREFIX_LEN] = header[HEADER_LEN..].try_into().expect("16 bytes");
    match sealed_form(header)? {
        Form::Resumable if generation(prefix) < GENERATIONS => Some(Form::Resumable),
        Form::Stream => Some(Form::Stream),
        _ => None,
    }
}

/// The file a stream that is taken up again ([`SealWriter::resume`]) is in: what it was
/// written makes the disk at once when asked, and the stream can move whole to a new file
/// that takes its place at once.
pub trait Durable: Read + Write + Seek + Sized {
    /// Makes what was written so far reach the disk.
    fn settle(&mut self) -> io::Result<()>;
    /// Cuts the file to `len` bytes.
    fn cut(&mut self, len: u64) -> io::Result<()>;
    /// A new empty file beside this one, whatever an earlier one cut short left there.
    fn beside(&mut self) -> io::Result<Self>;
    /// Puts `new`, which reached the disk, in the place of this file in one step that
    /// reaches the disk too, and gives it back: before it this file is there whole, after it
    /// `new` is.
    fn replace(self, new: Self) -> io::Result<Self>;
}

impl Durable for io::Cursor<Vec<u8>> {
    fn settle(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn cut(&mut self, len: u64) -> io::Result<()> {
        self.get_mut().truncate(len as usize);
        Ok(())
    }

    fn beside(&mut self) -> io::Result<Self> {
        Ok(io::Cursor::new(Vec::new()))
    }

    fn replace(self, new: Self) -> io::Result<Self> {
        Ok(new)
    }
}

/// A file of a stream on the disk, with the path it is at, so a stream that has to move to
/// a new file ([`Durable::replace`]) can.
#[derive(Debug)]
pub struct StreamFile {
    file: std::fs::File,
    path: std::path::PathBuf,
}

impl StreamFile {
    /// `file`, opened to read and write at `path`.
    pub fn new(file: std::fs::File, path: std::path::PathBuf) -> Self {
        Self { file, path }
    }

    /// Where a stream moving to a new file is written first: ending as a download cut short
    /// does, so whatever leaves such files alone (an account's move) leaves it alone too.
    fn moving_path(&self) -> std::path::PathBuf {
        let mut path = self.path.clone().into_os_string();
        path.push(".moving.part");
        path.into()
    }
}

impl Read for StreamFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }
}

impl Write for StreamFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl Seek for StreamFile {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.file.seek(to)
    }
}

impl Durable for StreamFile {
    fn settle(&mut self) -> io::Result<()> {
        self.file.sync_data()
    }

    fn cut(&mut self, len: u64) -> io::Result<()> {
        self.file.set_len(len)
    }

    fn beside(&mut self) -> io::Result<Self> {
        let path = self.moving_path();
        // What a move cut short left (or a link put there) goes; nothing is followed.
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        Ok(Self {
            file: options.open(&path)?,
            path,
        })
    }

    fn replace(self, mut new: Self) -> io::Result<Self> {
        new.settle()?;
        // Windows does not put a file in the place of one still open.
        drop(self.file);
        std::fs::rename(&new.path, &self.path)?;
        #[cfg(unix)]
        if let Some(dir) = self.path.parent() {
            std::fs::File::open(dir)?.sync_all()?;
        }
        Ok(Self {
            file: new.file,
            path: self.path,
        })
    }
}

/// The size of a stream that holds `plain` bytes.
pub fn stream_len(plain: u64) -> u64 {
    let chunks = plain.div_ceil(CHUNK as u64).max(1);
    STREAM_HEADER_LEN as u64 + plain + chunks * TAG_LEN as u64
}

/// The plaintext size of a stream of `sealed` bytes, and its number of chunks; `None` when
/// no stream has that size.
fn stream_plain_len(sealed: u64) -> Option<(u64, u64)> {
    let body = sealed.checked_sub(STREAM_HEADER_LEN as u64)?;
    let chunks = body.div_ceil(SEALED_CHUNK as u64);
    let last = body.checked_sub(chunks.checked_sub(1)? * SEALED_CHUNK as u64)?;
    if last < TAG_LEN as u64 {
        return None;
    }
    Some((body - chunks * TAG_LEN as u64, chunks))
}

/// Seals what is written to it as a stream into `inner`. [`Self::finish`] seals the last
/// chunk; a writer dropped without it leaves a stream that does not read whole, whose
/// complete chunks [`SealWriter::resume`] can take up again.
pub struct SealWriter<W: Write> {
    inner: W,
    key: DataKey,
    context: String,
    /// What the nonces of the chunks this writer seals start with: the stream's random
    /// prefix and this writer's generation.
    prefix: [u8; PREFIX_LEN],
    index: u64,
    /// The chunk being filled. A full one waits for the next byte, to know whether it is
    /// the last.
    pending: Zeroizing<Vec<u8>>,
    /// A chunk was sealed and not written: the writer takes nothing more.
    failed: bool,
}

impl<W: Write> std::fmt::Debug for SealWriter<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealWriter")
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl<W: Write> SealWriter<W> {
    /// Starts a stream at the start of `inner`, under a new random prefix, generation 0.
    pub fn new(key: &DataKey, context: &str, mut inner: W) -> io::Result<Self> {
        let prefix = fresh_prefix()?;
        inner.write_all(&Form::Resumable.header())?;
        inner.write_all(&prefix)?;
        Ok(Self::at(key, context, inner, prefix, 0))
    }

    fn at(key: &DataKey, context: &str, inner: W, prefix: [u8; PREFIX_LEN], index: u64) -> Self {
        Self {
            inner,
            key: key.clone(),
            context: context.to_string(),
            prefix,
            index,
            pending: Zeroizing::new(Vec::with_capacity(SEALED_CHUNK)),
            failed: false,
        }
    }

    fn write_pending(&mut self, last: bool) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::other("a sealed stream failed to be written"));
        }
        // Whatever happens next, this chunk's nonce is spent: it is never sealed again.
        self.failed = true;
        self.key
            .seal_chunk(
                &self.context,
                &self.prefix,
                self.index,
                last,
                &mut self.pending,
            )
            .map_err(io::Error::other)?;
        self.inner.write_all(&self.pending)?;
        self.pending.clear();
        self.index += 1;
        self.failed = false;
        Ok(())
    }

    /// Seals the last chunk and returns the writer it went to.
    pub fn finish(mut self) -> io::Result<W> {
        self.write_pending(true)?;
        self.inner.flush()?;
        Ok(self.inner)
    }

    /// The plaintext bytes written so far.
    pub fn written(&self) -> u64 {
        self.index * CHUNK as u64 + self.pending.len() as u64
    }
}

impl<W: Durable> SealWriter<W> {
    /// Takes up a stream that a writer left unfinished in `file`: its chunks that read as
    /// complete are kept, handed to `kept` in order, anything after them is cut off, and
    /// writing goes on after them as the next generation. A file that holds no stream's
    /// start begins a new one.
    ///
    /// No nonce seals twice under the key, because the nonce of a chunk is the stream's
    /// random prefix (96 bits), the generation of its writer and the chunk's number:
    /// - each writer seals each number once (a chunk sealed and not written stops it);
    /// - a writer of generation 0 starts a new prefix, drawn at random: two streams share
    ///   one with a chance of about 2^-33 after 2^32 streams under one key;
    /// - a writer of a later generation `g + 1` is made only here, from a header that says
    ///   `g`; it writes `g + 1` there and makes it reach the disk before it seals anything.
    ///   So the header's generation never goes back (not even when a crash loses writes
    ///   that came after) and no later writer is of `g + 1` again. The chunk after the kept
    ///   ones may have been sealed by an earlier writer (a crash in the middle of it, or
    ///   the last chunk of a finished stream), but under an earlier generation;
    /// - the writer that would be generation [`GENERATIONS`], and any that takes up a
    ///   [`Form::Stream`], moves the kept chunks to a new file instead
    ///   ([`Durable::beside`]), sealed under a new prefix of generation 0, and puts it in
    ///   the place of the old one in one step ([`Durable::replace`]) once it reached the
    ///   disk. Until then the old file stays as it was, so a crash in the middle loses
    ///   nothing that was kept, and the next writer moves it again under another new prefix.
    pub fn resume(
        key: &DataKey,
        context: &str,
        mut file: W,
        mut kept: impl FnMut(&[u8]),
    ) -> io::Result<Self> {
        let mut header = [0u8; STREAM_HEADER_LEN];
        file.seek(SeekFrom::Start(0))?;
        let read = read_full(&mut file, &mut header)?;
        let prefix: [u8; PREFIX_LEN] = header[HEADER_LEN..].try_into().expect("16 bytes");
        let Some(form) = stream_form(&header).filter(|_| read == STREAM_HEADER_LEN) else {
            file.cut(0)?;
            file.seek(SeekFrom::Start(0))?;
            return Self::new(key, context, file);
        };
        let moving = form == Form::Stream || generation(&prefix) + 1 >= GENERATIONS;
        let (next, mut moved) = if moving {
            let fresh = fresh_prefix()?;
            let mut new = file.beside()?;
            new.write_all(&Form::Resumable.header())?;
            new.write_all(&fresh)?;
            (fresh, Some(new))
        } else {
            (with_generation(&prefix, generation(&prefix) + 1), None)
        };
        let mut index = 0u64;
        let mut near = 0u32;
        let mut chunk = Zeroizing::new(vec![0u8; SEALED_CHUNK]);
        loop {
            let at = STREAM_HEADER_LEN as u64 + index * SEALED_CHUNK as u64;
            chunk.resize(SEALED_CHUNK, 0);
            file.seek(SeekFrom::Start(at))?;
            if read_full(&mut file, &mut chunk)? < SEALED_CHUNK {
                break;
            }
            let Some(opened) = prefixes(form, &prefix, near, true)
                .find(|p| key.open_chunk(context, p, index, false, &mut chunk).is_ok())
            else {
                break;
            };
            if form == Form::Resumable {
                near = generation(&opened);
            }
            kept(&chunk);
            if let Some(new) = moved.as_mut() {
                key.seal_chunk(context, &next, index, false, &mut chunk)
                    .map_err(io::Error::other)?;
                new.write_all(&chunk)?;
            }
            index += 1;
        }
        let end = STREAM_HEADER_LEN as u64 + index * SEALED_CHUNK as u64;
        let mut file = match moved {
            // Written whole and on the disk before it takes the old one's place.
            Some(new) => file.replace(new)?,
            None => {
                file.seek(SeekFrom::Start(HEADER_LEN as u64))?;
                file.write_all(&next)?;
                file.cut(end)?;
                // The generation reaches the disk before any chunk is sealed under it.
                file.settle()?;
                file
            }
        };
        file.seek(SeekFrom::Start(end))?;
        Ok(Self::at(key, context, file, next, index))
    }
}

impl<W: Write> Write for SealWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.failed {
            return Err(io::Error::other("a sealed stream failed to be written"));
        }
        if buf.is_empty() {
            return Ok(0);
        }
        if self.pending.len() == CHUNK {
            self.write_pending(false)?;
        }
        let n = buf.len().min(CHUNK - self.pending.len());
        self.pending.extend_from_slice(&buf[..n]);
        Ok(n)
    }

    /// Flushes what is sealed already; the chunk being filled stays until it is full.
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Reads the plaintext of a stream, from any place in it.
pub struct SealReader<R: Read + Seek> {
    inner: R,
    key: DataKey,
    context: String,
    form: Form,
    prefix: [u8; PREFIX_LEN],
    /// The number and generation of the chunk opened last: the next one most likely has
    /// the same generation.
    near: (u64, u32),
    len: u64,
    chunks: u64,
    /// Where the next read starts, in the plaintext.
    pos: u64,
    /// The chunk read last, opened, and its number.
    chunk: Zeroizing<Vec<u8>>,
    chunk_index: Option<u64>,
}

impl<R: Read + Seek> std::fmt::Debug for SealReader<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealReader")
            .field("len", &self.len)
            .field("pos", &self.pos)
            .finish_non_exhaustive()
    }
}

impl<R: Read + Seek> SealReader<R> {
    /// Opens the stream in `inner` of `sealed_len` bytes. Its first chunk is opened at once,
    /// so a file that is not a stream sealed with this key and context is refused here.
    pub fn new(key: &DataKey, context: &str, mut inner: R, sealed_len: u64) -> io::Result<Self> {
        let refused = || io::Error::new(io::ErrorKind::InvalidData, "not sealed with this key");
        let (len, chunks) = stream_plain_len(sealed_len).ok_or_else(refused)?;
        let mut header = [0u8; STREAM_HEADER_LEN];
        inner.seek(SeekFrom::Start(0))?;
        inner.read_exact(&mut header)?;
        let form = stream_form(&header).ok_or_else(refused)?;
        let mut reader = Self {
            inner,
            key: key.clone(),
            context: context.to_string(),
            form,
            prefix: header[HEADER_LEN..].try_into().expect("16 bytes"),
            near: (0, 0),
            len,
            chunks,
            pos: 0,
            chunk: Zeroizing::new(Vec::with_capacity(SEALED_CHUNK)),
            chunk_index: None,
        };
        reader.load(0).map_err(|_| refused())?;
        Ok(reader)
    }

    /// The plaintext size.
    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn load(&mut self, index: u64) -> io::Result<()> {
        if self.chunk_index == Some(index) {
            return Ok(());
        }
        self.chunk_index = None;
        let last = index + 1 == self.chunks;
        let start = STREAM_HEADER_LEN as u64 + index * SEALED_CHUNK as u64;
        let size = if last {
            self.len - index * CHUNK as u64 + TAG_LEN as u64
        } else {
            SEALED_CHUNK as u64
        };
        self.chunk.resize(size as usize, 0);
        self.inner.seek(SeekFrom::Start(start))?;
        self.inner.read_exact(&mut self.chunk)?;
        // Each generation that may have sealed it, at most [`GENERATIONS`] tries; reading
        // in order, nearly always the first.
        let (before, near) = self.near;
        let (key, context, chunk) = (&self.key, &self.context, &mut self.chunk);
        let opened = prefixes(self.form, &self.prefix, near, index >= before)
            .find(|p| key.open_chunk(context, p, index, last, chunk).is_ok())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "a sealed file is damaged")
            })?;
        if self.form == Form::Resumable {
            self.near = (index, generation(&opened));
        }
        self.chunk_index = Some(index);
        Ok(())
    }
}

impl<R: Read + Seek> Read for SealReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len || buf.is_empty() {
            return Ok(0);
        }
        let index = self.pos / CHUNK as u64;
        self.load(index)?;
        let at = (self.pos - index * CHUNK as u64) as usize;
        let n = buf.len().min(self.chunk.len() - at);
        buf[..n].copy_from_slice(&self.chunk[at..at + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl<R: Read + Seek> Seek for SealReader<R> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let pos = match to {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::End(n) => self.len.checked_add_signed(n),
            SeekFrom::Current(n) => self.pos.checked_add_signed(n),
        };
        self.pos = pos.ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        Ok(self.pos)
    }
}

/// Reads until `buf` is full or the end; returns how much was read.
fn read_full(r: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(m) => n += m,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn stream(key: &DataKey, plain: &[u8]) -> Vec<u8> {
        let mut w = SealWriter::new(key, "file", Vec::new()).unwrap();
        // In odd pieces, as a download arrives.
        for piece in plain.chunks(7919) {
            w.write_all(piece).unwrap();
        }
        w.finish().unwrap()
    }

    fn read_all(key: &DataKey, sealed: &[u8]) -> io::Result<Vec<u8>> {
        let mut r = SealReader::new(key, "file", Cursor::new(sealed), sealed.len() as u64)?;
        let mut out = Vec::new();
        r.read_to_end(&mut out)?;
        Ok(out)
    }

    #[test]
    fn a_whole_file_opens_with_its_key_and_context_only() {
        let key = DataKey::generate().unwrap();
        let sealed = key.seal_whole("identity", b"secret").unwrap();
        assert_eq!(sealed_form(&sealed), Some(Form::Whole));
        assert_eq!(&*key.open_whole("identity", &sealed).unwrap(), b"secret");
        let other = DataKey::generate().unwrap();
        assert_eq!(
            other.open_whole("identity", &sealed),
            Err(CryptoError::Decrypt)
        );
        assert_eq!(
            key.open_whole("friends", &sealed),
            Err(CryptoError::Decrypt)
        );
        let mut changed = sealed.clone();
        *changed.last_mut().unwrap() ^= 1;
        assert_eq!(
            key.open_whole("identity", &changed),
            Err(CryptoError::Decrypt)
        );
        // Two seals of the same bytes differ.
        assert_ne!(sealed, key.seal_whole("identity", b"secret").unwrap());
        // A plain file is none of the forms.
        assert_eq!(sealed_form(b"\x05friends"), None);
        assert_eq!(sealed_form(b"0a1b2c"), None);
    }

    #[test]
    fn a_named_key_writes_its_name_before_the_box() {
        let key = DataKey::generate().unwrap();
        let named = key.clone().named("data-0123").unwrap();
        let sealed = named.seal_whole("identity", b"secret").unwrap();
        assert_eq!(sealed_form(&sealed), Some(Form::Named));
        assert_eq!(key_name(&sealed), Some("data-0123"));
        // Either key opens it, and the box after the name is the one an unnamed key writes.
        assert_eq!(&*key.open_whole("identity", &sealed).unwrap(), b"secret");
        assert_eq!(&*named.open_whole("identity", &sealed).unwrap(), b"secret");
        let unnamed = [&Form::Whole.header()[..], &sealed[HEADER_LEN + 1 + 9..]].concat();
        assert_eq!(&*key.open_whole("identity", &unnamed).unwrap(), b"secret");
        assert_eq!(key_name(&unnamed), None);
        assert_eq!(
            DataKey::generate().unwrap().open_whole("identity", &sealed),
            Err(CryptoError::Decrypt)
        );
        // A name longer than the file, or no name at all, is refused.
        assert_eq!(key_name(&sealed[..HEADER_LEN + 4]), None);
        assert_eq!(
            key.open_whole("identity", &sealed[..HEADER_LEN + 4]),
            Err(CryptoError::Malformed)
        );
        assert!(key.clone().named("").is_err());
        assert!(key.clone().named(&"x".repeat(256)).is_err());
    }

    #[test]
    fn records_of_a_chain_open_only_in_their_place() {
        let key = DataKey::generate().unwrap();
        let mut before = CHAIN_START;
        let mut boxes = Vec::new();
        for plain in [&b"one"[..], b"two", b"three"] {
            let sealed = key.seal_link("journal", &before, plain).unwrap();
            before = chain_link(&sealed).unwrap();
            boxes.push(sealed);
        }
        let open_all = |boxes: &[&Vec<u8>]| {
            let mut before = CHAIN_START;
            let mut out = Vec::new();
            for sealed in boxes {
                out.push(key.open_link("journal", &before, sealed)?.to_vec());
                before = chain_link(sealed).unwrap();
            }
            Ok::<_, CryptoError>(out)
        };
        let [a, b, c] = [&boxes[0], &boxes[1], &boxes[2]];
        assert_eq!(
            open_all(&[a, b, c]).unwrap(),
            [b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]
        );
        // Dropped at the start or in the middle, repeated or swapped: what follows does not
        // open. Cut at the end, the rest reads.
        for changed in [vec![b, c], vec![a, c], vec![a, a, b], vec![a, c, b]] {
            assert_eq!(open_all(&changed), Err(CryptoError::Decrypt));
        }
        assert_eq!(open_all(&[a, b]).unwrap().len(), 2);
        // Not for another file, nor as a record on its own.
        assert_eq!(
            key.open_link("pins", &CHAIN_START, a),
            Err(CryptoError::Decrypt)
        );
        assert_eq!(
            key.open(Form::Records, "journal", a),
            Err(CryptoError::Decrypt)
        );
        let lone = key.seal(Form::Records, "journal", b"one").unwrap();
        assert_eq!(
            key.open_link("journal", &CHAIN_START, &lone),
            Err(CryptoError::Decrypt)
        );
        assert_eq!(sealed_form(&chain_header()), Some(Form::Chain));
    }

    #[test]
    fn the_header_reads_as_a_later_friends_file_to_older_copies() {
        // Copies from before sealing take a first byte from their version + 1 up to 32 as
        // a later version; their versions were 4 and 5.
        assert_eq!(MAGIC[0], 32);
    }

    #[test]
    fn streams_read_back_whole_and_from_any_place() {
        let key = DataKey::generate().unwrap();
        for size in [0, 1, CHUNK - 1, CHUNK, CHUNK + 1, 3 * CHUNK, 3 * CHUNK + 5] {
            let plain: Vec<u8> = (0..size).map(|i| (i * 31 % 251) as u8).collect();
            let sealed = stream(&key, &plain);
            assert_eq!(sealed.len() as u64, stream_len(size as u64), "size {size}");
            assert_eq!(read_all(&key, &sealed).unwrap(), plain, "size {size}");
            let mut r =
                SealReader::new(&key, "file", Cursor::new(&sealed), sealed.len() as u64).unwrap();
            assert_eq!(r.len(), size as u64);
            for at in [0, size / 3, size.saturating_sub(2), size] {
                r.seek(SeekFrom::Start(at as u64)).unwrap();
                let mut rest = Vec::new();
                r.read_to_end(&mut rest).unwrap();
                assert_eq!(rest, plain[at..], "size {size} from {at}");
            }
        }
    }

    #[test]
    fn a_stream_cut_short_or_changed_does_not_read() {
        let key = DataKey::generate().unwrap();
        let plain = vec![7u8; 2 * CHUNK + 100];
        let sealed = stream(&key, &plain);
        // Cut at a chunk's end: the chunk now last was not sealed as the last.
        let cut = &sealed[..STREAM_HEADER_LEN + 2 * SEALED_CHUNK];
        assert!(read_all(&key, cut).is_err());
        // A byte changed in the middle.
        let mut changed = sealed.clone();
        changed[STREAM_HEADER_LEN + SEALED_CHUNK + 10] ^= 1;
        assert!(read_all(&key, &changed).is_err());
        // Another key, another context, or a plain file: refused at once.
        let other = DataKey::generate().unwrap();
        assert!(
            SealReader::new(&other, "file", Cursor::new(&sealed), sealed.len() as u64).is_err()
        );
        assert!(SealReader::new(&key, "x", Cursor::new(&sealed), sealed.len() as u64).is_err());
        for form in [Form::Stream, Form::Resumable] {
            let mut look_alike = form.header().to_vec();
            look_alike.extend_from_slice(&[0; 100]);
            assert!(
                SealReader::new(
                    &key,
                    "file",
                    Cursor::new(&look_alike),
                    look_alike.len() as u64
                )
                .is_err()
            );
        }
    }

    thread_local! {
        /// Every nonce a chunk was sealed under on this thread.
        pub(super) static SEALED: std::cell::RefCell<Vec<[u8; NONCE_LEN]>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }

    fn generation_of(file: &[u8]) -> u32 {
        generation(file[HEADER_LEN..STREAM_HEADER_LEN].try_into().unwrap())
    }

    /// The random part of a stream's nonce prefix.
    fn random_of(file: &[u8]) -> Vec<u8> {
        file[HEADER_LEN..STREAM_HEADER_LEN - GENERATION_LEN].to_vec()
    }

    #[test]
    fn an_unfinished_stream_goes_on_from_its_complete_chunks() {
        let key = DataKey::generate().unwrap();
        let plain: Vec<u8> = (0..3 * CHUNK + 999).map(|i| (i % 253) as u8).collect();
        // A download that stopped after two chunks and a half: the half never reached the
        // file, and a crash left a few bytes of the third.
        let mut w = SealWriter::new(&key, "file", Cursor::new(Vec::new())).unwrap();
        w.write_all(&plain[..2 * CHUNK + CHUNK / 2]).unwrap();
        let mut file = w.inner.into_inner();
        assert_eq!(sealed_form(&file), Some(Form::Resumable));
        assert_eq!(generation_of(&file), 0);
        let chunks = file[STREAM_HEADER_LEN..].to_vec();
        file.extend_from_slice(&[1, 2, 3]);
        let mut kept = Vec::new();
        let mut w = SealWriter::resume(&key, "file", Cursor::new(file.clone()), |c| {
            kept.extend_from_slice(c)
        })
        .unwrap();
        // The two complete chunks stay as they are on the disk, not sealed again; the half
        // one was still being filled. What comes now is sealed by the next generation: the
        // crash may have left part of the third chunk sealed by the one before, with other
        // bytes than come now.
        assert_eq!(kept, plain[..2 * CHUNK]);
        assert_eq!(w.written(), 2 * CHUNK as u64);
        let taken = w.inner.get_ref();
        assert_eq!(taken[STREAM_HEADER_LEN..], chunks[..]);
        assert_eq!(random_of(taken), random_of(&file));
        assert_eq!(generation_of(taken), 1);
        w.write_all(&plain[2 * CHUNK..]).unwrap();
        let sealed = w.finish().unwrap().into_inner();
        assert_eq!(read_all(&key, &sealed).unwrap(), plain);
        // A finished stream taken up again keeps all but its last chunk, whose number seals
        // other bytes now: by a later generation too.
        let mut kept = Vec::new();
        let mut w = SealWriter::resume(&key, "file", Cursor::new(sealed.clone()), |c| {
            kept.extend_from_slice(c)
        })
        .unwrap();
        assert_eq!(kept.len(), 3 * CHUNK);
        assert_eq!(w.written(), 3 * CHUNK as u64);
        let taken = w.inner.get_ref();
        assert_eq!(
            taken[STREAM_HEADER_LEN..],
            sealed[STREAM_HEADER_LEN..STREAM_HEADER_LEN + 3 * SEALED_CHUNK]
        );
        assert_eq!(generation_of(taken), 2);
        w.write_all(b"more").unwrap();
        let sealed = w.finish().unwrap().into_inner();
        assert_eq!(
            read_all(&key, &sealed).unwrap(),
            [&plain[..3 * CHUNK], b"more"].concat()
        );
        // Something that is no stream starts a new one.
        let w = SealWriter::resume(&key, "file", Cursor::new(b"plain bytes".to_vec()), |_| {
            panic!("nothing to keep")
        })
        .unwrap();
        let sealed = w.finish().unwrap().into_inner();
        assert_eq!(read_all(&key, &sealed).unwrap(), b"");
    }

    /// A file on a disk that, when it crashes, may lose, or keep in part, each write made
    /// since it was last made durable, in any combination. A file put in the place of
    /// another ([`Durable::replace`]) is there for good once that returns. Set `steps`, and
    /// the disk stops (each change after that many fails) as when the computer stops there.
    #[derive(Default)]
    struct File {
        now: Vec<u8>,
        durable: Vec<u8>,
        since: Vec<(u64, Option<Vec<u8>>)>,
    }

    #[derive(Default)]
    struct Fs {
        files: Vec<File>,
        /// The file at the stream's path.
        at_path: usize,
        steps: Option<u32>,
    }

    #[derive(Clone)]
    struct Disk {
        fs: std::rc::Rc<std::cell::RefCell<Fs>>,
        id: usize,
        pos: u64,
    }

    impl Default for Disk {
        fn default() -> Self {
            let fs = Fs {
                files: vec![File::default()],
                ..Default::default()
            };
            Disk {
                fs: std::rc::Rc::new(std::cell::RefCell::new(fs)),
                id: 0,
                pos: 0,
            }
        }
    }

    impl Disk {
        fn step(&self) -> io::Result<()> {
            let mut fs = self.fs.borrow_mut();
            match fs.steps.as_mut() {
                Some(0) => Err(io::Error::other("the computer stopped")),
                Some(n) => {
                    *n -= 1;
                    Ok(())
                }
                None => Ok(()),
            }
        }

        fn stop_after(&self, steps: Option<u32>) {
            self.fs.borrow_mut().steps = steps;
        }

        fn now(&self) -> Vec<u8> {
            self.fs.borrow().files[self.id].now.clone()
        }

        fn durable(&self) -> Vec<u8> {
            self.fs.borrow().files[self.id].durable.clone()
        }

        /// The file at the stream's path.
        fn at_path(&self) -> Disk {
            let id = self.fs.borrow().at_path;
            Disk {
                fs: self.fs.clone(),
                id,
                pos: 0,
            }
        }

        fn files(&self) -> usize {
            self.fs.borrow().files.len()
        }

        /// The computer stopped and starts again: the file at the stream's path, as the
        /// disk kept it.
        fn crash(&self, rng: &mut Rng) -> Disk {
            let mut fs = self.fs.borrow_mut();
            for file in &mut fs.files {
                let mut after = std::mem::take(&mut file.durable);
                for (at, write) in std::mem::take(&mut file.since) {
                    match (write, rng.below(3)) {
                        (_, 0) => {}
                        (None, _) => after.resize(at as usize, 0),
                        (Some(bytes), kept) => {
                            let n = if kept == 1 {
                                bytes.len()
                            } else {
                                rng.below(bytes.len() as u64 + 1) as usize
                            };
                            let end = at as usize + n;
                            if after.len() < end {
                                after.resize(end, 0);
                            }
                            after[at as usize..end].copy_from_slice(&bytes[..n]);
                        }
                    }
                }
                file.now = after.clone();
                file.durable = after;
            }
            fs.steps = None;
            Disk {
                fs: self.fs.clone(),
                id: fs.at_path,
                pos: 0,
            }
        }
    }

    impl Read for Disk {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let fs = self.fs.borrow();
            let now = &fs.files[self.id].now;
            let at = (self.pos as usize).min(now.len());
            let n = buf.len().min(now.len() - at);
            buf[..n].copy_from_slice(&now[at..at + n]);
            self.pos += n as u64;
            Ok(n)
        }
    }

    impl Write for Disk {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.step()?;
            let mut fs = self.fs.borrow_mut();
            let file = &mut fs.files[self.id];
            let at = self.pos as usize;
            if file.now.len() < at + buf.len() {
                file.now.resize(at + buf.len(), 0);
            }
            file.now[at..at + buf.len()].copy_from_slice(buf);
            file.since.push((self.pos, Some(buf.to_vec())));
            self.pos += buf.len() as u64;
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Seek for Disk {
        fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
            let len = self.fs.borrow().files[self.id].now.len() as u64;
            self.pos = match to {
                SeekFrom::Start(n) => n,
                SeekFrom::End(n) => len + n as u64,
                SeekFrom::Current(n) => self.pos + n as u64,
            };
            Ok(self.pos)
        }
    }

    impl Durable for Disk {
        fn settle(&mut self) -> io::Result<()> {
            self.step()?;
            let mut fs = self.fs.borrow_mut();
            let file = &mut fs.files[self.id];
            file.durable = file.now.clone();
            file.since.clear();
            Ok(())
        }

        fn cut(&mut self, len: u64) -> io::Result<()> {
            self.step()?;
            let mut fs = self.fs.borrow_mut();
            let file = &mut fs.files[self.id];
            file.now.resize(len as usize, 0);
            file.since.push((len, None));
            Ok(())
        }

        fn beside(&mut self) -> io::Result<Self> {
            self.step()?;
            let mut fs = self.fs.borrow_mut();
            fs.files.push(File::default());
            Ok(Disk {
                fs: self.fs.clone(),
                id: fs.files.len() - 1,
                pos: 0,
            })
        }

        fn replace(self, mut new: Self) -> io::Result<Self> {
            new.settle()?;
            self.step()?;
            self.fs.borrow_mut().at_path = new.id;
            Ok(new)
        }
    }

    /// Numbers that look random enough for a test, the same each run.
    struct Rng(u64);

    impl Rng {
        fn below(&mut self, n: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % n.max(1)
        }
    }

    #[test]
    fn no_nonce_seals_twice_however_a_stream_is_cut_and_taken_up() {
        let key = DataKey::generate().unwrap();
        let byte = |i: usize| (i * 31 % 251) as u8;
        let plain: Vec<u8> = (0..3 * CHUNK + 100).map(byte).collect();
        SEALED.with(|n| n.borrow_mut().clear());
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let mut w = SealWriter::new(&key, "file", Disk::default()).unwrap();
        let (mut takes, mut sealed_again) = (0, 0);
        for _ in 0..100 {
            // Some more bytes, then a crash, or a crash once the stream is finished.
            let from = w.written() as usize;
            let to = (from + rng.below(2 * CHUNK as u64) as usize + 1).min(plain.len());
            w.write_all(&plain[from..to]).unwrap();
            let mut disk = if to == plain.len() || rng.below(4) == 0 {
                w.finish().unwrap()
            } else {
                w.inner
            };
            // A third of the time everything reached the disk before it stopped.
            if rng.below(3) == 0 {
                disk.settle().unwrap();
            }
            let disk = disk.crash(&mut rng);
            let before = disk.now();
            let mut kept = Vec::new();
            w = SealWriter::resume(&key, "file", disk, |c| kept.extend_from_slice(c)).unwrap();
            // What is kept is what was written, from the start.
            assert_eq!(kept, plain[..kept.len()]);
            assert_eq!(w.written(), kept.len() as u64);
            let after = &w.inner.now();
            let end = STREAM_HEADER_LEN + kept.len() / CHUNK * SEALED_CHUNK;
            if before.len() >= STREAM_HEADER_LEN
                && sealed_form(&before) == Some(Form::Resumable)
                && random_of(after) == random_of(&before)
            {
                // Taken up by the next generation, the kept chunks as they were, and the
                // generation on the disk before anything is sealed by it.
                takes += 1;
                assert_eq!(generation_of(after), generation_of(&before) + 1);
                assert_eq!(
                    after[STREAM_HEADER_LEN..end],
                    before[STREAM_HEADER_LEN..end]
                );
                assert_eq!(
                    w.inner.durable()[..STREAM_HEADER_LEN],
                    after[..STREAM_HEADER_LEN]
                );
            } else if !kept.is_empty() {
                sealed_again += 1;
                assert_eq!(generation_of(after), 0);
            }
        }
        // Nearly every time without sealing again; once every 31 times or so, a new prefix.
        assert!(takes > 80, "{takes} taken up as they were");
        assert!(
            (1..=5).contains(&sealed_again),
            "{sealed_again} sealed again"
        );
        let disk = w.finish().unwrap();
        let len = plain.len().min(disk.now().len());
        let read = read_all(&key, &disk.now()).unwrap();
        assert_eq!(read, plain[..read.len()]);
        assert!(read.len() <= len);
        let nonces = SEALED.with(|n| n.borrow().clone());
        let unique: std::collections::HashSet<_> = nonces.iter().collect();
        assert!(nonces.len() > 100, "{} chunks sealed", nonces.len());
        assert_eq!(unique.len(), nonces.len(), "a nonce sealed twice");
    }

    /// Takes up `disk`, stopping the computer after `steps` changes to the disk (then
    /// starting again and taking it up whole), and checks that what was kept before is kept
    /// still and that the stream finished reads `plain`.
    fn taken_up_after_a_stop(
        key: &DataKey,
        disk: Disk,
        steps: u32,
        rng: &mut Rng,
        plain: &[u8],
        kept_before: usize,
    ) -> bool {
        disk.stop_after(Some(steps));
        let probe = disk.clone();
        let stopped = SealWriter::resume(key, "file", disk, |_| {}).is_err();
        let disk = if stopped {
            probe.crash(rng)
        } else {
            probe.stop_after(None);
            probe.at_path()
        };
        let mut kept = Vec::new();
        let mut w = SealWriter::resume(key, "file", disk, |c| kept.extend_from_slice(c)).unwrap();
        assert_eq!(kept.len(), kept_before, "stopped after {steps} steps");
        assert_eq!(kept, plain[..kept.len()]);
        w.write_all(&plain[kept.len()..]).unwrap();
        let disk = w.finish().unwrap();
        assert_eq!(read_all(key, &disk.now()).unwrap(), plain);
        stopped
    }

    #[test]
    fn a_stream_moved_to_a_new_file_keeps_its_chunks_wherever_the_computer_stops() {
        SEALED.with(|n| n.borrow_mut().clear());
        let mut rng = Rng(0x2545_f491_4f6c_dd1d);
        // Two moves: a stream written before generations, and one whose prefix served
        // every generation it can.
        let old_key = DataKey::from_bytes(&[7; 32]);
        let old_plain: Vec<u8> = (0..CHUNK + 10).map(|i| (i * 31 % 251) as u8).collect();
        let key = DataKey::generate().unwrap();
        let plain: Vec<u8> = (0..2 * CHUNK + 77).map(|i| (i % 241) as u8).collect();
        let mut last = SealWriter::new(&key, "file", Cursor::new(Vec::new())).unwrap();
        last.write_all(&plain[..2 * CHUNK + 1]).unwrap();
        let mut last = last.inner.into_inner();
        last.truncate(STREAM_HEADER_LEN + 2 * SEALED_CHUNK);
        last[STREAM_HEADER_LEN - GENERATION_LEN..STREAM_HEADER_LEN]
            .copy_from_slice(&(GENERATIONS - 1).to_be_bytes());
        // Sealed under generation 0, it reads under the last one too: the reader tries each.
        let cases: [(&DataKey, &[u8], &[u8], usize); 2] = [
            (&old_key, BEFORE_GENERATIONS, &old_plain, CHUNK),
            (&key, &last, &plain, 2 * CHUNK),
        ];
        for (key, file, plain, kept_before) in cases {
            let mut steps = 0;
            loop {
                for _ in 0..3 {
                    let disk = Disk::default();
                    disk.fs.borrow_mut().files[0] = File {
                        now: file.to_vec(),
                        durable: file.to_vec(),
                        since: Vec::new(),
                    };
                    if !taken_up_after_a_stop(key, disk, steps, &mut rng, plain, kept_before) {
                        break;
                    }
                }
                let disk = Disk::default();
                disk.fs.borrow_mut().files[0].now = file.to_vec();
                disk.stop_after(Some(steps));
                if SealWriter::resume(key, "file", disk.clone(), |_| {}).is_ok() {
                    // Moved: a new file took the old one's place.
                    assert_eq!(disk.files(), 2);
                    break;
                }
                steps += 1;
            }
            // Each step of the move: the new file, its header, each chunk, reaching the
            // disk, taking the old one's place.
            assert!(steps >= 5, "{steps} steps");
        }
        let nonces = SEALED.with(|n| n.borrow().clone());
        let unique: std::collections::HashSet<_> = nonces.iter().collect();
        assert_eq!(unique.len(), nonces.len(), "a nonce sealed twice");
    }

    /// A stream as written before nonces held a generation ([`Form::Stream`]), of
    /// [`CHUNK`] + 10 bytes, `(i * 31) % 251` each, under the key of 32 bytes 7, for "file".
    const BEFORE_GENERATIONS: &[u8] = include_bytes!("testdata/stream-before-generations.bin");

    #[test]
    fn a_stream_written_before_generations_reads_and_goes_on() {
        let key = DataKey::from_bytes(&[7; 32]);
        let plain: Vec<u8> = (0..CHUNK + 10).map(|i| (i * 31 % 251) as u8).collect();
        assert_eq!(sealed_form(BEFORE_GENERATIONS), Some(Form::Stream));
        assert_eq!(read_all(&key, BEFORE_GENERATIONS).unwrap(), plain);
        let mut r = SealReader::new(
            &key,
            "file",
            Cursor::new(BEFORE_GENERATIONS),
            BEFORE_GENERATIONS.len() as u64,
        )
        .unwrap();
        r.seek(SeekFrom::Start(CHUNK as u64 + 3)).unwrap();
        let mut rest = Vec::new();
        r.read_to_end(&mut rest).unwrap();
        assert_eq!(rest, plain[CHUNK + 3..]);
        // Taken up, it is sealed again under a prefix of its own and goes on as a stream
        // of generations.
        let mut kept = Vec::new();
        let mut w = SealWriter::resume(
            &key,
            "file",
            Cursor::new(BEFORE_GENERATIONS.to_vec()),
            |c| kept.extend_from_slice(c),
        )
        .unwrap();
        assert_eq!(kept, plain[..CHUNK]);
        let taken = w.inner.get_ref();
        assert_eq!(sealed_form(taken), Some(Form::Resumable));
        assert_eq!(generation_of(taken), 0);
        assert_ne!(
            taken[STREAM_HEADER_LEN..STREAM_HEADER_LEN + SEALED_CHUNK],
            BEFORE_GENERATIONS[STREAM_HEADER_LEN..STREAM_HEADER_LEN + SEALED_CHUNK]
        );
        w.write_all(&plain[CHUNK..]).unwrap();
        let sealed = w.finish().unwrap().into_inner();
        assert_eq!(read_all(&key, &sealed).unwrap(), plain);
    }

    #[test]
    fn a_stream_file_on_the_disk_moves_to_a_new_one_in_its_place() {
        let dir = std::env::temp_dir().join(format!("fogata-at-rest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.jpg.0a0b0c0d.part");
        std::fs::write(&path, BEFORE_GENERATIONS).unwrap();
        // What a move cut short left beside it goes.
        std::fs::write(dir.join("x.jpg.0a0b0c0d.part.moving.part"), b"left").unwrap();
        let key = DataKey::from_bytes(&[7; 32]);
        let plain: Vec<u8> = (0..CHUNK + 10).map(|i| (i * 31 % 251) as u8).collect();
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let mut kept = Vec::new();
        let mut w = SealWriter::resume(&key, "file", StreamFile::new(file, path.clone()), |c| {
            kept.extend_from_slice(c)
        })
        .unwrap();
        assert_eq!(kept, plain[..CHUNK]);
        w.write_all(&plain[CHUNK..]).unwrap();
        w.finish().unwrap().settle().unwrap();
        let sealed = std::fs::read(&path).unwrap();
        assert_eq!(sealed_form(&sealed), Some(Form::Resumable));
        assert_eq!(read_all(&key, &sealed).unwrap(), plain);
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(names, ["x.jpg.0a0b0c0d.part"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn chunks_of_many_generations_read_from_any_place_at_a_small_cost() {
        // Four chunks sealed by generations 0, 11, 22 and the last one a prefix serves, 31:
        // a stream taken up again and again while nothing came.
        let key = DataKey::generate().unwrap();
        let plain: Vec<u8> = (0..4 * CHUNK).map(|i| (i % 249) as u8).collect();
        let mut w = SealWriter::new(&key, "file", Cursor::new(Vec::new())).unwrap();
        for (chunk, takes) in [(0, 10), (1, 10), (2, GENERATIONS as usize - 24)] {
            w.write_all(&plain[chunk * CHUNK..(chunk + 1) * CHUNK + 1])
                .unwrap();
            let mut file = w.inner.into_inner();
            file.truncate(STREAM_HEADER_LEN + (chunk + 1) * SEALED_CHUNK);
            for _ in 0..takes {
                w = SealWriter::resume(&key, "file", Cursor::new(file), |_| {}).unwrap();
                file = w.inner.into_inner();
            }
            w = SealWriter::resume(&key, "file", Cursor::new(file), |_| {}).unwrap();
        }
        assert_eq!(generation_of(w.inner.get_ref()), GENERATIONS - 1);
        w.write_all(&plain[3 * CHUNK..]).unwrap();
        let sealed = w.finish().unwrap().into_inner();
        assert_eq!(read_all(&key, &sealed).unwrap(), plain);
        // From any chunk to any other: the most a chunk is tried with is every generation.
        let mut r =
            SealReader::new(&key, "file", Cursor::new(&sealed), sealed.len() as u64).unwrap();
        let started = std::time::Instant::now();
        for chunk in [3, 0, 2, 1, 3, 1, 0] {
            r.seek(SeekFrom::Start((chunk * CHUNK) as u64)).unwrap();
            let mut piece = [0u8; 5];
            r.read_exact(&mut piece).unwrap();
            assert_eq!(piece, plain[chunk * CHUNK..chunk * CHUNK + 5]);
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        // A header with a generation no writer reaches does not read.
        let mut beyond = sealed.clone();
        beyond[STREAM_HEADER_LEN - GENERATION_LEN..STREAM_HEADER_LEN]
            .copy_from_slice(&GENERATIONS.to_be_bytes());
        assert!(read_all(&key, &beyond).is_err());
    }
}
