// What kind of device the app runs on. The Rust shell of the phone and tablet app sets `__NOTATNIK_MOBILE__` before
// the page loads (`src-tauri/src/lib.rs`, the `device` plugin); on a computer and in the browser it is not there.
// A plain constant, not state: it never changes while the app runs.
//
// Used to leave out what a phone cannot do or has not got: the folder chooser and the free path field of the start
// screen (a graph lives in the app's own storage there) and the texts that name a path of a computer (AS2).
export const isMobile: boolean =
  typeof window !== 'undefined' &&
  (window as unknown as { __NOTATNIK_MOBILE__?: boolean }).__NOTATNIK_MOBILE__ === true
