//! Witness tests for the ETNA workload. Each witness is a concrete frozen
//! input that exercises a specific bug. On base HEAD all witnesses pass; on
//! the corresponding marauder/patch variant, they fail.

use ahash::etna::{property_null_padding_distinct, PropertyResult};

fn assert_pass(r: PropertyResult) {
    match r {
        PropertyResult::Pass => {}
        PropertyResult::Fail(m) => panic!("witness FAIL: {}", m),
        PropertyResult::Discard => panic!("witness DISCARDED"),
    }
}

// ---- null_padding_distinct witnesses ----

#[test]
fn witness_null_padding_distinct_case_zero_vs_one() {
    assert_pass(property_null_padding_distinct((0, 1)));
}

#[test]
fn witness_null_padding_distinct_case_short_within_path() {
    // Both lengths fall through `read_small` (len <= 8); read_small returns
    // [0, 0] for either, so without `add_in_length` they collide.
    assert_pass(property_null_padding_distinct((5, 8)));
}

#[test]
fn witness_null_padding_distinct_case_zero_vs_eight() {
    assert_pass(property_null_padding_distinct((0, 8)));
}

#[test]
fn witness_null_padding_distinct_case_two_vs_seven() {
    assert_pass(property_null_padding_distinct((2, 7)));
}
