# ahash — ETNA Tasks

Total tasks: 4

## Task Index

| Task | Variant | Framework | Property | Witness |
|------|---------|-----------|----------|---------|
| 001 | `null_padding_collisions_5c99070_1` | proptest | `NullPaddingDistinct` | `witness_null_padding_distinct_case_zero_vs_one` |
| 002 | `null_padding_collisions_5c99070_1` | quickcheck | `NullPaddingDistinct` | `witness_null_padding_distinct_case_zero_vs_one` |
| 003 | `null_padding_collisions_5c99070_1` | crabcheck | `NullPaddingDistinct` | `witness_null_padding_distinct_case_zero_vs_one` |
| 004 | `null_padding_collisions_5c99070_1` | hegel | `NullPaddingDistinct` | `witness_null_padding_distinct_case_zero_vs_one` |

## Witness Catalog

- `witness_null_padding_distinct_case_zero_vs_one` — base passes, variant fails
- `witness_null_padding_distinct_case_short_within_path` — base passes, variant fails
- `witness_null_padding_distinct_case_zero_vs_eight` — base passes, variant fails
- `witness_null_padding_distinct_case_two_vs_seven` — base passes, variant fails
