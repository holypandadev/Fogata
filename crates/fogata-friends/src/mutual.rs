//! Friends in common, found without either device showing its list of friends.
//!
//! Each device hides its friends' keys under a secret of its own on the Ristretto group:
//! the asker sends `H(x)^a` for each of its friends `x`; the friend asked raises those to its
//! secret `b` and sends them back with `H(y)^b` for each of its own friends `y`, shuffled;
//! the asker raises the latter to `a`, and the keys whose `H(x)^ab` match are the friends in
//! common. The friend asked learns nothing but roughly how many keys the asker sent; the
//! asker learns which of the keys it sent are the other's friends, and roughly how many
//! friends the other has: both lists are padded with random points to a multiple of [`PAD`].
//! The asker picks the keys it sends: an honest one sends its friends, but a modified one may
//! send any keys it knows (up to [`MAX_MUTUAL_POINTS`]) to learn whether each is a friend of
//! the other. It learns nothing of keys it does not know. How often a friend may ask is
//! bounded by the device answering (fogata-net: once an hour, and 4,096 keys a day).

use curve25519_dalek::{RistrettoPoint, Scalar, ristretto::CompressedRistretto};
use fogata_crypto::{DevicePublicKey, random_bytes};
use sha2::{Digest, Sha512};

use crate::FriendError;

const LABEL: &[u8] = b"fogata/v1/mutual-friend";
/// Most points either side sends.
pub const MAX_MUTUAL_POINTS: usize = 1024;
/// Lists are padded to a multiple of this.
const PAD: usize = 16;

/// A point, compressed.
pub type Point = [u8; 32];

fn hash_to_point(device: &DevicePublicKey) -> RistrettoPoint {
    let mut hash = Sha512::new();
    hash.update(LABEL);
    hash.update(device.as_bytes());
    let mut wide = [0u8; 64];
    wide.copy_from_slice(&hash.finalize());
    RistrettoPoint::from_uniform_bytes(&wide)
}

fn random_wide() -> Result<[u8; 64], FriendError> {
    let mut wide = [0u8; 64];
    random_bytes(&mut wide)?;
    Ok(wide)
}

fn random_scalar() -> Result<Scalar, FriendError> {
    Ok(Scalar::from_bytes_mod_order_wide(&random_wide()?))
}

fn decompress(point: &Point) -> Result<RistrettoPoint, FriendError> {
    CompressedRistretto(*point)
        .decompress()
        .ok_or(FriendError::Malformed)
}

/// Shuffles `items` with the system's random numbers.
fn shuffle<T>(items: &mut [T]) -> Result<(), FriendError> {
    for i in (1..items.len()).rev() {
        let mut bytes = [0u8; 8];
        random_bytes(&mut bytes)?;
        let j = (u64::from_le_bytes(bytes) % (i as u64 + 1)) as usize;
        items.swap(i, j);
    }
    Ok(())
}

/// `keys` hidden under `secret`, with random points added up to a multiple of [`PAD`], in a
/// random order. Each entry says which key it hides, if any.
fn hide(
    keys: &[DevicePublicKey],
    secret: &Scalar,
) -> Result<Vec<(Option<DevicePublicKey>, Point)>, FriendError> {
    let real = keys.len().min(MAX_MUTUAL_POINTS - PAD);
    let total = (real / PAD + 1) * PAD;
    let mut out = Vec::with_capacity(total);
    for key in &keys[..real] {
        out.push((Some(*key), (hash_to_point(key) * secret).compress().0));
    }
    while out.len() < total {
        let filler = RistrettoPoint::from_uniform_bytes(&random_wide()?);
        out.push((None, filler.compress().0));
    }
    shuffle(&mut out)?;
    Ok(out)
}

/// A question about friends in common, waiting for its answer.
pub struct MutualAsk {
    secret: Scalar,
    /// The key behind each point sent, in order; `None` for the random ones.
    sent: Vec<Option<DevicePublicKey>>,
}

impl std::fmt::Debug for MutualAsk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MutualAsk")
            .field("sent", &self.sent.len())
            .finish_non_exhaustive()
    }
}

/// The answer to a [`MutualAsk`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutualAnswer {
    /// The asker's points raised to the answerer's secret, in the order they came.
    pub doubled: Vec<Point>,
    /// The answerer's friends hidden under its secret, in a random order.
    pub hidden: Vec<Point>,
}

impl MutualAsk {
    /// Starts a question with this device's `friends`, the friend asked left out. Returns
    /// the points to send.
    pub fn start(friends: &[DevicePublicKey]) -> Result<(Self, Vec<Point>), FriendError> {
        let secret = random_scalar()?;
        let hidden = hide(friends, &secret)?;
        let points = hidden.iter().map(|(_, p)| *p).collect();
        let sent = hidden.into_iter().map(|(key, _)| key).collect();
        Ok((Self { secret, sent }, points))
    }

    /// The friends in common, from the answer.
    pub fn finish(&self, answer: &MutualAnswer) -> Result<Vec<DevicePublicKey>, FriendError> {
        if answer.doubled.len() != self.sent.len() || answer.hidden.len() > MAX_MUTUAL_POINTS {
            return Err(FriendError::Malformed);
        }
        let mut theirs = std::collections::HashSet::with_capacity(answer.hidden.len());
        for point in &answer.hidden {
            theirs.insert((decompress(point)? * self.secret).compress().0);
        }
        let mut out = Vec::new();
        for (key, point) in self.sent.iter().zip(&answer.doubled) {
            if let Some(key) = key
                && theirs.contains(point)
            {
                out.push(*key);
            }
        }
        out.sort();
        Ok(out)
    }
}

/// Answers a friend's question with this device's `friends`, the asker left out.
pub fn mutual_answer(
    asked: &[Point],
    friends: &[DevicePublicKey],
) -> Result<MutualAnswer, FriendError> {
    if asked.len() > MAX_MUTUAL_POINTS {
        return Err(FriendError::Malformed);
    }
    let secret = random_scalar()?;
    let mut doubled = Vec::with_capacity(asked.len());
    for point in asked {
        doubled.push((decompress(point)? * secret).compress().0);
    }
    let hidden = hide(friends, &secret)?
        .into_iter()
        .map(|(_, p)| p)
        .collect();
    Ok(MutualAnswer { doubled, hidden })
}

#[cfg(test)]
mod tests {
    use fogata_crypto::DeviceIdentity;

    use super::*;

    fn keys(n: usize) -> Vec<DevicePublicKey> {
        (0..n)
            .map(|_| DeviceIdentity::generate().unwrap().public_key())
            .collect()
    }

    #[test]
    fn only_the_friends_in_common_come_out() {
        let shared = keys(3);
        let mut mine = keys(5);
        mine.extend_from_slice(&shared);
        let mut theirs = keys(20);
        theirs.extend_from_slice(&shared);

        let (ask, points) = MutualAsk::start(&mine).unwrap();
        assert_eq!(points.len(), 16, "8 friends padded to 16");
        let answer = mutual_answer(&points, &theirs).unwrap();
        assert_eq!(answer.hidden.len(), 32, "23 friends padded to 32");
        let mut expected = shared.clone();
        expected.sort();
        assert_eq!(ask.finish(&answer).unwrap(), expected);
    }

    #[test]
    fn nobody_in_common_and_nobody_at_all() {
        let (ask, points) = MutualAsk::start(&keys(4)).unwrap();
        let answer = mutual_answer(&points, &keys(4)).unwrap();
        assert!(ask.finish(&answer).unwrap().is_empty());

        let (ask, points) = MutualAsk::start(&[]).unwrap();
        let answer = mutual_answer(&points, &[]).unwrap();
        assert!(ask.finish(&answer).unwrap().is_empty());
    }

    #[test]
    fn the_same_friends_hide_differently_each_time() {
        let mine = keys(2);
        let (_, first) = MutualAsk::start(&mine).unwrap();
        let (_, second) = MutualAsk::start(&mine).unwrap();
        assert!(first.iter().all(|p| !second.contains(p)));
    }

    #[test]
    fn bad_answers_are_refused() {
        let (ask, points) = MutualAsk::start(&keys(2)).unwrap();
        let mut answer = mutual_answer(&points, &keys(2)).unwrap();
        answer.doubled.pop();
        assert_eq!(ask.finish(&answer), Err(FriendError::Malformed));
        let mut not_a_point = [0xFFu8; 32];
        not_a_point[31] = 0x7F;
        assert_eq!(
            mutual_answer(&[not_a_point], &[]),
            Err(FriendError::Malformed)
        );
    }
}
