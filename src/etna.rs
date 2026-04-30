//! ETNA benchmark harness for the `ahash` crate.
//!
//! Each `property_*` function below is a framework-neutral, deterministic
//! invariant check used by `src/bin/etna.rs` and the witness tests under
//! `tests/etna_witnesses.rs`. They take owned concrete inputs, return
//! `PropertyResult`, and never panic on the property side: panics from the
//! library-under-test are caught and surfaced as `Fail`.

use core::hash::{BuildHasher, Hasher};
use std::format;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::string::String;

use crate::random_state::RandomState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyResult {
    Pass,
    Fail(String),
    Discard,
}

/// Hash `bytes` through the public `RandomState`-built hasher with a fixed
/// seed. Routes through whichever underlying `AHasher` is enabled for the
/// current target (aes_hash on AES-capable arches, fallback_hash otherwise).
fn hash_bytes(bytes: &[u8]) -> u64 {
    let state = RandomState::with_seeds(0, 0, 0, 0);
    let mut hasher = state.build_hasher();
    hasher.write(bytes);
    hasher.finish()
}

// ---------------------------------------------------------------------------
// Bug 1: null padding collisions (5c99070 — Prevent null padding collisions)
// ---------------------------------------------------------------------------
//
// `AHasher::write` historically did not mix the input length into its state.
// All-zero inputs of different lengths therefore collapsed to identical
// internal state and produced identical hash outputs. The fix added
// `add_in_length(&mut self.enc, length as u64)` (aes_hash) / a length-add
// inside the buffer mix (fallback_hash) so length differences propagate.
//
// The property below exercises the surface directly: two distinct lengths
// of all-zero bytes must not hash to the same value.

/// `hash([0u8; n]) != hash([0u8; m])` for distinct `n != m` in 0..=64.
pub fn property_null_padding_distinct(args: (u8, u8)) -> PropertyResult {
    let (n, m) = args;
    if n == m {
        return PropertyResult::Discard;
    }
    let v1 = vec![0u8; n as usize];
    let v2 = vec![0u8; m as usize];
    let r = catch_unwind(AssertUnwindSafe(|| (hash_bytes(&v1), hash_bytes(&v2))));
    match r {
        Err(_) => PropertyResult::Fail(format!("hash panicked at lengths n={n}, m={m}")),
        Ok((h1, h2)) => {
            if h1 == h2 {
                PropertyResult::Fail(format!(
                    "hash([0u8; {n}]) == hash([0u8; {m}]) == {h1:#x}"
                ))
            } else {
                PropertyResult::Pass
            }
        }
    }
}
