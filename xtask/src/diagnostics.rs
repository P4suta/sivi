//! The catalog of diagnostics, generated from the one enum of codes and its documentation so the two never drift.

use crate::fs;
use anyhow::{Context, Result, bail, ensure};
use sivi_core::diagnostic::{Code, Severity};
use std::path::Path;

/// The enum the catalog is generated from.
pub const SOURCE: &str = "crates/sivi-core/src/diagnostic.rs";
/// The catalog the generator writes and the check compares.
pub const CATALOG: &str = "docs/diagnostics.md";
/// The enum's name in the source.
const ENUM: &str = "Code";

/// The opening of the catalog, before the table of codes.
const PREAMBLE: &str = "# Diagnostics\n\n\
    Every message of sivi carries a code from this catalog, which `cargo xtask gen` writes from the enum in `crates/sivi-core/src/diagnostic.rs`.\n\
    A code never changes meaning and is never reused.\n\
    An `E` code is an error, which stops the build; a `W` code is a warning, which `--deny-warnings` turns into a failure of the exit status.\n\
    The [command line](spec/cli.md) explains the exit statuses and the JSON document that carries these codes.\n\n\
    ## Codes\n\n\
    | Code | Severity | Name | Meaning |\n\
    | --- | --- | --- | --- |\n";

/// One code of the catalog with the line that documents it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub code: Code,
    pub name: String,
    pub meaning: String,
}

/// The documentation line of each variant of the enum, by variant name and in order.
fn documented(source: &str) -> Result<Vec<(String, String)>> {
    let file = syn::parse_file(source).context("parse the diagnostic source")?;
    let item = file
        .items
        .iter()
        .find_map(|item| {
            if let syn::Item::Enum(item) = item
                && item.ident == ENUM
            {
                Some(item)
            } else {
                None
            }
        })
        .with_context(|| format!("{SOURCE} defines the enum {ENUM}"))?;
    let mut found = Vec::new();
    for variant in &item.variants {
        let mut lines = Vec::new();
        for attribute in &variant.attrs {
            if !attribute.path().is_ident("doc") {
                continue;
            }
            let value = &attribute
                .meta
                .require_name_value()
                .context("a documentation attribute")?
                .value;
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(text),
                ..
            }) = value
            {
                lines.push(text.value().trim().to_owned());
            }
        }
        ensure!(
            !lines.is_empty(),
            "the variant {} of {ENUM} has no documentation line",
            variant.ident
        );
        found.push((variant.ident.to_string(), lines.join(" ")));
    }
    Ok(found)
}

/// Every entry of the catalog, with the enum's list and the source's variants held to each other.
///
/// # Errors
///
/// Returns an error when the source cannot be read or parsed, a variant lacks its line, or the line does not start with the variant's code.
pub fn entries(root: &Path) -> Result<Vec<Entry>> {
    let source = fs::read_to_string(&root.join(SOURCE))?;
    let documented = documented(&source)?;
    ensure!(
        documented.len() == Code::ALL.len(),
        "{SOURCE} defines {} variants of {ENUM} while {ENUM}::ALL lists {}",
        documented.len(),
        Code::ALL.len()
    );
    let mut entries = Vec::new();
    for (code, (name, line)) in Code::ALL.iter().zip(documented) {
        let variant = format!("{code:?}");
        ensure!(
            variant == name,
            "{ENUM}::ALL lists {variant} where {SOURCE} defines {name}"
        );
        let Some(meaning) = line.strip_prefix(&format!("{}: ", code.as_str())) else {
            bail!(
                "the documentation of {name} must start with `{}: `, not `{line}`",
                code.as_str()
            );
        };
        entries.push(Entry {
            code: *code,
            name,
            meaning: meaning.to_owned(),
        });
    }
    Ok(entries)
}

const fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

const fn severity_phrase(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "an error",
        Severity::Warning => "a warning",
    }
}

/// The sentence with its first letter raised, as a catalog entry starts.
fn first_capital(sentence: &str) -> String {
    let mut characters = sentence.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

fn row(entry: &Entry) -> String {
    format!(
        "| [{}](#{}) | {} | `{}` | {} |\n",
        entry.code.as_str(),
        entry.code.as_str().to_lowercase(),
        severity_name(entry.code.severity()),
        entry.name,
        entry.meaning.trim_end_matches('.')
    )
}

fn section(entry: &Entry) -> String {
    format!(
        "\n## {}\n\n`{}`, {}.\n{}\n",
        entry.code.as_str(),
        entry.name,
        severity_phrase(entry.code.severity()),
        first_capital(&entry.meaning)
    )
}

/// The catalog as Markdown, one sentence per line.
#[must_use]
pub fn render(entries: &[Entry]) -> String {
    let mut pieces = vec![String::from(PREAMBLE)];
    pieces.extend(entries.iter().map(row));
    pieces.extend(entries.iter().map(section));
    pieces.concat()
}

/// Writes the catalog.
///
/// # Errors
///
/// Returns an error when the entries cannot be read or the catalog cannot be written.
pub fn generate(root: &Path) -> Result<()> {
    let text = render(&entries(root)?);
    fs::write(&root.join(CATALOG), text.as_bytes())
}

/// Refuses a catalog that differs from what the enum would generate.
///
/// # Errors
///
/// Returns an error naming the drift.
pub fn check(root: &Path) -> Result<()> {
    let expected = render(&entries(root)?);
    let path = root.join(CATALOG);
    ensure!(
        fs::is_file(&path)?,
        "{CATALOG} is missing; run `cargo xtask gen`"
    );
    ensure!(
        fs::read_to_string(&path)? == expected,
        "{CATALOG} is stale; run `cargo xtask gen`"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Entry, documented, entries, first_capital, render};
    use sivi_core::diagnostic::Code;

    #[test]
    fn the_source_documents_every_code_in_the_order_of_the_list() {
        let root = crate::root().unwrap();
        let found = entries(&root).unwrap();
        assert_eq!(found.len(), Code::ALL.len());
        assert_eq!(found[0].code, Code::Internal);
        assert!(
            found
                .iter()
                .any(|entry| entry.code == Code::VerticalOverflow
                    && entry.meaning == "content taller than its region."),
            "{found:?}"
        );
    }

    #[test]
    fn a_variant_without_a_line_or_with_the_wrong_code_is_refused() {
        let bare = "pub enum Code { /// E0001: a defect.\n Internal, Other }";
        assert!(documented(bare).unwrap_err().to_string().contains("Other"));
        let listed = documented("pub enum Code { /// E0401: taller.\n VerticalOverflow }").unwrap();
        assert_eq!(
            listed,
            vec![(
                String::from("VerticalOverflow"),
                String::from("E0401: taller.")
            )]
        );
    }

    #[test]
    fn the_catalog_lists_every_entry_and_anchors_each_code() {
        let text = render(&[Entry {
            code: Code::VerticalOverflow,
            name: String::from("VerticalOverflow"),
            meaning: String::from("content taller than its region."),
        }]);
        assert!(
            text.contains(
                "| [E0401](#e0401) | error | `VerticalOverflow` | content taller than its region |"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "\n## E0401\n\n`VerticalOverflow`, an error.\nContent taller than its region.\n"
            ),
            "{text}"
        );
        assert_eq!(first_capital("über"), "Über");
        assert_eq!(first_capital(""), "");
    }
}
