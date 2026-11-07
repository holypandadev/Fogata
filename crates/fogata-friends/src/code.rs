//! Short friend codes, `FGT-XXXXX-XXXXX`: ten characters that someone can read out or type.
//!
//! A code is too short to carry a key. Both sides first stretch it with Argon2id, so that
//! DHT nodes that see the rendezvous points cannot simply try every code, and then run SPAKE2
//! with it: whoever does not know the code gets one guess per run, and the inviting device
//! stops answering after [`MAX_FAILED_RUNS`] failed runs.
//!
//! The stretch is salted with the UTC hour the code was made in, so that a table of stretched
//! codes computed in advance serves one hour only. The last bit of a code is the parity of
//! that hour, and the other 49 bits are random: whoever uses the code knows which hour to
//! stretch it with while the two clocks differ by less than about 25 minutes, and stretches
//! it once.
//!
//! Codes of an earlier format, eight characters under one fixed salt, are refused as not a
//! code: only builds before any release made them.

use std::fmt;

use argon2::{Algorithm, Argon2, Params, Version};
use fogata_crypto::{CryptoError, random_bytes};
use zeroize::Zeroizing;

use crate::{FriendError, secret::derive};

/// How long a code stays valid.
pub const CODE_LIFETIME_MS: u64 = 10 * 60 * 1000;
/// Failed runs after which the inviting device stops answering a code.
pub const MAX_FAILED_RUNS: u32 = 3;
/// Length of the window a code is stretched with: the UTC hour it was made in.
pub const CODE_WINDOW_SECS: u64 = 3600;
const SALT: &[u8] = b"fogata/v2/friend-code";
/// Argon2id cost: 64 MiB and three passes, a fraction of a second on a laptop.
const ARGON2_MEMORY_KIB: u32 = 64 * 1024;
const ARGON2_PASSES: u32 = 3;
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const CODE_LEN: usize = 10;

/// The ten Crockford base32 characters of a code, wiped from memory when dropped.
#[derive(Clone, PartialEq, Eq)]
pub struct FriendCode(Zeroizing<[u8; CODE_LEN]>);

impl FriendCode {
    /// A new code made at `unix_secs`: 49 random bits and the parity of the current hour.
    pub fn generate(unix_secs: u64) -> Result<Self, CryptoError> {
        let mut bytes = Zeroizing::new([0u8; 8]);
        random_bytes(bytes.as_mut())?;
        let bits = Zeroizing::new(u64::from_le_bytes(*bytes));
        let mut chars = Zeroizing::new([0u8; CODE_LEN]);
        for (i, c) in chars.iter_mut().enumerate() {
            let mut value = (*bits >> (5 * i)) & 31;
            if i == CODE_LEN - 1 {
                value = (value & !1) | ((unix_secs / CODE_WINDOW_SECS) % 2);
            }
            *c = CROCKFORD[value as usize];
        }
        Ok(Self(chars))
    }

    /// Reads a code as people type it: with or without the `FGT` prefix, dashes or spaces,
    /// in any case, and with O, I and L read as 0, 1 and 1.
    pub fn parse(text: &str) -> Result<Self, FriendError> {
        // Read into a place of its own size: a list that grew as it was read would leave
        // copies of the code behind.
        let mut read = Zeroizing::new([0u8; CODE_LEN + 3]);
        let mut n = 0;
        for c in text.bytes().filter(u8::is_ascii_alphanumeric) {
            let slot = read.get_mut(n).ok_or(FriendError::BadCode)?;
            *slot = c.to_ascii_uppercase();
            n += 1;
        }
        let mut chars = &mut read[..n];
        if n == CODE_LEN + 3 && chars.starts_with(b"FGT") {
            chars = &mut chars[3..];
        }
        for c in chars.iter_mut() {
            *c = match *c {
                b'O' => b'0',
                b'I' | b'L' => b'1',
                other => other,
            };
            if !CROCKFORD.contains(c) {
                return Err(FriendError::BadCode);
            }
        }
        let chars: &[u8; CODE_LEN] = (&*chars).try_into().map_err(|_| FriendError::BadCode)?;
        Ok(Self(Zeroizing::new(*chars)))
    }

    fn chars(&self) -> &[u8] {
        &self.0[..]
    }

    /// The hour (counted from the Unix epoch) a code used at `unix_secs` was made in: of the
    /// hours with the code's parity, the one nearest to the middle of the code's lifetime
    /// before `unix_secs`. Right for clocks that differ by up to about 25 minutes.
    fn window(&self, unix_secs: u64) -> u64 {
        let last = CROCKFORD
            .iter()
            .position(|c| *c == self.0[CODE_LEN - 1])
            .unwrap_or_default() as u64;
        let middle = unix_secs.saturating_sub(CODE_LIFETIME_MS / 2000);
        let hour = middle / CODE_WINDOW_SECS;
        if hour % 2 == last % 2 {
            hour
        } else if middle % CODE_WINDOW_SECS >= CODE_WINDOW_SECS / 2 {
            hour + 1
        } else {
            hour.saturating_sub(1)
        }
    }
}

impl fmt::Display for FriendCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = std::str::from_utf8(self.chars()).map_err(|_| fmt::Error)?;
        let half = text.len() / 2;
        write!(f, "FGT-{}-{}", &text[..half], &text[half..])
    }
}

impl fmt::Debug for FriendCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FriendCode(..)")
    }
}

/// A stretched code. Making one is slow on purpose; do it away from the network thread.
#[derive(Clone)]
pub struct CodeSecrets {
    code: FriendCode,
    master: Zeroizing<[u8; 32]>,
}

impl fmt::Debug for CodeSecrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CodeSecrets(..)")
    }
}

impl CodeSecrets {
    /// Stretches `code`, made or used at `unix_secs`.
    pub fn stretch(code: &FriendCode, unix_secs: u64) -> Self {
        let mut salt = Vec::with_capacity(SALT.len() + 8);
        salt.extend_from_slice(SALT);
        salt.extend_from_slice(&code.window(unix_secs).to_be_bytes());
        let params = Params::new(ARGON2_MEMORY_KIB, ARGON2_PASSES, 1, Some(32))
            .expect("valid Argon2 parameters");
        let mut master = Zeroizing::new([0u8; 32]);
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into(code.chars(), &salt, master.as_mut())
            .expect("valid Argon2 input");
        Self {
            code: code.clone(),
            master,
        }
    }

    pub fn code(&self) -> &FriendCode {
        &self.code
    }

    /// The secret behind the rendezvous points of this code.
    pub fn rendezvous(&self) -> [u8; 32] {
        *derive(SALT, self.master.as_ref(), b"rendezvous")
    }

    pub(crate) fn gate_key(&self) -> Zeroizing<[u8; 32]> {
        derive(SALT, self.master.as_ref(), b"gate")
    }

    pub(crate) fn password(&self) -> Zeroizing<[u8; 32]> {
        derive(SALT, self.master.as_ref(), b"spake2")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-27 13:00:00 UTC, the start of an odd hour.
    const HOUR_START: u64 = 1_793_106_000;

    /// What makes a code is wiped from memory when the code goes.
    #[test]
    fn a_code_is_kept_where_it_is_wiped() {
        let code = FriendCode::generate(0).unwrap();
        let _: &Zeroizing<[u8; CODE_LEN]> = &code.0;
    }

    #[test]
    fn codes_print_and_parse() {
        let code = FriendCode::generate(HOUR_START).unwrap();
        let text = code.to_string();
        assert_eq!(text.len(), 15);
        assert!(text.starts_with("FGT-"));
        assert_eq!(&text[9..10], "-");
        assert_eq!(FriendCode::parse(&text).unwrap(), code);
        assert_eq!(
            FriendCode::parse(&text.to_lowercase().replace('-', " ")).unwrap(),
            code
        );
        assert_eq!(FriendCode::parse(&text[4..]).unwrap(), code);
        assert_eq!(
            FriendCode::parse("fgt-oil0a-bcdef").unwrap(),
            FriendCode::parse("0110ABCDEF").unwrap()
        );
        // A code whose own characters start with FGT.
        let fgt = FriendCode::parse("FGT45-67890").unwrap();
        assert_eq!(fgt.to_string(), "FGT-FGT45-67890");
        assert_eq!(FriendCode::parse("FGT-FGT45-67890").unwrap(), fgt);
        assert_eq!(FriendCode::parse("FGT-1234"), Err(FriendError::BadCode));
        assert_eq!(
            FriendCode::parse("FGT-12345-6789"),
            Err(FriendError::BadCode)
        );
        assert_eq!(
            FriendCode::parse("FGT-12345-678901"),
            Err(FriendError::BadCode)
        );
        assert_eq!(
            FriendCode::parse("FGT-UUUUU-12345"),
            Err(FriendError::BadCode)
        );
        // Far more than a code: refused once it is longer than any.
        let long = format!("{text} {}", "A".repeat(10_000));
        assert_eq!(FriendCode::parse(&long), Err(FriendError::BadCode));
        assert_eq!(format!("{code:?}"), "FriendCode(..)");
    }

    #[test]
    fn codes_of_the_earlier_format_are_not_codes() {
        for text in ["FGT-7Q4M-K2XR", "7q4m k2xr", "fgt-oil0-abcd", "0110ABCD"] {
            assert_eq!(FriendCode::parse(text), Err(FriendError::BadCode), "{text}");
        }
    }

    #[test]
    fn codes_carry_the_parity_of_their_hour() {
        for hour in 0..8 {
            for _ in 0..8 {
                let code = FriendCode::generate(HOUR_START + hour * CODE_WINDOW_SECS + 17).unwrap();
                let last = code.chars()[CODE_LEN - 1];
                let value = CROCKFORD.iter().position(|c| *c == last).unwrap() as u64;
                assert_eq!(value % 2, (HOUR_START / CODE_WINDOW_SECS + hour) % 2);
            }
        }
    }

    #[test]
    fn the_hour_a_code_was_made_in_is_found_from_when_it_is_used() {
        let lifetime = CODE_LIFETIME_MS / 1000;
        let skew = 20 * 60;
        // Made at every minute of two hours, the seconds just before and after the hour
        // included, and used at any moment of its lifetime by a clock up to 20 minutes off
        // either way.
        let made_at = (0..2 * CODE_WINDOW_SECS)
            .step_by(60)
            .chain([
                CODE_WINDOW_SECS - 1,
                CODE_WINDOW_SECS,
                2 * CODE_WINDOW_SECS - 1,
            ])
            .map(|s| HOUR_START + s);
        for made in made_at {
            let code = FriendCode::generate(made).unwrap();
            let hour = made / CODE_WINDOW_SECS;
            assert_eq!(code.window(made), hour, "made at {made}");
            for used in [made, made + 1, made + lifetime / 2, made + lifetime] {
                for clock in [used - skew, used, used + skew] {
                    assert_eq!(code.window(clock), hour, "made {made}, used {clock}");
                }
            }
        }
    }

    #[test]
    fn a_code_made_just_before_the_hour_meets_its_user_after_it() {
        let made = HOUR_START + CODE_WINDOW_SECS - 2;
        let code = FriendCode::generate(made).unwrap();
        let shown = CodeSecrets::stretch(&code, made);
        let used = CodeSecrets::stretch(&code, made + 90);
        assert_eq!(shown.rendezvous(), used.rendezvous());
        assert_eq!(*shown.password(), *used.password());
        // Another hour of the same parity gives other secrets: a table of one hour does not
        // serve the next ones.
        let later = CodeSecrets::stretch(&code, made + 2 * CODE_WINDOW_SECS);
        assert_ne!(shown.rendezvous(), later.rendezvous());
    }

    #[test]
    fn stretching_is_deterministic_per_code() {
        let a = FriendCode::parse("FGT-AAAAA-00000").unwrap();
        let b = FriendCode::parse("FGT-AAAAA-00002").unwrap();
        let sa = CodeSecrets::stretch(&a, HOUR_START + 1);
        assert_eq!(
            sa.rendezvous(),
            CodeSecrets::stretch(&a, HOUR_START + 600).rendezvous()
        );
        assert_ne!(
            sa.rendezvous(),
            CodeSecrets::stretch(&b, HOUR_START + 1).rendezvous()
        );
        assert_ne!(*sa.gate_key(), *sa.password());
    }
}
