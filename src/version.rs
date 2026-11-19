//! `kioku version`: the version and a fingerprint of what the install step sets up (the
//! installer itself, the skill names, the skeleton files), which changes exactly when an update
//! needs the install step to run again.

use sha2::{Digest, Sha256};
use std::path::Path;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The installer's code is part of the binary; its source text stands for it in the fingerprint.
const INSTALLER: &str = include_str!("install.rs");

pub fn fingerprint(sys: &Path) -> String {
    let mut h = Sha256::new();
    h.update(INSTALLER.as_bytes());
    let mut names: Vec<String> = ["skills", "skeleton"]
        .iter()
        .flat_map(|d| {
            walkdir::WalkDir::new(sys.join(d))
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .map(|e| crate::util::rel(sys, e.path()))
                .collect::<Vec<_>>()
        })
        .collect();
    names.sort();
    for n in names {
        h.update(n.as_bytes());
        h.update(b"\n");
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    hex[..12].to_string()
}

pub fn line(sys: &Path) -> String {
    format!("{VERSION} {}", fingerprint(sys))
}
