//! Notatnik graph service: owns a graph folder on disk.
//!
//! Responsible for atomic writes (ST-4), watching external changes (ST-5),
//! conflict detection (ST-6) and keeping the index in sync with the files.
