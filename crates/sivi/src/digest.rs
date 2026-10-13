//! The SHA-256 digests that identify a build's inputs and check a theme's font files.

use sha2::{Digest as _, Sha256};

const HEX_RADIX: u32 = 16;
const HIGH_NIBBLE_SHIFT: u32 = 4;
const LOW_NIBBLE_MASK: u8 = 0x0f;

fn nibble(value: u8) -> char {
    char::from_digit(u32::from(value), HEX_RADIX).unwrap_or('0')
}

/// The lowercase hexadecimal SHA-256 of `parts` in order.
#[must_use]
pub fn sha256_hex(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    let mut hex = String::new();
    for byte in hasher.finalize() {
        hex.push(nibble(byte >> HIGH_NIBBLE_SHIFT));
        hex.push(nibble(byte & LOW_NIBBLE_MASK));
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::sha256_hex;

    #[test]
    fn the_digest_of_the_empty_input_and_of_split_parts_is_the_known_one() {
        assert_eq!(
            sha256_hex(&[]),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(sha256_hex(&[b"ab", b"c"]), sha256_hex(&[b"abc"]));
        assert_eq!(
            sha256_hex(&[b"abc"]),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
