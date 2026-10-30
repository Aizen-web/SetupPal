use std::io::Write;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

use super::enforce_download_hash;
use crate::adapters::AdapterError;
use crate::config::CoreError;
use crate::safety::hash::compute_file_sha256;

fn write_temp(bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "setuppal-hash-test-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("artifact.bin");
    let mut f = std::fs::File::create(&path).expect("create");
    f.write_all(bytes).expect("write");
    path
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[test]
fn enforce_download_hash_accepts_matching_sha256() {
    let bytes = b"setuppal-real-payload";
    let path = write_temp(bytes);
    let expected = sha256_hex(bytes);

    let result = enforce_download_hash(&path, Some(&expected), None).expect("hash ok");
    assert!(result.passed);
    assert!(path.is_file(), "matching file must remain");
    assert_eq!(compute_file_sha256(&path).unwrap(), expected);

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(path.parent().unwrap());
}

#[test]
fn enforce_download_hash_rejects_mismatch_and_deletes_file() {
    let path = write_temp(b"setuppal-real-payload");
    let wrong = sha256_hex(b"different-bytes");

    let err = enforce_download_hash(&path, Some(&wrong), None).expect_err("mismatch");
    match err {
        CoreError::Adapter(AdapterError::HashMismatch { .. }) => {}
        other => panic!("expected HashMismatch, got {other:?}"),
    }
    assert!(!path.exists(), "mismatched download must be deleted");

    let _ = std::fs::remove_dir(path.parent().unwrap());
}

#[test]
fn enforce_download_hash_requires_some_digest() {
    let path = write_temp(b"no-digest");
    let err = enforce_download_hash(&path, None, None).expect_err("no digest");
    assert!(matches!(err, CoreError::Adapter(_)));
    assert!(!path.exists(), "unverified download must be deleted");
    let _ = std::fs::remove_dir(path.parent().unwrap());
}
