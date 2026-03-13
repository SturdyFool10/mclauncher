use sha1::Sha1;
use sha2::{Digest, Sha512};
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
        sha1.update(chunk);
        sha512.update(chunk);
    }

    Ok((
        format!("{:x}", sha1.finalize()),
        format!("{:x}", sha512.finalize()),
    ))
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
