//! The Alloy gate over the models of `specs/`.
//!
//! Each assertion of a model finds no counterexample within its scope, each instance exists,
//! and each assertion without its validator finds a counterexample, which shows the gate can fail.
//! Alloy runs in a pinned image with no network, the checkout mounted read-only, and one writable output directory.

use crate::containers::Campaign;
use crate::fs;
use crate::obligations::{self, AlloyCheck, Obligation};
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::ffi::OsString;
use std::path::Path;

/// The Alloy release the image's Dockerfile pins by digest.
pub const ALLOY_VERSION: &str = "6.2.0";

/// The directory that defines the image.
pub const IMAGE_DIRECTORY: &str = "xtask/proofs/alloy";

/// Where the container mounts the read-only checkout and the writable output directory.
const SOURCE: &str = "/source";
const OUTPUT: &str = "/out";

/// The jar the image installs.
const JAR: &str = "/opt/alloy/alloy.jar";

/// How many hexadecimal digits of the Dockerfile's digest the image name carries.
const DIGEST_DIGITS: usize = 16;

/// The image name for the Dockerfile, which changes with it.
///
/// # Errors
///
/// Returns an error when the Dockerfile cannot be read.
pub fn image(root: &Path) -> Result<String> {
    let definition = fs::read(&root.join(IMAGE_DIRECTORY).join("Dockerfile"))?;
    let digest = crate::fonts::digest(&definition);
    Ok(format!(
        "sivi-alloy:{}",
        digest.get(..DIGEST_DIGITS).unwrap_or(&digest)
    ))
}

/// Builds the pinned image unless it exists.
///
/// # Errors
///
/// Returns an error when Docker cannot build it.
pub fn build(root: &Path, image: &str, campaign: &Campaign<'_>) -> Result<()> {
    campaign.build(image, IMAGE_DIRECTORY, &root.join(IMAGE_DIRECTORY))
}

/// Where one gate run happens: the checkout, the output directory, and the image.
#[derive(Clone, Copy, Debug)]
pub struct Session<'a> {
    pub root: &'a Path,
    pub output: &'a Path,
    pub image: &'a str,
}

/// One model and the solver that decides it.
#[derive(Clone, Copy, Debug)]
pub struct Model<'a> {
    pub spec: &'a str,
    pub solver: &'a str,
}

fn mount(source: &Path, target: &str, readonly: bool) -> OsString {
    // Docker reads `--mount` as one CSV record, so quoting keeps a comma in the path inside the field.
    let mut mount = OsString::from("type=bind,\"source=");
    mount.push(source);
    mount.push(format!("\",target={target}"));
    if readonly {
        mount.push(",readonly");
    }
    mount
}

/// The arguments that run one Alloy command in the image with no network, the checkout read-only, and the output writable.
#[must_use]
pub fn run_arguments(session: Session<'_>, model: Model<'_>, command: &str) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = ["run", "--rm", "--network", "none"]
        .map(OsString::from)
        .into();
    arguments.push("--mount".into());
    arguments.push(mount(session.root, SOURCE, true));
    arguments.push("--mount".into());
    arguments.push(mount(session.output, OUTPUT, false));
    let receipt = format!("{OUTPUT}/{command}");
    let spec = format!("{SOURCE}/{}", model.spec);
    arguments.extend(
        [
            session.image,
            "java",
            "-jar",
            JAR,
            "exec",
            "--quiet",
            "--force",
            "--solver",
            model.solver,
            "--command",
            command,
            "--output",
            &receipt,
            "--type",
            "json",
            &spec,
        ]
        .map(OsString::from),
    );
    arguments
}

/// Whether a receipt holds a solution for `command`: an instance of a run, or a counterexample of a check.
///
/// # Errors
///
/// Returns an error when the receipt does not name the command or holds a malformed outcome.
pub fn solved(receipt: &Value, command: &str) -> Result<bool> {
    let entry = receipt
        .get("commands")
        .and_then(|commands| commands.get(command))
        .with_context(|| format!("the Alloy receipt names no command {command}"))?;
    ensure!(
        entry.is_object()
            && matches!(
                entry.get("type").and_then(Value::as_str),
                Some("check" | "run")
            ),
        "the Alloy receipt has no command outcome for {command}"
    );
    match entry.get("solution") {
        None => Ok(false),
        Some(Value::Array(solutions)) => Ok(!solutions.is_empty()),
        Some(_) => bail!("the Alloy receipt has a malformed solution for {command}"),
    }
}

impl Session<'_> {
    fn run(self, campaign: &Campaign<'_>, model: Model<'_>, command: &str) -> Result<bool> {
        let ran = campaign.run(self.image, &run_arguments(self, model, command))?;
        ensure!(
            ran.status.success(),
            "Alloy failed on {command} of {}: {}\n{}",
            model.spec,
            ran.status,
            String::from_utf8_lossy(&ran.stderr)
        );
        let receipt: Value =
            serde_json::from_slice(&fs::read(&self.output.join(command).join("receipt.json"))?)
                .context("parse the Alloy receipt")?;
        solved(&receipt, command)
    }

    fn verify_check(
        self,
        campaign: &Campaign<'_>,
        model: Model<'_>,
        check: &AlloyCheck,
    ) -> Result<()> {
        ensure!(
            !self.run(campaign, model, &check.assertion)?,
            "Alloy found a counterexample to {} in {} {}",
            check.assertion,
            model.spec,
            check.scope
        );
        ensure!(
            self.run(campaign, model, &check.instance)?,
            "Alloy found no instance of {} in {}, so {} may hold vacuously",
            check.instance,
            model.spec,
            check.assertion
        );
        ensure!(
            self.run(campaign, model, &check.counterexample)?,
            "Alloy found no counterexample to {} in {}, so nothing shows {} can fail",
            check.counterexample,
            model.spec,
            check.assertion
        );
        Ok(())
    }
}

/// Discharges every Alloy obligation of the manifest in the pinned image, offline.
///
/// # Errors
///
/// Returns an error when Docker cannot build or run the image, or a command ends otherwise than its obligation requires.
pub fn verify(root: &Path, campaign: &Campaign<'_>) -> Result<()> {
    let root = std::fs::canonicalize(root).context("resolve the checkout")?;
    let manifest = obligations::load(&root)?;
    let output = crate::containers::job(&root, "alloy-")?;
    let image = image(&root)?;
    build(&root, &image, campaign)?;
    let session = Session {
        root: &root,
        output: &output,
        image: &image,
    };
    let mut checked: usize = 0;
    for obligation in &manifest.obligation {
        let Obligation::Alloy {
            spec,
            solver,
            checks,
        } = obligation
        else {
            continue;
        };
        ensure!(
            fs::is_file(&root.join(spec))?,
            "the manifest names the Alloy model {spec}, which does not exist"
        );
        let model = Model { spec, solver };
        for check in checks {
            session.verify_check(campaign, model, check)?;
            checked = checked.saturating_add(1);
        }
    }
    ensure!(checked > 0, "the manifest declares no Alloy obligation");
    eprintln!(
        "alloy: {checked} assertions hold within their scope, have an instance, and fail without their validator"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Model, Session, run_arguments, solved};
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn a_receipt_reports_a_solution_only_for_a_known_command_with_one() {
        let receipt = json!({
            "commands": {
                "Holds": { "type": "check" },
                "Found": { "type": "run", "solution": [{}] },
                "Odd": { "type": "check", "solution": 3 },
                "Other": { "type": "fact" }
            }
        });
        assert!(!solved(&receipt, "Holds").unwrap());
        assert!(solved(&receipt, "Found").unwrap());
        solved(&receipt, "Odd").unwrap_err();
        solved(&receipt, "Other").unwrap_err();
        solved(&receipt, "Missing").unwrap_err();
    }

    #[test]
    fn the_run_mounts_the_checkout_read_only_and_names_the_command() {
        let session = Session {
            root: Path::new("/checkout"),
            output: Path::new("/scratch"),
            image: "sivi-alloy:abc",
        };
        let model = Model {
            spec: "specs/theme.als",
            solver: "sat4j",
        };
        let arguments = run_arguments(session, model, "Holds");
        let text: Vec<String> = arguments
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        assert_eq!(text[0..4], ["run", "--rm", "--network", "none"]);
        assert!(text.contains(&String::from(
            "type=bind,\"source=/checkout\",target=/source,readonly"
        )));
        assert!(text.contains(&String::from("type=bind,\"source=/scratch\",target=/out")));
        assert!(text.contains(&String::from("/out/Holds")));
        assert_eq!(
            text.last().map(String::as_str),
            Some("/source/specs/theme.als")
        );
    }
}
