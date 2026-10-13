//! Front matter typing never panics and refuses a block exactly when it reports a diagnostic.
#![no_main]

use sivi_core::front_matter::{Entry, parse};
use sivi_core::front_matter_rules::Position;
use sivi_core::span::Span;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let entries: Vec<Entry> = text
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let (key, value) = line.split_once(": ")?;
            Some(Entry {
                key: key.to_owned(),
                value: value.to_owned(),
                span: Span::line(index + 1, line.len()),
                value_span: Span::line(index + 1, line.len()),
            })
        })
        .collect();
    for position in [Position::Head, Position::Slide] {
        match parse(position, &entries) {
            Ok(_) => {}
            Err(diagnostics) => assert!(!diagnostics.is_empty(), "a refusal names a diagnostic"),
        }
    }
});
