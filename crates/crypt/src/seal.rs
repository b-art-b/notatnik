//! A stream encrypted to a vault's public key (EN-5). Writing needs no password: a random file key is wrapped to
//! the public key with an ephemeral X25519 key pair, so only whoever unlocks the vault can read it.
//!
//! Layout (all numbers little endian): `NTNKSEAL` | version (1) | descriptor length (u32) | descriptor (the vault's
//! JSON, so the file opens with nothing but the password or recovery key) | comment length (u16) | comment (plain
//! text for the writer's own use, authenticated) | ephemeral public key (32) | wrap nonce (24) | wrapped file key
//! (48) | stream nonce (19) | chunks (see `stream.rs`). The chunks are bound to a hash of everything before them.

use std::io::{Read, Write};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroize;

use crate::{Error, Keys, Result, Vault, random, stream};

const MAGIC: &[u8; 8] = b"NTNKSEAL";
const VERSION: u8 = 1;
const MAX_DESCRIPTOR: usize = 64 * 1024;

/// What the start of a sealed file says, before anything is decrypted.
#[derive(Debug, Clone)]
pub struct SealedHeader {
    /// The vault it was sealed to; unlock it to read the file.
    pub vault: Vault,
    /// The writer's comment.
    pub comment: Vec<u8>,
    eph_pub: [u8; 32],
    wrap_nonce: [u8; 24],
    wrapped: [u8; 48],
    stream_nonce: [u8; 19],
    /// Everything up to and including the ephemeral key.
    head_a: Vec<u8>,
    /// Hash of the whole header: the chunks are bound to it.
    head_hash: [u8; 32],
}

fn wrap_key(shared: &[u8; 32], eph_pub: &[u8; 32], recipient: &[u8; 32]) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key("notatnik seal v1");
    h.update(shared);
    h.update(eph_pub);
    h.update(recipient);
    *h.finalize().as_bytes()
}

/// Encrypts everything from `plain` to `vault`'s public key and writes the sealed file to `out`.
pub fn seal(
    vault: &Vault,
    comment: &[u8],
    plain: &mut dyn Read,
    out: &mut dyn Write,
) -> Result<()> {
    let recipient = vault.public()?;
    let descriptor = vault.to_json();
    if descriptor.len() > MAX_DESCRIPTOR || comment.len() > u16::MAX as usize {
        return Err(Error::Unsupported("descriptor or comment too long".into()));
    }
    let eph = StaticSecret::from(random::<32>()?);
    let eph_pub = PublicKey::from(&eph);
    let shared = eph.diffie_hellman(&recipient);
    if !shared.was_contributory() {
        return Err(Error::Corrupt(
            "the vault's public key is not usable".into(),
        ));
    }
    let mut wk = wrap_key(shared.as_bytes(), eph_pub.as_bytes(), recipient.as_bytes());
    let mut file_key = random::<32>()?;
    let mut head_a = Vec::new();
    head_a.extend_from_slice(MAGIC);
    head_a.push(VERSION);
    head_a.extend_from_slice(&(descriptor.len() as u32).to_le_bytes());
    head_a.extend_from_slice(&descriptor);
    head_a.extend_from_slice(&(comment.len() as u16).to_le_bytes());
    head_a.extend_from_slice(comment);
    head_a.extend_from_slice(eph_pub.as_bytes());
    let wrap_nonce = random::<24>()?;
    let wrapped = XChaCha20Poly1305::new((&wk).into())
        .encrypt(
            XNonce::from_slice(&wrap_nonce),
            Payload {
                msg: &file_key,
                aad: &head_a,
            },
        )
        .map_err(|_| Error::Corrupt("could not wrap the file key".into()))?;
    wk.zeroize();
    let stream_nonce = random::<19>()?;
    let mut head = head_a;
    head.extend_from_slice(&wrap_nonce);
    head.extend_from_slice(&wrapped);
    head.extend_from_slice(&stream_nonce);
    out.write_all(&head)?;
    let aad = blake3::hash(&head);
    let r = stream::encrypt(&file_key, &stream_nonce, aad.as_bytes(), plain, out);
    file_key.zeroize();
    r
}

/// Reads a read-exactly buffer and keeps a copy for the header hash.
fn take(r: &mut dyn Read, n: usize, all: &mut Vec<u8>) -> Result<Vec<u8>> {
    let mut b = vec![0u8; n];
    r.read_exact(&mut b).map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            Error::Corrupt("the file ends inside its header".into())
        } else {
            Error::Io(e)
        }
    })?;
    all.extend_from_slice(&b);
    Ok(b)
}

/// Reads the header of a sealed file and leaves `r` at the first chunk.
pub fn read_header(r: &mut dyn Read) -> Result<SealedHeader> {
    let mut all = Vec::new();
    let magic = take(r, 8, &mut all)?;
    if magic != MAGIC {
        return Err(Error::Corrupt("not a sealed notatnik file".into()));
    }
    let version = take(r, 1, &mut all)?[0];
    if version != VERSION {
        return Err(Error::Unsupported(format!("sealed file version {version}")));
    }
    let dl = u32::from_le_bytes(take(r, 4, &mut all)?.try_into().unwrap_or_default()) as usize;
    if dl > MAX_DESCRIPTOR {
        return Err(Error::Corrupt("descriptor too long".into()));
    }
    let vault = Vault::from_json(&take(r, dl, &mut all)?)?;
    let cl = u16::from_le_bytes(take(r, 2, &mut all)?.try_into().unwrap_or_default()) as usize;
    let comment = take(r, cl, &mut all)?;
    let eph_pub: [u8; 32] = take(r, 32, &mut all)?.try_into().unwrap_or([0; 32]);
    let head_a = all.clone();
    let wrap_nonce: [u8; 24] = take(r, 24, &mut all)?.try_into().unwrap_or([0; 24]);
    let wrapped: [u8; 48] = take(r, 48, &mut all)?.try_into().unwrap_or([0; 48]);
    let stream_nonce: [u8; 19] = take(r, 19, &mut all)?.try_into().unwrap_or([0; 19]);
    let head_hash = *blake3::hash(&all).as_bytes();
    Ok(SealedHeader {
        vault,
        comment,
        eph_pub,
        wrap_nonce,
        wrapped,
        stream_nonce,
        head_a,
        head_hash,
    })
}

/// Decrypts a sealed file with the vault's unlocked keys and writes the plaintext to `out`. Nothing usable is
/// written for a file that was changed or cut off at the end: the error comes after the chunks before the damage
/// were written, so the caller should discard `out` on error.
pub fn open(keys: &Keys, r: &mut dyn Read, out: &mut dyn Write) -> Result<SealedHeader> {
    let h = read_header(r)?;
    if h.vault.key_id != keys.key_id() {
        return Err(Error::WrongSecret);
    }
    let shared = keys.secret().diffie_hellman(&PublicKey::from(h.eph_pub));
    if !shared.was_contributory() {
        return Err(Error::Corrupt("unusable ephemeral key".into()));
    }
    let recipient = h.vault.public()?;
    let mut wk = wrap_key(shared.as_bytes(), &h.eph_pub, recipient.as_bytes());
    let mut fk = XChaCha20Poly1305::new((&wk).into())
        .decrypt(
            XNonce::from_slice(&h.wrap_nonce),
            Payload {
                msg: &h.wrapped,
                aad: &h.head_a,
            },
        )
        .map_err(|_| Error::Corrupt("the header was changed".into()))?;
    wk.zeroize();
    let mut file_key = [0u8; 32];
    if fk.len() != 32 {
        return Err(Error::Corrupt("bad file key".into()));
    }
    file_key.copy_from_slice(&fk);
    fk.zeroize();
    let res = stream::decrypt(&file_key, &h.stream_nonce, &h.head_hash, r, out);
    file_key.zeroize();
    res?;
    Ok(h)
}
