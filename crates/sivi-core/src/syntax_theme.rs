//! How a base16 scheme paints code: the sixteen colors by their roles, and the `TextMate` theme Typst reads them as.
//!
//! The roles follow the base16 styling guide: `base00` is the background, `base05` the foreground, `base03` the comments,
//! and `base08` to `base0F` the variables, constants, classes, strings, support, functions, keywords, and deprecated code.

use crate::theme::Color;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// The colors of a base16 palette.
pub const PALETTE_SIZE: usize = 16;

/// The roles of the palette, as indexes into it.
pub const BACKGROUND: usize = 0x00;
pub const LINE_HIGHLIGHT: usize = 0x01;
pub const SELECTION: usize = 0x02;
pub const COMMENT: usize = 0x03;
pub const FOREGROUND: usize = 0x05;
pub const VARIABLES: usize = 0x08;
pub const CONSTANTS: usize = 0x09;
pub const CLASSES: usize = 0x0A;
pub const STRINGS: usize = 0x0B;
pub const SUPPORT: usize = 0x0C;
pub const FUNCTIONS: usize = 0x0D;
pub const KEYWORDS: usize = 0x0E;
pub const DEPRECATED: usize = 0x0F;

/// The virtual directory the generated themes live in, and the extension Typst reads them by.
pub const THEME_PREFIX: &str = "code/";
pub const THEME_EXTENSION: &str = ".tmTheme";
/// The document type every property list names.
const PLIST_DTD: &str = "http://www.apple.com/DTDs/PropertyList-1.0.dtd";

/// Whether a scheme is dark or light.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Variant {
    Dark,
    Light,
}

impl Variant {
    /// The variant as the scheme spells it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    /// The variant named `text`, or `None`.
    #[must_use]
    pub fn from_name(text: &str) -> Option<Self> {
        [Self::Dark, Self::Light]
            .into_iter()
            .find(|variant| variant.name() == text)
    }
}

/// A base16 scheme: the slug a theme names it by, the name and author it shows, its variant, and its sixteen colors.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Scheme {
    pub slug: String,
    pub name: String,
    pub author: String,
    pub variant: Variant,
    pub palette: [Color; PALETTE_SIZE],
}

/// The theme file of a scheme: the virtual path the source names it by, and the property list Typst's highlighter reads.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ThemeFile {
    path: String,
    text: String,
}

impl ThemeFile {
    /// The virtual path, which the emitter names and the world serves.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The property list.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl Scheme {
    /// The color of a role, with the background for an index outside the palette.
    #[must_use]
    pub fn color(&self, role: usize) -> Color {
        self.palette
            .get(role)
            .copied()
            .unwrap_or(self.palette[BACKGROUND])
    }

    /// The background of code set in the scheme.
    #[must_use]
    pub fn background(&self) -> Color {
        self.color(BACKGROUND)
    }

    /// The color of code the highlighter leaves plain.
    #[must_use]
    pub fn foreground(&self) -> Color {
        self.color(FOREGROUND)
    }

    /// The scheme's theme file, under its slug.
    #[must_use]
    pub fn theme_file(&self) -> ThemeFile {
        ThemeFile {
            path: format!("{THEME_PREFIX}{}{THEME_EXTENSION}", self.slug),
            text: plist(self),
        }
    }
}

/// One rule of the theme: the scopes it covers, the role it paints them with, and the role behind them.
struct Rule {
    name: &'static str,
    scopes: &'static str,
    foreground: usize,
    background: Option<usize>,
}

/// The rules, following the base16 `TextMate` template; the highlighter takes the most specific scope that matches.
const RULES: [Rule; 12] = [
    Rule {
        name: "Comment",
        scopes: "comment, punctuation.definition.comment",
        foreground: COMMENT,
        background: None,
    },
    Rule {
        name: "Text, punctuation, operators",
        scopes: "punctuation, keyword.operator, meta.brace",
        foreground: FOREGROUND,
        background: None,
    },
    Rule {
        name: "Keywords and storage",
        scopes: "keyword, storage, keyword.operator.new, keyword.operator.expression, keyword.operator.logical, meta.selector, markup.changed, markup.italic, punctuation.section.embedded, variable.interpolation",
        foreground: KEYWORDS,
        background: None,
    },
    Rule {
        name: "Variables and tags",
        scopes: "variable, entity.name.tag, markup.deleted, markup.list, string.other.link",
        foreground: VARIABLES,
        background: None,
    },
    Rule {
        name: "Constants",
        scopes: "constant, entity.other.attribute-name, keyword.other.unit, markup.quote, meta.link",
        foreground: CONSTANTS,
        background: None,
    },
    Rule {
        name: "Classes and types",
        scopes: "entity.name.class, entity.name.type, entity.name.namespace, entity.name.struct, entity.name.enum, support.class, support.type, markup.bold",
        foreground: CLASSES,
        background: None,
    },
    Rule {
        name: "Strings",
        scopes: "string, constant.other.symbol, entity.other.inherited-class, markup.inserted, markup.raw",
        foreground: STRINGS,
        background: None,
    },
    Rule {
        name: "Support, regular expressions, escapes",
        scopes: "support.function, string.regexp, constant.character.escape, constant.other.color",
        foreground: SUPPORT,
        background: None,
    },
    Rule {
        name: "Functions and headings",
        scopes: "entity.name.function, support.function.any-method, variable.function, entity.name.section, markup.heading, entity.other.attribute-name.id, support.macro",
        foreground: FUNCTIONS,
        background: None,
    },
    Rule {
        name: "Parameters",
        scopes: "variable.parameter",
        foreground: FOREGROUND,
        background: None,
    },
    Rule {
        name: "Illegal",
        scopes: "invalid, invalid.illegal",
        foreground: BACKGROUND,
        background: Some(VARIABLES),
    },
    Rule {
        name: "Deprecated",
        scopes: "invalid.deprecated",
        foreground: BACKGROUND,
        background: Some(DEPRECATED),
    },
];

/// `text` with the characters XML reserves written as entities.
fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
    out
}

/// A key and a string value of a property list, on one line.
fn entry(key: &str, value: &str) -> String {
    format!("<key>{key}</key><string>{}</string>\n", escaped(value))
}

/// The `TextMate` theme of `scheme`, as a property list.
fn plist(scheme: &Scheme) -> String {
    let mut lines: Vec<String> = alloc::vec![
        String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"),
        format!("<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"{PLIST_DTD}\">\n"),
        String::from("<plist version=\"1.0\">\n<dict>\n"),
        entry("name", &scheme.name),
        entry("author", &scheme.author),
        String::from("<key>settings</key>\n<array>\n<dict>\n<key>settings</key>\n<dict>\n"),
    ];
    for (key, role) in [
        ("background", BACKGROUND),
        ("caret", FOREGROUND),
        ("foreground", FOREGROUND),
        ("invisibles", COMMENT),
        ("lineHighlight", LINE_HIGHLIGHT),
        ("selection", SELECTION),
    ] {
        lines.push(entry(key, &scheme.color(role).to_string()));
    }
    lines.push(String::from("</dict>\n</dict>\n"));
    for rule in &RULES {
        lines.push(String::from("<dict>\n"));
        lines.push(entry("name", rule.name));
        lines.push(entry("scope", rule.scopes));
        lines.push(String::from("<key>settings</key>\n<dict>\n"));
        lines.push(entry(
            "foreground",
            &scheme.color(rule.foreground).to_string(),
        ));
        if let Some(background) = rule.background {
            lines.push(entry("background", &scheme.color(background).to_string()));
        }
        lines.push(String::from("</dict>\n</dict>\n"));
    }
    lines.push(String::from("</array>\n</dict>\n</plist>\n"));
    lines.concat()
}

#[cfg(test)]
mod tests {
    use super::{
        BACKGROUND, COMMENT, FOREGROUND, KEYWORDS, PALETTE_SIZE, Scheme, Variant, escaped,
    };
    use crate::theme::Color;
    use alloc::string::{String, ToString};

    /// A scheme whose colors count up by role, so each role has its own color.
    fn counting() -> Scheme {
        let mut palette = [Color::default(); PALETTE_SIZE];
        for (index, color) in palette.iter_mut().enumerate() {
            color.red = u8::try_from(index).unwrap();
            color.green = 0x10;
            color.blue = 0x20;
        }
        Scheme {
            slug: String::from("counting"),
            name: String::from("Counting <Scheme> & co"),
            author: String::from("a \"tester\""),
            variant: Variant::Dark,
            palette,
        }
    }

    #[test]
    fn the_theme_paints_each_role_with_its_color_and_escapes_its_names() {
        let scheme = counting();
        let file = scheme.theme_file();
        let text = file.text();
        assert_eq!(file.path(), "code/counting.tmTheme");
        assert!(text.starts_with("<?xml version=\"1.0\""), "{text}");
        assert!(text.ends_with("</plist>\n"), "{text}");
        assert!(text.contains("PropertyList-1.0.dtd\">\n<plist"), "{text}");
        assert!(text.contains("<key>name</key><string>Counting &lt;Scheme&gt; &amp; co</string>"));
        assert!(text.contains("<key>author</key><string>a &quot;tester&quot;</string>"));
        assert!(text.contains("<key>background</key><string>#001020</string>"));
        assert!(text.contains("<key>foreground</key><string>#051020</string>"));
        assert!(text.contains("<key>scope</key><string>comment, punctuation.definition.comment</string>\n<key>settings</key>\n<dict>\n<key>foreground</key><string>#031020</string>"));
        assert!(text.contains("<key>scope</key><string>invalid, invalid.illegal</string>\n<key>settings</key>\n<dict>\n<key>foreground</key><string>#001020</string>\n<key>background</key><string>#081020</string>"));
        assert_eq!(
            text.matches("<dict>").count(),
            text.matches("</dict>").count()
        );
        assert_eq!(scheme.color(KEYWORDS).to_string(), "#0e1020");
        assert_eq!(scheme.color(COMMENT).to_string(), "#031020");
        assert_eq!(scheme.color(PALETTE_SIZE), scheme.color(BACKGROUND));
        assert_eq!(scheme.background(), scheme.color(BACKGROUND));
        assert_eq!(scheme.foreground(), scheme.color(FOREGROUND));
        assert_eq!(escaped("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
        assert_eq!(Variant::from_name("light"), Some(Variant::Light));
        assert_eq!(Variant::from_name("dark").map(Variant::name), Some("dark"));
        assert_eq!(Variant::from_name("dim"), None);
    }
}
