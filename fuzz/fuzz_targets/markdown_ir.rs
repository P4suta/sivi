//! The front end never panics, withholds the model exactly when it reports an error, and reads the same bytes the same way twice.
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let parsed = sivi_markdown::parse(data);
    let errors = sivi_core::diagnostic::has_error(&parsed.diagnostics);
    assert_eq!(
        parsed.deck.is_none(),
        errors,
        "the model is withheld exactly on an error"
    );
    if let Some(deck) = &parsed.deck {
        for (index, slide) in deck.slides.iter().enumerate() {
            assert_eq!(slide.number, index + 1, "slides are numbered in order");
        }
    }
    assert_eq!(
        sivi_markdown::parse(data),
        parsed,
        "the parse is deterministic"
    );
});
