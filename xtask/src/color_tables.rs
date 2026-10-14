//! The tables of the core's fixed-point color engine, written from the sRGB transfer curve and the cosine.
//!
//! The core reaches no floating point, so the two curves it needs are tabulated here once and held to the source on every check.

use crate::fs;
use anyhow::{Context, Result, ensure};
use std::path::Path;

/// Where the tables live in the core.
pub const TABLES: &str = "crates/sivi-core/src/color_tables.rs";
/// One, in the micro-units every table holds.
const UNIT: f64 = 1_000_000.0;
/// The 8-bit channels and the tenths of a degree the tables cover.
const CHANNELS: usize = 256;
const TENTHS: usize = 3600;
/// The tenths in one degree.
const TENTHS_PER_DEGREE: f64 = 10.0;
/// The sRGB transfer curve: a linear toe below the knee, then a power curve.
const KNEE: f64 = 0.04045;
const TOE_SLOPE: f64 = 12.92;
const OFFSET: f64 = 0.055;
const SCALE: f64 = 1.055;
const GAMMA: f64 = 2.4;
const CHANNEL_MAX: f64 = 255.0;
/// How many values share a line of the written tables, and how many digits share a group of a literal.
const PER_LINE: usize = 8;
const GROUP: usize = 3;

/// `value` rounded to the nearest whole number, written as text, so no float cast appears.
fn rounded(value: f64) -> Result<i64> {
    let text = format!("{value:.0}");
    text.parse()
        .with_context(|| format!("{text} is not a whole number"))
}

/// The linear light of an 8-bit channel, in micro-units.
fn linear(channel: usize) -> Result<i64> {
    let encoded = f64::from(u8::try_from(channel).context("a channel is a byte")?) / CHANNEL_MAX;
    let light = if encoded <= KNEE {
        encoded / TOE_SLOPE
    } else {
        ((encoded + OFFSET) / SCALE).powf(GAMMA)
    };
    rounded(light * UNIT)
}

/// The cosine of a tenth of a degree, in micro-units.
fn cosine(tenth: usize) -> Result<i64> {
    let degrees =
        f64::from(u16::try_from(tenth).context("a tenth fits sixteen bits")?) / TENTHS_PER_DEGREE;
    rounded(degrees.to_radians().cos() * UNIT)
}

/// `value` as a Rust literal with an underscore every three digits, as the lints ask of a long literal.
fn literal(value: i64) -> String {
    let digits: Vec<char> = value.unsigned_abs().to_string().chars().collect();
    let mut grouped = String::new();
    for (index, digit) in digits.iter().enumerate() {
        let remaining = digits.len().checked_sub(index);
        if index > 0 && remaining.and_then(|rest| rest.checked_rem(GROUP)) == Some(0) {
            grouped.push('_');
        }
        grouped.push(*digit);
    }
    if value < 0 {
        format!("-{grouped}")
    } else {
        grouped
    }
}

/// One table as Rust source: a documented, unformatted constant array.
fn table(name: &str, doc: &str, values: &[i64]) -> String {
    let mut lines = vec![
        format!("/// {doc}\n"),
        String::from("#[rustfmt::skip]\n"),
        format!("pub static {name}: [i64; {}] = [\n", values.len()),
    ];
    for row in values.chunks(PER_LINE) {
        let cells: Vec<String> = row
            .iter()
            .map(|value| format!("{},", literal(*value)))
            .collect();
        lines.push(format!("    {}\n", cells.join(" ")));
    }
    lines.push(String::from("];\n"));
    lines.concat()
}

/// The whole source of the tables module.
///
/// # Errors
///
/// Returns an error when a value cannot be rounded to a whole number.
pub fn source() -> Result<String> {
    let linear = (0..CHANNELS).map(linear).collect::<Result<Vec<_>>>()?;
    let cosine = (0..TENTHS).map(cosine).collect::<Result<Vec<_>>>()?;
    Ok([
        String::from("//! The tables of the fixed-point color engine, written by `cargo xtask gen` from the sRGB transfer curve and the cosine; edit nothing by hand.\n"),
        String::from("//!\n"),
        String::from("//! Every value is in micro-units of one, so the core turns a channel into linear light and a hue in tenths of a degree into a direction without floating point.\n\n"),
        table("LINEAR", "The linear light of each 8-bit sRGB channel.", &linear),
        String::from("\n"),
        table("COSINE", "The cosine of each tenth of a degree.", &cosine),
    ]
    .concat())
}

/// Writes the tables module.
///
/// # Errors
///
/// Returns an error when the source cannot be written.
pub fn generate(root: &Path) -> Result<()> {
    fs::write(&root.join(TABLES), source()?.as_bytes())
}

/// Refuses a tables module that differs from what the generator writes.
///
/// # Errors
///
/// Returns an error when the module is missing or stale.
pub fn check(root: &Path) -> Result<()> {
    let path = root.join(TABLES);
    ensure!(
        fs::is_file(&path)?,
        "{TABLES} is missing; run `cargo xtask gen`"
    );
    ensure!(
        fs::read_to_string(&path)? == source()?,
        "{TABLES} is stale; run `cargo xtask gen`"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{cosine, linear, rounded, source};

    #[test]
    fn the_tables_follow_their_curves_and_stay_in_order() {
        assert_eq!(linear(0).unwrap(), 0);
        assert_eq!(linear(255).unwrap(), 1_000_000);
        assert_eq!(linear(1).unwrap(), 304);
        assert_eq!(linear(128).unwrap(), 215_861);
        let table: Vec<i64> = (0..256).map(|channel| linear(channel).unwrap()).collect();
        assert!(table.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(cosine(0).unwrap(), 1_000_000);
        assert_eq!(cosine(600).unwrap(), 500_000);
        assert_eq!(cosine(900).unwrap(), 0);
        assert_eq!(cosine(1800).unwrap(), -1_000_000);
        assert_eq!(cosine(2700).unwrap(), 0);
    }

    #[test]
    fn values_round_and_spell_as_literals_and_the_source_names_both_tables() {
        assert_eq!(rounded(2.4).unwrap(), 2);
        assert_eq!(rounded(-2.6).unwrap(), -3);
        assert_eq!(rounded(-0.2).unwrap(), 0);
        assert_eq!(super::literal(0), "0");
        assert_eq!(super::literal(304), "304");
        assert_eq!(super::literal(1000), "1_000");
        assert_eq!(super::literal(102_242), "102_242");
        assert_eq!(super::literal(-1_000_000), "-1_000_000");
        let text = source().unwrap();
        assert!(text.contains("pub static LINEAR: [i64; 256] = ["), "{text}");
        assert!(
            text.contains("pub static COSINE: [i64; 3600] = ["),
            "{text}"
        );
        assert!(text.ends_with("];\n"), "{text}");
    }
}
