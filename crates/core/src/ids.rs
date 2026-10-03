//! Block IDs (`^abc123`): format check and generation (file-format.md §3.4).

use std::collections::HashSet;

/// Length of a block ID.
pub const ID_LEN: usize = 6;
const ALPHABET: &[u8; 36] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// True for a valid block ID: exactly 6 characters of `[a-z0-9]`.
pub fn is_valid_block_id(s: &str) -> bool {
    s.len() == ID_LEN
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// Draws random IDs from `rng` until one is not in `existing`.
///
/// The RNG is injected so tests are deterministic; each call to `rng` must return a fresh `u64`.
pub fn generate_block_id(existing: &HashSet<String>, rng: &mut impl FnMut() -> u64) -> String {
    loop {
        let mut v = rng();
        let mut id = String::with_capacity(ID_LEN);
        for _ in 0..ID_LEN {
            let idx = (v % ALPHABET.len() as u64) as usize;
            v /= ALPHABET.len() as u64;
            id.push(char::from(ALPHABET.get(idx).copied().unwrap_or(b'a')));
        }
        if !existing.contains(&id) {
            return id;
        }
    }
}
