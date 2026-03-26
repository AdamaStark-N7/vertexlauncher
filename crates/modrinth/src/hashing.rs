use sha1::{Digest as Sha1Digest, Sha1};
use sha2::{Digest as Sha2Digest, Sha512};
use std::io::Read as _;
use std::path::Path;

const HASH_BUFFER_SIZE: usize = 64 * 1024;

pub fn hash_file_sha1_hex(path: &Path) -> Result<String, std::io::Error> {
    let (sha1, _) = hash_file_sha1_and_sha512_hex(path)?;
    Ok(sha1)
}

pub fn hash_file_sha512_hex(path: &Path) -> Result<String, std::io::Error> {
    let (_, sha512) = hash_file_sha1_and_sha512_hex(path)?;
    Ok(sha512)
}

pub fn hash_file_sha1_and_sha512_hex(path: &Path) -> Result<(String, String), std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut buffer = [0_u8; HASH_BUFFER_SIZE];
    let mut sha1 = Sha1::new();
    let mut sha512 = Sha512::new();

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        let chunk = &buffer[..bytes_read];
        Sha1Digest::update(&mut sha1, chunk);
        Sha2Digest::update(&mut sha512, chunk);
    }

    let sha1_bytes = Sha1Digest::finalize(sha1);
    let sha512_bytes = Sha2Digest::finalize(sha512);

    Ok((
        bytes_to_lower_hex(sha1_bytes.as_slice()),
        bytes_to_lower_hex(sha512_bytes.as_slice()),
    ))
}

fn bytes_to_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_known_abc_values() {
        let temp_path = std::env::temp_dir().join(format!(
            "vertexlauncher-modrinth-hash-test-{}.txt",
            std::process::id()
        ));
        std::fs::write(temp_path.as_path(), b"abc").expect("write temp file");

        let (sha1, sha512) =
            hash_file_sha1_and_sha512_hex(temp_path.as_path()).expect("hash temp file");

        let _ = std::fs::remove_file(temp_path.as_path());

        assert_eq!(sha1, "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            sha512,
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
            2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
    }
}
