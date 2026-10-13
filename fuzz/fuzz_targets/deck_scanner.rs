//! The deck scanner never panics, keeps every line of a slide in place, and reads the same deck the same way twice.
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let scan = sivi_markdown::scanner::parse(text);
    for slide in &scan.slides {
        assert!(
            slide.first_line >= 1,
            "a slide starts on a line of the deck"
        );
        let lines = slide.text.matches('\n').count();
        let spanned = if slide.text.is_empty() {
            0
        } else {
            slide.span.end.line - slide.first_line + 1
        };
        assert_eq!(
            lines, spanned,
            "a slide's text covers exactly the lines it spans"
        );
        assert!(slide.text.ends_with('\n') || slide.text.is_empty());
        for entry in &slide.front {
            assert!(!entry.key.is_empty() && !entry.value.is_empty());
            assert!(entry.value_span.start.column <= entry.value_span.end.column);
        }
    }
    assert_eq!(
        scan,
        sivi_markdown::scanner::parse(text),
        "the scan is deterministic"
    );
});
