//! The one owner of writes to the output: the PDF is staged beside its path, synced, and renamed into place, so the path never names a partial file.
//!
//! The order of the steps and what a failure undoes come from `publish_rules`; `specs/publish.tla` checks the protocol with a crash at every step, and the tests here drive every failure through a fake file system.

use sivi_core::publish_rules::{Action, Cleanup, Stage, after, cleanup, next};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// The suffix of the staging file beside the output.
const STAGING_SUFFIX: &str = ".tmp";

/// The file operations the publication needs, which the disk provides and the tests fake.
pub trait Fs {
    /// Creates the staging file, empty, and keeps it open.
    ///
    /// # Errors
    ///
    /// Returns the error of the file system.
    fn create(&mut self, staging: &Path) -> io::Result<()>;

    /// Writes every byte to the open staging file.
    ///
    /// # Errors
    ///
    /// Returns the error of the file system.
    fn write(&mut self, bytes: &[u8]) -> io::Result<()>;

    /// Forces the staging file's bytes to the disk and closes it.
    ///
    /// # Errors
    ///
    /// Returns the error of the file system.
    fn sync(&mut self) -> io::Result<()>;

    /// Replaces `target` with `staging` in one step.
    ///
    /// # Errors
    ///
    /// Returns the error of the file system.
    fn rename(&mut self, staging: &Path, target: &Path) -> io::Result<()>;

    /// Deletes the staging file after a failure.
    ///
    /// # Errors
    ///
    /// Returns the error of the file system.
    fn remove(&mut self, staging: &Path) -> io::Result<()>;
}

mod raw {
    #![expect(
        clippy::disallowed_methods,
        reason = "the binary's one publication owner reaches the file system in single statements"
    )]

    use std::fs::File;
    use std::io::{self, Write as _};
    use std::path::Path;

    pub(super) fn create(path: &Path) -> io::Result<File> {
        File::create(path)
    }

    pub(super) fn write(file: &mut File, bytes: &[u8]) -> io::Result<()> {
        file.write_all(bytes)
    }

    pub(super) fn sync(file: &File) -> io::Result<()> {
        file.sync_all()
    }

    pub(super) fn rename(from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    pub(super) fn remove(path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }
}

/// The real file system, holding the staging file open between the steps.
#[derive(Debug, Default)]
pub struct Disk {
    open: Option<std::fs::File>,
}

impl Fs for Disk {
    fn create(&mut self, staging: &Path) -> io::Result<()> {
        self.open = Some(raw::create(staging)?);
        Ok(())
    }

    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.open.as_mut().map_or_else(
            || Err(io::Error::other("no staging file is open")),
            |file| raw::write(file, bytes),
        )
    }

    fn sync(&mut self) -> io::Result<()> {
        self.open.take().map_or_else(
            || Err(io::Error::other("no staging file is open")),
            |file| raw::sync(&file),
        )
    }

    fn rename(&mut self, staging: &Path, target: &Path) -> io::Result<()> {
        raw::rename(staging, target)
    }

    fn remove(&mut self, staging: &Path) -> io::Result<()> {
        self.open = None;
        raw::remove(staging)
    }
}

/// Why the output was not published, with the step that failed and what the cleanup left.
#[derive(Debug)]
pub struct PublishError {
    pub action: Action,
    pub error: io::Error,
    /// The error of removing the staging file, when that failed too.
    pub cleanup: Option<io::Error>,
    pub staging: PathBuf,
}

impl fmt::Display for PublishError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let step = match self.action {
            Action::Create => "create the staging file",
            Action::Write => "write the staging file",
            Action::Sync => "sync the staging file to the disk",
            Action::Rename => "rename the staging file into place",
        };
        write!(
            formatter,
            "cannot {step} {}: {}",
            self.staging.display(),
            self.error
        )?;
        if let Some(cleanup) = &self.cleanup {
            write!(
                formatter,
                "; the staging file could not be removed either: {cleanup}"
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for PublishError {}

/// The staging path beside `target`, distinct per process.
#[must_use]
pub fn staging_path(target: &Path, process: u32) -> PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(format!(".sivi-{process}{STAGING_SUFFIX}"));
    PathBuf::from(name)
}

/// Publishes `bytes` at `target` through `fs`, or leaves `target` as it was.
///
/// # Errors
///
/// Returns the step that failed; the staging file is removed when it exists.
pub fn publish(fs: &mut impl Fs, target: &Path, bytes: &[u8]) -> Result<(), PublishError> {
    let staging = staging_path(target, std::process::id());
    let mut stage = Stage::default();
    while let Some(action) = next(stage) {
        let outcome = match action {
            Action::Create => fs.create(&staging),
            Action::Write => fs.write(bytes),
            Action::Sync => fs.sync(),
            Action::Rename => fs.rename(&staging, target),
        };
        if let Err(error) = outcome {
            let cleanup = match cleanup(stage) {
                Cleanup::RemoveStaged => fs.remove(&staging).err(),
                Cleanup::Nothing => None,
            };
            return Err(PublishError {
                action,
                error,
                cleanup,
                staging,
            });
        }
        stage = after(action);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Fs, publish, staging_path};
    use sivi_core::publish_rules::Action;
    use std::collections::BTreeMap;
    use std::io;
    use std::path::{Path, PathBuf};

    /// A file system in memory whose step `fails_at` fails, recording every file as the steps leave it.
    #[derive(Debug, Default)]
    struct Fake {
        files: BTreeMap<PathBuf, Vec<u8>>,
        open: Option<PathBuf>,
        fails_at: Option<Action>,
        log: Vec<&'static str>,
    }

    impl Fake {
        fn failing(target: &Path, fails_at: Option<Action>) -> Self {
            let mut files = BTreeMap::new();
            files.insert(target.to_path_buf(), b"old".to_vec());
            Self {
                files,
                open: None,
                fails_at,
                log: Vec::new(),
            }
        }

        fn fail_if(&mut self, action: Action, step: &'static str) -> io::Result<()> {
            self.log.push(step);
            if self.fails_at == Some(action) {
                Err(io::Error::other(step))
            } else {
                Ok(())
            }
        }
    }

    impl Fs for Fake {
        fn create(&mut self, staging: &Path) -> io::Result<()> {
            self.fail_if(Action::Create, "create")?;
            self.files.insert(staging.to_path_buf(), Vec::new());
            self.open = Some(staging.to_path_buf());
            Ok(())
        }

        fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
            self.fail_if(Action::Write, "write")?;
            let open = self
                .open
                .clone()
                .ok_or_else(|| io::Error::other("nothing open"))?;
            self.files
                .get_mut(&open)
                .ok_or_else(|| io::Error::other("the open file vanished"))?
                .extend_from_slice(bytes);
            Ok(())
        }

        fn sync(&mut self) -> io::Result<()> {
            self.fail_if(Action::Sync, "sync")?;
            self.open = None;
            Ok(())
        }

        fn rename(&mut self, staging: &Path, target: &Path) -> io::Result<()> {
            self.fail_if(Action::Rename, "rename")?;
            let bytes = self
                .files
                .remove(staging)
                .ok_or_else(|| io::Error::other("no staging file"))?;
            self.files.insert(target.to_path_buf(), bytes);
            Ok(())
        }

        fn remove(&mut self, staging: &Path) -> io::Result<()> {
            self.log.push("remove");
            self.open = None;
            self.files
                .remove(staging)
                .map(|_bytes| ())
                .ok_or_else(|| io::Error::other("no staging file to remove"))
        }
    }

    const ACTIONS: [Action; 4] = [Action::Create, Action::Write, Action::Sync, Action::Rename];

    #[test]
    fn a_publication_writes_syncs_then_renames_and_leaves_no_staging_file() {
        let target = Path::new("/out/deck.pdf");
        let mut fake = Fake::failing(target, None);
        publish(&mut fake, target, b"new").unwrap();
        assert_eq!(fake.log, ["create", "write", "sync", "rename"]);
        assert_eq!(fake.files.len(), 1);
        assert_eq!(fake.files.get(target).map(Vec::as_slice), Some(&b"new"[..]));
    }

    #[test]
    fn a_failure_at_any_step_leaves_the_old_output_and_removes_the_staging_file() {
        let target = Path::new("/out/deck.pdf");
        for action in ACTIONS {
            let mut fake = Fake::failing(target, Some(action));
            let error = publish(&mut fake, target, b"new").unwrap_err();
            assert_eq!(error.action, action);
            assert!(error.cleanup.is_none(), "{action:?}: {error}");
            assert_eq!(
                fake.files.get(target).map(Vec::as_slice),
                Some(&b"old"[..]),
                "{action:?}"
            );
            assert_eq!(fake.files.len(), 1, "{action:?}: {:?}", fake.files);
            assert_eq!(
                fake.log.contains(&"remove"),
                action != Action::Create,
                "{action:?}: {:?}",
                fake.log
            );
            assert!(
                error
                    .to_string()
                    .contains(&format!("{}", error.staging.display()))
            );
        }
    }

    #[test]
    fn the_staging_path_sits_beside_the_target_and_names_the_process() {
        let staging = staging_path(Path::new("out/deck.pdf"), 42);
        assert_eq!(staging, PathBuf::from("out/deck.pdf.sivi-42.tmp"));
    }
}
