//! Chunked authenticated encryption (STREAM, XChaCha20-Poly1305): 64 KiB of plaintext per chunk, the last chunk
//! marked as last, so a file that is cut off, extended or reordered fails to decrypt instead of opening partly.

use std::io::{Read, Write};

use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::generic_array::GenericArray;
use chacha20poly1305::aead::stream::{DecryptorBE32, EncryptorBE32};
use chacha20poly1305::aead::{KeyInit, Payload};

use crate::{Error, Result};

/// Plaintext bytes per chunk.
pub(crate) const CHUNK: usize = 64 * 1024;
const TAG: usize = 16;

/// Reads until `buf` is full or the input ends; returns the bytes read.
fn fill(r: &mut dyn Read, buf: &mut Vec<u8>, want: usize) -> Result<()> {
    buf.clear();
    buf.resize(want, 0);
    let mut n = 0;
    while n < want {
        let k = r.read(&mut buf[n..])?;
        if k == 0 {
            break;
        }
        n += k;
    }
    buf.truncate(n);
    Ok(())
}

pub(crate) fn encrypt(
    key: &[u8; 32],
    nonce: &[u8; 19],
    aad: &[u8],
    r: &mut dyn Read,
    w: &mut dyn Write,
) -> Result<()> {
    let mut enc = EncryptorBE32::from_aead(
        XChaCha20Poly1305::new(key.into()),
        GenericArray::from_slice(nonce),
    );
    let (mut cur, mut next) = (Vec::new(), Vec::new());
    fill(r, &mut cur, CHUNK)?;
    loop {
        // one chunk of look-ahead tells whether `cur` is the last
        fill(r, &mut next, CHUNK)?;
        let payload = Payload { msg: &cur, aad };
        if next.is_empty() {
            let ct = enc
                .encrypt_last(payload)
                .map_err(|_| Error::Corrupt("encryption failed".into()))?;
            w.write_all(&ct)?;
            return Ok(());
        }
        let ct = enc
            .encrypt_next(payload)
            .map_err(|_| Error::Corrupt("encryption failed".into()))?;
        w.write_all(&ct)?;
        std::mem::swap(&mut cur, &mut next);
    }
}

pub(crate) fn decrypt(
    key: &[u8; 32],
    nonce: &[u8; 19],
    aad: &[u8],
    r: &mut dyn Read,
    w: &mut dyn Write,
) -> Result<()> {
    let mut dec = DecryptorBE32::from_aead(
        XChaCha20Poly1305::new(key.into()),
        GenericArray::from_slice(nonce),
    );
    let bad =
        || Error::Corrupt("a chunk does not check out (wrong key, changed or cut off)".into());
    let (mut cur, mut next) = (Vec::new(), Vec::new());
    fill(r, &mut cur, CHUNK + TAG)?;
    loop {
        fill(r, &mut next, CHUNK + TAG)?;
        let payload = Payload { msg: &cur, aad };
        if next.is_empty() {
            let pt = dec.decrypt_last(payload).map_err(|_| bad())?;
            w.write_all(&pt)?;
            return Ok(());
        }
        let pt = dec.decrypt_next(payload).map_err(|_| bad())?;
        w.write_all(&pt)?;
        std::mem::swap(&mut cur, &mut next);
    }
}
