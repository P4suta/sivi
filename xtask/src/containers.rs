//! The resource-limited owner of verification containers and their image builder.

use crate::fs;
use crate::tool::{self, TIMEOUT_PROGRAM, Tool};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use sha2::Digest as _;
use sivi_core::verification_rules::{self, Limits};
use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::fmt;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const LEASE_IMAGE: &str = "docker.io/library/busybox:1.37.0@sha256:bdf57e528e45e4433820e045b29b4597825a1c9e38353532d90a01445013f82e";
pub const BUILDKIT_IMAGE: &str = "docker.io/moby/buildkit:v0.34.1-rootless@sha256:f68752a5a94acfcdad2d07867b7075601a421638e17fc5f28efc1c814d0c4f89";
const LEASE: &str = "sivi-proof-campaign";
const BUILDER_PREFIX: &str = "sivi-proof-";
const BUILD_DIRECTORY: &str = "target/proofs/container-build";
const LOG_DIRECTORY: &str = "target/proofs/containers";
const POLL: Duration = Duration::from_secs(1);
const CAMPAIGN_LIMIT: Duration = Duration::from_hours(2);
const WORKER_TIMEOUT: &str = "7200s";
const LOG_LIMIT: &str = "10m";
const PLATFORM: &str = "linux/amd64";
const ID_DIGITS: usize = 12;
const BUILDKIT_CONFIG: &str = "[worker.oci]\nrootless = true\nsnapshotter = \"native\"\nmax-parallelism = 1\nmaxUsedSpace = \"10GB\"\nreservedSpace = \"1GB\"\n";
const BUILDKIT_CONFIG_PATH: &str = "/etc/buildkit/sivi.toml";
const PROOF_WORKER: &str = "/usr/local/bin/sivi-proof-worker";
const RUST_IMAGE_DIRECTORY: &str = "xtask/proofs/rust";
const PROLOG_RECIPE: &str = "xtask/proofs/prolog/Dockerfile";
const JOB_DIRECTORY: &str = "target/proofs/jobs";
const DOCKER_MARKER: &str = "/.dockerenv";
const CGROUP_CPU: &str = "/sys/fs/cgroup/cpu.max";
const CGROUP_MEMORY: &str = "/sys/fs/cgroup/memory.max";
const CGROUP_SWAP: &str = "/sys/fs/cgroup/memory.swap.max";
const CGROUP_PIDS: &str = "/sys/fs/cgroup/pids.max";
const BOUNDARY_OUTPUT: &str = "/out/boundary-evidence";
const BOUNDARY_TIMEOUT: u8 = 124;
const BOUNDARY_DEADLINE: &str = "1s";
const BOUNDARY_PAUSE: &str = "10s";
const CORPORA: [&str; 2] = ["fuzz/corpus", "xtask/proofs/fuzz-fixture/corpus"];
const WRITABLE_DIRECTORIES: [&str; 8] = [
    "target/proofs",
    "target/miri",
    "fuzz/target",
    "fuzz/artifacts",
    "fuzz/corpus",
    "xtask/proofs/fuzz-fixture/target",
    "xtask/proofs/fuzz-fixture/artifacts",
    "xtask/proofs/fuzz-fixture/corpus",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct HostConfig {
    privileged: bool,
    network_mode: String,
    nano_cpus: i64,
    cpu_quota: i64,
    cpu_period: i64,
    memory: i64,
    memory_swap: i64,
    pids_limit: i64,
}

impl HostConfig {
    const fn limits(&self) -> Limits {
        Limits {
            cpu_nanos: self.nano_cpus,
            cpu_quota: self.cpu_quota,
            cpu_period: self.cpu_period,
            memory: self.memory,
            memory_swap: self.memory_swap,
            processes: self.pids_limit,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct State {
    running: bool,
    #[serde(rename = "OOMKilled")]
    oom_killed: bool,
    exit_code: i32,
    error: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Inspection {
    id: String,
    image: String,
    config: Configuration,
    mounts: Vec<Mount>,
    host_config: HostConfig,
    state: State,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Configuration {
    user: String,
    labels: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct Mount {
    #[serde(rename = "Destination")]
    destination: String,
    #[serde(rename = "RW")]
    writable: bool,
}

fn inspect(root: &Path, name: &str) -> Result<Inspection> {
    let text = tool::capture(
        Tool::Docker.command(root).args(["inspect", name]),
        "docker inspect",
    )?;
    let items: Vec<Inspection> =
        serde_json::from_str(&text).context("read the container identity")?;
    let mut items = items.into_iter();
    let item = items.next().context("the container inspection is empty")?;
    ensure!(items.next().is_none(), "ambiguous container identity");
    Ok(item)
}

/// The actual exit status of the worker, apart from the status of `docker logs`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Status(i32);

impl Status {
    #[must_use]
    pub const fn success(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn code(self) -> i32 {
        self.0
    }
}

impl fmt::Display for Status {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "worker exit code {}", self.0)
    }
}

/// A worker's bounded log and actual status.
#[derive(Debug)]
pub struct Output {
    pub status: Status,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug)]
struct Incomplete {
    oom: bool,
    exit: i32,
}

impl fmt::Display for Incomplete {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "incomplete worker: exit {}, OOM {}",
            self.exit, self.oom
        )
    }
}

impl std::error::Error for Incomplete {}

/// An owned Docker object, removed only by its immutable ID.
#[derive(Debug)]
struct Owned<'a> {
    root: &'a Path,
    id: String,
}

impl Owned<'_> {
    fn remove(&self) -> Result<()> {
        tool::succeed(
            Tool::Docker
                .command(self.root)
                .args(["rm", "--force", &self.id]),
            "remove the owned verification container",
        )
    }
}

/// One campaign per Docker engine, shared by every checkout.
#[derive(Debug)]
pub struct Campaign<'a> {
    root: &'a Path,
    lease: Owned<'a>,
    started: tool::Timer,
    limit: Duration,
    clean: Cell<bool>,
    evidence: RefCell<Vec<serde_json::Value>>,
}

impl<'a> Campaign<'a> {
    /// Reserves the engine before any image build or verification starts.
    ///
    /// # Errors
    ///
    /// Refuses unsupported limits or an existing campaign without stealing its reservation.
    pub fn open(root: &'a Path) -> Result<Self> {
        Self::open_for(root, CAMPAIGN_LIMIT)
    }

    fn open_for(root: &'a Path, limit: Duration) -> Result<Self> {
        tool::handle_interrupts()?;
        let information = tool::capture(
            Tool::Docker
                .command(root)
                .args(["info", "--format", "{{json .}}"]),
            "docker info",
        )?;
        let information: serde_json::Value = serde_json::from_str(&information)?;
        for capability in [
            "MemoryLimit",
            "SwapLimit",
            "PidsLimit",
            "CpuCfsQuota",
            "CpuCfsPeriod",
        ] {
            ensure!(
                information
                    .get(capability)
                    .and_then(serde_json::Value::as_bool)
                    == Some(true),
                "Docker lacks {capability}; verification remains incomplete"
            );
        }
        ensure!(
            information
                .get("CgroupVersion")
                .and_then(serde_json::Value::as_str)
                == Some("2"),
            "verification requires cgroup v2"
        );
        let mut create = Tool::Docker.command(root);
        create.args(["create", "--name", LEASE, "--network", "none"]);
        create.args(limit_arguments());
        create.args([
            "--label",
            "org.sivi.verification=lease",
            LEASE_IMAGE,
            "true",
        ]);
        let id = tool::capture(&mut create, "reserve the verification engine")?;
        let id = id.trim().to_owned();
        let lease = Owned { root, id };
        let checked = inspect(root, &lease.id).and_then(|checked| {
            ensure!(
                checked.host_config.limits().bounded(),
                "the campaign reservation has no enforceable resource limits"
            );
            Ok(())
        });
        if checked.is_err() {
            return finish(checked, lease.remove()).map(|()| Self {
                root,
                lease,
                started: tool::Timer::start(),
                limit,
                clean: Cell::new(true),
                evidence: RefCell::new(Vec::new()),
            });
        }
        Ok(Self {
            root,
            lease,
            started: tool::Timer::start(),
            limit,
            clean: Cell::new(true),
            evidence: RefCell::new(Vec::new()),
        })
    }

    /// Releases the reservation after every owned worker has stopped.
    ///
    /// # Errors
    ///
    /// Reports a cleanup failure rather than leaving the next campaign to guess.
    pub fn close(self) -> Result<()> {
        ensure!(
            self.clean.get(),
            "owned Docker work needs inspection; the {LEASE} reservation is retained"
        );
        self.lease.remove()
    }

    fn within_budget(&self) -> Result<()> {
        ensure!(
            !tool::interrupted(),
            "verification was cancelled; evidence remains incomplete"
        );
        ensure!(
            self.started.elapsed() < self.limit,
            "the campaign reached its safety stop; verification remains incomplete"
        );
        Ok(())
    }

    /// The immutable container and image identities observed during this campaign.
    #[must_use]
    pub fn evidence(&self) -> Vec<serde_json::Value> {
        self.evidence.borrow().clone()
    }

    /// Builds an image with a private, bounded `BuildKit` instance.
    ///
    /// # Errors
    ///
    /// Refuses an unbounded builder and stops it before returning to verification.
    pub fn build(&self, image: &str, directory: &str, context: &Path) -> Result<()> {
        self.within_budget()?;
        let present = tool::output(
            Tool::Docker
                .command(self.root)
                .args(["image", "inspect", image]),
            "inspect the verification image",
        )?;
        if present.status.success() {
            return Ok(());
        }
        let name = format!(
            "{BUILDER_PREFIX}{}",
            self.lease
                .id
                .get(..ID_DIGITS)
                .context("short container ID")?
        );
        let builder = self.prepare_builder(&name)?;
        self.clean.set(false);
        let node = &builder.id;
        let log = self.root.join(BUILD_DIRECTORY).join(format!("{name}.log"));
        let result = (|| {
            tool::controlled(
                Tool::Docker
                    .command(self.root)
                    .args(["buildx", "inspect", "--bootstrap", &name]),
                "start the bounded builder",
                &log.with_extension("bootstrap.log"),
                || self.within_budget(),
            )?;
            let checked = inspect(self.root, node)?;
            ensure!(
                checked.host_config.limits().bounded()
                    && !checked.host_config.privileged
                    && matches!(checked.config.user.as_str(), "1000:1000" | "1000"),
                "the builder requires bounded, unprivileged rootless execution"
            );
            tool::controlled(
                Tool::Docker
                    .command(self.root)
                    .args([
                        "buildx",
                        "build",
                        "--builder",
                        &name,
                        "--load",
                        "--platform",
                        PLATFORM,
                        "--tag",
                        image,
                        "--file",
                    ])
                    .arg(self.root.join(directory).join("Dockerfile"))
                    .arg(context),
                "build the verification image",
                &log,
                || self.within_budget(),
            )
        })();
        self.finish_builder(&name, &builder, result)
    }

    fn finish_builder(&self, name: &str, builder: &Owned<'_>, result: Result<()>) -> Result<()> {
        let stopped = tool::succeed(
            Tool::Docker.command(self.root).args(["stop", &builder.id]),
            "stop the owned builder",
        );
        let evidence = self.collect(&builder.id);
        let inspected = evidence.and_then(|(state, _)| {
            ensure!(
                !state.state.running && !state.state.oom_killed,
                "the image build is incomplete; inspect {LOG_DIRECTORY}/{}.json",
                state.id
            );
            Ok(())
        });
        if inspected.is_err() {
            return finish(finish(result, stopped), inspected);
        }
        let removed = tool::succeed(
            Tool::Docker.command(self.root).args(["buildx", "rm", name]),
            "remove the owned builder connection",
        );
        let removed = finish(removed, builder.remove());
        if removed.is_ok() {
            self.clean.set(true);
        }
        finish(finish(result, stopped), removed)
    }

    fn prepare_builder(&self, name: &str) -> Result<Owned<'a>> {
        fs::create_dir_all(&self.root.join(BUILD_DIRECTORY))?;
        let config = self.root.join(BUILD_DIRECTORY).join("buildkitd.toml");
        fs::write(&config, BUILDKIT_CONFIG.as_bytes())?;
        let id = tool::capture(
            Tool::Docker
                .command(self.root)
                .args(builder_arguments(&config, &self.lease.id)),
            "create the bounded rootless builder",
        )?;
        let builder = Owned {
            root: self.root,
            id: id.trim().to_owned(),
        };
        self.clean.set(false);
        let result = (|| {
            tool::succeed(
                Tool::Docker.command(self.root).args(["start", &builder.id]),
                "start the rootless builder",
            )?;
            tool::succeed(
                Tool::Docker.command(self.root).args([
                    "buildx",
                    "create",
                    "--name",
                    name,
                    "--driver",
                    "remote",
                    &format!("docker-container://{}", builder.id),
                ]),
                "connect the private builder",
            )
        })();
        match result {
            Ok(()) => Ok(builder),
            Err(error) => {
                let stopped = tool::succeed(
                    Tool::Docker.command(self.root).args(["stop", &builder.id]),
                    "stop the incomplete builder",
                );
                let collected = self.collect(&builder.id);
                if let Err(collection) = collected {
                    return finish(finish(Err(error), stopped), Err(collection));
                }
                let removed = builder.remove();
                if removed.is_ok() {
                    self.clean.set(true);
                }
                finish(finish(Err(error), stopped), removed)
            }
        }
    }

    /// Runs the legacy argument plan through a bounded, inspected container.
    ///
    /// # Errors
    ///
    /// Reports OOM, timeout, and missing evidence as incomplete verification.
    pub fn run(&self, image: &str, arguments: &[OsString]) -> Result<Output> {
        self.within_budget()?;
        let identity = tool::capture(
            Tool::Docker
                .command(self.root)
                .args(["image", "inspect", "--format", "{{.Id}}", image]),
            "resolve the immutable worker image",
        )?;
        let mut arguments = create_arguments(image, arguments)?;
        arguments.splice(
            1..1,
            [
                OsString::from("--label"),
                format!("org.sivi.campaign={}", self.lease.id).into(),
            ],
        );
        let pinned = arguments
            .iter_mut()
            .find(|value| *value == image)
            .context("missing worker image")?;
        *pinned = identity.trim().into();
        let id = tool::capture(
            Tool::Docker.command(self.root).args(arguments),
            "create the verification worker",
        )?;
        let worker = Owned {
            root: self.root,
            id: id.trim().to_owned(),
        };
        self.clean.set(false);
        let monitored = self.wait(&worker);
        let stopped = if monitored.is_err() {
            tool::succeed(
                Tool::Docker.command(self.root).args(["stop", &worker.id]),
                "stop the owned worker",
            )
        } else {
            Ok(())
        };
        let (after, logs) = match self.collect(&worker.id) {
            Ok(collected) => collected,
            Err(error) => return finish(finish(Err(error), monitored), stopped),
        };
        ensure!(
            !after.state.running,
            "the owned worker still runs; its reservation is retained"
        );
        let result = finish(monitored, stopped).and_then(|()| {
            if !verification_rules::completed(
                after.state.exit_code,
                after.state.oom_killed,
                !after.state.error.is_empty(),
            ) {
                return Err(anyhow::Error::new(Incomplete {
                    oom: after.state.oom_killed,
                    exit: after.state.exit_code,
                })
                .context(format!(
                    "container {}; infrastructure: {}",
                    after.id, after.state.error
                )));
            }
            Ok(Output {
                status: Status(after.state.exit_code),
                stdout: logs.stdout,
                stderr: logs.stderr,
            })
        });
        let removed = worker.remove();
        if removed.is_ok() {
            self.clean.set(true);
        }
        finish(result, removed)
    }

    fn wait(&self, worker: &Owned<'_>) -> Result<()> {
        let before = inspect(self.root, &worker.id)?;
        ensure!(
            before.host_config.limits().bounded()
                && !before.host_config.privileged
                && before.host_config.network_mode == "none",
            "worker resource, privilege, or network limits weren't applied"
        );
        ensure!(
            before
                .mounts
                .iter()
                .any(|mount| mount.destination == crate::quint::SOURCE && !mount.writable),
            "the worker source mount isn't read-only"
        );
        let evidence = self
            .root
            .join(LOG_DIRECTORY)
            .join(format!("{}.json", before.id));
        fs::write(&evidence, serde_json::to_vec_pretty(&serde_json::json!({
            "container": before.id, "image": before.image,
            "cpu_nanos": before.host_config.nano_cpus, "memory": before.host_config.memory,
            "memory_swap": before.host_config.memory_swap, "processes": before.host_config.pids_limit,
            "state": "created"
        }))?.as_slice())?;
        tool::succeed(
            Tool::Docker.command(self.root).args(["start", &worker.id]),
            "start the verification worker",
        )?;
        loop {
            self.within_budget()?;
            let state = inspect(self.root, &worker.id)?;
            if !state.state.running {
                return Ok(());
            }
            tool::wait_for_interrupt(POLL)?;
        }
    }

    fn collect(&self, name: &str) -> Result<(Inspection, std::process::Output)> {
        let after = inspect(self.root, name)?;
        let evidence = self
            .root
            .join(LOG_DIRECTORY)
            .join(format!("{}.json", after.id));
        let logs = tool::output(
            Tool::Docker.command(self.root).args(["logs", name]),
            "read the bounded worker log",
        )?;
        ensure!(logs.status.success(), "the worker log is unavailable");
        fs::write(&evidence.with_extension("stdout"), &logs.stdout)?;
        fs::write(&evidence.with_extension("stderr"), &logs.stderr)?;
        fs::write(&evidence, serde_json::to_vec_pretty(&serde_json::json!({
            "container": after.id, "image": after.image,
            "cpu_nanos": after.host_config.nano_cpus, "memory": after.host_config.memory,
            "cpu_quota": after.host_config.cpu_quota, "cpu_period": after.host_config.cpu_period,
            "memory_swap": after.host_config.memory_swap, "processes": after.host_config.pids_limit,
            "role": after.config.labels.get("org.sivi.verification"),
            "exit_code": after.state.exit_code, "oom_killed": after.state.oom_killed,
            "error": after.state.error, "elapsed_seconds": self.started.elapsed().as_secs()
        }))?.as_slice())?;
        self.evidence.borrow_mut().push(serde_json::json!({"container": after.id, "image": after.image, "evidence": crate::portable(&evidence)}));
        Ok((after, logs))
    }
}

pub(crate) fn finish<T>(result: Result<T>, cleanup: Result<()>) -> Result<T> {
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("cleanup also failed: {cleanup:#}")))
        }
    }
}

fn builder_arguments(config: &Path, campaign: &str) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = [
        "create",
        "--init",
        "--label",
        "org.sivi.verification=builder",
        "--label",
        &format!("org.sivi.campaign={campaign}"),
        "--log-driver",
        "local",
        "--log-opt",
        "max-size=10m",
        "--log-opt",
        "max-file=1",
        "--security-opt",
        "seccomp=unconfined",
        "--security-opt",
        "apparmor=unconfined",
        "--security-opt",
        "systempaths=unconfined",
        "--entrypoint",
        "timeout",
    ]
    .map(OsString::from)
    .into();
    arguments.extend(limit_arguments().map(OsString::from));
    arguments.extend(
        [
            "--mount",
            "type=volume,source=sivi-proof-build-cache,target=/home/user/.local/share/buildkit",
            "--mount",
        ]
        .map(OsString::from),
    );
    arguments.push(crate::quint::mount(config, BUILDKIT_CONFIG_PATH, true));
    arguments.extend(
        [
            BUILDKIT_IMAGE,
            "-k",
            "10",
            WORKER_TIMEOUT,
            "rootlesskit",
            "buildkitd",
            "--config",
            BUILDKIT_CONFIG_PATH,
        ]
        .map(OsString::from),
    );
    arguments
}

const fn limit_arguments() -> [&'static str; 12] {
    [
        "--cpus",
        "2",
        "--memory",
        "4g",
        "--memory-swap",
        "4g",
        "--pids-limit",
        "256",
        "--restart",
        "no",
        "--stop-timeout",
        "10",
    ]
}

/// Converts the command plan into a worker with mandatory resource and log ceilings.
///
/// # Errors
///
/// Refuses an ambiguous image, a changed network plan, or competing resource options.
pub fn create_arguments(image: &str, arguments: &[OsString]) -> Result<Vec<OsString>> {
    let prefix: Vec<OsString> = ["run", "--rm", "--network", "none"]
        .map(OsString::from)
        .into();
    ensure!(
        arguments.starts_with(&prefix),
        "the worker plan must disable networking"
    );
    let indices: Vec<usize> = arguments
        .iter()
        .enumerate()
        .filter_map(|(index, value)| (value == image).then_some(index))
        .collect();
    let [image_index] = indices.as_slice() else {
        bail!("ambiguous worker image");
    };
    let options = arguments
        .get(prefix.len()..*image_index)
        .context("worker options precede its image")?;
    let mut options_to_check = options.iter();
    while let Some(flag) = options_to_check.next() {
        ensure!(
            matches!(flag.to_str(), Some("--mount" | "--workdir")),
            "unsupported worker option: {}",
            flag.display()
        );
        options_to_check
            .next()
            .context("worker option has no value")?;
    }
    let mut result: Vec<OsString> = [
        "create",
        "--network",
        "none",
        "--init",
        "--security-opt",
        "no-new-privileges=true",
        "--label",
        "org.sivi.verification=worker",
        "--log-driver",
        "local",
        "--log-opt",
        &format!("max-size={LOG_LIMIT}"),
        "--log-opt",
        "max-file=1",
        "--entrypoint",
        TIMEOUT_PROGRAM,
        "--platform",
        PLATFORM,
    ]
    .map(OsString::from)
    .into();
    result.extend(limit_arguments().map(OsString::from));
    result.extend_from_slice(options);
    result.push(image.into());
    result.extend(["--kill-after=10s", WORKER_TIMEOUT].map(OsString::from));
    result.extend_from_slice(
        arguments
            .get(image_index.saturating_add(1)..)
            .context("the worker has a program")?,
    );
    Ok(result)
}

/// The image tag binds its recipe and the dependency lockfiles it consumes.
///
/// # Errors
///
/// Reports missing image inputs.
pub fn source_digest(root: &Path) -> Result<String> {
    let listing = tool::capture(
        Tool::Git.command(root).args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ]),
        "list verification inputs",
    )?;
    let mut paths: Vec<&str> = listing
        .split('\0')
        .filter(|path| !path.is_empty())
        .collect();
    paths.sort_unstable();
    paths.dedup();
    let mut digest = sha2::Sha256::new();
    for path in paths {
        if fs::is_file(&root.join(path))? {
            digest.update(path.len().to_le_bytes());
            digest.update(path.as_bytes());
            let bytes = fs::read(&root.join(path))?;
            digest.update(bytes.len().to_le_bytes());
            digest.update(bytes);
        }
    }
    let mut text = String::new();
    for byte in digest.finalize() {
        write!(text, "{byte:02x}")?;
    }
    Ok(text)
}

/// The image includes the exact checkout and pinned tools.
///
/// # Errors
///
/// Reports missing source inputs.
pub fn rust_image(root: &Path) -> Result<String> {
    Ok(format!("sivi-proofs:{}", source_digest(root)?))
}

/// The reference image changes with its immutable base pin.
///
/// # Errors
///
/// Reports an unreadable image recipe.
pub fn prolog_image(root: &Path) -> Result<String> {
    Ok(format!(
        "sivi-prolog:{}",
        crate::fonts::digest(&fs::read(&root.join(PROLOG_RECIPE))?)
    ))
}

/// Refuses the internal worker outside the bounded Linux container.
///
/// # Errors
///
/// Reports missing or unbounded kernel cgroup limits.
pub fn require_worker() -> Result<()> {
    ensure!(
        std::env::consts::OS == "linux" && std::env::consts::ARCH == "x86_64",
        "proof workers require the pinned linux/amd64 image"
    );
    ensure!(
        fs::is_file(Path::new(DOCKER_MARKER))?,
        "proof workers require Docker"
    );
    let cpu = fs::read_to_string(Path::new(CGROUP_CPU))?;
    let cpu: Vec<&str> = cpu.split_whitespace().collect();
    let [quota, period] = cpu.as_slice() else {
        bail!("invalid cpu.max");
    };
    let memory: i64 = fs::read_to_string(Path::new(CGROUP_MEMORY))?
        .trim()
        .parse()?;
    let swap: i64 = fs::read_to_string(Path::new(CGROUP_SWAP))?.trim().parse()?;
    let processes: i64 = fs::read_to_string(Path::new(CGROUP_PIDS))?.trim().parse()?;
    let limits = Limits {
        cpu_nanos: 0,
        cpu_quota: quota.parse()?,
        cpu_period: period.parse()?,
        memory,
        memory_swap: memory.checked_add(swap).context("invalid swap limit")?,
        processes,
    };
    ensure!(
        limits.bounded(),
        "the worker's kernel limits exceed the proof budget"
    );
    Ok(())
}

/// Runs the compiled worker against read-only source and job-scoped output mounts.
///
/// # Errors
///
/// Reports incomplete or failed verification and preserves the job's outputs.
pub fn rust_worker(root: &Path, campaign: &Campaign<'_>, words: &[&str]) -> Result<()> {
    let output = rust_output(root, campaign, words)?;
    ensure!(
        output.status.success(),
        "proof worker failed with {}; see target/proofs/jobs",
        output.status
    );
    Ok(())
}

/// Lists reservations and workers without starting or changing them.
///
/// # Errors
///
/// Reports an unavailable Docker engine.
pub fn status(root: &Path) -> Result<()> {
    tool::succeed(
        Tool::Docker.command(root).args([
            "ps",
            "--all",
            "--no-trunc",
            "--filter",
            "label=org.sivi.verification",
            "--format",
            "{{.ID}} {{.Names}} {{.State}}",
        ]),
        "inspect verification occupancy",
    )
}

/// Collects and removes only the exact reservation explicitly named by the caller.
///
/// # Errors
///
/// Refuses a changed reservation, unknown ownership, or incomplete cleanup evidence.
pub fn cleanup(root: &Path, identity: &str) -> Result<()> {
    let lease = inspect(root, LEASE)?;
    ensure!(
        lease.id == identity
            && lease
                .config
                .labels
                .get("org.sivi.verification")
                .is_some_and(|role| role == "lease"),
        "cleanup requires the exact current reservation ID"
    );
    let campaign = Campaign {
        root,
        lease: Owned { root, id: lease.id },
        started: tool::Timer::start(),
        limit: CAMPAIGN_LIMIT,
        clean: Cell::new(false),
        evidence: RefCell::new(Vec::new()),
    };
    let filter = format!("label=org.sivi.campaign={identity}");
    let members = tool::capture(
        Tool::Docker.command(root).args([
            "ps",
            "--all",
            "--quiet",
            "--no-trunc",
            "--filter",
            &filter,
        ]),
        "find owned verification objects",
    )?;
    for id in members.lines().filter(|id| !id.is_empty()) {
        let member = inspect(root, id)?;
        ensure!(
            member
                .config
                .labels
                .get("org.sivi.campaign")
                .is_some_and(|owner| owner == identity)
                && member
                    .config
                    .labels
                    .get("org.sivi.verification")
                    .is_some_and(|role| matches!(role.as_str(), "builder" | "worker")),
            "refuse cleanup of an object without matching ownership: {id}"
        );
        tool::succeed(
            Tool::Docker.command(root).args(["stop", &member.id]),
            "stop the owned verification object",
        )?;
        let (stopped, _) = campaign.collect(&member.id)?;
        ensure!(!stopped.state.running, "owned object still runs: {id}");
        Owned {
            root,
            id: member.id,
        }
        .remove()?;
    }
    let remaining = tool::capture(
        Tool::Docker
            .command(root)
            .args(["ps", "--all", "--quiet", "--filter", &filter]),
        "confirm verification cleanup",
    )?;
    ensure!(
        remaining.trim().is_empty(),
        "verification objects remain; reservation retained"
    );
    campaign.clean.set(true);
    campaign.close()
}

/// Keeps verifier outputs available after success, refusal, or cancellation.
pub(crate) fn job(root: &Path, prefix: &str) -> Result<PathBuf> {
    let jobs = root.join(JOB_DIRECTORY);
    fs::create_dir_all(&jobs)?;
    let job = tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(&jobs)
        .context("create the proof job directory")?
        .keep();
    eprintln!("proof outputs: {}", job.display());
    Ok(job)
}

fn rust_output(root: &Path, campaign: &Campaign<'_>, words: &[&str]) -> Result<Output> {
    let image = rust_image(root)?;
    campaign.build(&image, RUST_IMAGE_DIRECTORY, root)?;
    let job = job(root, "rust-")?;
    fs::create_dir_all(&job.join("tmp"))?;
    for corpus in CORPORA {
        fs::copy_directory(&root.join(corpus), &job.join(corpus))?;
    }
    let session = crate::quint::Session {
        root,
        output: &job,
        image: &image,
    };
    let mut arguments = crate::quint::program_arguments(session, PROOF_WORKER, words);
    let image_index = arguments
        .iter()
        .position(|value| value == image.as_str())
        .context("worker image")?;
    let mut mounts = Vec::new();
    for directory in WRITABLE_DIRECTORIES {
        let output = job.join(directory);
        fs::create_dir_all(&output)?;
        mounts.extend([
            OsString::from("--mount"),
            crate::quint::mount(
                &output,
                &format!("{}/{directory}", crate::quint::SOURCE),
                false,
            ),
        ]);
    }
    arguments.splice(image_index..image_index, mounts);
    let output = campaign.run(&image, &arguments)?;
    eprint!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}

/// Exercises the real Docker boundary, including intentional incomplete results.
///
/// # Errors
///
/// Reports missing limits, accepted resource exhaustion, or a broken worker boundary.
pub fn self_check(root: &Path) -> Result<()> {
    let started = tool::Timer::start();
    for probe in ["mounts", "failure", "timeout", "oom"] {
        let remaining = CAMPAIGN_LIMIT
            .checked_sub(started.elapsed())
            .context("resource boundary checks reached their safety stop")?;
        let campaign = Campaign::open_for(root, remaining)?;
        let result = rust_output(root, &campaign, &["proof-boundary", probe]);
        let checked = match (probe, result) {
            ("mounts", Ok(output)) if output.status.success() => Ok(()),
            ("failure", Ok(output)) if output.status.code() == i32::from(BOUNDARY_FAILURE) => {
                Ok(())
            }
            ("timeout", Err(error))
                if error.downcast_ref::<Incomplete>().is_some_and(|state| {
                    !state.oom && state.exit == i32::from(BOUNDARY_TIMEOUT)
                }) =>
            {
                Ok(())
            }
            ("oom", Err(error))
                if error
                    .downcast_ref::<Incomplete>()
                    .is_some_and(|state| state.oom) =>
            {
                Ok(())
            }
            (_, Err(error)) => Err(error),
            (_, Ok(output)) => Err(anyhow::anyhow!(
                "boundary {probe} returned an unexpected {}",
                output.status
            )),
        };
        finish(checked, campaign.close())?;
    }
    Ok(())
}

const BOUNDARY_FAILURE: u8 = 42;
const OVER_BUDGET_ALLOCATION: usize = 5 * 1024 * 1024 * 1024;

/// The isolated negative fixtures of the container runner.
///
/// # Errors
///
/// Refuses native execution or an unknown fixture.
pub fn boundary(root: &Path, probe: &str) -> Result<u8> {
    require_worker()?;
    match probe {
        "mounts" => {
            let input = root.join("Cargo.toml");
            let bytes = fs::read(&input)?;
            ensure!(
                fs::write(&input, &bytes).is_err(),
                "the source mount is writable"
            );
            fs::write(Path::new(BOUNDARY_OUTPUT), b"writable job output")?;
            Ok(0)
        }
        "failure" => Ok(BOUNDARY_FAILURE),
        "timeout" => {
            let output = tool::output(
                Tool::Timeout.command(root).args([
                    "--kill-after=1s",
                    BOUNDARY_DEADLINE,
                    "sleep",
                    BOUNDARY_PAUSE,
                ]),
                "exercise the worker deadline",
            )?;
            ensure!(
                output.status.code() == Some(i32::from(BOUNDARY_TIMEOUT)),
                "the deadline fixture didn't stop its child: {}",
                output.status
            );
            Ok(BOUNDARY_TIMEOUT)
        }
        "oom" => {
            let bytes = vec![1_u8; OVER_BUDGET_ALLOCATION];
            drop(std::hint::black_box(bytes));
            bail!("the over-budget allocation completed without an OOM stop")
        }
        _ => bail!("unknown resource boundary {probe}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{create_arguments, finish};
    use std::ffi::OsString;

    #[test]
    fn model_jobs_keep_separate_evidence_after_their_creation_scope_ends() {
        let root = tempfile::tempdir().unwrap();
        let first = super::job(root.path(), "quint-").unwrap();
        let second = super::job(root.path(), "quint-").unwrap();
        assert_ne!(first, second);
        assert!(first.starts_with(root.path().join(super::JOB_DIRECTORY)));
        crate::fs::write(&first.join("evidence.json"), b"first").unwrap();
        crate::fs::write(&second.join("evidence.json"), b"second").unwrap();
        assert_eq!(
            crate::fs::read(&first.join("evidence.json")).unwrap(),
            b"first"
        );
        assert_eq!(
            crate::fs::read(&second.join("evidence.json")).unwrap(),
            b"second"
        );
    }

    #[test]
    fn image_recipes_keep_the_verifier_versions_the_gate_requires() {
        let recipe = include_str!("../proofs/rust/Dockerfile");
        for (package, version) in [
            ("kani-verifier", crate::tool::KANI_VERSION),
            ("cargo-fuzz", crate::tool::FUZZ_VERSION),
            ("cargo-mutants", crate::tool::MUTANTS_VERSION),
        ] {
            assert!(recipe.contains(&format!("{package} --version {version}")));
        }
        assert!(recipe.contains(crate::tool::NIGHTLY));
        assert!(recipe.contains(crate::verus::TOOLCHAIN));
        assert!(recipe.contains(crate::verus::VERSION));
        assert!(recipe.contains(crate::smt::Z3_VERSION));
        let reference = include_str!("../proofs/prolog/Dockerfile");
        assert!(reference.contains(crate::reference::SWIPL_VERSION));
    }

    #[test]
    fn the_builder_is_rootless_bounded_and_mount_options_stay_paired() {
        let arguments = super::builder_arguments(std::path::Path::new("/config"), "lease");
        assert!(arguments.contains(&OsString::from(super::BUILDKIT_IMAGE)));
        assert!(!arguments.contains(&OsString::from("--privileged")));
        let mounts: Vec<_> = arguments
            .windows(2)
            .filter(|pair| pair[0] == "--mount")
            .collect();
        assert_eq!(mounts.len(), 2);
        assert!(
            mounts
                .iter()
                .all(|pair| pair[1].to_string_lossy().starts_with("type="))
        );
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["--memory-swap", "4g"].map(OsString::from))
        );
    }

    #[test]
    fn every_worker_has_limits_an_internal_deadline_and_bounded_logs() {
        let input: Vec<OsString> = ["run", "--rm", "--network", "none", "image", "false"]
            .map(OsString::from)
            .into();
        let output = create_arguments("image", &input).unwrap();
        for pair in [
            ["--memory", "4g"],
            ["--memory-swap", "4g"],
            ["--cpus", "2"],
            ["--pids-limit", "256"],
            ["--log-driver", "local"],
        ] {
            assert!(
                output
                    .windows(2)
                    .any(|window| window == pair.map(OsString::from))
            );
        }
        assert_eq!(output.last(), Some(&OsString::from("false")));
        assert!(!output.contains(&OsString::from("--rm")));
        let mut unbounded = input.clone();
        unbounded.insert(4, "--memory=0".into());
        create_arguments("image", &unbounded).unwrap_err();
        create_arguments("missing", &input).unwrap_err();
    }

    #[test]
    fn a_cleanup_failure_preserves_the_primary_error() {
        let error = finish::<()>(
            Err(anyhow::anyhow!("primary")),
            Err(anyhow::anyhow!("cleanup")),
        )
        .unwrap_err();
        let text = format!("{error:#}");
        assert!(text.contains("primary") && text.contains("cleanup"));
    }
}
