//! The catalog of Google Fonts families the binary can install, generated from google/fonts at a pinned commit.
//!
//! A theme names any family of the catalog in its styles; the binary loads the family's files once `sivi fonts install` has placed them, checking each digest the catalog pins.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// The catalog text the binary carries.
pub const TEXT: &str = include_str!("../../../fonts/catalog.toml");

/// Where the files of a family live, before the commit and the family's path.
pub const RAW_ORIGIN: &str = "https://raw.githubusercontent.com/google/fonts";

/// Every family the catalog pins.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    /// The commit of google/fonts every file comes from.
    pub commit: String,
    #[serde(default)]
    pub family: Vec<Family>,
}

/// One family with the files that provide it.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Family {
    /// The family name as its files carry it and as Typst lists it, which a theme writes.
    pub name: String,
    /// The name Google Fonts lists the family under, when it differs from `name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listed: Option<String>,
    /// The directory under the repository, such as `ofl/notosansjp`.
    pub path: String,
    pub category: Category,
    /// The writing systems the family covers, as Google Fonts names its subsets.
    pub scripts: Vec<String>,
    /// The license identifier, such as `OFL-1.1`.
    pub license: String,
    /// The license file beside the fonts, such as `OFL.txt`, when the directory carries one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_sha256: Option<String>,
    pub file: Vec<FontEntry>,
}

/// The broad shape of a family.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Sans,
    Serif,
    Display,
    Handwriting,
    Mono,
}

/// One font file of a family.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FontEntry {
    /// The file name inside the family's directory.
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
    pub italic: bool,
    /// The lowest and the highest weight the file provides, which agree for a static face.
    pub weights: [u16; 2],
}

impl Category {
    /// The category as the catalog and the listings spell it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sans => "sans",
            Self::Serif => "serif",
            Self::Display => "display",
            Self::Handwriting => "handwriting",
            Self::Mono => "mono",
        }
    }
}

impl FontEntry {
    /// Whether the file provides `weight`, upright or italic as `italic` says.
    #[must_use]
    pub const fn provides(&self, weight: u16, italic: bool) -> bool {
        let [lowest, highest] = self.weights;
        self.italic == italic && lowest <= weight && weight <= highest
    }
}

impl Family {
    /// Whether any file of the family provides `weight`, upright or italic as `italic` says.
    #[must_use]
    pub fn provides(&self, weight: u16, italic: bool) -> bool {
        self.file.iter().any(|file| file.provides(weight, italic))
    }

    /// Whether the family covers `script`, as Google Fonts names its subsets.
    #[must_use]
    pub fn covers(&self, script: &str) -> bool {
        self.scripts.iter().any(|known| known == script)
    }

    /// The bytes of every file of the family.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.file
            .iter()
            .fold(0, |total, file| total.saturating_add(file.bytes))
    }

    /// Where `file` of the family lives at `commit`.
    #[must_use]
    pub fn url(&self, commit: &str, file: &str) -> String {
        format!("{RAW_ORIGIN}/{commit}/{}/{}", self.path, encoded(file))
    }

    /// The name Google Fonts lists the family under.
    #[must_use]
    pub fn listing(&self) -> &str {
        self.listed.as_deref().unwrap_or(&self.name)
    }
}

/// A file name as a URL spells it, with the brackets of a variable font encoded.
#[must_use]
pub fn encoded(name: &str) -> String {
    name.replace('[', "%5B").replace(']', "%5D")
}

impl Catalog {
    /// The family named `name`, as its files carry it.
    #[must_use]
    pub fn family(&self, name: &str) -> Option<&Family> {
        self.family.iter().find(|family| family.name == name)
    }

    /// Every family name, in catalog order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.family.iter().map(|family| family.name.as_str())
    }
}

/// The catalog the binary carries, parsed once.
///
/// # Errors
///
/// Returns why the carried text does not parse, which is a defect of the build.
pub fn catalog() -> Result<&'static Catalog, String> {
    static PARSED: OnceLock<Result<Catalog, String>> = OnceLock::new();
    PARSED
        .get_or_init(|| toml::from_str(TEXT).map_err(|error| error.to_string()))
        .as_ref()
        .map_err(Clone::clone)
}

#[cfg(test)]
mod tests {
    use super::{Catalog, Category, Family, FontEntry, catalog};

    fn family() -> Family {
        Family {
            name: String::from("Example"),
            listed: None,
            path: String::from("ofl/example"),
            category: Category::Sans,
            scripts: vec![String::from("latin")],
            license: String::from("OFL-1.1"),
            license_file: Some(String::from("OFL.txt")),
            license_sha256: Some(String::from("00")),
            file: vec![
                FontEntry {
                    name: String::from("Example[wght].ttf"),
                    sha256: String::from("01"),
                    bytes: 10,
                    italic: false,
                    weights: [100, 900],
                },
                FontEntry {
                    name: String::from("Example-Italic.ttf"),
                    sha256: String::from("02"),
                    bytes: 5,
                    italic: true,
                    weights: [400, 400],
                },
            ],
        }
    }

    #[test]
    fn a_family_provides_the_weights_of_its_files_and_covers_its_scripts() {
        let family = family();
        assert!(family.provides(700, false));
        assert!(family.provides(400, true));
        assert!(!family.provides(700, true));
        assert!(!family.provides(50, false));
        assert!(family.covers("latin"));
        assert!(!family.covers("japanese"));
        assert_eq!(family.bytes(), 15);
        assert_eq!(Category::Mono.name(), "mono");
        assert_eq!(family.listing(), "Example");
        assert_eq!(
            family.url("abc", "Example[wght].ttf"),
            "https://raw.githubusercontent.com/google/fonts/abc/ofl/example/Example%5Bwght%5D.ttf"
        );
    }

    #[test]
    fn the_catalog_finds_a_family_by_its_exact_name() {
        let catalog = Catalog {
            commit: String::from("abc"),
            family: vec![family()],
        };
        assert!(catalog.family("Example").is_some());
        assert!(catalog.family("example").is_none());
        assert_eq!(catalog.names().collect::<Vec<_>>(), vec!["Example"]);
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "the carried catalog is twenty times the plain theme, which parses in minutes under Miri"
    )]
    fn the_carried_catalog_parses_and_pins_every_file_with_a_digest() {
        let carried = catalog().unwrap();
        assert_ne!(carried.commit, "");
        for family in &carried.family {
            assert!(!family.file.is_empty(), "{}", family.name);
            for file in &family.file {
                assert_eq!(file.sha256.len(), 64, "{}/{}", family.name, file.name);
                assert!(file.weights[0] <= file.weights[1], "{}", file.name);
            }
        }
        let mut names: Vec<&str> = carried.names().collect();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "a family appears twice");
    }
}
