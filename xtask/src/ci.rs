//! The hosted entry points, which run the same checks as the local commands.

use crate::{check, proofs, tool};
use anyhow::{Context, Result, bail, ensure};
use std::path::Path;

fn proof_base() -> Result<Option<String>> {
    match std::env::var("PROOF_BASE") {
        Ok(base) if base.is_empty() => Ok(None),
        Ok(base) => Ok(Some(base)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error).context("read PROOF_BASE"),
    }
}

fn proof_options() -> Result<proofs::Options> {
    let full = match std::env::var("PROOF_FULL") {
        Ok(value) if value == "true" => true,
        Ok(value) if value == "false" || value.is_empty() => false,
        Err(std::env::VarError::NotPresent) => false,
        Ok(value) => bail!("invalid PROOF_FULL value {value}"),
        Err(error) => return Err(error).context("read PROOF_FULL"),
    };
    Ok(proofs::Options {
        ci: !full,
        only: None,
        base: if full { None } else { proof_base()? },
        plan: false,
    })
}

/// The variable a workflow sets to the JSON of its `needs` context.
pub const NEEDS_VARIABLE: &str = "NEEDS_JSON";

/// The jobs that did not succeed, from the JSON of a workflow's `needs` context.
///
/// # Errors
///
/// Returns an error when the JSON is not an object of jobs with a `result`.
pub fn unsuccessful(needs: &str) -> Result<Vec<String>> {
    let needs: serde_json::Value =
        serde_json::from_str(needs).context("parse the needs context")?;
    let jobs = needs
        .as_object()
        .context("the needs context is an object of jobs")?;
    ensure!(!jobs.is_empty(), "the gate received no job results");
    let mut failed = Vec::new();
    for (job, outcome) in jobs {
        let result = outcome
            .get("result")
            .and_then(serde_json::Value::as_str)
            .with_context(|| format!("the job {job} reports no result"))?;
        if result != "success" {
            failed.push(format!("{job}: {result}"));
        }
    }
    Ok(failed)
}

fn gate(name: &str) -> Result<()> {
    let needs = std::env::var(NEEDS_VARIABLE)
        .with_context(|| format!("the {name} gate reads {NEEDS_VARIABLE}"))?;
    validate_jobs(name, &needs)?;
    Ok(())
}

fn validate_jobs(name: &str, needs: &str) -> Result<()> {
    let expected: &[&str] = match name {
        "ci" => &["check", "test", "proofs"],
        "codeql" => &["analyze"],
        "dependency-review" => &["review"],
        "scorecard" => &["analysis"],
        _ => bail!("unknown gate {name}"),
    };
    let context: serde_json::Value = serde_json::from_str(needs)?;
    let jobs = context
        .as_object()
        .context("the gate requires named job results")?;
    ensure!(
        jobs.len() == expected.len() && expected.iter().all(|job| jobs.contains_key(*job)),
        "the {name} gate requires the exact jobs {}",
        expected.join(", ")
    );
    let failed = unsuccessful(needs)?;
    ensure!(
        failed.is_empty(),
        "the {name} gate requires every selected job to succeed:\n{}",
        failed.join("\n")
    );
    Ok(())
}

/// Runs one hosted entry point.
///
/// # Errors
///
/// Returns an error when the entry point is unknown or its checks fail.
pub fn run(root: &Path, words: &[&str]) -> Result<()> {
    match words {
        ["check"] => check::run(root, check::Scope::Everything),
        ["test"] => tool::succeed(
            tool::Tool::Cargo.command(root).args([
                "nextest",
                "run",
                "--locked",
                "--workspace",
                "--all-targets",
                "--no-tests=fail",
            ]),
            "cargo nextest",
        ),
        ["proofs"] => proofs::run(root, &proof_options()?),
        ["gate", name] => gate(name),
        _ => bail!("usage: cargo xtask ci check|test|proofs|gate NAME"),
    }
}

#[cfg(test)]
mod tests {
    use super::{unsuccessful, validate_jobs};

    #[test]
    fn every_job_must_succeed() {
        let failed = unsuccessful(
            r#"{"check":{"result":"success"},"test":{"result":"failure"},"proofs":{"result":"skipped"}}"#,
        )
        .unwrap();
        assert_eq!(failed, ["proofs: skipped", "test: failure"]);
        unsuccessful(r#"{"check":{}}"#).unwrap_err();
        unsuccessful("[]").unwrap_err();
        unsuccessful("{}").unwrap_err();
        validate_jobs("ci", r#"{"check":{"result":"success"}}"#).unwrap_err();
        validate_jobs("codeql", r#"{"other":{"result":"success"}}"#).unwrap_err();
        validate_jobs("codeql", r#"{"analyze":{"result":"success"}}"#).unwrap();
    }
}
