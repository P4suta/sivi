//! The one owner of child processes in the repository tasks, which names each pinned tool.

use anyhow::{Context, Result, ensure};
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);
static INTERRUPT_HANDLER: OnceLock<Result<(), String>> = OnceLock::new();
static INTERRUPT_EVENT: (Mutex<()>, Condvar) = (Mutex::new(()), Condvar::new());
const PROCESS_POLL: Duration = Duration::from_millis(250);

/// The nightly toolchain that Kani 0.68.0 pins, which Miri and cargo-fuzz share.
pub const NIGHTLY: &str = "nightly-2026-08-21";

/// The pinned versions the proof gate requires before it trusts a result.
pub const KANI_VERSION: &str = "0.68.0";
pub const MUTANTS_VERSION: &str = "27.1.0";
pub const FUZZ_VERSION: &str = "0.13.2";
pub const TIMEOUT_PROGRAM: &str = "/usr/bin/timeout";

mod raw {
    #![expect(
        clippy::disallowed_methods,
        reason = "the repository tasks' one process owner constructs each command in a single statement"
    )]
    #![expect(
        clippy::disallowed_types,
        reason = "the process owner reads a clock only for safety stops, never proof verdicts"
    )]

    use std::process::Command;

    pub(super) fn command(program: &str) -> Command {
        Command::new(program)
    }

    #[derive(Debug)]
    pub(super) struct Timer(std::time::Instant);

    impl Timer {
        pub(super) fn start() -> Self {
            Self(std::time::Instant::now())
        }
        pub(super) fn elapsed(&self) -> std::time::Duration {
            self.0.elapsed()
        }
    }
}

/// A safety clock owned by the process adapter, independent of a proof's verdict.
#[derive(Debug)]
pub struct Timer(raw::Timer);

impl Timer {
    #[must_use]
    pub fn start() -> Self {
        Self(raw::Timer::start())
    }
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }
}

/// A tool the tasks run, each resolved through the pinned `mise` environment on `PATH`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tool {
    Cargo,
    /// Cargo on the pinned nightly, for Miri and cargo-fuzz.
    CargoNightly,
    Rustfmt,
    Taplo,
    Typos,
    Actionlint,
    Zizmor,
    Git,
    Dotfiles,
    Z3,
    Gh,
    Ouch,
    Rustup,
    /// Docker, which hosts the verifiers that ship as Java programs.
    Docker,
    Timeout,
    /// SWI-Prolog, which runs the executable references.
    Swipl,
}

impl Tool {
    const fn program(self) -> &'static str {
        match self {
            Self::Cargo | Self::CargoNightly => "cargo",
            Self::Rustfmt => "rustfmt",
            Self::Taplo => "taplo",
            Self::Typos => "typos",
            Self::Actionlint => "actionlint",
            Self::Zizmor => "zizmor",
            Self::Git => "git",
            Self::Dotfiles => "dotfiles",
            Self::Z3 => "z3",
            Self::Gh => "gh",
            Self::Ouch => "ouch",
            Self::Rustup => "rustup",
            Self::Docker => "docker",
            Self::Timeout => TIMEOUT_PROGRAM,
            Self::Swipl => "swipl",
        }
    }

    /// A command for the tool, run from `root`.
    #[must_use]
    pub fn command(self, root: &Path) -> Command {
        let mut command = raw::command(self.program());
        command.current_dir(root);
        if self == Self::CargoNightly {
            command.arg(format!("+{NIGHTLY}"));
        }
        command
    }
}

/// Runs a command with inherited streams and requires success.
///
/// # Errors
///
/// Returns an error naming `what` when the command cannot start or exits with failure.
pub fn succeed(command: &mut Command, what: &str) -> Result<()> {
    let status = command.status().with_context(|| format!("start {what}"))?;
    ensure!(status.success(), "{what} failed with {status}");
    Ok(())
}

/// Runs a command with inherited streams and returns whether it succeeded.
///
/// # Errors
///
/// Returns an error naming `what` when the command cannot start.
pub fn status(command: &mut Command, what: &str) -> Result<bool> {
    let status = command.status().with_context(|| format!("start {what}"))?;
    Ok(status.success())
}

/// Runs a command with captured streams.
///
/// # Errors
///
/// Returns an error naming `what` when the command cannot start.
pub fn output(command: &mut Command, what: &str) -> Result<Output> {
    command
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("start {what}"))
}

/// Installs cancellation handling before reserving a verification engine.
///
/// # Errors
///
/// Refuses to start when cancellation handling is unavailable.
pub fn handle_interrupts() -> Result<()> {
    INTERRUPT_HANDLER
        .get_or_init(|| {
            ctrlc::try_set_handler(|| {
                INTERRUPTED.store(true, Ordering::SeqCst);
                INTERRUPT_EVENT.1.notify_all();
            })
            .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|error| anyhow::anyhow!("install verification cancellation: {error}"))
        .copied()
}

/// Whether cancellation has been requested for this process.
#[must_use]
pub fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

/// Waits for cancellation or the next external-process observation.
///
/// # Errors
///
/// Reports cancellation or an unavailable notification primitive.
pub fn wait_for_interrupt(interval: Duration) -> Result<()> {
    let (notified, _) = INTERRUPT_EVENT
        .1
        .wait_timeout_while(
            INTERRUPT_EVENT
                .0
                .lock()
                .map_err(|error| anyhow::anyhow!("cancellation lock: {error}"))?,
            interval,
            |()| !interrupted(),
        )
        .map_err(|error| anyhow::anyhow!("cancellation wait: {error}"))?;
    drop(notified);
    ensure!(
        !interrupted(),
        "verification was cancelled; evidence remains incomplete"
    );
    Ok(())
}

/// Runs a builder client while the campaign permits work, preserving its log.
///
/// # Errors
///
/// Stops the client on cancellation or a safety deadline, then reports incomplete work.
pub fn controlled(
    command: &mut Command,
    what: &str,
    log: &Path,
    active: impl Fn() -> Result<()>,
) -> Result<()> {
    let output = crate::fs::create(log)?;
    let errors = output
        .try_clone()
        .context("duplicate the builder log handle")?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(output)
        .stderr(errors)
        .spawn()
        .with_context(|| format!("start {what}"))?;
    loop {
        if let Some(status) = child
            .try_wait()
            .with_context(|| format!("inspect {what}"))?
        {
            ensure!(
                status.success(),
                "{what} failed with {status}; see {}",
                log.display()
            );
            return Ok(());
        }
        if let Err(error) = active() {
            child.kill().with_context(|| format!("stop {what}"))?;
            child.wait().with_context(|| format!("reap {what}"))?;
            return Err(error.context(format!("{what} is incomplete; see {}", log.display())));
        }
        if let Err(error) = wait_for_interrupt(PROCESS_POLL) {
            child.kill().with_context(|| format!("stop {what}"))?;
            child.wait().with_context(|| format!("reap {what}"))?;
            return Err(error);
        }
    }
}

/// The standard output of a command that must succeed.
///
/// # Errors
///
/// Returns an error naming `what` when the command cannot start, fails, or prints invalid UTF-8.
pub fn capture(command: &mut Command, what: &str) -> Result<String> {
    let output = output(command, what)?;
    ensure!(
        output.status.success(),
        "{what} failed with {}:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).with_context(|| format!("{what} printed invalid UTF-8"))
}

/// Requires that `command --version` names `version`.
///
/// # Errors
///
/// Returns an error when the tool is missing or reports another version.
pub fn require_version(command: &mut Command, what: &str, version: &str) -> Result<()> {
    let text = capture(command.arg("--version"), what)?;
    ensure!(
        text.contains(version),
        "the pinned {what} {version} is unavailable; found: {}",
        text.trim()
    );
    Ok(())
}

/// Appends arguments to a command and returns it for chaining.
pub fn with_args<I, S>(command: &mut Command, arguments: I) -> &mut Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    command.args(arguments);
    command
}
