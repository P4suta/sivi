//! The Verus gate: the pinned release verifies the core's unbounded specifications and refuses a false one.
//!
//! Verus runs as the standalone driver over the core's root, so the core keeps no Verus dependency.
//! Specifications sit behind `cfg_attr(verus_keep_ghost, ..)`, which the driver sets and `cargo` never does.

use crate::fs;
use crate::tool::{Tool, capture, output};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The pinned release, whose assets `fetch` downloads and whose digest it checks.
pub const VERSION: &str = "0.2026.10.04.426d8b0";
pub const COMMIT: &str = "426d8b01e7ffb910a36c15e1f870ff985bb61eef";
pub const RELEASE_TAG: &str = "release/0.2026.10.04.426d8b0";

/// The Rust toolchain the release drives, which `rustup` must hold.
pub const TOOLCHAIN: &str = "1.98.1";

/// Where the proof image installs the release.
pub const INSTALL: &str = "/opt/sivi/verus";

/// The crate root Verus verifies.
pub const SOURCE: &str = "crates/sivi-core/src/lib.rs";
pub const CRATE_NAME: &str = "sivi_core";
pub const EDITION: &str = "2024";

/// A crate whose one specification is false, which Verus must refuse.
pub const NEGATIVE: &str = "xtask/proofs/verus-negative/src/lib.rs";

/// One release asset of the pinned version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Asset {
    pub platform: &'static str,
    pub file: &'static str,
    pub sha256: &'static str,
    /// The directory the archive unpacks to.
    pub directory: &'static str,
}

/// The assets of the pinned release, by `target_os` and `target_arch`.
pub const ASSETS: [Asset; 2] = [
    Asset {
        platform: "macos-aarch64",
        file: "verus-0.2026.10.04.426d8b0-arm64-macos.zip",
        sha256: "83cafc50284caea07fa8762df04b00cc0af441f25cdf1631d6c30987c65af446",
        directory: "verus-arm64-macos",
    },
    Asset {
        platform: "linux-x86_64",
        file: "verus-0.2026.10.04.426d8b0-x86-linux.zip",
        sha256: "624018662868563a7bec0dd07be5e4dd0c3d61124b4b55105765884bf2d4d1a0",
        directory: "verus-x86-linux",
    },
];

#[derive(Debug, Deserialize)]
struct Report {
    #[serde(rename = "func-details", default)]
    functions: serde_json::Map<String, serde_json::Value>,
    #[serde(rename = "verification-results")]
    results: Results,
    verus: Identity,
}

#[derive(Debug, Deserialize)]
struct Results {
    success: bool,
    #[serde(rename = "encountered-vir-error", default)]
    vir_error: bool,
    #[serde(default)]
    verified: u64,
    #[serde(default)]
    errors: u64,
}

#[derive(Debug, Deserialize)]
struct Identity {
    version: String,
    commit: String,
}

/// The asset for this host.
///
/// # Errors
///
/// Returns an error when the release has no build for this host.
pub fn asset() -> Result<Asset> {
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    ASSETS
        .into_iter()
        .find(|asset| asset.platform == platform)
        .with_context(|| format!("the pinned Verus release has no build for {platform}"))
}

fn driver(root: &Path) -> Result<PathBuf> {
    let asset = asset()?;
    Ok(root.join(INSTALL).join(asset.directory).join("verus"))
}

/// Downloads, checks, and unpacks the pinned release unless its driver is already present.
///
/// # Errors
///
/// Returns an error when the download fails, the digest differs, or the archive cannot be unpacked.
pub fn fetch(root: &Path) -> Result<()> {
    let asset = asset()?;
    let install = root.join(INSTALL);
    if fs::is_file(&driver(root)?)? {
        return Ok(());
    }
    fs::create_dir_all(&install)?;
    let mut download = Tool::Gh.command(root);
    download
        .args([
            "release",
            "download",
            RELEASE_TAG,
            "-R",
            "verus-lang/verus",
            "-p",
            asset.file,
            "--clobber",
            "-D",
        ])
        .arg(&install);
    crate::tool::succeed(&mut download, "gh release download")?;
    let archive = install.join(asset.file);
    let actual = crate::fonts::digest(&fs::read(&archive)?);
    ensure!(
        actual == asset.sha256,
        "{} has the digest {actual}, not the pinned {}",
        archive.display(),
        asset.sha256
    );
    let mut unpack = Tool::Ouch.command(root);
    unpack
        .args(["--quiet", "--yes", "decompress"])
        .arg(&archive)
        .arg("--dir")
        .arg(&install);
    crate::tool::succeed(&mut unpack, "ouch decompress")?;
    ensure!(
        fs::is_file(&driver(root)?)?,
        "the unpacked release holds no driver at {}",
        driver(root)?.display()
    );
    Ok(())
}

fn run(root: &Path, source: &str, crate_name: &str) -> Result<Report> {
    let mut command = Tool::Rustup.command(root);
    command
        .args(["run", TOOLCHAIN])
        .arg(driver(root)?)
        .args([
            "--crate-type=lib",
            &format!("--crate-name={crate_name}"),
            &format!("--edition={EDITION}"),
            "--no-cheating",
            "--output-json",
        ])
        .arg(root.join(source));
    let result = output(&mut command, "verus")?;
    let text = String::from_utf8(result.stdout).context("verus printed invalid UTF-8")?;
    let start = text.find('{').with_context(|| {
        format!(
            "verus printed no report:\n{}",
            String::from_utf8_lossy(&result.stderr)
        )
    })?;
    serde_json::from_str(text.get(start..).unwrap_or_default()).context("parse the Verus report")
}

fn require_identity(report: &Report) -> Result<()> {
    ensure!(
        report.verus.version == VERSION && report.verus.commit == COMMIT,
        "the report comes from Verus {} ({}), not the pinned {VERSION} ({COMMIT})",
        report.verus.version,
        report.verus.commit
    );
    Ok(())
}

/// Verifies the core and refuses the negative control.
///
/// # Errors
///
/// Returns an error when a specification fails, no function was verified, or the false specification passes.
pub fn verify(root: &Path) -> Result<()> {
    crate::containers::require_worker()?;
    let version = capture(
        Tool::Rustup
            .command(root)
            .args(["run", TOOLCHAIN, "rustc", "--version"]),
        "rustup run",
    )?;
    ensure!(
        version.contains(TOOLCHAIN),
        "the toolchain {TOOLCHAIN} Verus drives is unavailable; found: {}",
        version.trim()
    );
    fetch(root)?;
    let report = run(root, SOURCE, CRATE_NAME)?;
    require_identity(&report)?;
    ensure!(
        report.results.success && !report.results.vir_error && report.results.errors == 0,
        "Verus refused the core's specifications: {} verified, {} errors",
        report.results.verified,
        report.results.errors
    );
    ensure!(
        report.results.verified > 0 && !report.functions.is_empty(),
        "Verus verified nothing in the core; the specifications are not reaching the driver"
    );
    let control = run(root, NEGATIVE, "verus_negative")?;
    require_identity(&control)?;
    if control.results.success || control.results.errors == 0 {
        bail!("the Verus fixture with a false specification passed; the gate cannot show it fails");
    }
    ensure!(
        !control.results.vir_error,
        "the Verus fixture failed with a tool error, not a refuted specification"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ASSETS, Report};

    #[test]
    fn the_report_reads_its_outcome_and_identity() {
        let report: Report = serde_json::from_str(
            r#"{"func-details":{"a":{}},"verification-results":{"encountered-error":false,"encountered-vir-error":false,"success":true,"verified":3,"errors":0},"verus":{"profile":"release","version":"0.2026.10.04.426d8b0","platform":{"os":"macos","arch":"aarch64"},"toolchain":"1.98.1","commit":"426d8b01e7ffb910a36c15e1f870ff985bb61eef"}}"#,
        )
        .unwrap();
        assert!(report.results.success);
        assert_eq!(report.results.verified, 3);
        assert_eq!(report.functions.len(), 1);
        super::require_identity(&report).unwrap();
        let stale: Report = serde_json::from_str(
            r#"{"verification-results":{"success":true},"verus":{"version":"0","commit":"0"}}"#,
        )
        .unwrap();
        super::require_identity(&stale).unwrap_err();
    }

    #[test]
    fn every_asset_pins_a_digest() {
        for asset in ASSETS {
            assert_eq!(asset.sha256.len(), 64);
            assert!(asset.file.contains(super::VERSION));
        }
    }
}
