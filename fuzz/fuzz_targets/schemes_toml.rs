//! The schemes reader never panics, finds every entry it accepts by its slug, refuses with a reason, and reads the same text the same way twice.
#![no_main]

/// The length of a color as `#rrggbb`.
const COLOR_LENGTH: usize = 7;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    match sivi_theme::schemes::parse(text) {
        Ok(schemes) => {
            assert_eq!(
                schemes.all().len(),
                schemes.slugs().count(),
                "every entry has a slug"
            );
            for scheme in schemes.all() {
                assert_eq!(
                    schemes.find(&scheme.slug).map(|found| found.slug.as_str()),
                    Some(scheme.slug.as_str()),
                    "an entry is found by its slug"
                );
                assert_eq!(
                    scheme.background().to_string().len(),
                    COLOR_LENGTH,
                    "a color spells as #rrggbb"
                );
            }
        }
        Err(why) => assert!(!why.is_empty(), "a refusal names a reason"),
    }
    assert_eq!(
        sivi_theme::schemes::parse(text).is_ok(),
        sivi_theme::schemes::parse(text).is_ok(),
        "the read is deterministic"
    );
});
