//! The TLA+ obligations: TLC checks every invariant and the property of each model over all its behaviors in the pinned image, and refutes each probe configuration.

use crate::obligations::{self, Obligation, TlaModel};
use crate::quint::{self, Session};
use crate::tool::{self, Tool};
use anyhow::{Context, Result, ensure};
use std::path::Path;
use std::process::Output;

/// The model checker's entry class, which the Apalache distribution of the image carries.
const TLC_CLASS: &str = "tlc2.TLC";
/// The exit status TLC ends with when an invariant is violated.
const SAFETY_VIOLATION: i32 = 12;
/// One worker keeps the state count exact and the run repeatable.
const WORKERS: &str = "1";
/// A fixed fingerprint function keeps the run repeatable.
const FINGERPRINT: &str = "0";
/// What TLC prints when every invariant and property held.
const CLEAN: &str = "No error has been found";
/// The directory of an Apalache distribution under the image's Quint home, before its version.
const DISTRIBUTION_PREFIX: &str = "apalache-dist-";
/// The jar inside a distribution, which holds TLC.
const JAR_IN_DISTRIBUTION: &str = "apalache/lib/apalache.jar";

/// The jar of the Apalache distribution in the image, which holds TLC.
fn jar() -> String {
    format!(
        "{}/{DISTRIBUTION_PREFIX}{}/{JAR_IN_DISTRIBUTION}",
        quint::QUINT_HOME,
        quint::APALACHE_VERSION
    )
}

/// Runs TLC on `spec` under `config`, returning its output.
fn tlc(session: Session<'_>, spec: &str, config: &str) -> Result<Output> {
    let jar = jar();
    let states = format!("{}/tlc-states", quint::OUTPUT);
    let arguments = [
        "-cp",
        &jar,
        TLC_CLASS,
        "-config",
        config,
        "-workers",
        WORKERS,
        "-fp",
        FINGERPRINT,
        "-noGenerateSpecTE",
        "-metadir",
        &states,
        spec,
    ];
    let mut docker = Tool::Docker.command(session.root);
    docker.args(quint::program_arguments(session, "java", &arguments));
    tool::output(&mut docker, "docker run of TLC")
}

fn models(manifest: &obligations::Manifest) -> Vec<&TlaModel> {
    manifest
        .obligation
        .iter()
        .filter_map(|obligation| match obligation {
            Obligation::Tla(model) => Some(model),
            Obligation::Kani { .. }
            | Obligation::Verus { .. }
            | Obligation::Alloy { .. }
            | Obligation::Quint(_)
            | Obligation::Prolog { .. } => None,
        })
        .collect()
}

fn check(session: Session<'_>, model: &TlaModel) -> Result<()> {
    let spec = format!("{}/{}", quint::SOURCE, model.spec);
    let checked = tlc(
        session,
        &spec,
        &format!("{}/{}", quint::SOURCE, model.config),
    )?;
    let report = String::from_utf8_lossy(&checked.stdout);
    let found = quint::distinct_states(&report)?;
    ensure!(
        checked.status.success() && report.contains(CLEAN) && found == Some(model.distinct_states),
        "the model checker found {found:?} distinct states of {} and ended with {}, while {} records {}; review the model change, then record the new count:\n{report}{}",
        model.spec,
        checked.status,
        obligations::MANIFEST,
        model.distinct_states,
        String::from_utf8_lossy(&checked.stderr)
    );
    let probed = tlc(
        session,
        &spec,
        &format!("{}/{}", quint::SOURCE, model.probe_config),
    )?;
    let probe_report = String::from_utf8_lossy(&probed.stdout);
    ensure!(
        probed.status.code() == Some(SAFETY_VIOLATION)
            && probe_report.contains(&format!("Invariant {} is violated", model.probe_invariant)),
        "the model checker did not refute {} under {}, so nothing shows the check can fail; it ended with {}:\n{probe_report}",
        model.probe_invariant,
        model.probe_config,
        probed.status
    );
    Ok(())
}

/// Discharges every TLA+ obligation of the manifest in the pinned image, offline.
///
/// # Errors
///
/// Returns an error when Docker cannot build or run the image, a model fails its check, or a probe is not refuted.
pub fn verify(root: &Path) -> Result<()> {
    let root = std::fs::canonicalize(root).context("resolve the checkout")?;
    let manifest = obligations::load(&root)?;
    let output = tempfile::tempdir().context("create the TLC output directory")?;
    let image = quint::image(&root)?;
    quint::build(&root, &image)?;
    let session = Session {
        root: &root,
        output: output.path(),
        image: &image,
    };
    let found = models(&manifest);
    ensure!(
        !found.is_empty(),
        "the manifest declares no TLA+ obligation"
    );
    for model in &found {
        check(session, model)?;
    }
    eprintln!(
        "tla: {} models hold every invariant and their property in every behavior, and each probe configuration is refuted",
        found.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::jar;

    #[test]
    fn the_jar_names_the_pinned_apalache_release() {
        assert_eq!(
            jar(),
            "/opt/quint/apalache-dist-0.62.1/apalache/lib/apalache.jar"
        );
    }
}
