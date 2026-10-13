//! The bundled font gate: every file under `fonts/` is the exact one its manifest pins, with its license beside it.

use crate::{fs, portable};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Where the bundled fonts and their manifest live.
pub const DIRECTORY: &str = "fonts";
pub const MANIFEST: &str = "fonts/manifest.toml";
/// The suffix of the directory's own documents, which the manifest does not pin.
const TOML_SUFFIX: &str = ".toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub font: Vec<Font>,
    pub license: Vec<License>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Font {
    pub family: String,
    pub file: String,
    pub sha256: String,
    pub license: String,
    pub source: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct License {
    pub file: String,
    pub sha256: String,
}

const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
const NIBBLE_BITS: u32 = 4;
const LOW_NIBBLE: u8 = 0x0f;

/// The lowercase hexadecimal SHA-256 of `bytes`.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    let mut text = String::new();
    for byte in Sha256::digest(bytes) {
        for nibble in [byte.wrapping_shr(NIBBLE_BITS), byte & LOW_NIBBLE] {
            if let Some(digit) = HEX_DIGITS.get(usize::from(nibble)) {
                text.push(char::from(*digit));
            }
        }
    }
    text
}

/// Every way the bundled fonts disagree with their manifest.
///
/// # Errors
///
/// Returns an error when the manifest or a file cannot be read.
pub fn findings(root: &Path) -> Result<Vec<String>> {
    let manifest: Manifest = toml::from_str(&fs::read_to_string(&root.join(MANIFEST))?)
        .with_context(|| format!("parse {MANIFEST}"))?;
    let mut findings = Vec::new();
    let mut pinned: Vec<(&str, &str)> = manifest
        .font
        .iter()
        .map(|font| (font.file.as_str(), font.sha256.as_str()))
        .collect();
    pinned.extend(
        manifest
            .license
            .iter()
            .map(|license| (license.file.as_str(), license.sha256.as_str())),
    );
    for (file, expected) in &pinned {
        let path = root.join(DIRECTORY).join(file);
        if !fs::is_file(&path)? {
            findings.push(format!("{} is missing", portable(&path)));
            continue;
        }
        let actual = digest(&fs::read(&path)?);
        if actual != *expected {
            findings.push(format!(
                "{} has the digest {actual}, not the pinned {expected}",
                portable(&path)
            ));
        }
    }
    for font in &manifest.font {
        if !manifest
            .license
            .iter()
            .any(|license| license.file == font.license)
        {
            findings.push(format!(
                "{} names the license {}, which the manifest does not pin",
                font.file, font.license
            ));
        }
    }
    for path in fs::entries(&root.join(DIRECTORY))? {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        // The manifest, the family list, and the catalog are the directory's TOML; every other file is a font or a license the manifest pins.
        if !name.ends_with(TOML_SUFFIX) && !pinned.iter().any(|(file, _)| *file == name) {
            findings.push(format!("{} is not in the manifest", portable(&path)));
        }
    }
    Ok(findings)
}

/// Refuses bundled fonts that disagree with their manifest, naming each file.
///
/// # Errors
///
/// Returns an error listing every finding, or when a file cannot be read.
pub fn verify(root: &Path) -> Result<()> {
    let found = findings(root)?;
    ensure!(
        found.is_empty(),
        "the bundled fonts disagree with {MANIFEST}:\n{}",
        found.join("\n")
    );
    let drift = crate::font_catalog::findings(root)?;
    ensure!(
        drift.is_empty(),
        "the font catalog disagrees with its family list:\n{}",
        drift.join("\n")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{DIRECTORY, MANIFEST, digest, findings};
    use crate::fs;

    fn manifest(sha: &str) -> String {
        format!(
            "[[font]]\nfamily = \"F\"\nfile = \"f.ttf\"\nsha256 = \"{sha}\"\nlicense = \"OFL.txt\"\nsource = \"https://example.invalid/f\"\n[[license]]\nfile = \"OFL.txt\"\nsha256 = \"{sha}\"\n"
        )
    }

    #[test]
    fn digests_are_lowercase_hexadecimal() {
        assert_eq!(
            digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn each_disagreement_is_named() {
        let root = tempfile::tempdir().unwrap();
        fs::write(&root.path().join(DIRECTORY).join("f.ttf"), b"abc").unwrap();
        fs::write(&root.path().join(DIRECTORY).join("OFL.txt"), b"abc").unwrap();
        fs::write(
            &root.path().join(MANIFEST),
            manifest(&digest(b"abc")).as_bytes(),
        )
        .unwrap();
        assert_eq!(findings(root.path()).unwrap(), Vec::<String>::new());
        fs::write(&root.path().join(DIRECTORY).join("stray.ttf"), b"x").unwrap();
        fs::write(&root.path().join(DIRECTORY).join("OFL.txt"), b"changed").unwrap();
        let drift = findings(root.path()).unwrap().join("\n");
        assert!(
            drift.contains("stray.ttf is not in the manifest"),
            "{drift}"
        );
        assert!(drift.contains("OFL.txt has the digest"), "{drift}");
        fs::write(&root.path().join(MANIFEST), manifest("0").as_bytes()).unwrap();
        let mismatch = findings(root.path()).unwrap().join("\n");
        assert!(mismatch.contains("f.ttf has the digest"), "{mismatch}");
    }
}
