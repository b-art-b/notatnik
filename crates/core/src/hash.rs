//! Small stable content hash (FNV-1a, 64 bit) used to detect stale edit operations.
//! Not cryptographic; the graph layer uses blake3 for file hashes.

/// Hex FNV-1a hash of `text`.
pub fn content_hash(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}
