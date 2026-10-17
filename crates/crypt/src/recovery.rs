//! The recovery key: 32 random bytes written as 56 letters and digits in 14 groups of four, with a 3 byte check
//! so a typing mistake is reported instead of being taken for a wrong key.

use std::fmt;
use std::str::FromStr;

use crate::{Error, Result, random};

const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// A recovery key. Shown once when it is made; whoever has it can open the vault.
#[derive(Clone, PartialEq, Eq)]
pub struct RecoveryKey([u8; 32]);

impl RecoveryKey {
    pub(crate) fn generate() -> Result<RecoveryKey> {
        Ok(RecoveryKey(random()?))
    }

    pub(crate) fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

fn check(bytes: &[u8; 32]) -> [u8; 3] {
    let h = blake3::derive_key("notatnik recovery check v1", bytes);
    [h[0], h[1], h[2]]
}

impl fmt::Display for RecoveryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut data = self.0.to_vec();
        data.extend_from_slice(&check(&self.0));
        // 35 bytes = 280 bits = exactly 56 base32 characters
        let (mut acc, mut bits, mut out) = (0u32, 0, String::new());
        for b in data {
            acc = (acc << 8) | u32::from(b);
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                out.push(char::from(ALPHABET[((acc >> bits) & 31) as usize]));
            }
        }
        for (i, c) in out.chars().enumerate() {
            if i > 0 && i % 4 == 0 {
                write!(f, "-")?;
            }
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for RecoveryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RecoveryKey(..)")
    }
}

impl FromStr for RecoveryKey {
    type Err = Error;

    /// Accepts groups separated by dashes or spaces, in any case.
    fn from_str(s: &str) -> Result<RecoveryKey> {
        let chars: Vec<u8> = s
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .map(|c| c.to_ascii_uppercase() as u8)
            .collect();
        if chars.len() != 56 {
            return Err(Error::BadRecoveryKey);
        }
        let (mut acc, mut bits, mut data) = (0u32, 0, Vec::with_capacity(35));
        for c in chars {
            let v = ALPHABET
                .iter()
                .position(|a| *a == c)
                .ok_or(Error::BadRecoveryKey)? as u32;
            acc = (acc << 5) | v;
            bits += 5;
            if bits >= 8 {
                bits -= 8;
                data.push(((acc >> bits) & 0xff) as u8);
            }
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&data[..32]);
        if data[32..] != check(&key) {
            return Err(Error::BadRecoveryKey);
        }
        Ok(RecoveryKey(key))
    }
}
