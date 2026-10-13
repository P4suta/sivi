//! The theme loader never panics, refuses a file exactly when it reports a diagnostic, and reads the same text the same way twice.
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    match sivi_theme::parse(text) {
        Ok(theme) => {
            assert!(!theme.masters.is_empty(), "a theme that loads has a master");
            for master in &theme.masters {
                assert!(!master.slots.is_empty(), "a master that loads has a slot");
            }
        }
        Err(diagnostics) => assert!(!diagnostics.is_empty(), "a refusal names a diagnostic"),
    }
    assert_eq!(
        sivi_theme::parse(text).is_ok(),
        sivi_theme::parse(text).is_ok(),
        "the load is deterministic"
    );
});
