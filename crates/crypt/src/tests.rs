use super::*;

fn vault() -> (Vault, Keys, RecoveryKey) {
    Vault::create("correct horse", KdfParams::insecure_for_tests()).unwrap()
}

fn pattern(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 31 % 251) as u8).collect()
}

#[test]
fn password_and_recovery_key_open_the_same_keys() {
    let (v, keys, rk) = vault();
    let by_pw = v.unlock_password("correct horse").unwrap();
    let by_rk = v.unlock_recovery(&rk.to_string()).unwrap();
    assert_eq!(by_pw.key_id(), keys.key_id());
    assert_eq!(by_rk.key_id(), keys.key_id());
    assert_eq!(by_pw.file_name("pages/a.md"), keys.file_name("pages/a.md"));
    // `unlock` takes either
    assert!(v.unlock("correct horse").is_ok());
    assert!(v.unlock(&rk.to_string()).is_ok());
    assert!(matches!(v.unlock("wrong"), Err(Error::WrongSecret)));
    assert!(matches!(
        v.unlock_password("correct horse "),
        Err(Error::WrongSecret)
    ));
}

#[test]
fn the_recovery_key_is_readable_and_checked() {
    let (_, _, rk) = vault();
    let s = rk.to_string();
    assert_eq!(s.len(), 56 + 13, "14 groups of four, 13 dashes: {s}");
    assert_eq!(s.split('-').count(), 14);
    // dashes, spaces and case do not matter
    let loose = s.replace('-', " ").to_lowercase();
    assert_eq!(loose.parse::<RecoveryKey>().unwrap(), rk);
    // a typing mistake is reported as such
    let mut typo: Vec<char> = s.chars().collect();
    typo[0] = if typo[0] == 'A' { 'B' } else { 'A' };
    let typo: String = typo.into_iter().collect();
    assert!(matches!(
        typo.parse::<RecoveryKey>(),
        Err(Error::BadRecoveryKey)
    ));
    assert!("short".parse::<RecoveryKey>().is_err());
    assert!(
        "0000-0000-0000-0000-0000-0000-0000-0000-0000-0000-0000-0000-0000-0000"
            .parse::<RecoveryKey>()
            .is_err()
    );
}

#[test]
fn a_changed_password_keeps_the_keys_and_the_old_one_stops_working() {
    let (mut v, keys, rk) = vault();
    let sealed = keys.encrypt(b"hello", b"p").unwrap();
    v.change_password(&keys, "new secret", KdfParams::insecure_for_tests())
        .unwrap();
    assert!(matches!(v.unlock("correct horse"), Err(Error::WrongSecret)));
    let again = v.unlock_password("new secret").unwrap();
    // files made before still open: nothing was re-encrypted
    assert_eq!(again.decrypt(&sealed, b"p").unwrap(), b"hello");
    // the recovery key still works
    assert!(v.unlock_recovery(&rk.to_string()).is_ok());
    // keys of another vault cannot change this one
    let (_, other, _) = vault();
    assert!(matches!(
        v.change_password(&other, "x", KdfParams::insecure_for_tests()),
        Err(Error::WrongSecret)
    ));
}

#[test]
fn a_new_recovery_key_replaces_the_old_one() {
    let (mut v, keys, old) = vault();
    let new = v.new_recovery_key(&keys).unwrap();
    assert_ne!(new, old);
    assert!(v.unlock_recovery(&new.to_string()).is_ok());
    assert!(matches!(
        v.unlock_recovery(&old.to_string()),
        Err(Error::WrongSecret)
    ));
    assert!(v.unlock_password("correct horse").is_ok());
}

#[test]
fn the_same_password_in_another_unicode_form_opens_the_vault() {
    let composed = "caf\u{e9}-haslo-\u{142}\u{f3}d\u{17a}";
    let decomposed = "cafe\u{301}-haslo-\u{142}o\u{301}dz\u{301}";
    assert_ne!(composed, decomposed);
    let (v, _, _) = Vault::create(composed, KdfParams::insecure_for_tests()).unwrap();
    assert!(v.unlock_password(decomposed).is_ok());
}

#[test]
fn a_descriptor_survives_json_and_rejects_damage() {
    let (v, _, _) = vault();
    let back = Vault::from_json(&v.to_json()).unwrap();
    assert_eq!(back, v);
    // an absurd cost is refused before anything is computed
    let mut huge = v.clone();
    huge.kdf.m_kib = 64 * 1024 * 1024;
    assert!(Vault::from_json(&huge.to_json()).is_err());
    assert!(Vault::from_json(b"{}").is_err());
    let mut v2 = v.clone();
    v2.version = 2;
    assert!(matches!(
        Vault::from_json(&v2.to_json()),
        Err(Error::Unsupported(_))
    ));
    // a slot moved into another descriptor does not open it (its keys are bound to the descriptor)
    let (other, _, _) = vault();
    let mut grafted = v.clone();
    grafted.key_id = other.key_id.clone();
    assert!(grafted.unlock_password("correct horse").is_err());
    let mut grafted = v.clone();
    grafted.public_key = other.public_key.clone();
    assert!(grafted.unlock_password("correct horse").is_err());
}

#[test]
fn files_round_trip_in_every_chunk_shape() {
    let (_, keys, _) = vault();
    for n in [
        0,
        1,
        100,
        stream::CHUNK - 1,
        stream::CHUNK,
        stream::CHUNK + 1,
        3 * stream::CHUNK,
        3 * stream::CHUNK + 17,
    ] {
        let plain = pattern(n);
        let enc = keys.encrypt(&plain, b"pages/a.md").unwrap();
        assert!(
            !enc.windows(32)
                .any(|w| plain.len() >= 32 && w == &plain[..32]),
            "{n}: plaintext visible"
        );
        assert_eq!(keys.decrypt(&enc, b"pages/a.md").unwrap(), plain, "{n}");
    }
}

#[test]
fn a_file_is_bound_to_its_path_its_key_and_its_end() {
    let (_, keys, _) = vault();
    let plain = pattern(2 * stream::CHUNK + 5);
    let enc = keys.encrypt(&plain, b"pages/a.md").unwrap();
    assert!(
        keys.decrypt(&enc, b"pages/b.md").is_err(),
        "moved to another path"
    );
    let (_, other, _) = vault();
    assert!(matches!(
        other.decrypt(&enc, b"pages/a.md"),
        Err(Error::WrongSecret)
    ));
    assert_eq!(file_key_id(&enc).unwrap(), keys.key_id());
    // a flipped byte anywhere is detected
    for at in [0, 12, 30, 100, enc.len() / 2, enc.len() - 1] {
        let mut bad = enc.clone();
        bad[at] ^= 1;
        assert!(keys.decrypt(&bad, b"pages/a.md").is_err(), "byte {at}");
    }
    // cut off at a chunk boundary, and extended
    let boundary = FILE_HEADER + stream::CHUNK + 16;
    assert!(keys.decrypt(&enc[..boundary], b"pages/a.md").is_err());
    assert!(keys.decrypt(&enc[..enc.len() - 1], b"pages/a.md").is_err());
    let mut longer = enc.clone();
    longer.push(0);
    assert!(keys.decrypt(&longer, b"pages/a.md").is_err());
    assert!(keys.decrypt(b"short", b"").is_err());
    // the same text encrypted twice differs (random nonce)
    assert_ne!(enc, keys.encrypt(&plain, b"pages/a.md").unwrap());
}

#[test]
fn names_hide_the_path_and_are_stable() {
    let (_, keys, _) = vault();
    let a = keys.file_name("pages/Secret plans.md");
    assert_eq!(a.len(), 32);
    assert!(!a.contains("ecret"));
    assert_eq!(a, keys.file_name("pages/Secret plans.md"));
    assert_ne!(a, keys.file_name("pages/Secret plans 2.md"));
    let (_, other, _) = vault();
    assert_ne!(a, other.file_name("pages/Secret plans.md"));
}

fn sealed(v: &Vault, comment: &[u8], plain: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    seal(v, comment, &mut &plain[..], &mut out).unwrap();
    out
}

fn opened(keys: &Keys, data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    open(keys, &mut &data[..], &mut out)?;
    Ok(out)
}

#[test]
fn sealed_data_opens_with_the_unlocked_vault_only() {
    let (v, keys, rk) = vault();
    for n in [0, 1, stream::CHUNK, 2 * stream::CHUNK + 3] {
        let plain = pattern(n);
        let enc = sealed(&v, b"fp-123", &plain);
        // written without any secret; opened by password or recovery key
        let by_pw = v.unlock_password("correct horse").unwrap();
        assert_eq!(opened(&by_pw, &enc).unwrap(), plain, "{n}");
        let by_rk = v.unlock_recovery(&rk.to_string()).unwrap();
        assert_eq!(opened(&by_rk, &enc).unwrap(), plain, "{n}");
        assert_eq!(opened(&keys, &enc).unwrap(), plain);
    }
    // the header is readable without a key; the descriptor inside opens the file on its own
    let enc = sealed(&v, b"fp-123", b"secret");
    let h = read_header(&mut &enc[..]).unwrap();
    assert_eq!(h.comment, b"fp-123");
    let from_file = h.vault.unlock("correct horse").unwrap();
    assert_eq!(opened(&from_file, &enc).unwrap(), b"secret");
    assert!(!enc.windows(6).any(|w| w == b"secret"));
    // another vault's keys do not open it
    let (_, other, _) = vault();
    assert!(matches!(opened(&other, &enc), Err(Error::WrongSecret)));
}

#[test]
fn sealed_data_detects_every_kind_of_damage() {
    let (v, keys, _) = vault();
    let plain = pattern(2 * stream::CHUNK + 9);
    let enc = sealed(&v, b"comment", &plain);
    let header_len = {
        let mut r = &enc[..];
        read_header(&mut r).unwrap();
        enc.len() - r.len()
    };
    // any single flipped byte, in the header and in the body, is detected
    let mut spots = vec![
        0,
        8,
        9,
        20,
        header_len - 1,
        header_len,
        header_len + 5,
        enc.len() - 1,
    ];
    spots.push(header_len - 19 - 48 - 24 - 32 - 3); // inside the comment
    for at in spots {
        let mut bad = enc.clone();
        bad[at] ^= 1;
        assert!(opened(&keys, &bad).is_err(), "byte {at}");
    }
    // cut off at a chunk boundary, one byte short, extended
    let boundary = header_len + stream::CHUNK + 16;
    assert!(opened(&keys, &enc[..boundary]).is_err());
    assert!(opened(&keys, &enc[..enc.len() - 1]).is_err());
    let mut longer = enc.clone();
    longer.push(7);
    assert!(opened(&keys, &longer).is_err());
    assert!(opened(&keys, &enc[..10]).is_err());
    assert!(opened(&keys, b"not a sealed file at all").is_err());
}

#[test]
fn the_default_cost_is_accepted_and_opens_the_vault() {
    let (v, _, _) = Vault::create("pw", KdfParams::default()).unwrap();
    assert!(v.unlock_password("pw").is_ok());
    assert!(KdfParams::default().check().is_ok());
}

/// The formats must stay readable: `fixtures/crypt/` holds a vault, a file and a sealed file made by an earlier
/// build. Regenerate them (only when the format version changes on purpose) with
/// `NOTATNIK_WRITE_FIXTURES=1 cargo test -p notatnik-crypt fixtures`.
#[test]
fn fixtures_from_an_earlier_build_still_open() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/crypt");
    let password = "fixture password";
    let plain = b"- a block\n- another one with \xc5\xbc\xc3\xb3\xc5\x82w\n".to_vec();
    if std::env::var_os("NOTATNIK_WRITE_FIXTURES").is_some() {
        let (v, keys, rk) = Vault::create(password, KdfParams::insecure_for_tests()).unwrap();
        std::fs::write(dir.join("vault.json"), v.to_json()).unwrap();
        std::fs::write(dir.join("recovery-key.txt"), format!("{rk}\n")).unwrap();
        std::fs::write(
            dir.join("file.enc"),
            keys.encrypt(&plain, b"pages/a.md").unwrap(),
        )
        .unwrap();
        std::fs::write(dir.join("sealed.bin"), sealed(&v, b"fixture", &plain)).unwrap();
    }
    let v = Vault::from_json(&std::fs::read(dir.join("vault.json")).unwrap()).unwrap();
    let keys = v.unlock_password(password).unwrap();
    let rk = std::fs::read_to_string(dir.join("recovery-key.txt")).unwrap();
    assert_eq!(v.unlock_recovery(&rk).unwrap().key_id(), keys.key_id());
    let file = std::fs::read(dir.join("file.enc")).unwrap();
    assert_eq!(keys.decrypt(&file, b"pages/a.md").unwrap(), plain);
    let sealed_file = std::fs::read(dir.join("sealed.bin")).unwrap();
    assert_eq!(opened(&keys, &sealed_file).unwrap(), plain);
}

#[test]
fn exported_keys_open_the_same_vault_again_and_only_that_one() {
    let (vault, keys, _) = Vault::create("pw", KdfParams::insecure_for_tests()).unwrap();
    let text = keys.export_hex();
    let again = vault.unlock_exported(&text).unwrap();
    let sealed = keys.encrypt(b"hello", b"p").unwrap();
    assert_eq!(again.decrypt(&sealed, b"p").unwrap(), b"hello");
    // another vault, damaged or empty text: refused
    let (other, _, _) = Vault::create("pw", KdfParams::insecure_for_tests()).unwrap();
    assert!(other.unlock_exported(&text).is_err());
    assert!(vault.unlock_exported("").is_err());
    assert!(vault.unlock_exported("zz").is_err());
    assert!(vault.unlock_exported(&text[..text.len() - 2]).is_err());
}

#[test]
fn a_named_file_carries_its_path_and_opens_only_as_one() {
    let (_, keys, _) = vault();
    let (_, other, _) = vault();
    for (path, body) in [
        ("pages/a.md", pattern(10)),
        ("pages/Zażółć gęślą jaźń/日本語 😀.md", pattern(0)),
        ("a", pattern(70_000)),
    ] {
        let sealed = keys.encrypt_named(path, &body).unwrap();
        assert!(path.len() < 4 || !sealed.windows(path.len()).any(|w| w == path.as_bytes()));
        assert_eq!(
            keys.decrypt_named(&sealed).unwrap(),
            (path.to_string(), body)
        );
    }
    let sealed = keys.encrypt_named("pages/a.md", b"text").unwrap();
    // another key, a changed byte, and a file that is not a named one
    assert!(other.decrypt_named(&sealed).is_err());
    let mut bad = sealed.clone();
    let last = bad.len() - 1;
    bad[last] ^= 1;
    assert!(keys.decrypt_named(&bad).is_err());
    let plain_file = keys.encrypt(b"\x00\x05hello", b"pages/a.md").unwrap();
    assert!(keys.decrypt_named(&plain_file).is_err());
    assert!(
        keys.decrypt_named(&keys.encrypt(b"\xff", HIDDEN_AAD).unwrap())
            .is_err()
    );
    // a path too long for the length field
    assert!(keys.encrypt_named(&"x".repeat(70_000), b"").is_err());
}
