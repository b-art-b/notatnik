//! Notatnik desktop entry point; the app itself is in `lib.rs` (the same code starts on iOS and Android).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    notatnik_desktop_lib::run();
}
