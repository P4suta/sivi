//! What the pickers list: the carried themes, the font catalog, the carried palettes, and the code schemes, each entry with a line of detail, its swatches, and the theme text that would choose it.

use crate::args::Environment;
use crate::library::{self, Filter};
use sivi_core::palette_rules::{self, CURATED};
use sivi_core::syntax_theme::{BACKGROUND, FOREGROUND, PALETTE_SIZE};
use sivi_core::theme::Color;
use sivi_core::ui_rules::Picker;
use sivi_theme::{BUILTINS, catalog::catalog, schemes::schemes};

/// The first accent of a code scheme's palette, after the eight shades of background and foreground.
const FIRST_ACCENT: usize = 0x08;

/// One entry of a picker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Entry {
    pub name: String,
    pub detail: String,
    pub swatches: Vec<Color>,
    /// The theme text that chooses the entry, shown beside it.
    pub snippet: String,
}

/// Every picker's entries, read once when the interface opens.
#[derive(Clone, Debug, Default)]
pub(super) struct Pickers {
    pub themes: Vec<Entry>,
    pub fonts: Vec<Entry>,
    pub palettes: Vec<Entry>,
    pub schemes: Vec<Entry>,
}

impl Pickers {
    /// Reads every list, leaving a list empty where its source fails.
    #[must_use]
    pub(super) fn read(environment: &Environment) -> Self {
        Self {
            themes: themes(),
            fonts: fonts(environment),
            palettes: palettes(),
            schemes: code_schemes(),
        }
    }

    /// The entries of `kind`.
    #[must_use]
    pub(super) fn of(&self, kind: Picker) -> &[Entry] {
        match kind {
            Picker::Theme => &self.themes,
            Picker::Font => &self.fonts,
            Picker::Palette => &self.palettes,
            Picker::CodeTheme => &self.schemes,
        }
    }

    /// How many entries `kind` lists, as the machine counts them.
    #[must_use]
    pub(super) fn count(&self, kind: Picker) -> usize {
        self.of(kind).len()
    }
}

/// The carried themes with their paper, ink, and accent.
fn themes() -> Vec<Entry> {
    BUILTINS
        .iter()
        .map(|theme| {
            let swatches = sivi_theme::parse(theme.text).map_or_else(
                |_refused| Vec::new(),
                |loaded| {
                    ["paper", "ink", "accent", "highlight"]
                        .iter()
                        .filter_map(|role| {
                            sivi_core::theme::StyleName::new(role)
                                .map_or(None, |name| loaded.color(&name))
                        })
                        .collect()
                },
            );
            Entry {
                name: String::from(theme.name),
                detail: String::from(theme.summary),
                swatches,
                snippet: format!("theme: {}", theme.name),
            }
        })
        .collect()
}

/// The font catalog with each family's category, scripts, and state on this host.
fn fonts(environment: &Environment) -> Vec<Entry> {
    let Ok(found) = catalog() else {
        return Vec::new();
    };
    let fonts_dir = library::fonts_dir(environment);
    let rows = library::rows(
        found,
        &library::bundled_names(),
        fonts_dir.as_deref(),
        &Filter::default(),
    );
    rows.map_or_else(
        |_unreadable| Vec::new(),
        |rows| {
            rows.into_iter()
                .map(|row| Entry {
                    detail: format!(
                        "{}  {}  {}  {}",
                        row.category,
                        row.scripts.join(" "),
                        library::megabytes(row.bytes),
                        row.state.name()
                    ),
                    snippet: format!("font = \"{}\"", row.name),
                    name: row.name,
                    swatches: Vec::new(),
                })
                .collect()
        },
    )
}

/// The carried palettes with their ten roles as swatches.
fn palettes() -> Vec<Entry> {
    CURATED
        .iter()
        .map(|named| {
            let grown = palette_rules::palette(named.seed, named.mode, named.harmony);
            Entry {
                name: String::from(named.name),
                detail: format!("{} · {}", named.mode.name(), named.harmony.name()),
                swatches: grown.map_or_else(Vec::new, |palette| {
                    palette.roles().iter().map(|(_, color)| *color).collect()
                }),
                snippet: format!("[palette]\nname = \"{}\"", named.name),
            }
        })
        .collect()
}

/// The carried code schemes with their background, foreground, and accents as swatches.
fn code_schemes() -> Vec<Entry> {
    let Ok(all) = schemes() else {
        return Vec::new();
    };
    all.all()
        .iter()
        .map(|scheme| Entry {
            name: scheme.slug.clone(),
            detail: format!(
                "{} · {} · {}",
                scheme.variant.name(),
                scheme.name,
                scheme.author
            ),
            swatches: (BACKGROUND..PALETTE_SIZE)
                .filter(|role| *role == BACKGROUND || *role == FOREGROUND || *role >= FIRST_ACCENT)
                .map(|role| scheme.color(role))
                .collect(),
            snippet: format!("[code]\ntheme = \"{}\"", scheme.slug),
        })
        .collect()
}
