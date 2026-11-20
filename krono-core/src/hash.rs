use sha1::{Digest, Sha1};

/// Normalizes UTF-8 text newlines (`CRLF` / lone `CR` -> `LF`) so Windows, macOS,
/// Linux and Web produce bit-identical Git blob SHA-1 hashes.
pub fn normalize_lf_bytes(bytes: &[u8]) -> Vec<u8> {
    match std::str::from_utf8(bytes) {
        Ok(text) => {
            if text.contains('\r') {
                text.replace("\r\n", "\n").replace('\r', "\n").into_bytes()
            } else {
                bytes.to_vec()
            }
        }
        Err(_) => bytes.to_vec(),
    }
}

/// Computes the exact lowercase hex Git blob SHA-1 hash (`SHA1("blob <len>\0<bytes>")`),
/// matching Git and the GitHub Git Trees API bit-for-bit.
pub fn git_blob_sha1(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", bytes.len()).as_bytes());
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(40);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{:02x}", byte);
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_git_hash_object() {
        assert_eq!(
            git_blob_sha1(b"hello\n"),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
        assert_eq!(
            git_blob_sha1(b""),
            "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
        );
    }

    #[test]
    fn normalizes_crlf_and_lf_to_identical_sha1() {
        let crlf = normalize_lf_bytes(b"hello\r\nworld\r");
        let lf = normalize_lf_bytes(b"hello\nworld\n");
        assert_eq!(crlf, lf);
        assert_eq!(git_blob_sha1(&crlf), git_blob_sha1(&lf));
    }
}
