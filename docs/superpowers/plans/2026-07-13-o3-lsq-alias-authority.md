# O3 LSQ Gem5 Alias Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace duplicated O3 LSQ gem5 alias mappings with one crate-private typed descriptor authority while preserving every runtime, JSON, text, and host-action output invariant.

**Architecture:** A new root-level `o3_lsq_aliases` module owns only operation, ordering, bucket, and data-response metric vocabulary. Existing emitters keep their local path construction, ordering, serialization, metadata, filtering, and duplicate-suppression behavior; black-box tests state expected public names explicitly instead of translating them through production-like helpers.

**Tech Stack:** Rust 2021 workspace, `rem6` CLI crate, `rem6-cpu` O3 runtime enums/stats, `rem6-stats`, Cargo integration tests, source-policy structural tests, top-level `rem6 run --execute` compatibility evidence.

---

## File Map

- Create `crates/rem6/src/o3_lsq_aliases.rs`: own typed LSQ operation, ordering, bucket, and latency-metric aliases plus lookup and completeness tests.
- Modify `crates/rem6/src/lib.rs`: declare the focused crate-private alias module.
- Modify `crates/rem6/src/stats_output/o3_runtime_gem5_lsq.rs`: consume descriptors while retaining registry emission and value extraction.
- Modify `crates/rem6/src/stats_output/json_aliases.rs`: derive LSQ count buckets from descriptors while retaining JSON record behavior.
- Modify `crates/rem6/src/stats_output/text_o3.rs`: derive LSQ count buckets from descriptors while retaining text behavior.
- Modify `crates/rem6/src/host_actions/o3_stats_dump_aliases.rs`: replace local LSQ count/latency mapping matches with descriptor lookup.
- Modify `crates/rem6/tests/source_policy.rs`: register a focused O3 alias policy module while keeping the source-policy driver below 1,500 lines.
- Create `crates/rem6/tests/source_policy/o3_alias_authority.rs`: own the existing IEW authority policy and the new LSQ production/test authority policies.
- Modify `crates/rem6/tests/cli_run/m5_host_actions.rs`: make the shared LSQ assertion helper consume explicit expected aliases.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/lsq.rs`: provide explicit operation/ordering alias expectations in two CLI matrices.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/lsq/runtime.rs`: provide explicit alias expectations in the ordered atomic runtime matrix.
- Do not modify `docs/architecture/gem5-to-rem6-migration.md`: executable evidence and score remain unchanged, and the ledger stays exactly 1,200 lines.

### Task 1: Extract O3 Alias Policy And Add The Failing LSQ Check

**Files:**
- Modify: `crates/rem6/tests/source_policy.rs`
- Create: `crates/rem6/tests/source_policy/o3_alias_authority.rs`

- [ ] **Step 1: Extract the existing IEW authority policy**

Add this module declaration after the imports in `crates/rem6/tests/source_policy.rs`:

```rust
#[path = "source_policy/o3_alias_authority.rs"]
mod o3_alias_authority;
```

Create `crates/rem6/tests/source_policy/o3_alias_authority.rs` with:

```rust
use std::fs;
use std::path::Path;
```

Move the complete existing
`o3_iew_gem5_aliases_have_one_projection_authority` test from
`crates/rem6/tests/source_policy.rs` into the new module without changing its
body. The root driver must fall from 1,496 lines to well below its 1,500-line
guard.

- [ ] **Step 2: Verify and commit the mechanical extraction**

Run:

```bash
cargo test -p rem6 --test source_policy o3_iew_gem5_aliases_have_one_projection_authority -- --nocapture
cargo test -p rem6 --test source_policy source_policy_driver_keeps_anchor_data_out_of_root -- --nocapture
```

Expected: both pass, proving the test still runs under its module namespace and
the root driver remains within policy.

Commit:

```bash
git add crates/rem6/tests/source_policy.rs \
  crates/rem6/tests/source_policy/o3_alias_authority.rs
git commit -m "test: extract O3 alias source policy"
```

- [ ] **Step 3: Add the structural LSQ policy test**

Append this test to `crates/rem6/tests/source_policy/o3_alias_authority.rs`:

```rust
#[test]
fn o3_lsq_gem5_aliases_have_one_projection_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let authority_path = crate_dir.join("src/o3_lsq_aliases.rs");

    assert!(
        lib.contains("mod o3_lsq_aliases;"),
        "src/lib.rs must declare the shared O3 LSQ alias authority"
    );
    assert!(
        authority_path.exists(),
        "O3 LSQ gem5 alias mappings belong in src/o3_lsq_aliases.rs"
    );
    let authority = fs::read_to_string(authority_path).unwrap();
    for anchor in [
        "pub(crate) struct O3LsqOperationGem5Alias",
        "pub(crate) struct O3LsqOrderingGem5Alias",
        "pub(crate) struct O3LsqDataResponseGem5Alias",
        "pub(crate) const O3_LSQ_OPERATION_GEM5_ALIASES",
        "pub(crate) const O3_LSQ_ORDERING_GEM5_ALIASES",
        "pub(crate) const O3_LSQ_DATA_RESPONSE_GEM5_ALIASES",
    ] {
        assert!(
            authority.contains(anchor),
            "shared O3 LSQ alias authority is missing `{anchor}`"
        );
    }

    let mapping_tokens = [
        r#""loadReserved""#,
        r#""storeConditional""#,
        r#""floatLoad""#,
        r#""floatStore""#,
        r#""vectorLoad""#,
        r#""vectorStore""#,
        r#""acquireRelease""#,
        r#""LoadReserved""#,
        r#""StoreConditional""#,
        r#""FloatLoad""#,
        r#""FloatStore""#,
        r#""VectorLoad""#,
        r#""VectorStore""#,
        r#""AcquireRelease""#,
        r#""totalLatency""#,
        r#""maxLatency""#,
        r#""minLatency""#,
        r#""avgLatency""#,
    ];
    for mapping in mapping_tokens {
        assert!(
            authority.contains(mapping),
            "shared O3 LSQ alias authority is missing `{mapping}`"
        );
    }

    let runtime = fs::read_to_string(
        crate_dir.join("src/stats_output/o3_runtime_gem5_lsq.rs"),
    )
    .unwrap();
    let json = fs::read_to_string(crate_dir.join("src/stats_output/json_aliases.rs")).unwrap();
    let text = fs::read_to_string(crate_dir.join("src/stats_output/text_o3.rs")).unwrap();
    let stats_dump =
        fs::read_to_string(crate_dir.join("src/host_actions/o3_stats_dump_aliases.rs")).unwrap();
    let stats_dump_impl = stats_dump.split("#[cfg(test)]").next().unwrap();
    for (name, consumer) in [
        ("runtime O3 LSQ stats", runtime.as_str()),
        ("JSON aliases", json.as_str()),
        ("text O3 stats", text.as_str()),
        ("host-action stats-dump aliases", stats_dump_impl),
    ] {
        assert!(
            consumer.contains("crate::o3_lsq_aliases"),
            "{name} must consume the shared O3 LSQ alias authority"
        );
        for local_mapping in mapping_tokens {
            assert!(
                !consumer.contains(local_mapping),
                "{name} must not retain local O3 LSQ mapping `{local_mapping}`"
            );
        }
    }
}
```

- [ ] **Step 4: Run the policy test and observe the intended failure**

Run:

```bash
cargo test -p rem6 --test source_policy o3_lsq_gem5_aliases_have_one_projection_authority -- --nocapture
```

Expected: FAIL at `src/lib.rs must declare the shared O3 LSQ alias authority`.

Do not commit this failing state. Continue directly to Task 2.

### Task 2: Create The Descriptor Authority And Migrate Production Consumers

**Files:**
- Create: `crates/rem6/src/o3_lsq_aliases.rs`
- Modify: `crates/rem6/src/lib.rs`
- Modify: `crates/rem6/src/stats_output/o3_runtime_gem5_lsq.rs`
- Modify: `crates/rem6/src/stats_output/json_aliases.rs`
- Modify: `crates/rem6/src/stats_output/text_o3.rs`
- Modify: `crates/rem6/src/host_actions/o3_stats_dump_aliases.rs`
- Test: `crates/rem6/tests/source_policy/o3_alias_authority.rs`

- [ ] **Step 1: Create the typed authority module**

Create `crates/rem6/src/o3_lsq_aliases.rs` with this API and descriptor data:

```rust
use rem6_cpu::{O3RuntimeLsqOperation, O3RuntimeLsqOrdering};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3LsqOperationGem5Alias {
    operation: O3RuntimeLsqOperation,
    alias: &'static str,
    bucket_alias: &'static str,
}

impl O3LsqOperationGem5Alias {
    const fn new(
        operation: O3RuntimeLsqOperation,
        alias: &'static str,
        bucket_alias: &'static str,
    ) -> Self {
        Self {
            operation,
            alias,
            bucket_alias,
        }
    }

    pub(crate) const fn operation(self) -> O3RuntimeLsqOperation {
        self.operation
    }

    pub(crate) const fn source_name(self) -> &'static str {
        self.operation.as_str()
    }

    pub(crate) const fn alias(self) -> &'static str {
        self.alias
    }

    pub(crate) const fn bucket_alias(self) -> &'static str {
        self.bucket_alias
    }
}

pub(crate) const O3_LSQ_OPERATION_GEM5_ALIASES: &[O3LsqOperationGem5Alias] = &[
    O3LsqOperationGem5Alias::new(O3RuntimeLsqOperation::Load, "load", "Load"),
    O3LsqOperationGem5Alias::new(O3RuntimeLsqOperation::Store, "store", "Store"),
    O3LsqOperationGem5Alias::new(
        O3RuntimeLsqOperation::LoadReserved,
        "loadReserved",
        "LoadReserved",
    ),
    O3LsqOperationGem5Alias::new(
        O3RuntimeLsqOperation::StoreConditional,
        "storeConditional",
        "StoreConditional",
    ),
    O3LsqOperationGem5Alias::new(O3RuntimeLsqOperation::Atomic, "atomic", "Atomic"),
    O3LsqOperationGem5Alias::new(
        O3RuntimeLsqOperation::FloatLoad,
        "floatLoad",
        "FloatLoad",
    ),
    O3LsqOperationGem5Alias::new(
        O3RuntimeLsqOperation::FloatStore,
        "floatStore",
        "FloatStore",
    ),
    O3LsqOperationGem5Alias::new(
        O3RuntimeLsqOperation::VectorLoad,
        "vectorLoad",
        "VectorLoad",
    ),
    O3LsqOperationGem5Alias::new(
        O3RuntimeLsqOperation::VectorStore,
        "vectorStore",
        "VectorStore",
    ),
];

pub(crate) fn o3_lsq_operation_gem5_alias_from_source(
    source: &str,
) -> Option<&'static O3LsqOperationGem5Alias> {
    O3_LSQ_OPERATION_GEM5_ALIASES
        .iter()
        .find(|alias| alias.source_name() == source)
}

pub(crate) fn o3_lsq_operation_gem5_alias_from_alias(
    name: &str,
) -> Option<&'static O3LsqOperationGem5Alias> {
    O3_LSQ_OPERATION_GEM5_ALIASES
        .iter()
        .find(|alias| alias.alias() == name)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3LsqOrderingGem5Alias {
    ordering: O3RuntimeLsqOrdering,
    alias: &'static str,
    bucket_alias: &'static str,
}

impl O3LsqOrderingGem5Alias {
    const fn new(
        ordering: O3RuntimeLsqOrdering,
        alias: &'static str,
        bucket_alias: &'static str,
    ) -> Self {
        Self {
            ordering,
            alias,
            bucket_alias,
        }
    }

    pub(crate) const fn ordering(self) -> O3RuntimeLsqOrdering {
        self.ordering
    }

    pub(crate) const fn source_name(self) -> &'static str {
        self.ordering.as_str()
    }

    pub(crate) const fn alias(self) -> &'static str {
        self.alias
    }

    pub(crate) const fn bucket_alias(self) -> &'static str {
        self.bucket_alias
    }
}

pub(crate) const O3_LSQ_ORDERING_GEM5_ALIASES: &[O3LsqOrderingGem5Alias] = &[
    O3LsqOrderingGem5Alias::new(O3RuntimeLsqOrdering::Acquire, "acquire", "Acquire"),
    O3LsqOrderingGem5Alias::new(O3RuntimeLsqOrdering::Release, "release", "Release"),
    O3LsqOrderingGem5Alias::new(
        O3RuntimeLsqOrdering::AcquireRelease,
        "acquireRelease",
        "AcquireRelease",
    ),
];

pub(crate) fn o3_lsq_ordering_gem5_alias_from_alias(
    name: &str,
) -> Option<&'static O3LsqOrderingGem5Alias> {
    O3_LSQ_ORDERING_GEM5_ALIASES
        .iter()
        .find(|alias| alias.alias() == name)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3LsqDataResponseMetric {
    Samples,
    TotalLatency,
    MaxLatency,
    MinLatency,
    AverageLatency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3LsqDataResponseGem5Alias {
    metric: O3LsqDataResponseMetric,
    source_suffix: &'static str,
    alias: &'static str,
    unit: &'static str,
}

impl O3LsqDataResponseGem5Alias {
    const fn new(
        metric: O3LsqDataResponseMetric,
        source_suffix: &'static str,
        alias: &'static str,
        unit: &'static str,
    ) -> Self {
        Self {
            metric,
            source_suffix,
            alias,
            unit,
        }
    }

    pub(crate) const fn metric(self) -> O3LsqDataResponseMetric {
        self.metric
    }

    pub(crate) const fn source_suffix(self) -> &'static str {
        self.source_suffix
    }

    pub(crate) const fn alias(self) -> &'static str {
        self.alias
    }

    pub(crate) const fn unit(self) -> &'static str {
        self.unit
    }
}

pub(crate) const O3_LSQ_DATA_RESPONSE_GEM5_ALIASES: &[O3LsqDataResponseGem5Alias] = &[
    O3LsqDataResponseGem5Alias::new(
        O3LsqDataResponseMetric::Samples,
        "samples",
        "samples",
        "Count",
    ),
    O3LsqDataResponseGem5Alias::new(
        O3LsqDataResponseMetric::TotalLatency,
        "ticks",
        "totalLatency",
        "Tick",
    ),
    O3LsqDataResponseGem5Alias::new(
        O3LsqDataResponseMetric::MaxLatency,
        "max_ticks",
        "maxLatency",
        "Tick",
    ),
    O3LsqDataResponseGem5Alias::new(
        O3LsqDataResponseMetric::MinLatency,
        "min_ticks",
        "minLatency",
        "Tick",
    ),
    O3LsqDataResponseGem5Alias::new(
        O3LsqDataResponseMetric::AverageLatency,
        "avg_ticks",
        "avgLatency",
        "Tick",
    ),
];

pub(crate) fn o3_lsq_data_response_gem5_alias_from_source(
    source_suffix: &str,
) -> Option<&'static O3LsqDataResponseGem5Alias> {
    O3_LSQ_DATA_RESPONSE_GEM5_ALIASES
        .iter()
        .find(|alias| alias.source_suffix() == source_suffix)
}
```

Append these unit tests in the same file:

```rust
#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn operation_aliases_match_tracked_order_and_public_spellings() {
        let operations = O3_LSQ_OPERATION_GEM5_ALIASES
            .iter()
            .map(|alias| alias.operation())
            .collect::<Vec<_>>();
        assert_eq!(
            operations.as_slice(),
            O3RuntimeLsqOperation::TRACKED.as_slice()
        );

        let aliases = O3_LSQ_OPERATION_GEM5_ALIASES
            .iter()
            .map(|alias| (alias.source_name(), alias.alias(), alias.bucket_alias()))
            .collect::<Vec<_>>();
        assert_eq!(
            aliases.as_slice(),
            &[
                ("load", "load", "Load"),
                ("store", "store", "Store"),
                ("load_reserved", "loadReserved", "LoadReserved"),
                (
                    "store_conditional",
                    "storeConditional",
                    "StoreConditional",
                ),
                ("atomic", "atomic", "Atomic"),
                ("float_load", "floatLoad", "FloatLoad"),
                ("float_store", "floatStore", "FloatStore"),
                ("vector_load", "vectorLoad", "VectorLoad"),
                ("vector_store", "vectorStore", "VectorStore"),
            ]
        );
        assert!(operations
            .iter()
            .all(|operation| *operation != O3RuntimeLsqOperation::None));
    }

    #[test]
    fn ordering_aliases_match_tracked_order_and_public_spellings() {
        let orderings = O3_LSQ_ORDERING_GEM5_ALIASES
            .iter()
            .map(|alias| alias.ordering())
            .collect::<Vec<_>>();
        assert_eq!(
            orderings.as_slice(),
            O3RuntimeLsqOrdering::TRACKED.as_slice()
        );
        assert_eq!(
            O3_LSQ_ORDERING_GEM5_ALIASES
                .iter()
                .map(|alias| (alias.source_name(), alias.alias(), alias.bucket_alias()))
                .collect::<Vec<_>>()
                .as_slice(),
            &[
                ("acquire", "acquire", "Acquire"),
                ("release", "release", "Release"),
                (
                    "acquire_release",
                    "acquireRelease",
                    "AcquireRelease",
                ),
            ]
        );
        assert!(orderings
            .iter()
            .all(|ordering| *ordering != O3RuntimeLsqOrdering::None));
    }

    #[test]
    fn data_response_aliases_preserve_metric_order_units_and_spellings() {
        assert_eq!(
            O3_LSQ_DATA_RESPONSE_GEM5_ALIASES
                .iter()
                .map(|alias| {
                    (
                        alias.metric(),
                        alias.source_suffix(),
                        alias.alias(),
                        alias.unit(),
                    )
                })
                .collect::<Vec<_>>()
                .as_slice(),
            &[
                (O3LsqDataResponseMetric::Samples, "samples", "samples", "Count"),
                (
                    O3LsqDataResponseMetric::TotalLatency,
                    "ticks",
                    "totalLatency",
                    "Tick",
                ),
                (
                    O3LsqDataResponseMetric::MaxLatency,
                    "max_ticks",
                    "maxLatency",
                    "Tick",
                ),
                (
                    O3LsqDataResponseMetric::MinLatency,
                    "min_ticks",
                    "minLatency",
                    "Tick",
                ),
                (
                    O3LsqDataResponseMetric::AverageLatency,
                    "avg_ticks",
                    "avgLatency",
                    "Tick",
                ),
            ]
        );
    }

    #[test]
    fn lsq_alias_names_are_unique_within_each_namespace() {
        let mut operation_sources = BTreeSet::new();
        let mut operation_aliases = BTreeSet::new();
        let mut operation_buckets = BTreeSet::new();
        for alias in O3_LSQ_OPERATION_GEM5_ALIASES {
            assert!(operation_sources.insert(alias.source_name()));
            assert!(operation_aliases.insert(alias.alias()));
            assert!(operation_buckets.insert(alias.bucket_alias()));
        }

        let mut ordering_sources = BTreeSet::new();
        let mut ordering_aliases = BTreeSet::new();
        let mut ordering_buckets = BTreeSet::new();
        for alias in O3_LSQ_ORDERING_GEM5_ALIASES {
            assert!(ordering_sources.insert(alias.source_name()));
            assert!(ordering_aliases.insert(alias.alias()));
            assert!(ordering_buckets.insert(alias.bucket_alias()));
        }

        let mut metric_sources = BTreeSet::new();
        let mut metric_aliases = BTreeSet::new();
        for alias in O3_LSQ_DATA_RESPONSE_GEM5_ALIASES {
            assert!(metric_sources.insert(alias.source_suffix()));
            assert!(metric_aliases.insert(alias.alias()));
        }
    }
}
```

- [ ] **Step 2: Declare the module**

In `crates/rem6/src/lib.rs`, add this beside the other O3 alias modules:

```rust
mod o3_lsq_aliases;
```

- [ ] **Step 3: Migrate runtime registry emission**

In `o3_runtime_gem5_lsq.rs`, replace direct operation/ordering imports and local
alias functions with:

```rust
use rem6_cpu::{O3RuntimeLsqOperation, O3RuntimeStats};

use crate::o3_lsq_aliases::{
    O3LsqDataResponseMetric, O3_LSQ_DATA_RESPONSE_GEM5_ALIASES,
    O3_LSQ_OPERATION_GEM5_ALIASES, O3_LSQ_ORDERING_GEM5_ALIASES,
};
```

Iterate `O3_LSQ_OPERATION_GEM5_ALIASES` and
`O3_LSQ_ORDERING_GEM5_ALIASES`, using `descriptor.operation()`,
`descriptor.ordering()`, and `descriptor.alias()` in the existing loops. Delete
`o3_lsq_operation_alias` and `o3_lsq_ordering_alias`.

Replace the hard-coded aggregate and operation latency tuples with descriptor
iteration and these local value selectors:

```rust
fn lsq_data_response_value(o3: O3RuntimeStats, metric: O3LsqDataResponseMetric) -> u64 {
    match metric {
        O3LsqDataResponseMetric::Samples => o3.lsq_data_latency_samples(),
        O3LsqDataResponseMetric::TotalLatency => o3.lsq_data_latency_ticks(),
        O3LsqDataResponseMetric::MaxLatency => o3.lsq_data_latency_max_ticks(),
        O3LsqDataResponseMetric::MinLatency => o3.lsq_data_latency_min_ticks(),
        O3LsqDataResponseMetric::AverageLatency => o3.lsq_data_latency_avg_ticks(),
    }
}

fn lsq_operation_data_response_value(
    o3: O3RuntimeStats,
    operation: O3RuntimeLsqOperation,
    metric: O3LsqDataResponseMetric,
) -> u64 {
    match metric {
        O3LsqDataResponseMetric::Samples => o3.lsq_operation_latency_samples(operation),
        O3LsqDataResponseMetric::TotalLatency => o3.lsq_operation_latency_ticks(operation),
        O3LsqDataResponseMetric::MaxLatency => o3.lsq_operation_latency_max_ticks(operation),
        O3LsqDataResponseMetric::MinLatency => o3.lsq_operation_latency_min_ticks(operation),
        O3LsqDataResponseMetric::AverageLatency => o3.lsq_operation_latency_avg_ticks(operation),
    }
}
```

Each descriptor supplies `alias()` and `unit()`; the existing registry paths,
`StatResetPolicy::Monotonic`, total placement, and error propagation remain
unchanged.

- [ ] **Step 4: Migrate JSON and text count bucket aliases**

Import the operation and ordering descriptor constants into both
`json_aliases.rs` and `text_o3.rs`.

In `json_aliases.rs`, replace the complete LSQ bucket function with:

```rust
fn append_gem5_o3_lsq_count_bucket_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    alias_prefix: &str,
) {
    let mut append_count_bucket = |source_suffix: &str, bucket_suffix: &str| {
        append_gem5_json_alias_from_paths(
            snapshot,
            records,
            next_id,
            &format!("{alias_prefix}.{source_suffix}"),
            &format!("{alias_prefix}.{bucket_suffix}"),
        );
    };
    for alias in O3_LSQ_OPERATION_GEM5_ALIASES {
        append_count_bucket(
            &format!("lsq0.operation.{}", alias.alias()),
            &format!("lsq0.operation_0::{}", alias.bucket_alias()),
        );
    }
    append_count_bucket("lsq0.operation.total", "lsq0.operation_0::total");
    for alias in O3_LSQ_ORDERING_GEM5_ALIASES {
        append_count_bucket(
            &format!("lsq0.ordering.{}", alias.alias()),
            &format!("lsq0.ordering_0::{}", alias.bucket_alias()),
        );
    }
    append_count_bucket("lsq0.ordering.total", "lsq0.ordering_0::total");
}
```

In `text_o3.rs`, replace its complete LSQ bucket function with:

```rust
fn append_gem5_o3_lsq_count_bucket_alias_stats(
    output: &mut String,
    snapshot: &StatSnapshot,
    alias_prefix: &str,
) {
    let mut append_count_bucket = |source_suffix: &str, bucket_suffix: &str| {
        append_derived_stat_from_snapshot_if_absent(
            output,
            snapshot,
            &format!("{alias_prefix}.{source_suffix}"),
            &format!("{alias_prefix}.{bucket_suffix}"),
            "Count",
        );
    };
    for alias in O3_LSQ_OPERATION_GEM5_ALIASES {
        append_count_bucket(
            &format!("lsq0.operation.{}", alias.alias()),
            &format!("lsq0.operation_0::{}", alias.bucket_alias()),
        );
    }
    append_count_bucket("lsq0.operation.total", "lsq0.operation_0::total");
    for alias in O3_LSQ_ORDERING_GEM5_ALIASES {
        append_count_bucket(
            &format!("lsq0.ordering.{}", alias.alias()),
            &format!("lsq0.ordering_0::{}", alias.bucket_alias()),
        );
    }
    append_count_bucket("lsq0.ordering.total", "lsq0.ordering_0::total");
}
```

These closures keep source metadata, ids, units, prefixes, and duplicate
handling local to each consumer.

- [ ] **Step 5: Migrate host-action count and latency lookup**

Import these helpers and constants in `o3_stats_dump_aliases.rs`:

```rust
use crate::o3_lsq_aliases::{
    o3_lsq_data_response_gem5_alias_from_source,
    o3_lsq_operation_gem5_alias_from_alias,
    o3_lsq_operation_gem5_alias_from_source,
    o3_lsq_ordering_gem5_alias_from_alias,
};
```

Replace the two local bucket match functions with descriptor lookup while
retaining explicit total handling:

```rust
let bucket = if suffix == "total" {
    "total"
} else {
    o3_lsq_operation_gem5_alias_from_alias(suffix)?.bucket_alias()
};
```

Use the equivalent ordering lookup for ordering paths. In
`o3_stats_dump_lsq_data_response_alias_paths`, replace operation and metric
matches with:

```rust
let operation_alias = o3_lsq_operation_gem5_alias_from_source(operation)?.alias();
let metric = o3_lsq_data_response_gem5_alias_from_source(metric_suffix)?.alias();
```

Delete `o3_stats_dump_lsq_operation_bucket_alias`,
`o3_stats_dump_lsq_ordering_bucket_alias`,
`o3_stats_dump_lsq_data_response_metric_alias`, and
`o3_stats_dump_lsq_operation_alias`. Keep the flat/nested parser unchanged.

- [ ] **Step 6: Format and run focused green checks**

Run:

```bash
cargo fmt --all -- --check
cargo test -p rem6 --lib o3_lsq_aliases -- --nocapture
cargo test -p rem6 --lib lsq_data_response_dump_aliases_accept_nested_operation_latency_sources -- --nocapture
cargo test -p rem6 --test source_policy o3_lsq_gem5_aliases_have_one_projection_authority -- --nocapture
cargo test -p rem6 --test source_policy o3_iew_gem5_aliases_have_one_projection_authority -- --nocapture
cargo test -p rem6 --test source_policy cli_host_action_o3_stats_dump_aliases_live_in_focused_module -- --nocapture
```

Expected: every command passes; the new production policy no longer finds local
mapping spellings in any production consumer.

- [ ] **Step 7: Commit the production migration**

```bash
git add crates/rem6/src/lib.rs \
  crates/rem6/src/o3_lsq_aliases.rs \
  crates/rem6/src/stats_output/o3_runtime_gem5_lsq.rs \
  crates/rem6/src/stats_output/json_aliases.rs \
  crates/rem6/src/stats_output/text_o3.rs \
  crates/rem6/src/host_actions/o3_stats_dump_aliases.rs \
  crates/rem6/tests/source_policy/o3_alias_authority.rs
git commit -m "stats: unify O3 LSQ alias authority"
```

### Task 3: Replace Test-Side Name Translation With Explicit Oracles

**Files:**
- Modify: `crates/rem6/tests/source_policy/o3_alias_authority.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/lsq.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/lsq/runtime.rs`

- [ ] **Step 1: Extend policy and observe the mapper-helper failure**

Append this check to `o3_lsq_gem5_aliases_have_one_projection_authority` in the
focused policy module:

```rust
let cli_helpers =
    fs::read_to_string(crate_dir.join("tests/cli_run/m5_host_actions.rs")).unwrap();
for obsolete_helper in [
    "fn o3_lsq_operation_count_alias(",
    "fn o3_lsq_ordering_count_alias(",
    "fn o3_lsq_operation_bucket_alias(",
    "fn o3_lsq_ordering_bucket_alias(",
] {
    assert!(
        !cli_helpers.contains(obsolete_helper),
        "CLI LSQ tests must use explicit expected aliases, not mapper `{obsolete_helper}`"
    );
}
```

Run the focused source-policy command from Task 1. Expected: FAIL naming
`fn o3_lsq_operation_count_alias(`. Do not commit the failing state.

- [ ] **Step 2: Make the assertion helper consume explicit names**

Replace `assert_o3_lsq_count_alias` with:

```rust
fn assert_o3_lsq_count_alias(
    json: &Value,
    family: &str,
    alias: &str,
    bucket_alias: &str,
    value: u64,
) {
    assert_json_stat(
        json,
        &format!("system.cpu.lsq0.{family}.{alias}"),
        "Count",
        value,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("system.cpu.lsq0.{family}_0::{bucket_alias}"),
        "Count",
        value,
        "monotonic",
    );
}
```

Delete the four mapping helpers named by the source-policy check. Keep
`assert_o3_lsq_count_alias_totals` unchanged because it performs no spelling
translation.

- [ ] **Step 3: Add explicit expectations to the store-conditional matrix**

In `o3/lsq.rs`, replace the first `(field, value)` array with:

```rust
for (field, value, alias) in [
    ("lsq_operation_store", 1, Some(("operation", "store", "Store"))),
    (
        "lsq_operation_store_conditional",
        1,
        Some(("operation", "storeConditional", "StoreConditional")),
    ),
    ("lsq_ordering_acquire", 0, Some(("ordering", "acquire", "Acquire"))),
    ("lsq_ordering_release", 0, Some(("ordering", "release", "Release"))),
    (
        "lsq_ordering_acquire_release",
        0,
        Some(("ordering", "acquireRelease", "AcquireRelease")),
    ),
    ("lsq_store_conditional_failures", 1, None),
] {
```

After the canonical stat assertion, replace the old helper call with:

```rust
if let Some((family, alias, bucket_alias)) = alias {
    assert_o3_lsq_count_alias(&json, family, alias, bucket_alias, value);
}
```

- [ ] **Step 4: Add explicit expectations to float/vector and atomic matrices**

In the float/vector matrix in `o3/lsq.rs`, replace the mixed field loop with:

```rust
for (field, value, alias, bucket_alias) in [
    ("lsq_operation_load", 0, "load", "Load"),
    ("lsq_operation_store", 0, "store", "Store"),
    ("lsq_operation_load_reserved", 0, "loadReserved", "LoadReserved"),
    (
        "lsq_operation_store_conditional",
        0,
        "storeConditional",
        "StoreConditional",
    ),
    ("lsq_operation_atomic", 0, "atomic", "Atomic"),
    ("lsq_operation_float_load", 1, "floatLoad", "FloatLoad"),
    ("lsq_operation_float_store", 1, "floatStore", "FloatStore"),
    ("lsq_operation_vector_load", 1, "vectorLoad", "VectorLoad"),
    ("lsq_operation_vector_store", 1, "vectorStore", "VectorStore"),
] {
    assert_eq!(
        o3_runtime
            .pointer(&format!("/{field}"))
            .and_then(Value::as_u64),
        Some(value),
        "structured O3 runtime JSON should expose {field}: {o3_runtime}"
    );
    let operation = field.strip_prefix("lsq_operation_").unwrap();
    assert_eq!(
        json_stat_value(json, &format!("sim.cpu0.o3.lsq_operation.{operation}")),
        value,
        "stat registry should match structured runtime {field}"
    );
    assert_o3_lsq_count_alias(json, "operation", alias, bucket_alias, value);
}
for (field, value, alias, bucket_alias) in [
    ("lsq_ordering_acquire", 0, "acquire", "Acquire"),
    ("lsq_ordering_release", 0, "release", "Release"),
    (
        "lsq_ordering_acquire_release",
        0,
        "acquireRelease",
        "AcquireRelease",
    ),
] {
    assert_eq!(
        o3_runtime
            .pointer(&format!("/{field}"))
            .and_then(Value::as_u64),
        Some(value),
        "structured O3 runtime JSON should expose {field}: {o3_runtime}"
    );
    let ordering = field.strip_prefix("lsq_ordering_").unwrap();
    assert_eq!(
        json_stat_value(json, &format!("sim.cpu0.o3.lsq_ordering.{ordering}")),
        value,
        "stat registry should match structured runtime {field}"
    );
    assert_o3_lsq_count_alias(json, "ordering", alias, bucket_alias, value);
}
```

In `o3/lsq/runtime.rs`, replace the mixed field loop with these exact operation
and ordering tables:

```rust
for (field, value, alias, bucket_alias) in [
    ("lsq_operation_load", 1, "load", "Load"),
    ("lsq_operation_store", 3, "store", "Store"),
    ("lsq_operation_load_reserved", 1, "loadReserved", "LoadReserved"),
    (
        "lsq_operation_store_conditional",
        1,
        "storeConditional",
        "StoreConditional",
    ),
    ("lsq_operation_atomic", 1, "atomic", "Atomic"),
    ("lsq_operation_float_load", 0, "floatLoad", "FloatLoad"),
    ("lsq_operation_float_store", 0, "floatStore", "FloatStore"),
    ("lsq_operation_vector_load", 0, "vectorLoad", "VectorLoad"),
    ("lsq_operation_vector_store", 0, "vectorStore", "VectorStore"),
] {
    assert_eq!(
        o3_runtime
            .pointer(&format!("/{field}"))
            .and_then(Value::as_u64),
        Some(value),
        "structured O3 runtime JSON should expose {field}: {o3_runtime}"
    );
    let operation = field.strip_prefix("lsq_operation_").unwrap();
    assert_eq!(
        json_stat_value(json, &format!("sim.cpu0.o3.lsq_operation.{operation}")),
        value,
        "stat registry should match structured runtime {field}"
    );
    assert_o3_lsq_count_alias(json, "operation", alias, bucket_alias, value);
}
for (field, value, alias, bucket_alias) in [
    ("lsq_ordering_acquire", 1, "acquire", "Acquire"),
    ("lsq_ordering_release", 1, "release", "Release"),
    (
        "lsq_ordering_acquire_release",
        1,
        "acquireRelease",
        "AcquireRelease",
    ),
] {
    assert_eq!(
        o3_runtime
            .pointer(&format!("/{field}"))
            .and_then(Value::as_u64),
        Some(value),
        "structured O3 runtime JSON should expose {field}: {o3_runtime}"
    );
    let ordering = field.strip_prefix("lsq_ordering_").unwrap();
    assert_eq!(
        json_stat_value(json, &format!("sim.cpu0.o3.lsq_ordering.{ordering}")),
        value,
        "stat registry should match structured runtime {field}"
    );
    assert_o3_lsq_count_alias(json, "ordering", alias, bucket_alias, value);
}
assert_eq!(
    o3_runtime
        .pointer("/lsq_store_conditional_failures")
        .and_then(Value::as_u64),
    Some(0)
);
assert_eq!(
    json_stat_value(json, "sim.cpu0.o3.lsq_store_conditional_failures"),
    0
);
```

- [ ] **Step 5: Run policy and black-box matrices**

Run:

```bash
cargo fmt --all -- --check
cargo test -p rem6 --test source_policy o3_lsq_gem5_aliases_have_one_projection_authority -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_m5_dump_reset_stats_scopes_o3_lsq_matrix_snapshot -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_o3_runtime_json_exposes_ordered_atomic_lsq_matrix -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_o3_runtime_json_exposes_float_vector_lsq_matrix -- --nocapture
```

Expected: all pass, proving the independent expected names match production
output after the helper translation is removed.

- [ ] **Step 6: Commit the independent oracle cleanup**

```bash
git add crates/rem6/tests/source_policy/o3_alias_authority.rs \
  crates/rem6/tests/cli_run/m5_host_actions.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/lsq.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/lsq/runtime.rs
git commit -m "test: make O3 LSQ alias oracles explicit"
```

### Task 4: Verify Compatibility, Review, And Push

**Files:**
- Verify all changed files from Tasks 1-3.
- Do not modify the migration ledger unless a failing executable test proves an
  existing documented claim is false.

- [ ] **Step 1: Run focused final JSON and text compatibility tests**

```bash
cargo test -p rem6 --test cli_run rem6_run_json_stats_emit_gem5_o3_lsq_store_conditional_failure_alias -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_text_stats_emit_gem5_o3_lsq_store_conditional_failure_alias -- --nocapture
```

- [ ] **Step 2: Run reset, multicore, restore, and timing suppression tests**

```bash
cargo test -p rem6 --test cli_run rem6_run_m5_dump_stats_exposes_multicore_o3_lsq_forwarding_by_active_hart -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_m5_reset_between_o3_lsq_request_and_response_keeps_latency -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_m5_dump_stats_restores_multicore_o3_lsq_data_response_by_active_hart -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_does_not_record_o3_runtime_stats_after_timing_switch -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_text_stats_omit_o3_runtime_aliases_after_timing_switch -- --nocapture
```

- [ ] **Step 3: Run package and workspace verification**

```bash
cargo fmt --all -- --check
cargo test -p rem6 --lib --quiet
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6 --test cli_run --quiet
cargo test --workspace --all-targets --quiet
git diff --check origin/main..HEAD
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: all tests pass, `git diff --check` is silent, and the ledger reports
exactly 1,200 lines.

- [ ] **Step 4: Perform mandatory whole-diff review**

Dispatch a fresh `gpt-5.5:xhigh` read-only reviewer over `origin/main..HEAD`.
Require findings first and an explicit `APPROVE` or `REJECT` verdict. The review
must check output order, prefixes, units, reset policies, active-hart filtering,
flat/nested latency parsing, `None` suppression, test-oracle independence,
source-policy strength, and the unchanged ledger.

If the reviewer reports a concrete issue, fix it, rerun every affected focused
test plus the package/workspace gates, and obtain a new approval.

- [ ] **Step 5: Push and verify the remote ref**

```bash
git push origin main
git rev-parse HEAD
git rev-parse origin/main
git ls-remote origin refs/heads/main
git status --short --branch
```

Expected: local `HEAD`, `origin/main`, and the remote `main` hash match, and the
worktree is clean.
