//! The tests of the checksum implementation.
//!
//! The launcher verifies every download with a sha1, an implementation that is slow for large
//! files would make the installation unusable.

use aether_launcher::http::{hex_digest, sha1_digest};

#[test]
fn the_sha1_matches_the_reference_vectors() {
    assert_eq!(
        hex_digest(&sha1_digest(b"")),
        "da39a3ee5e6b4b0d3255bfef95601890afd80709"
    );
    assert_eq!(
        hex_digest(&sha1_digest(b"abc")),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
    assert_eq!(
        hex_digest(&sha1_digest(
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
        )),
        "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
    );
}

#[test]
fn the_sha1_handles_a_large_input_quickly() {
    // A megabyte of data, the implementation must not copy the buffer for every block.
    let data = vec![0x61u8; 1024 * 1024];

    let start = std::time::Instant::now();
    let digest = hex_digest(&sha1_digest(&data));
    let elapsed = start.elapsed();

    assert_eq!(digest.len(), 40);
    assert!(
        elapsed.as_secs_f64() < 2.0,
        "hashing a megabyte took {:?}, the implementation is too slow",
        elapsed
    );
}

#[test]
fn the_sha1_of_a_large_input_is_correct() {
    // The value of a megabyte of 'a', the same as java.security.MessageDigest and Get-FileHash.
    let data = vec![0x61u8; 1024 * 1024];
    assert_eq!(
        hex_digest(&sha1_digest(&data)),
        "454027d64e3b855735552d42230eea1cbd645fa0"
    );
}
