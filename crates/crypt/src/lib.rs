//! Encryption for notatnik (EN-1..EN-6): no file or network access, only bytes in and out.
//!
//! The pieces:
//! - [`Vault`]: a small public descriptor (JSON) with *key slots*. A random bundle of two secrets, the *data key*
//!   (symmetric, for files) and an X25519 *secret key* (for sealed backups), is wrapped once by a key derived
//!   from the password (Argon2id) and once by the *recovery key*. Either one opens the bundle. Changing the
//!   password only rewraps the bundle; no file is touched.
//! - [`Keys`]: the unlocked bundle. Wiped from memory when dropped. Encrypts files with the data key
//!   ([`Keys::encrypt`]) and hides names ([`Keys::file_name`]).
//! - [`seal`] and [`open`]: a stream encrypted to the vault's public key, so it can be written without a password
//!   and read only by whoever unlocks the vault. Used for backups.
//!
//! Format details: `docs/file-format.md` ("Encryption"). Primitives: Argon2id, XChaCha20-Poly1305 in 64 KiB
//! chunks (STREAM construction, so truncation and reordering are detected), X25519, BLAKE3 for key derivation.

mod recovery;
mod seal;
mod stream;

use std::fmt;

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, ZeroizeOnDrop};

pub use recovery::RecoveryKey;
pub use seal::{SealedHeader, open, read_header, seal};

/// What can go wrong.
#[derive(Debug)]
pub enum Error {
    /// The password or recovery key does not open the vault.
    WrongSecret,
    /// The data is not what it claims to be: damaged, cut off, or changed.
    Corrupt(String),
    /// A version or setting this build does not know.
    Unsupported(String),
    /// A recovery key that is not written correctly.
    BadRecoveryKey,
    /// Reading or writing failed.
    Io(std::io::Error),
    /// The system could not provide random numbers.
    Random(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::WrongSecret => write!(f, "wrong password or recovery key"),
            Error::Corrupt(m) => write!(f, "damaged or changed data: {m}"),
            Error::Unsupported(m) => write!(f, "not supported: {m}"),
            Error::BadRecoveryKey => write!(f, "the recovery key is not written correctly"),
            Error::Io(e) => write!(f, "{e}"),
            Error::Random(m) => write!(f, "no random numbers: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// Result of this crate.
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn random<const N: usize>() -> Result<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|e| Error::Random(e.to_string()))?;
    Ok(b)
}

pub(crate) fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub(crate) fn unhex<const N: usize>(s: &str) -> Result<[u8; N]> {
    let bad = || Error::Corrupt("bad hex".into());
    if s.len() != N * 2 || !s.is_ascii() {
        return Err(bad());
    }
    let mut out = [0u8; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| bad())?;
    }
    Ok(out)
}

fn unhex_vec(s: &str) -> Result<Vec<u8>> {
    if s.len() % 2 != 0 || !s.is_ascii() {
        return Err(Error::Corrupt("bad hex".into()));
    }
    (0..s.len() / 2)
        .map(|i| {
            u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
                .map_err(|_| Error::Corrupt("bad hex".into()))
        })
        .collect()
}

/// Argon2id cost. Higher is slower for an attacker and for you.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memory in KiB.
    pub m_kib: u32,
    /// Passes.
    pub t: u32,
    /// Lanes.
    pub p: u32,
}

impl Default for KdfParams {
    /// 64 MiB, 3 passes, 1 lane: about a quarter of a second on a laptop, and fine on a phone.
    fn default() -> Self {
        KdfParams {
            m_kib: 64 * 1024,
            t: 3,
            p: 1,
        }
    }
}

impl KdfParams {
    /// Far too cheap for real use: only for tests that create many vaults.
    pub fn insecure_for_tests() -> Self {
        KdfParams {
            m_kib: 8,
            t: 1,
            p: 1,
        }
    }

    /// The limits that keep a damaged or hostile descriptor from asking for gigabytes or hours.
    fn check(&self) -> Result<()> {
        let ok = (8..=1024 * 1024).contains(&self.m_kib)
            && (1..=64).contains(&self.t)
            && (1..=16).contains(&self.p)
            && self.m_kib >= 8 * self.p;
        if ok {
            Ok(())
        } else {
            Err(Error::Unsupported("key derivation settings".into()))
        }
    }
}

/// Which secret opens a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlotKind {
    /// The password.
    Password,
    /// The recovery key.
    Recovery,
}

/// The key bundle, wrapped by one secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    /// Which secret opens it.
    pub kind: SlotKind,
    /// Argon2 salt (hex); empty for a recovery slot.
    pub salt: String,
    /// XChaCha20-Poly1305 nonce (hex).
    pub nonce: String,
    /// The wrapped bundle (hex).
    pub wrapped: String,
}

/// The public descriptor of a vault. Safe to store next to the encrypted data: it holds no secret that works
/// without the password or the recovery key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vault {
    /// Descriptor version (1).
    pub version: u32,
    /// Names the data key (hex, 16 bytes), so a file says which key it needs.
    pub key_id: String,
    /// The public half of the sealing key pair (hex, 32 bytes).
    pub public_key: String,
    /// Cost settings of the password slot.
    pub kdf: KdfParams,
    /// The slots.
    pub slots: Vec<Slot>,
}

/// The unlocked secrets. Wiped when dropped.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Keys {
    data_key: [u8; 32],
    secret: [u8; 32],
}

impl fmt::Debug for Keys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Keys(..)")
    }
}

const BUNDLE_VERSION: u8 = 1;
const BUNDLE_LEN: usize = 1 + 32 + 32;

fn key_id_of(data_key: &[u8; 32]) -> [u8; 16] {
    let h = blake3::derive_key("notatnik 2026 key id", data_key);
    let mut id = [0u8; 16];
    id.copy_from_slice(&h[..16]);
    id
}

/// What a slot's wrapped bundle is bound to: the descriptor's identity and the kind of slot.
fn slot_aad(v: &Vault, kind: SlotKind) -> Vec<u8> {
    let mut a = b"notatnik slot v1".to_vec();
    a.extend_from_slice(v.key_id.as_bytes());
    a.extend_from_slice(v.public_key.as_bytes());
    a.push(kind as u8);
    a
}

fn password_kek(password: &str, salt: &[u8], p: &KdfParams) -> Result<[u8; 32]> {
    use unicode_normalization::UnicodeNormalization;
    p.check()?;
    // the same password typed on another keyboard or system may be composed differently
    let pw: String = password.nfc().collect();
    let params =
        Params::new(p.m_kib, p.t, p.p, Some(32)).map_err(|e| Error::Unsupported(e.to_string()))?;
    let mut out = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(pw.as_bytes(), salt, &mut out)
        .map_err(|e| Error::Unsupported(e.to_string()))?;
    Ok(out)
}

fn recovery_kek(key: &[u8; 32]) -> [u8; 32] {
    blake3::derive_key("notatnik recovery kek v1", key)
}

fn wrap(kek: &[u8; 32], keys: &Keys, aad: &[u8]) -> Result<([u8; 24], Vec<u8>)> {
    let mut bundle = [0u8; BUNDLE_LEN];
    bundle[0] = BUNDLE_VERSION;
    bundle[1..33].copy_from_slice(&keys.data_key);
    bundle[33..].copy_from_slice(&keys.secret);
    let nonce = random::<24>()?;
    let wrapped = XChaCha20Poly1305::new(kek.into())
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: &bundle, aad })
        .map_err(|_| Error::Corrupt("could not wrap the keys".into()))?;
    bundle.zeroize();
    Ok((nonce, wrapped))
}

impl Vault {
    /// Makes a new vault: fresh keys, a password slot and a recovery slot. Returns the descriptor, the unlocked
    /// keys and the recovery key, which must be shown to the user once.
    pub fn create(password: &str, params: KdfParams) -> Result<(Vault, Keys, RecoveryKey)> {
        params.check()?;
        let keys = Keys {
            data_key: random()?,
            secret: random()?,
        };
        let public = PublicKey::from(&StaticSecret::from(keys.secret));
        let mut vault = Vault {
            version: 1,
            key_id: hex(&key_id_of(&keys.data_key)),
            public_key: hex(public.as_bytes()),
            kdf: params,
            slots: Vec::new(),
        };
        vault.set_password(&keys, password, params)?;
        let recovery = vault.set_recovery(&keys)?;
        Ok((vault, keys, recovery))
    }

    fn put(&mut self, slot: Slot) {
        self.slots.retain(|s| s.kind != slot.kind);
        self.slots.push(slot);
    }

    fn set_password(&mut self, keys: &Keys, password: &str, params: KdfParams) -> Result<()> {
        let salt = random::<16>()?;
        let mut kek = password_kek(password, &salt, &params)?;
        self.kdf = params;
        let (nonce, wrapped) = wrap(&kek, keys, &slot_aad(self, SlotKind::Password))?;
        kek.zeroize();
        self.put(Slot {
            kind: SlotKind::Password,
            salt: hex(&salt),
            nonce: hex(&nonce),
            wrapped: hex(&wrapped),
        });
        Ok(())
    }

    fn set_recovery(&mut self, keys: &Keys) -> Result<RecoveryKey> {
        let recovery = RecoveryKey::generate()?;
        let mut kek = recovery_kek(recovery.bytes());
        let (nonce, wrapped) = wrap(&kek, keys, &slot_aad(self, SlotKind::Recovery))?;
        kek.zeroize();
        self.put(Slot {
            kind: SlotKind::Recovery,
            salt: String::new(),
            nonce: hex(&nonce),
            wrapped: hex(&wrapped),
        });
        Ok(recovery)
    }

    fn unwrap(&self, kind: SlotKind, kek: &[u8; 32]) -> Result<Keys> {
        let slot = self
            .slots
            .iter()
            .find(|s| s.kind == kind)
            .ok_or(Error::WrongSecret)?;
        let nonce = unhex::<24>(&slot.nonce)?;
        let mut bundle = XChaCha20Poly1305::new(kek.into())
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &unhex_vec(&slot.wrapped)?,
                    aad: &slot_aad(self, kind),
                },
            )
            .map_err(|_| Error::WrongSecret)?;
        if bundle.len() != BUNDLE_LEN || bundle[0] != BUNDLE_VERSION {
            bundle.zeroize();
            return Err(Error::Unsupported("key bundle version".into()));
        }
        let mut keys = Keys {
            data_key: [0; 32],
            secret: [0; 32],
        };
        keys.data_key.copy_from_slice(&bundle[1..33]);
        keys.secret.copy_from_slice(&bundle[33..]);
        bundle.zeroize();
        // the descriptor must describe these keys
        let public = PublicKey::from(&StaticSecret::from(keys.secret));
        if hex(&key_id_of(&keys.data_key)) != self.key_id
            || hex(public.as_bytes()) != self.public_key
        {
            return Err(Error::Corrupt(
                "the descriptor does not match its keys".into(),
            ));
        }
        Ok(keys)
    }

    /// Opens the vault with the password.
    pub fn unlock_password(&self, password: &str) -> Result<Keys> {
        let slot = self
            .slots
            .iter()
            .find(|s| s.kind == SlotKind::Password)
            .ok_or(Error::WrongSecret)?;
        let mut kek = password_kek(password, &unhex::<16>(&slot.salt)?, &self.kdf)?;
        let r = self.unwrap(SlotKind::Password, &kek);
        kek.zeroize();
        r
    }

    /// Opens the vault with the recovery key as the user wrote it down.
    pub fn unlock_recovery(&self, recovery_key: &str) -> Result<Keys> {
        let key: RecoveryKey = recovery_key.parse()?;
        let mut kek = recovery_kek(key.bytes());
        let r = self.unwrap(SlotKind::Recovery, &kek);
        kek.zeroize();
        r
    }

    /// Opens the vault with whatever the user typed: the password, or if that does not fit and it reads as a
    /// recovery key, the recovery key.
    pub fn unlock(&self, secret: &str) -> Result<Keys> {
        match self.unlock_password(secret) {
            Err(Error::WrongSecret) if secret.parse::<RecoveryKey>().is_ok() => {
                self.unlock_recovery(secret)
            }
            r => r,
        }
    }

    /// Replaces the password. `keys` must be this vault's unlocked keys. Nothing that was encrypted changes.
    pub fn change_password(
        &mut self,
        keys: &Keys,
        new_password: &str,
        params: KdfParams,
    ) -> Result<()> {
        self.check_keys(keys)?;
        params.check()?;
        self.set_password(keys, new_password, params)
    }

    /// Replaces the recovery key and returns the new one. The old one stops opening this descriptor (copies of
    /// older descriptors, for example inside old backups, keep theirs).
    pub fn new_recovery_key(&mut self, keys: &Keys) -> Result<RecoveryKey> {
        self.check_keys(keys)?;
        self.set_recovery(keys)
    }

    fn check_keys(&self, keys: &Keys) -> Result<()> {
        if hex(&key_id_of(&keys.data_key)) == self.key_id {
            Ok(())
        } else {
            Err(Error::WrongSecret)
        }
    }

    /// The descriptor as JSON.
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).unwrap_or_default()
    }

    /// Reads and checks a descriptor.
    pub fn from_json(bytes: &[u8]) -> Result<Vault> {
        let v: Vault = serde_json::from_slice(bytes)
            .map_err(|e| Error::Corrupt(format!("descriptor: {e}")))?;
        if v.version != 1 {
            return Err(Error::Unsupported(format!("vault version {}", v.version)));
        }
        v.kdf.check()?;
        unhex::<16>(&v.key_id)?;
        unhex::<32>(&v.public_key)?;
        if v.slots.len() > 8 {
            return Err(Error::Corrupt("too many slots".into()));
        }
        Ok(v)
    }

    /// The sealing public key.
    pub(crate) fn public(&self) -> Result<PublicKey> {
        Ok(PublicKey::from(unhex::<32>(&self.public_key)?))
    }
}

const FILE_MAGIC: &[u8; 8] = b"NTNKFILE";
const FILE_VERSION: u8 = 1;
const FILE_HEADER: usize = 8 + 1 + 16 + 19;

impl Keys {
    /// The key's name (hex), as written in the vault and in every file made with it.
    pub fn key_id(&self) -> String {
        hex(&key_id_of(&self.data_key))
    }

    pub(crate) fn secret(&self) -> StaticSecret {
        StaticSecret::from(self.secret)
    }

    fn file_key(&self) -> [u8; 32] {
        blake3::derive_key("notatnik file v1", &self.data_key)
    }

    /// A stable name for a path that reveals nothing about it: a keyed hash (hex, 32 characters). The same path
    /// gives the same name, so a changed file overwrites its old version. The path itself belongs inside the
    /// encrypted payload.
    pub fn file_name(&self, path: &str) -> String {
        let key = blake3::derive_key("notatnik name v1", &self.data_key);
        let h = blake3::keyed_hash(&key, path.as_bytes());
        hex(&h.as_bytes()[..16])
    }

    /// Encrypts one file's content. `aad` (for example the path) is bound to it: the result opens only with the
    /// same `aad`, so a file moved to another path is detected.
    pub fn encrypt(&self, plain: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
        let nonce = random::<19>()?;
        let mut out = Vec::with_capacity(FILE_HEADER + plain.len() + 64);
        out.extend_from_slice(FILE_MAGIC);
        out.push(FILE_VERSION);
        out.extend_from_slice(&key_id_of(&self.data_key));
        out.extend_from_slice(&nonce);
        let mut full_aad = out.clone();
        full_aad.extend_from_slice(aad);
        let mut key = self.file_key();
        stream::encrypt(&key, &nonce, &full_aad, &mut &plain[..], &mut out)?;
        key.zeroize();
        Ok(out)
    }

    /// Decrypts what [`Keys::encrypt`] made.
    pub fn decrypt(&self, data: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
        if data.len() < FILE_HEADER || &data[..8] != FILE_MAGIC {
            return Err(Error::Corrupt("not an encrypted file".into()));
        }
        if data[8] != FILE_VERSION {
            return Err(Error::Unsupported(format!("file version {}", data[8])));
        }
        if data[9..25] != key_id_of(&self.data_key) {
            return Err(Error::WrongSecret);
        }
        let mut nonce = [0u8; 19];
        nonce.copy_from_slice(&data[25..FILE_HEADER]);
        let mut full_aad = data[..FILE_HEADER].to_vec();
        full_aad.extend_from_slice(aad);
        let mut key = self.file_key();
        let mut out = Vec::with_capacity(data.len());
        let r = stream::decrypt(&key, &nonce, &full_aad, &mut &data[FILE_HEADER..], &mut out);
        key.zeroize();
        r?;
        Ok(out)
    }
}

/// The key a file was made with (hex), read from its header without decrypting; None when it is not one of ours.
pub fn file_key_id(data: &[u8]) -> Option<String> {
    (data.len() >= FILE_HEADER && &data[..8] == FILE_MAGIC).then(|| hex(&data[9..25]))
}

#[cfg(test)]
mod tests;
