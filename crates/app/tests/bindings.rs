//! The TypeScript bindings in `ui/src/lib/api/bindings.ts` must match the Rust types.
//! `UPDATE_BINDINGS=1 cargo test -p notatnik-app --test bindings` (or `scripts/gen-bindings.sh`) rewrites the file.

use std::path::Path;

#[test]
fn bindings_are_up_to_date() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/lib/api/bindings.ts");
    let generated = notatnik_app::typescript_bindings();
    if std::env::var_os("UPDATE_BINDINGS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &generated).unwrap();
        return;
    }
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(
        current, generated,
        "ui/src/lib/api/bindings.ts is stale: run scripts/gen-bindings.sh and commit the result"
    );
}
