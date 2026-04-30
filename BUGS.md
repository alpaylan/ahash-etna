# ahash — Injected Bugs

ahash — non-cryptographic AES-NI hash function — ETNA workload.

Total mutations: 1

## Bug Index

| # | Variant | Name | Location | Injection | Fix Commit |
|---|---------|------|----------|-----------|------------|
| 1 | `null_padding_collisions_5c99070_1` | `null_padding_collisions` | `src/aes_hash.rs:154` | `marauders` | `5c99070f97854557ec5e0e6451189798a8ad5853` |

## Property Mapping

| Variant | Property | Witness(es) |
|---------|----------|-------------|
| `null_padding_collisions_5c99070_1` | `NullPaddingDistinct` | `witness_null_padding_distinct_case_zero_vs_one`, `witness_null_padding_distinct_case_short_within_path`, `witness_null_padding_distinct_case_zero_vs_eight`, `witness_null_padding_distinct_case_two_vs_seven` |

## Framework Coverage

| Property | proptest | quickcheck | crabcheck | hegel |
|----------|---------:|-----------:|----------:|------:|
| `NullPaddingDistinct` | ✓ | ✓ | ✓ | ✓ |

## Bug Details

### 1. null_padding_collisions

- **Variant**: `null_padding_collisions_5c99070_1`
- **Location**: `src/aes_hash.rs:154` (inside `<AHasher as Hasher>::write`)
- **Property**: `NullPaddingDistinct`
- **Witness(es)**:
  - `witness_null_padding_distinct_case_zero_vs_one`
  - `witness_null_padding_distinct_case_short_within_path`
  - `witness_null_padding_distinct_case_zero_vs_eight`
  - `witness_null_padding_distinct_case_two_vs_seven`
- **Source**: internal report — Prevent null padding collisions.
  > `AHasher::write` historically did not mix the input length into its internal state. Because the binary-search load helpers (`read_small`, `large_update`) consume only data bytes, all-zero inputs of different lengths collapsed to identical internal state and hashed to the same value. The fix added a length-mixing step (`add_in_length(&mut self.enc, length as u64)` in the AES variant; `self.buffer = self.buffer.wrapping_add(length).wrapping_mul(MULTIPLE)` in the fallback variant) before the data is consumed, so length differences propagate into the final hash.
- **Fix commit**: `5c99070f97854557ec5e0e6451189798a8ad5853` — Prevent null padding collisions.
- **Invariant violated**: For any two distinct lengths `n` and `m`, `RandomState::with_seeds(0,0,0,0)` hashing `[0u8; n]` must produce a different value than hashing `[0u8; m]`. Equivalently: an all-zero input of length `n` is uniquely identified by `n` in the hash output.
- **How the mutation triggers**: The buggy `<AHasher as Hasher>::write` removes the length-mixing step (`add_in_length` / the buffer wrapping_add+wrapping_mul). For all-zero inputs of length 0..=8, `read_small` returns `[0, 0]` regardless of length; without the length mix the internal state is identical. The witness `(n=0, m=1)` therefore collides on the buggy code (both hash to `0xc3acadb48856bc1e` with the seed used) but produces distinct hashes on the fixed code.

## Dropped Candidates

- `4edd748` (Adopt key mixing from fallback in AES variant) — no observable public invariant — XORing the keys with PI hardens against the all-zero-key edge case, but the AES rounds still produce non-zero output, leaving no simple property whose pass/fail flips cleanly between fixed and buggy code on the public RandomState API.
