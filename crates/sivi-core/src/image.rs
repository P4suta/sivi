//! The pixel size an image file declares in its header, and whether an image laid out at a width is too coarse to print well.

use crate::length::{Length, MILLIPOINTS_PER_POINT};

/// The width and height of an image in pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
}

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
/// Where the `IHDR` chunk type sits, right after the signature and the chunk length.
const PNG_CHUNK_TYPE: usize = 12;
const PNG_IHDR: [u8; 4] = *b"IHDR";
/// Where the width and the height of the `IHDR` chunk sit.
const PNG_WIDTH: usize = 16;
const PNG_HEIGHT: usize = 20;

const JPEG_SIGNATURE: [u8; 2] = [0xff, 0xd8];
const JPEG_MARKER: u8 = 0xff;
/// The start-of-frame markers, each of which carries the size of the image.
const JPEG_FRAMES: [u8; 13] = [
    0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf,
];
/// The markers that carry no segment: start of image, temporary, and the restart markers.
const JPEG_BARE: [u8; 10] = [0xd8, 0x01, 0xd0, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7];
/// The start of scan, after which the size cannot appear.
const JPEG_SCAN: u8 = 0xda;
/// Where the height and the width sit inside a start-of-frame segment, after the marker.
const JPEG_HEIGHT: usize = 5;
const JPEG_WIDTH: usize = 7;
/// The bytes of a marker and of a segment length.
const MARKER_BYTES: usize = 2;
const LENGTH_BYTES: usize = 2;

/// The resolution below which an image prints coarse, in pixels per inch.
pub const MIN_PIXELS_PER_INCH: i64 = 144;
const POINTS_PER_INCH: i64 = 72;
/// The bytes of a big-endian thirty-two bit field, and the bits of a byte.
const U32_BYTES: usize = 4;
const BITS_PER_BYTE: u32 = 8;

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at.checked_add(U32_BYTES)?)?;
    let mut value: u32 = 0;
    for byte in slice {
        value = value
            .checked_shl(BITS_PER_BYTE)?
            .checked_add(u32::from(*byte))?;
    }
    Some(value)
}

fn be16(bytes: &[u8], at: usize) -> Option<u16> {
    let high = *bytes.get(at)?;
    let low = *bytes.get(at.checked_add(1)?)?;
    Some(u16::from_be_bytes([high, low]))
}

fn png(bytes: &[u8]) -> Option<Pixels> {
    if !bytes.starts_with(&PNG_SIGNATURE)
        || bytes.get(PNG_CHUNK_TYPE..PNG_CHUNK_TYPE.checked_add(PNG_IHDR.len())?)? != PNG_IHDR
    {
        return None;
    }
    Some(Pixels {
        width: be32(bytes, PNG_WIDTH)?,
        height: be32(bytes, PNG_HEIGHT)?,
    })
}

fn jpeg(bytes: &[u8]) -> Option<Pixels> {
    if !bytes.starts_with(&JPEG_SIGNATURE) {
        return None;
    }
    let mut at = MARKER_BYTES;
    while *bytes.get(at)? == JPEG_MARKER {
        let marker = *bytes.get(at.checked_add(1)?)?;
        if JPEG_FRAMES.contains(&marker) {
            return Some(Pixels {
                width: u32::from(be16(bytes, at.checked_add(JPEG_WIDTH)?)?),
                height: u32::from(be16(bytes, at.checked_add(JPEG_HEIGHT)?)?),
            });
        }
        if marker == JPEG_SCAN {
            return None;
        }
        at = if JPEG_BARE.contains(&marker) || marker == JPEG_MARKER {
            at.checked_add(if marker == JPEG_MARKER {
                1
            } else {
                MARKER_BYTES
            })?
        } else {
            // A length counts its own two bytes, so a shorter one is no segment at all.
            let length = usize::from(be16(bytes, at.checked_add(MARKER_BYTES)?)?);
            let payload = length.checked_sub(LENGTH_BYTES)?;
            at.checked_add(MARKER_BYTES)?
                .checked_add(LENGTH_BYTES)?
                .checked_add(payload)?
        };
    }
    None
}

/// The pixel size a PNG or JPEG file declares, or `None` for any other file or a header cut short.
#[must_use]
pub fn dimensions(bytes: &[u8]) -> Option<Pixels> {
    png(bytes).or_else(|| jpeg(bytes))
}

/// The pixels per inch of an image `pixels` wide shown `width` wide, or `None` for no width.
#[must_use]
pub fn pixels_per_inch(pixels: u32, width: Length) -> Option<i64> {
    if !width.is_positive() {
        return None;
    }
    i64::from(pixels)
        .checked_mul(POINTS_PER_INCH)?
        .checked_mul(MILLIPOINTS_PER_POINT)?
        .checked_div(width.0)
}

/// Whether an image at `pixels_per_inch` prints coarse.
#[must_use]
pub const fn is_coarse(pixels_per_inch: i64) -> bool {
    pixels_per_inch < MIN_PIXELS_PER_INCH
}

#[cfg(test)]
mod tests {
    use super::{Pixels, dimensions, is_coarse, pixels_per_inch};
    use crate::length::Length;
    use alloc::vec::Vec;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']);
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    fn jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = alloc::vec![0xff, 0xd8];
        bytes.extend_from_slice(&[0xff, 0xe0, 0x00, 0x04, 0x4a, 0x46]);
        bytes.extend_from_slice(&[0xff, 0xd0]);
        bytes.extend_from_slice(&[0xff, 0xc0, 0x00, 0x11, 0x08]);
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes
    }

    #[test]
    fn the_headers_of_png_and_jpeg_files_name_their_pixels() {
        assert_eq!(
            dimensions(&png(640, 480)),
            Some(Pixels {
                width: 640,
                height: 480
            })
        );
        assert_eq!(
            dimensions(&jpeg(300, 200)),
            Some(Pixels {
                width: 300,
                height: 200
            })
        );
        assert_eq!(dimensions(&png(1, 1)[..20]), None);
        assert_eq!(dimensions(b"<svg/>"), None);
        assert_eq!(dimensions(&[0xff, 0xd8, 0xff, 0xda, 0x00, 0x02]), None);
        assert_eq!(dimensions(&[0xff, 0xd8, 0xff, 0xe0, 0x00, 0x01]), None);
        assert_eq!(dimensions(&[]), None);
    }

    #[test]
    fn a_png_needs_both_its_signature_and_its_header_chunk() {
        let mut other_chunk = png(2, 2);
        other_chunk.splice(12..16, *b"IDAT");
        assert_eq!(dimensions(&other_chunk), None);
        let mut other_signature = png(2, 2);
        other_signature.splice(0..8, *b"notapng!");
        assert_eq!(dimensions(&other_signature), None);
    }

    #[test]
    fn a_jpeg_skips_an_empty_segment_and_stops_at_a_length_too_short() {
        let mut empty_segment = alloc::vec![0xff, 0xd8, 0xff, 0xe1, 0x00, 0x02];
        empty_segment.extend_from_slice(&[0xff, 0xc2, 0x00, 0x11, 0x08, 0x00, 0x10, 0x00, 0x20]);
        assert_eq!(
            dimensions(&empty_segment),
            Some(Pixels {
                width: 32,
                height: 16
            })
        );
        let mut short_length = alloc::vec![0xff, 0xd8, 0xff, 0xe1, 0x00, 0x00];
        short_length.extend_from_slice(&[0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x10, 0x00, 0x20]);
        assert_eq!(dimensions(&short_length), None);
    }

    #[test]
    fn the_resolution_follows_the_pixels_over_the_printed_width() {
        let two_inches = Length::from_millipoints(144_000);
        assert_eq!(pixels_per_inch(288, two_inches), Some(144));
        assert_eq!(pixels_per_inch(200, two_inches), Some(100));
        assert_eq!(pixels_per_inch(200, Length::ZERO), None);
        assert!(is_coarse(143));
        assert!(!is_coarse(144));
    }
}
