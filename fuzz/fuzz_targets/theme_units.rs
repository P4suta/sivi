//! A unit never panics, and a unit that reads prints back to text that reads to the same unit.
#![no_main]

use sivi_theme::units::{color, decimal, parse};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(unit) = parse(text) {
        let printed = format!("{unit}");
        assert_eq!(
            parse(&printed),
            Ok(unit),
            "a printed unit reads back to itself"
        );
    }
    if let Ok(value) = decimal(text) {
        assert!(value.0 >= 0, "a decimal without a sign is never negative");
    }
    let _ = color(text);
});
