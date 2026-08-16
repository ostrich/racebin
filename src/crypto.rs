use sha2::{Digest, Sha256};
use std::fmt::Write;

pub fn lower_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to a string cannot fail");
    }
    encoded
}

pub fn sha256_hex(value: impl AsRef<[u8]>) -> String {
    lower_hex(&Sha256::digest(value.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_is_lowercase_and_zero_padded() {
        assert_eq!(
            sha256_hex("racebin"),
            "817b1f88f1a496c41179404633007eafe34a25f13f8f43ea8b662b22e0722a43"
        );
    }
}
