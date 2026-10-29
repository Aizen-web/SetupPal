use std::io::Read;
use std::path::Path;
use sha2::{Digest, Sha256, Sha512};

use crate::adapters::AdapterError;

/// Computes the lowercase hex-encoded SHA-256 digest of a file on disk using streaming reads.
pub fn compute_file_sha256(path: &Path) -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Computes the lowercase hex-encoded SHA-512 digest of a file on disk (used by Modrinth).
pub fn compute_file_sha512(path: &Path) -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha512::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Verifies a file's SHA-256 digest against `expected_sha256` (case-insensitive hex).
pub fn verify_file_sha256(path: &Path, expected_sha256: &str) -> Result<String, AdapterError> {
    let actual = compute_file_sha256(path)?;
    let expected_norm = expected_sha256.trim().to_ascii_lowercase();
    if actual != expected_norm {
        return Err(AdapterError::HashMismatch {
            file: path.to_path_buf(),
            expected: expected_norm,
            actual,
        });
    }
    Ok(actual)
}
