//! The font library: where installed families live, how `sivi fonts` lists, installs, and locates them, and how a theme's catalog families are read back.

use crate::args::{Environment, FontsCommand, Format};
use crate::cli::{DIAGNOSTIC_EXIT_CODE, Run, SUCCESS_EXIT_CODE, USAGE_EXIT_CODE};
use crate::digest::sha256_hex;
use crate::io::{self, ReadErrorKind};
use crate::net::{Fetch, FetchError, Web};
use crate::publish::{self, Disk, Fs, PublishError};
use crate::render::{self, Sources};
use sivi_core::diagnostic::{Code, Diagnostic};
use sivi_core::home_rules::{self, HomeFacts};
use sivi_core::master_rules::closest_text;
use sivi_theme::catalog::{self, Catalog, Family};
use std::path::{Path, PathBuf};

/// The directory of installed fonts under the data directory.
pub const FONTS_DIRECTORY: &str = "fonts";
/// The format name of the JSON listing.
pub const LISTING_FORMAT: &str = "sivi.fonts";
/// The version of the JSON listing.
pub const LISTING_VERSION: u32 = 1;
/// Bytes per megabyte, as the listings round sizes.
const MEGABYTE: u64 = 1_000_000;
/// Tenths of a megabyte, for one decimal.
const TENTH_OF_MEGABYTE: u64 = MEGABYTE / 10;
const TENTHS: u64 = 10;

/// The state of a family on this host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    /// The binary carries the family.
    Bundled,
    /// Every file of the family sits in the library.
    Installed,
    /// The family can be installed.
    Available,
}

impl State {
    /// The state as the listings spell it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::Installed => "installed",
            Self::Available => "available",
        }
    }
}

/// The data directory the environment names, where the library lives.
#[must_use]
pub fn data_dir(environment: &Environment) -> Option<PathBuf> {
    let facts = HomeFacts {
        sivi_home: environment.sivi_home.as_deref(),
        xdg_data_home: environment.xdg_data_home.as_deref(),
        home: environment.home.as_deref(),
        local_app_data: environment.local_app_data.as_deref(),
        windows: environment.windows,
    };
    let decided = home_rules::data_dir(&facts)?;
    let mut path = PathBuf::from(decided.root);
    for segment in decided.tail {
        path.push(segment);
    }
    Some(path)
}

/// The directory of installed fonts, when the environment names a data directory.
#[must_use]
pub fn fonts_dir(environment: &Environment) -> Option<PathBuf> {
    data_dir(environment).map(|dir| dir.join(FONTS_DIRECTORY))
}

/// Where `file` of `family` lives once installed.
#[must_use]
pub fn installed_path(fonts_dir: &Path, family: &Family, file: &str) -> PathBuf {
    fonts_dir.join(&family.path).join(file)
}

/// A size in megabytes with one decimal, as the listings print it.
#[must_use]
pub fn megabytes(bytes: u64) -> String {
    let tenths = bytes / TENTH_OF_MEGABYTE;
    format!("{}.{} MB", tenths / TENTHS, tenths % TENTHS)
}

/// The command that installs `family`, as the help lines quote it.
fn install_command(family: &Family) -> String {
    format!("sivi fonts install \"{}\"", family.name)
}

/// The diagnostic of a family the theme sets text in that is neither bundled, declared, nor in the catalog.
#[must_use]
pub fn unknown_family(name: &str, catalog: &Catalog, bundled: &[String]) -> Diagnostic {
    let candidates = catalog.names().chain(bundled.iter().map(String::as_str));
    let diagnostic = Diagnostic::new(
        Code::FamilyUnknown,
        format!("the family `{name}` is neither bundled nor in the font catalog"),
    );
    match closest_text(name, candidates) {
        Some(closest) => diagnostic.help(format!("the closest family is `{closest}`")),
        None => diagnostic.help(String::from(
            "name a bundled family, a family of `sivi fonts list`, or a file under `[[fonts]]`",
        )),
    }
}

/// The diagnostic of a catalog family the theme needs that the library does not hold.
#[must_use]
pub fn not_installed(family: &Family, fonts_dir: Option<&Path>) -> Diagnostic {
    let diagnostic = Diagnostic::new(
        Code::FontNotInstalled,
        format!("the family `{}` is not installed", family.name),
    )
    .note(format!(
        "`{}` is {} from Google Fonts under {}",
        family.listing(),
        megabytes(family.bytes()),
        family.license
    ));
    match fonts_dir {
        Some(_) => diagnostic.help(format!("run `{}`", install_command(family))),
        None => diagnostic.help(format!(
            "set SIVI_HOME to a directory for installed fonts, then run `{}`",
            install_command(family)
        )),
    }
}

/// The diagnostic of an installed file whose bytes differ from the catalog's digest.
fn stale(family: &Family, file: &str, actual: &str) -> Diagnostic {
    Diagnostic::new(
        Code::FontDigest,
        format!(
            "the installed file `{file}` of `{}` does not match the digest the catalog pins",
            family.name
        ),
    )
    .note(format!("the file's digest is {actual}"))
    .help(format!("run `{}` again", install_command(family)))
}

/// The bytes of every file of an installed family, each checked against the catalog.
///
/// # Errors
///
/// Returns the diagnostic of the first file that is missing, unreadable, or stale.
pub fn read_installed(family: &Family, fonts_dir: &Path) -> Result<Vec<Vec<u8>>, Box<Diagnostic>> {
    let mut files = Vec::new();
    for file in &family.file {
        let path = installed_path(fonts_dir, family, &file.name);
        let bytes = match io::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                let missing = matches!(&error.kind, ReadErrorKind::Io(inner) if inner.kind() == std::io::ErrorKind::NotFound);
                return Err(Box::new(if missing {
                    not_installed(family, Some(fonts_dir))
                } else {
                    Diagnostic::new(
                        Code::FontUnreadable,
                        format!("the installed file `{}` cannot be read: {error}", file.name),
                    )
                }));
            }
        };
        let digest = sha256_hex(&[&bytes]);
        if digest != file.sha256 {
            return Err(Box::new(stale(family, &file.name, &digest)));
        }
        files.push(bytes);
    }
    Ok(files)
}

/// Whether every file of `family` sits in the library.
///
/// # Errors
///
/// Returns the read error when a file's presence cannot be told.
pub fn is_installed(family: &Family, fonts_dir: &Path) -> Result<bool, io::ReadError> {
    for file in &family.file {
        if !io::is_file(&installed_path(fonts_dir, family, &file.name))? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The state of `family`: bundled, installed, or available.
///
/// # Errors
///
/// Returns the read error when a file's presence cannot be told.
pub fn state(
    family: &Family,
    bundled: &[String],
    fonts_dir: Option<&Path>,
) -> Result<State, io::ReadError> {
    if bundled.contains(&family.name) {
        return Ok(State::Bundled);
    }
    match fonts_dir {
        Some(dir) if is_installed(family, dir)? => Ok(State::Installed),
        Some(_) | None => Ok(State::Available),
    }
}

/// What an installation brought.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Installed {
    pub files: usize,
    pub bytes: u64,
}

/// Why an installation stopped.
#[derive(Debug)]
pub enum InstallError {
    Fetch(FetchError),
    /// A fetched file whose digest differs from the catalog's.
    Digest {
        file: String,
        actual: String,
    },
    Directory(io::ReadError),
    Publish(PublishError),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fetch(error) => write!(formatter, "{error}"),
            Self::Digest { file, actual } => write!(
                formatter,
                "the fetched file `{file}` does not match the digest the catalog pins; its digest is {actual}"
            ),
            Self::Directory(error) => write!(formatter, "{error}"),
            Self::Publish(error) => write!(formatter, "{error}"),
        }
    }
}

/// The web and the file system an installation goes through.
pub struct Installer<'a, F: Fetch, S: Fs> {
    pub fetch: &'a mut F,
    pub fs: &'a mut S,
}

impl<F: Fetch, S: Fs> std::fmt::Debug for Installer<'_, F, S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Installer")
    }
}

/// One file to fetch, check, and publish.
struct Download<'a> {
    target: &'a Path,
    url: &'a str,
    /// The most bytes the file may hold.
    limit: u64,
    /// The digest the file must have.
    digest: &'a str,
}

impl<F: Fetch, S: Fs> Installer<'_, F, S> {
    /// Fetches, checks, and publishes one file.
    fn file(&mut self, download: &Download<'_>) -> Result<(), InstallError> {
        let bytes = self
            .fetch
            .fetch(download.url, download.limit)
            .map_err(InstallError::Fetch)?;
        let actual = sha256_hex(&[&bytes]);
        if actual != download.digest {
            return Err(InstallError::Digest {
                file: download.target.display().to_string(),
                actual,
            });
        }
        publish::publish(self.fs, download.target, &bytes).map_err(InstallError::Publish)
    }

    /// Installs `family`: fetches each file and the license at the catalog's commit, checks every digest, and publishes them under `fonts_dir`.
    ///
    /// # Errors
    ///
    /// Returns why the installation stopped; the files published before the stop stay.
    pub fn install(
        &mut self,
        catalog: &Catalog,
        family: &Family,
        fonts_dir: &Path,
    ) -> Result<Installed, InstallError> {
        io::create_dir_all(&fonts_dir.join(&family.path)).map_err(InstallError::Directory)?;
        let mut total: u64 = 0;
        for file in &family.file {
            let target = installed_path(fonts_dir, family, &file.name);
            self.file(&Download {
                target: &target,
                url: &family.url(&catalog.commit, &file.name),
                limit: file.bytes.saturating_add(1),
                digest: &file.sha256,
            })?;
            total = total.saturating_add(file.bytes);
        }
        if let (Some(name), Some(digest)) = (&family.license_file, &family.license_sha256) {
            let target = installed_path(fonts_dir, family, name);
            self.file(&Download {
                target: &target,
                url: &family.url(&catalog.commit, name),
                limit: io::MAX_READ,
                digest,
            })?;
        }
        Ok(Installed {
            files: family.file.len(),
            bytes: total,
        })
    }
}

/// One family as the listings show it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    pub name: String,
    pub listed: String,
    pub category: &'static str,
    pub scripts: Vec<String>,
    pub weights: [u16; 2],
    pub italic: bool,
    pub bytes: u64,
    pub state: State,
}

/// Which families a listing shows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Filter {
    /// Only families covering this writing system, as Google Fonts names its subsets.
    pub script: Option<String>,
    /// Only bundled and installed families.
    pub installed: bool,
}

/// The lowest and the highest weight any file of `family` provides.
fn weight_span(family: &Family) -> [u16; 2] {
    let lowest = family
        .file
        .iter()
        .map(|file| file.weights[0])
        .min()
        .unwrap_or_default();
    let highest = family
        .file
        .iter()
        .map(|file| file.weights[1])
        .max()
        .unwrap_or_default();
    [lowest, highest]
}

/// The rows of the catalog under `filter`, in catalog order.
///
/// # Errors
///
/// Returns the read error when a file's presence cannot be told.
pub fn rows(
    catalog: &Catalog,
    bundled: &[String],
    fonts_dir: Option<&Path>,
    filter: &Filter,
) -> Result<Vec<Row>, io::ReadError> {
    let mut rows = Vec::new();
    for family in &catalog.family {
        if let Some(script) = &filter.script
            && !family.covers(script)
        {
            continue;
        }
        let state = state(family, bundled, fonts_dir)?;
        if filter.installed && state == State::Available {
            continue;
        }
        rows.push(Row {
            name: family.name.clone(),
            listed: String::from(family.listing()),
            category: family.category.name(),
            scripts: family.scripts.clone(),
            weights: weight_span(family),
            italic: family.file.iter().any(|file| file.italic),
            bytes: family.bytes(),
            state,
        });
    }
    Ok(rows)
}

/// The listing for a person: one line per family, aligned in columns.
#[must_use]
pub fn human_listing(rows: &[Row]) -> String {
    let width = rows
        .iter()
        .map(|row| row.name.chars().count())
        .max()
        .unwrap_or(0);
    let lines: Vec<String> = rows
        .iter()
        .map(|row| {
            let weights = if row.weights[0] == row.weights[1] {
                format!("{}", row.weights[0])
            } else {
                format!("{}-{}", row.weights[0], row.weights[1])
            };
            let italic = if row.italic { " italic" } else { "" };
            let scripts = row.scripts.join(",");
            let listed = if row.listed == row.name {
                String::new()
            } else {
                format!("  (listed as {})", row.listed)
            };
            format!(
                "{:<width$}  {:<11}{:<16}{:<10}{:>9}  {}{}\n",
                row.name,
                row.category,
                format!("{weights}{italic}"),
                row.state.name(),
                megabytes(row.bytes),
                scripts,
                listed,
            )
        })
        .collect();
    lines.concat()
}

/// The listing as one JSON document.
#[must_use]
pub fn json_listing(commit: &str, rows: &[Row]) -> String {
    let families: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "name": row.name,
                "listed": row.listed,
                "category": row.category,
                "scripts": row.scripts,
                "weights": row.weights,
                "italic": row.italic,
                "bytes": row.bytes,
                "state": row.state.name(),
            })
        })
        .collect();
    let document = serde_json::json!({
        "format": LISTING_FORMAT,
        "version": LISTING_VERSION,
        "commit": commit,
        "families": families,
    });
    format!("{document}\n")
}

/// The bundled family names, as the listings and the theme loader know them.
#[must_use]
pub fn bundled_names() -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for font in &sivi_typst::fonts::BUNDLED {
        if !names.iter().any(|name| name == font.family) {
            names.push(String::from(font.family));
        }
    }
    names
}

fn usage(message: &str) -> Run {
    Run {
        stdout: Vec::new(),
        stderr: format!("error: {message}\n"),
        code: USAGE_EXIT_CODE,
    }
}

/// Diagnostics rendered as the build commands render them, with no source to quote.
fn diagnosed(diagnostics: &[Diagnostic], format: Format, styled: bool) -> Run {
    let sources = Sources {
        deck_path: "",
        deck_text: "",
        theme_label: "",
        theme_text: "",
    };
    match format {
        Format::Human => Run {
            stdout: Vec::new(),
            stderr: render::human(diagnostics, &sources, styled),
            code: DIAGNOSTIC_EXIT_CODE,
        },
        Format::Json => Run {
            stdout: render::json(diagnostics, &sources, None).into_bytes(),
            stderr: String::new(),
            code: DIAGNOSTIC_EXIT_CODE,
        },
    }
}

fn list_run(filter: &Filter, format: Format, environment: &Environment) -> Run {
    let catalog = match catalog::catalog() {
        Ok(catalog) => catalog,
        Err(message) => return usage(&format!("the font catalog does not parse: {message}")),
    };
    let fonts_dir = fonts_dir(environment);
    let rows = match rows(catalog, &bundled_names(), fonts_dir.as_deref(), filter) {
        Ok(rows) => rows,
        Err(error) => return usage(&error.to_string()),
    };
    let stdout = match format {
        Format::Human => human_listing(&rows),
        Format::Json => json_listing(&catalog.commit, &rows),
    };
    Run {
        stdout: stdout.into_bytes(),
        stderr: String::new(),
        code: SUCCESS_EXIT_CODE,
    }
}

fn install_run(
    families: &[String],
    format: Format,
    styled: bool,
    environment: &Environment,
) -> Run {
    let catalog = match catalog::catalog() {
        Ok(catalog) => catalog,
        Err(message) => return usage(&format!("the font catalog does not parse: {message}")),
    };
    let Some(fonts_dir) = fonts_dir(environment) else {
        return usage(
            "no data directory is known; set SIVI_HOME to a directory for installed fonts",
        );
    };
    let bundled = bundled_names();
    let mut diagnostics = Vec::new();
    let mut wanted = Vec::new();
    for name in families {
        match catalog.family(name) {
            Some(family) => wanted.push(family),
            None => diagnostics.push(unknown_family(name, catalog, &bundled)),
        }
    }
    if !diagnostics.is_empty() {
        return diagnosed(&diagnostics, format, styled);
    }
    let mut lines = Vec::new();
    for family in wanted {
        if bundled.contains(&family.name) {
            lines.push(format!("{} is bundled; nothing to install\n", family.name));
            continue;
        }
        let mut installer = Installer {
            fetch: &mut Web,
            fs: &mut Disk::default(),
        };
        match installer.install(catalog, family, &fonts_dir) {
            Ok(brought) => lines.push(format!(
                "installed {} ({} files, {}) under {}\n",
                family.name,
                brought.files,
                megabytes(brought.bytes),
                fonts_dir.join(&family.path).display()
            )),
            Err(error) => {
                lines.push(format!(
                    "error: {} could not be installed: {error}\n",
                    family.name
                ));
                return Run {
                    stdout: Vec::new(),
                    stderr: lines.concat(),
                    code: USAGE_EXIT_CODE,
                };
            }
        }
    }
    Run {
        stdout: Vec::new(),
        stderr: lines.concat(),
        code: SUCCESS_EXIT_CODE,
    }
}

/// Runs a `fonts` command.
#[must_use]
pub fn run(command: FontsCommand, environment: &Environment) -> Run {
    match command {
        FontsCommand::List {
            script,
            installed,
            rendering,
        } => list_run(&Filter { script, installed }, rendering.format, environment),
        FontsCommand::Install {
            families,
            rendering,
        } => install_run(
            &families,
            rendering.format,
            rendering.color.styled(environment),
            environment,
        ),
        FontsCommand::Dir => fonts_dir(environment).map_or_else(
            || {
                usage(
                    "no data directory is known; set SIVI_HOME to a directory for installed fonts",
                )
            },
            |dir| Run {
                stdout: format!("{}\n", dir.display()).into_bytes(),
                stderr: String::new(),
                code: SUCCESS_EXIT_CODE,
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Filter, Installed, Installer, State, bundled_names, data_dir, human_listing, json_listing,
        megabytes, not_installed, read_installed, rows, state, unknown_family,
    };
    use crate::args::Environment;
    use crate::digest::sha256_hex;
    use crate::net::{Fetch, FetchError, FetchErrorKind};
    use crate::publish::Disk;
    use sivi_core::diagnostic::Code;
    use sivi_theme::catalog::{Catalog, Category, Family, FontEntry};
    use std::path::PathBuf;

    /// A catalog of one two-file family whose bytes are known.
    fn catalog() -> Catalog {
        Catalog {
            commit: String::from("abc"),
            family: vec![Family {
                name: String::from("Example"),
                listed: Some(String::from("Example Listed")),
                path: String::from("ofl/example"),
                category: Category::Serif,
                scripts: vec![String::from("japanese"), String::from("latin")],
                license: String::from("OFL-1.1"),
                license_file: Some(String::from("OFL.txt")),
                license_sha256: Some(sha256_hex(&[b"license"])),
                file: vec![
                    FontEntry {
                        name: String::from("Example[wght].ttf"),
                        sha256: sha256_hex(&[b"regular"]),
                        bytes: 7,
                        italic: false,
                        weights: [100, 900],
                    },
                    FontEntry {
                        name: String::from("Example-Italic.ttf"),
                        sha256: sha256_hex(&[b"italic"]),
                        bytes: 6,
                        italic: true,
                        weights: [400, 400],
                    },
                ],
            }],
        }
    }

    /// A web of three files, which answers anything else with not found.
    struct FakeWeb;

    impl Fetch for FakeWeb {
        fn fetch(&mut self, url: &str, _limit: u64) -> Result<Vec<u8>, FetchError> {
            let bytes: &[u8] = if url.ends_with("Example%5Bwght%5D.ttf") {
                b"regular"
            } else if url.ends_with("Example-Italic.ttf") {
                b"italic"
            } else if url.ends_with("OFL.txt") {
                b"license"
            } else {
                return Err(FetchError {
                    url: String::from(url),
                    kind: FetchErrorKind::NotFound,
                });
            };
            Ok(bytes.to_vec())
        }
    }

    #[test]
    fn the_data_directory_follows_the_environment() {
        let environment = Environment {
            sivi_home: Some(String::from("/opt/sivi")),
            ..Environment::default()
        };
        assert_eq!(data_dir(&environment), Some(PathBuf::from("/opt/sivi")));
        let home = Environment {
            home: Some(String::from("/home/me")),
            ..Environment::default()
        };
        assert_eq!(
            data_dir(&home),
            Some(PathBuf::from("/home/me/.local/share/sivi"))
        );
        assert_eq!(data_dir(&Environment::default()), None);
        assert_eq!(megabytes(9_589_900), "9.5 MB");
        assert_eq!(megabytes(0), "0.0 MB");
    }

    #[test]
    fn an_installation_fetches_checks_and_publishes_every_file_and_then_reads_back() {
        let catalog = catalog();
        let family = &catalog.family[0];
        let root = tempfile::tempdir().unwrap();
        let fonts_dir = root.path().join("fonts");
        assert_eq!(
            state(family, &[], Some(&fonts_dir)).unwrap(),
            State::Available
        );
        let installed = Installer {
            fetch: &mut FakeWeb,
            fs: &mut Disk::default(),
        }
        .install(&catalog, family, &fonts_dir)
        .unwrap();
        assert_eq!(
            installed,
            Installed {
                files: 2,
                bytes: 13
            }
        );
        assert_eq!(
            state(family, &[], Some(&fonts_dir)).unwrap(),
            State::Installed
        );
        let files = read_installed(family, &fonts_dir).unwrap();
        assert_eq!(files, vec![b"regular".to_vec(), b"italic".to_vec()]);
        assert_eq!(
            state(family, &[String::from("Example")], None).unwrap(),
            State::Bundled
        );
        let listed = rows(&catalog, &[], Some(&fonts_dir), &Filter::default()).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].weights, [100, 900]);
        assert!(listed[0].italic);
        let text = human_listing(&listed);
        assert!(text.contains("Example  serif"), "{text}");
        assert!(text.contains("installed"), "{text}");
        assert!(text.contains("(listed as Example Listed)"), "{text}");
        let json = json_listing(&catalog.commit, &listed);
        assert!(json.contains("\"state\":\"installed\""), "{json}");
        let filtered = rows(
            &catalog,
            &[],
            Some(&fonts_dir),
            &Filter {
                script: Some(String::from("korean")),
                installed: false,
            },
        )
        .unwrap();
        assert_eq!(filtered, Vec::new());
    }

    #[test]
    fn a_missing_or_stale_installed_file_is_named_with_the_install_command() {
        let catalog = catalog();
        let family = &catalog.family[0];
        let root = tempfile::tempdir().unwrap();
        let fonts_dir = root.path().join("fonts");
        let missing = read_installed(family, &fonts_dir).unwrap_err();
        assert_eq!(missing.code, Code::FontNotInstalled);
        assert!(
            missing
                .helps
                .iter()
                .any(|help| help.contains("sivi fonts install \"Example\""))
        );
        let no_dir = not_installed(family, None);
        assert!(no_dir.helps.iter().any(|help| help.contains("SIVI_HOME")));
        Installer {
            fetch: &mut FakeWeb,
            fs: &mut Disk::default(),
        }
        .install(&catalog, family, &fonts_dir)
        .unwrap();
        let mut stale_catalog = catalog.clone();
        stale_catalog.family[0].file[0].sha256 = String::from("00");
        let stale = read_installed(&stale_catalog.family[0], &fonts_dir).unwrap_err();
        assert_eq!(stale.code, Code::FontDigest);
        let mut wrong = catalog.clone();
        wrong.family[0].file[1].sha256 = String::from("00");
        let refused = Installer {
            fetch: &mut FakeWeb,
            fs: &mut Disk::default(),
        }
        .install(&wrong, &wrong.family[0], &root.path().join("other"))
        .unwrap_err();
        assert!(refused.to_string().contains("does not match"), "{refused}");
    }

    #[test]
    fn an_unknown_family_names_its_closest_known_name() {
        let catalog = catalog();
        let close = unknown_family("Exalple", &catalog, &bundled_names());
        assert_eq!(close.code, Code::FamilyUnknown);
        assert!(close.helps.iter().any(|help| help.contains("`Example`")));
        let far = unknown_family("Nowhere Near", &catalog, &[]);
        assert!(
            far.helps
                .iter()
                .any(|help| help.contains("sivi fonts list"))
        );
        assert!(bundled_names().contains(&String::from("Shippori Mincho")));
    }
}
