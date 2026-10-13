//! The shape of a theme file as TOML spells it, before any value is read into the model.
//!
//! Every table refuses unknown keys, so a misspelled key is a schema error with its position.

use serde::Deserialize;
use std::collections::BTreeMap;

/// The schema version this reader understands.
pub const SCHEMA: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeFile {
    pub schema: u32,
    pub theme: ThemeSection,
    pub page: PageSection,
    pub colors: BTreeMap<String, String>,
    pub typography: TypographySection,
    pub text: BTreeMap<String, TextSection>,
    pub inline: InlineSection,
    pub lists: ListsSection,
    pub code: CodeSection,
    pub tables: TablesSection,
    pub quotes: QuotesSection,
    #[serde(default)]
    pub fonts: Vec<FontSection>,
    /// The masters by name, in declaration order.
    pub masters: toml::Table,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeSection {
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageSection {
    pub width: String,
    pub height: String,
    pub background: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypographySection {
    pub min_size: String,
    pub paragraph_gap: String,
    #[serde(default)]
    pub fallback: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextSection {
    pub font: String,
    pub size: String,
    pub weight: u16,
    pub color: String,
    pub line_height: f64,
    pub align: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkSection {
    pub style: Option<String>,
    pub color: Option<String>,
    pub weight: Option<u16>,
    pub font: Option<String>,
    pub background: Option<String>,
    #[serde(default)]
    pub underline: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InlineSection {
    pub emphasis: MarkSection,
    pub strong: MarkSection,
    pub code: MarkSection,
    pub link: MarkSection,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListsSection {
    pub bullets: Vec<String>,
    pub ordered: String,
    pub indent: String,
    pub gap: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeSection {
    pub font: String,
    pub size: String,
    pub line_height: f64,
    pub background: String,
    pub padding: String,
    pub highlight: bool,
    pub tab_width: u8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TablesSection {
    pub text: String,
    pub header_weight: u16,
    pub rule: String,
    pub padding: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotesSection {
    pub bar: String,
    pub bar_width: String,
    pub indent: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontSection {
    pub family: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MasterSection {
    /// The slots by name, in declaration order.
    pub slots: toml::Table,
    #[serde(default)]
    pub decor: Vec<DecorSection>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionSection {
    pub x: String,
    pub y: String,
    pub width: String,
    pub height: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotSection {
    pub binds: String,
    pub section: Option<u32>,
    #[serde(default)]
    pub accepts: Vec<String>,
    #[serde(default)]
    pub required: bool,
    pub follows: Option<String>,
    pub region: RegionSection,
    pub text: String,
    pub valign: Option<String>,
    pub align: Option<String>,
    pub max_blocks: Option<u32>,
    pub max_items: Option<u32>,
    pub max_depth: Option<u32>,
    pub max_rows: Option<u32>,
    pub max_columns: Option<u32>,
    pub max_lines: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecorSection {
    pub kind: String,
    pub region: RegionSection,
    pub color: Option<String>,
    pub path: Option<String>,
    pub text: Option<String>,
    pub align: Option<String>,
    pub format: Option<String>,
    pub field: Option<String>,
}
