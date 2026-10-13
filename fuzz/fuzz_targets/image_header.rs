//! An image header never panics, and the pixels it declares read the same on every call.
#![no_main]

use sivi_core::image::dimensions;

/// The bytes of the longest signature a header starts with.
const SIGNATURE_BYTES: usize = 8;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let first = dimensions(data);
    assert_eq!(first, dimensions(data), "a header reads the same twice");
    if let Some(pixels) = first {
        assert!(
            data.len() > SIGNATURE_BYTES,
            "a header that names {pixels:?} holds more than a signature"
        );
    }
});
