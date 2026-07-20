# O3 Instruction-Type Alias Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace repeated O3 instruction-type and gem5 op-class mappings with one typed 18-row descriptor authority owned by `rem6-cpu`.

**Architecture:** `O3RuntimeFuLatencyClass` gains a static descriptor containing its canonical instruction-type stem, gem5 alias, debug event suffixes, and zero-extension policy. `rem6-system` and top-level `rem6` consume that metadata while memory aliases and functional-unit latency metric descriptors remain independent.

**Tech Stack:** Rust workspace, typed const descriptors, source-policy tests, `rem6-cpu` integration tests, top-level `rem6 run` JSON/text/debug/m5 CLI tests.

---

### Task 1: Add the RED descriptor and source-policy boundaries

**Files:**
- Create: `crates/rem6-cpu/tests/o3_runtime_inst_type.rs`
- Modify: `crates/rem6/tests/source_policy/o3_alias_authority.rs`

- [ ] **Step 1: Add the complete descriptor contract test**

Create `crates/rem6-cpu/tests/o3_runtime_inst_type.rs`:

```rust
use std::collections::BTreeSet;

use rem6_cpu::{
    O3RuntimeFuLatencyClass, O3RuntimeInstTypeDescriptor,
    O3_RUNTIME_INST_TYPE_DESCRIPTORS,
};

#[test]
fn o3_runtime_inst_type_descriptors_cover_exact_class_order_and_spellings() {
    let expected = [
        (
            O3RuntimeFuLatencyClass::ScalarIntegerMul,
            "int_mul",
            "IntMult",
            "event.iq_issued_inst_type.int_mul",
            "event.commit_committed_inst_type.int_mul",
            false,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarIntegerDiv,
            "int_div",
            "IntDiv",
            "event.iq_issued_inst_type.int_div",
            "event.commit_committed_inst_type.int_div",
            false,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarFloatAdd,
            "float_add",
            "FloatAdd",
            "event.iq_issued_inst_type.float_add",
            "event.commit_committed_inst_type.float_add",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarFloatCompare,
            "float_compare",
            "FloatCmp",
            "event.iq_issued_inst_type.float_compare",
            "event.commit_committed_inst_type.float_compare",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarFloatMisc,
            "float_misc",
            "FloatMisc",
            "event.iq_issued_inst_type.float_misc",
            "event.commit_committed_inst_type.float_misc",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarFloatMul,
            "float_mul",
            "FloatMult",
            "event.iq_issued_inst_type.float_mul",
            "event.commit_committed_inst_type.float_mul",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarFloatFma,
            "float_fma",
            "FloatMultAcc",
            "event.iq_issued_inst_type.float_fma",
            "event.commit_committed_inst_type.float_fma",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarFloatDiv,
            "float_div",
            "FloatDiv",
            "event.iq_issued_inst_type.float_div",
            "event.commit_committed_inst_type.float_div",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarFloatSqrt,
            "float_sqrt",
            "FloatSqrt",
            "event.iq_issued_inst_type.float_sqrt",
            "event.commit_committed_inst_type.float_sqrt",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorIntegerMul,
            "vector_integer_mul",
            "SimdMult",
            "event.iq_issued_inst_type.vector_integer_mul",
            "event.commit_committed_inst_type.vector_integer_mul",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorIntegerDiv,
            "vector_integer_div",
            "SimdDiv",
            "event.iq_issued_inst_type.vector_integer_div",
            "event.commit_committed_inst_type.vector_integer_div",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorFloatAdd,
            "vector_float_add",
            "SimdFloatAdd",
            "event.iq_issued_inst_type.vector_float_add",
            "event.commit_committed_inst_type.vector_float_add",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorFloatCompare,
            "vector_float_compare",
            "SimdFloatCmp",
            "event.iq_issued_inst_type.vector_float_compare",
            "event.commit_committed_inst_type.vector_float_compare",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorFloatMisc,
            "vector_float_misc",
            "SimdFloatMisc",
            "event.iq_issued_inst_type.vector_float_misc",
            "event.commit_committed_inst_type.vector_float_misc",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorFloatMul,
            "vector_float_mul",
            "SimdFloatMult",
            "event.iq_issued_inst_type.vector_float_mul",
            "event.commit_committed_inst_type.vector_float_mul",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorFloatFma,
            "vector_float_fma",
            "SimdFloatMultAcc",
            "event.iq_issued_inst_type.vector_float_fma",
            "event.commit_committed_inst_type.vector_float_fma",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorFloatDiv,
            "vector_float_div",
            "SimdFloatDiv",
            "event.iq_issued_inst_type.vector_float_div",
            "event.commit_committed_inst_type.vector_float_div",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorFloatSqrt,
            "vector_float_sqrt",
            "SimdFloatSqrt",
            "event.iq_issued_inst_type.vector_float_sqrt",
            "event.commit_committed_inst_type.vector_float_sqrt",
            true,
        ),
    ];

    assert_eq!(O3_RUNTIME_INST_TYPE_DESCRIPTORS.len(), expected.len());
    for (index, (descriptor, expected)) in O3_RUNTIME_INST_TYPE_DESCRIPTORS
        .iter()
        .zip(expected)
        .enumerate()
    {
        let (class, source_stem, gem5_alias, iq_suffix, commit_suffix, zero_extended) =
            expected;
        assert_eq!(descriptor.class(), class);
        assert_eq!(descriptor.class().index(), index);
        assert_eq!(O3RuntimeFuLatencyClass::ALL[index], class);
        assert_eq!(descriptor.source_stem(), source_stem);
        assert_eq!(descriptor.gem5_alias(), gem5_alias);
        assert_eq!(descriptor.event_iq_stat_suffix(), iq_suffix);
        assert_eq!(descriptor.event_commit_stat_suffix(), commit_suffix);
        assert_eq!(descriptor.zero_extended_alias(), zero_extended);
        assert_eq!(class.inst_type_descriptor(), descriptor);
    }
}

#[test]
fn o3_runtime_inst_type_descriptor_names_are_unique() {
    fn unique<'a>(values: impl Iterator<Item = &'a str>) -> bool {
        let values = values.collect::<Vec<_>>();
        values.iter().copied().collect::<BTreeSet<_>>().len() == values.len()
    }

    let descriptors: &[O3RuntimeInstTypeDescriptor] = &O3_RUNTIME_INST_TYPE_DESCRIPTORS;
    assert!(unique(descriptors.iter().map(|descriptor| descriptor.source_stem())));
    assert!(unique(descriptors.iter().map(|descriptor| descriptor.gem5_alias())));
    assert!(unique(
        descriptors
            .iter()
            .map(|descriptor| descriptor.event_iq_stat_suffix())
    ));
    assert!(unique(
        descriptors
            .iter()
            .map(|descriptor| descriptor.event_commit_stat_suffix())
    ));
    assert_eq!(
        descriptors
            .iter()
            .filter(|descriptor| !descriptor.zero_extended_alias())
            .map(|descriptor| descriptor.class())
            .collect::<Vec<_>>(),
        [
            O3RuntimeFuLatencyClass::ScalarIntegerMul,
            O3RuntimeFuLatencyClass::ScalarIntegerDiv,
        ]
    );
}
```

- [ ] **Step 2: Add the source-policy authority test**

Append this test to `crates/rem6/tests/source_policy/o3_alias_authority.rs`:

```rust
#[test]
fn o3_inst_type_aliases_have_one_cpu_descriptor_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cpu_dir = crate_dir.join("../rem6-cpu");
    let trace = fs::read_to_string(cpu_dir.join("src/o3_runtime_trace.rs")).unwrap();
    let public_api = fs::read_to_string(cpu_dir.join("src/public_api.rs")).unwrap();

    for anchor in [
        "pub struct O3RuntimeInstTypeDescriptor",
        "pub const O3_RUNTIME_INST_TYPE_DESCRIPTORS",
        "pub const fn inst_type_descriptor(self)",
        "pub const fn source_stem(&self)",
        "pub const fn gem5_alias(&self)",
        "pub const fn event_iq_stat_suffix(&self)",
        "pub const fn event_commit_stat_suffix(&self)",
        "pub const fn zero_extended_alias(&self)",
    ] {
        assert!(trace.contains(anchor), "CPU descriptor authority is missing `{anchor}`");
    }
    for export in [
        "O3RuntimeInstTypeDescriptor",
        "O3_RUNTIME_INST_TYPE_DESCRIPTORS",
    ] {
        assert!(public_api.contains(export), "public API is missing `{export}`");
    }

    let removed_debug_helper = crate_dir.join("src/debug_output/o3_event_inst_type_stats.rs");
    assert!(
        !removed_debug_helper.exists(),
        "static event suffixes must live in the CPU descriptor authority"
    );

    let consumers = [
        (
            "system registration",
            crate_dir.join("../rem6-system/src/riscv_o3_runtime_stats/helpers.rs"),
        ),
        ("core summary JSON", crate_dir.join("src/core_summary_json.rs")),
        (
            "runtime stats",
            crate_dir.join("src/stats_output/o3_runtime.rs"),
        ),
        ("text aliases", crate_dir.join("src/stats_output/text_o3.rs")),
        (
            "JSON aliases",
            crate_dir.join("src/stats_output/json_aliases.rs"),
        ),
        (
            "debug summary JSON",
            crate_dir.join("src/debug_output/o3_summary_json.rs"),
        ),
        (
            "debug event summary JSON",
            crate_dir.join("src/debug_output/o3_event_summary_json.rs"),
        ),
        (
            "debug trace totals",
            crate_dir.join("src/debug_output/o3_trace_totals.rs"),
        ),
    ];
    for (name, path) in consumers {
        let source = fs::read_to_string(path).unwrap();
        assert!(
            source.contains("inst_type_descriptor")
                || source.contains("O3_RUNTIME_INST_TYPE_DESCRIPTORS"),
            "{name} must consume the shared O3 instruction-type descriptor authority"
        );
        for obsolete in [
            "fn o3_iq_fu_latency_class_stem(",
            "fn o3_fu_latency_class_inst_type_stem(",
            "fn o3_fu_latency_class_inst_type_alias(",
            "fn o3_runtime_inst_type_stem(",
            "fn o3_inst_type_stem(",
            "fn event_summary_inst_type_stem(",
            "fn o3_event_iq_issued_inst_type_stat_suffix(",
            "fn o3_event_commit_committed_inst_type_stat_suffix(",
        ] {
            assert!(
                !source.contains(obsolete),
                "{name} retains obsolete local mapper `{obsolete}`"
            );
        }
    }

    let debug_root = fs::read_to_string(crate_dir.join("src/debug_output/o3.rs")).unwrap();
    assert!(!debug_root.contains("o3_event_inst_type_stats"));
}
```

- [ ] **Step 3: Run the exact RED source-policy test**

Run:

```bash
cargo test -p rem6 --test source_policy o3_alias_authority::o3_inst_type_aliases_have_one_cpu_descriptor_authority -- --exact --nocapture
```

Expected: FAIL because `O3RuntimeInstTypeDescriptor` and its table do not yet
exist, consumers still contain local mappers, and the debug suffix file still
exists.

- [ ] **Step 4: Confirm the typed contract is also RED**

Run:

```bash
cargo test -p rem6-cpu --test o3_runtime_inst_type --no-run
```

Expected: compile failure because the descriptor type and table are not yet
exported.

### Task 2: Implement the CPU descriptor authority

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_trace.rs`
- Modify: `crates/rem6-cpu/src/public_api.rs`
- Test: `crates/rem6-cpu/tests/o3_runtime_inst_type.rs`

- [ ] **Step 1: Add the descriptor type and accessors**

Immediately after `O3RuntimeFuLatencyClass`, add:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3RuntimeInstTypeDescriptor {
    class: O3RuntimeFuLatencyClass,
    source_stem: &'static str,
    gem5_alias: &'static str,
    event_iq_stat_suffix: &'static str,
    event_commit_stat_suffix: &'static str,
    zero_extended_alias: bool,
}

impl O3RuntimeInstTypeDescriptor {
    const fn new(
        class: O3RuntimeFuLatencyClass,
        source_stem: &'static str,
        gem5_alias: &'static str,
        event_iq_stat_suffix: &'static str,
        event_commit_stat_suffix: &'static str,
        zero_extended_alias: bool,
    ) -> Self {
        Self {
            class,
            source_stem,
            gem5_alias,
            event_iq_stat_suffix,
            event_commit_stat_suffix,
            zero_extended_alias,
        }
    }

    pub const fn class(&self) -> O3RuntimeFuLatencyClass {
        self.class
    }

    pub const fn source_stem(&self) -> &'static str {
        self.source_stem
    }

    pub const fn gem5_alias(&self) -> &'static str {
        self.gem5_alias
    }

    pub const fn event_iq_stat_suffix(&self) -> &'static str {
        self.event_iq_stat_suffix
    }

    pub const fn event_commit_stat_suffix(&self) -> &'static str {
        self.event_commit_stat_suffix
    }

    pub const fn zero_extended_alias(&self) -> bool {
        self.zero_extended_alias
    }
}
```

- [ ] **Step 2: Add the complete ordered table**

After the descriptor implementation, add the 18 entries exactly as asserted
in Task 1:

```rust
pub const O3_RUNTIME_INST_TYPE_DESCRIPTORS: [O3RuntimeInstTypeDescriptor;
    O3RuntimeFuLatencyClass::COUNT] = [
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::ScalarIntegerMul,
        "int_mul",
        "IntMult",
        "event.iq_issued_inst_type.int_mul",
        "event.commit_committed_inst_type.int_mul",
        false,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::ScalarIntegerDiv,
        "int_div",
        "IntDiv",
        "event.iq_issued_inst_type.int_div",
        "event.commit_committed_inst_type.int_div",
        false,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::ScalarFloatAdd,
        "float_add",
        "FloatAdd",
        "event.iq_issued_inst_type.float_add",
        "event.commit_committed_inst_type.float_add",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::ScalarFloatCompare,
        "float_compare",
        "FloatCmp",
        "event.iq_issued_inst_type.float_compare",
        "event.commit_committed_inst_type.float_compare",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::ScalarFloatMisc,
        "float_misc",
        "FloatMisc",
        "event.iq_issued_inst_type.float_misc",
        "event.commit_committed_inst_type.float_misc",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::ScalarFloatMul,
        "float_mul",
        "FloatMult",
        "event.iq_issued_inst_type.float_mul",
        "event.commit_committed_inst_type.float_mul",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::ScalarFloatFma,
        "float_fma",
        "FloatMultAcc",
        "event.iq_issued_inst_type.float_fma",
        "event.commit_committed_inst_type.float_fma",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::ScalarFloatDiv,
        "float_div",
        "FloatDiv",
        "event.iq_issued_inst_type.float_div",
        "event.commit_committed_inst_type.float_div",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::ScalarFloatSqrt,
        "float_sqrt",
        "FloatSqrt",
        "event.iq_issued_inst_type.float_sqrt",
        "event.commit_committed_inst_type.float_sqrt",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::VectorIntegerMul,
        "vector_integer_mul",
        "SimdMult",
        "event.iq_issued_inst_type.vector_integer_mul",
        "event.commit_committed_inst_type.vector_integer_mul",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::VectorIntegerDiv,
        "vector_integer_div",
        "SimdDiv",
        "event.iq_issued_inst_type.vector_integer_div",
        "event.commit_committed_inst_type.vector_integer_div",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::VectorFloatAdd,
        "vector_float_add",
        "SimdFloatAdd",
        "event.iq_issued_inst_type.vector_float_add",
        "event.commit_committed_inst_type.vector_float_add",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::VectorFloatCompare,
        "vector_float_compare",
        "SimdFloatCmp",
        "event.iq_issued_inst_type.vector_float_compare",
        "event.commit_committed_inst_type.vector_float_compare",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::VectorFloatMisc,
        "vector_float_misc",
        "SimdFloatMisc",
        "event.iq_issued_inst_type.vector_float_misc",
        "event.commit_committed_inst_type.vector_float_misc",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::VectorFloatMul,
        "vector_float_mul",
        "SimdFloatMult",
        "event.iq_issued_inst_type.vector_float_mul",
        "event.commit_committed_inst_type.vector_float_mul",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::VectorFloatFma,
        "vector_float_fma",
        "SimdFloatMultAcc",
        "event.iq_issued_inst_type.vector_float_fma",
        "event.commit_committed_inst_type.vector_float_fma",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::VectorFloatDiv,
        "vector_float_div",
        "SimdFloatDiv",
        "event.iq_issued_inst_type.vector_float_div",
        "event.commit_committed_inst_type.vector_float_div",
        true,
    ),
    O3RuntimeInstTypeDescriptor::new(
        O3RuntimeFuLatencyClass::VectorFloatSqrt,
        "vector_float_sqrt",
        "SimdFloatSqrt",
        "event.iq_issued_inst_type.vector_float_sqrt",
        "event.commit_committed_inst_type.vector_float_sqrt",
        true,
    ),
];
```

- [ ] **Step 3: Add enum lookup and public exports**

Add this to `impl O3RuntimeFuLatencyClass`:

```rust
pub const fn inst_type_descriptor(self) -> &'static O3RuntimeInstTypeDescriptor {
    &O3_RUNTIME_INST_TYPE_DESCRIPTORS[self.index()]
}
```

Extend the `o3_runtime_trace` re-export in `src/public_api.rs`:

```rust
pub use crate::o3_runtime_trace::{
    O3RuntimeFuLatencyClass, O3RuntimeInstTypeDescriptor, O3RuntimeLsqOperation,
    O3RuntimeLsqOrdering, O3RuntimeTraceRecord, O3_RUNTIME_INST_TYPE_DESCRIPTORS,
};
```

- [ ] **Step 4: Run the focused descriptor test GREEN**

Run:

```bash
cargo test -p rem6-cpu --test o3_runtime_inst_type -- --nocapture
```

Expected: both descriptor tests PASS.

### Task 3: Migrate registration, output, and debug consumers

**Files:**
- Modify: `crates/rem6-system/src/riscv_o3_runtime_stats/helpers.rs`
- Modify: `crates/rem6/src/core_summary_json.rs`
- Modify: `crates/rem6/src/stats_output/o3_runtime.rs`
- Modify: `crates/rem6/src/stats_output/text_o3.rs`
- Modify: `crates/rem6/src/stats_output/json_aliases.rs`
- Modify: `crates/rem6/src/debug_output/o3_summary_json.rs`
- Modify: `crates/rem6/src/debug_output/o3_event_summary_json.rs`
- Modify: `crates/rem6/src/debug_output/o3_trace_totals.rs`
- Modify: `crates/rem6/src/debug_output/o3.rs`
- Delete: `crates/rem6/src/debug_output/o3_event_inst_type_stats.rs`

- [ ] **Step 1: Register system counters from descriptors**

Import `O3_RUNTIME_INST_TYPE_DESCRIPTORS` in
`riscv_o3_runtime_stats/helpers.rs`. Replace each of the four class loops with
descriptor iteration. The canonical IQ form is:

```rust
for descriptor in O3_RUNTIME_INST_TYPE_DESCRIPTORS {
    let class = descriptor.class();
    stats[class.index()] = register_o3_counter(
        registry,
        prefix,
        &format!("iq.issued_inst_type.{}", descriptor.source_stem()),
        "Count",
    )?;
}
```

Use `descriptor.gem5_alias()` for `iq.issuedInstType` and
`commit.committedInstType`, and `descriptor.source_stem()` for canonical
commit paths. Delete `o3_iq_fu_latency_class_stem`,
`o3_fu_latency_class_inst_type_stem`, and
`o3_fu_latency_class_inst_type_alias`.

- [ ] **Step 2: Replace canonical-stem helpers in top-level output**

In `core_summary_json.rs`, `stats_output/o3_runtime.rs`,
`debug_output/o3_summary_json.rs`, and
`debug_output/o3_event_summary_json.rs`, replace each local helper call with:

```rust
class.inst_type_descriptor().source_stem()
```

Delete the four local helper definitions. Leave every `class.stat_stem()` use
for functional-unit latency metrics unchanged.

- [ ] **Step 3: Drive text gem5 aliases from the shared table**

Import `O3_RUNTIME_INST_TYPE_DESCRIPTORS` in `stats_output/text_o3.rs`.
Replace the hard-coded 18-row IQ table with:

```rust
for descriptor in O3_RUNTIME_INST_TYPE_DESCRIPTORS {
    let source_path = format!(
        "sim.cpu{cpu}.o3.fu_{}_instructions",
        descriptor.class().stat_stem()
    );
    if let Some(value) = snapshot_value(snapshot, &source_path) {
        append_gem5_o3_iq_inst_type_alias_stats(
            output,
            snapshot,
            &alias_prefix,
            descriptor.gem5_alias(),
            value,
        );
    }
}
```

Replace the hard-coded 18-row commit table with:

```rust
for descriptor in O3_RUNTIME_INST_TYPE_DESCRIPTORS {
    let source_path = format!(
        "sim.cpu{cpu}.o3.commit.committed_inst_type.{}",
        descriptor.source_stem()
    );
    if let Some(value) = snapshot_value(snapshot, &source_path) {
        append_gem5_o3_commit_inst_type_alias_stats(
            output,
            snapshot,
            &alias_prefix,
            descriptor.gem5_alias(),
            value,
        );
    }
}
```

Keep the two memory rows unchanged and LSQ-derived.

- [ ] **Step 4: Preserve JSON alias suppression while iterating descriptors**

Import `O3_RUNTIME_INST_TYPE_DESCRIPTORS` in
`stats_output/json_aliases.rs`. Keep only the four memory source/alias pairs in
the first literal loop. Then emit both IQ and commit aliases per descriptor:

```rust
for descriptor in O3_RUNTIME_INST_TYPE_DESCRIPTORS {
    let include_zero_values =
        !descriptor.zero_extended_alias() || include_zero_extended_aliases;
    for (source_family, alias_family) in [
        ("iq.issued_inst_type", "iq.issuedInstType"),
        (
            "commit.committed_inst_type",
            "commit.committedInstType",
        ),
    ] {
        append_gem5_o3_json_alias_from_sample_with_policy(
            snapshot,
            records,
            next_id,
            cpu,
            &format!("{source_family}.{}", descriptor.source_stem()),
            alias_prefix,
            &format!("{alias_family}.{}", descriptor.gem5_alias()),
            include_zero_values,
        );
    }
}
```

Delete the two hard-coded integer rows and all 32 float/vector rows. Do not
change `gem5_o3_bucket_alias_suffix` or the alias-copy helper.

- [ ] **Step 5: Delete the debug suffix compatibility helper**

In `debug_output/o3_trace_totals.rs`, replace the two helper calls with:

```rust
let descriptor = class_stats.class.inst_type_descriptor();
stats.push(Rem6O3TraceStat {
    suffix: descriptor.event_iq_stat_suffix(),
    unit: "Count",
    value,
});
stats.push(Rem6O3TraceStat {
    suffix: descriptor.event_commit_stat_suffix(),
    unit: "Count",
    value,
});
```

Remove the module declaration and imports from `debug_output/o3.rs`, then
delete `debug_output/o3_event_inst_type_stats.rs`.

- [ ] **Step 6: Format and run the source-policy test GREEN**

Run:

```bash
cargo fmt --all
cargo test -p rem6 --test source_policy o3_alias_authority::o3_inst_type_aliases_have_one_cpu_descriptor_authority -- --exact --nocapture
```

Expected: PASS, with no obsolete local mapper or debug suffix module.

### Task 4: Verify behavior, protected boundaries, and commit

**Files:**
- Modify: `docs/superpowers/plans/2026-07-19-o3-inst-type-alias-authority.md`

- [ ] **Step 1: Run affected crate tests**

Run:

```bash
cargo test -p rem6-cpu --test o3_runtime_inst_type -- --nocapture
cargo test -p rem6-system --lib riscv_o3_runtime_stats -- --nocapture
cargo test -p rem6 --test source_policy o3_alias_authority::o3_inst_type_aliases_have_one_cpu_descriptor_authority -- --exact --nocapture
```

Expected: all PASS.

- [ ] **Step 2: Run representative top-level CLI evidence**

Run each exact row:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::runtime_stats::rem6_run_o3_runtime_json_exposes_iq_iew_commit_matrices -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::fu_latency::rem6_run_o3_runtime_json_exposes_extended_float_fu_latency_classes -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::fu_latency::rem6_run_text_stats_alias_o3_fu_latency_after_detailed_switch -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::fu_latency::rem6_run_text_stats_alias_o3_float_misc_fu_latency_after_detailed_switch -- --exact --nocapture
cargo test -p rem6 --test cli_run debug_flags::o3_fu_latency::rem6_run_o3_debug_flag_emits_fu_latency_event_classes -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::fu_latency::rem6_run_m5_dump_reset_stats_snapshots_nested_o3_fu_latency_classes -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::rem6_run_exposes_multicore_o3_float_misc_op_class_aliases_by_active_hart -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::rem6_run_m5_reset_stats_scopes_multicore_o3_fu_class_dump_aliases_by_active_hart -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::runtime_stats::rem6_run_text_stats_omit_o3_runtime_aliases_after_timing_switch -- --exact --nocapture
```

Expected: all PASS. The final row is the required suppression/negative case.

- [ ] **Step 3: Run broad verification**

Run:

```bash
cargo test -p rem6-cpu --all-targets
cargo test -p rem6-system --all-targets
cargo test -p rem6 --all-targets
cargo test --workspace --all-targets
cargo fmt --all -- --check
git diff --check
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
git diff --exit-code -- docs/architecture/gem5-to-rem6-migration.md temp/
```

Expected: all commands succeed; the ledger remains exactly 1,200 lines and
neither the ledger nor temporary files are modified.

- [ ] **Step 4: Run independent read-only review**

Dispatch a fresh `gpt-5.5:xhigh` reviewer with the design, plan, and complete
diff. Require findings first, explicit checks of the integer stem exception,
memory alias ownership, zero-extension suppression, static suffix lifetime,
source-policy strength, and test sufficiency. Apply valid findings and rerun
the affected verification rows.

- [ ] **Step 5: Close the plan and commit the implementation**

Mark completed checkboxes in this plan, then run:

```bash
git add crates/rem6-cpu/src/o3_runtime_trace.rs \
  crates/rem6-cpu/src/public_api.rs \
  crates/rem6-cpu/tests/o3_runtime_inst_type.rs \
  crates/rem6-system/src/riscv_o3_runtime_stats/helpers.rs \
  crates/rem6/src/core_summary_json.rs \
  crates/rem6/src/stats_output/o3_runtime.rs \
  crates/rem6/src/stats_output/text_o3.rs \
  crates/rem6/src/stats_output/json_aliases.rs \
  crates/rem6/src/debug_output/o3_summary_json.rs \
  crates/rem6/src/debug_output/o3_event_summary_json.rs \
  crates/rem6/src/debug_output/o3_trace_totals.rs \
  crates/rem6/src/debug_output/o3.rs \
  crates/rem6/src/debug_output/o3_event_inst_type_stats.rs \
  crates/rem6/tests/source_policy/o3_alias_authority.rs
git commit -m "refactor: unify o3 inst type alias authority"
git add docs/superpowers/plans/2026-07-19-o3-inst-type-alias-authority.md
git commit -m "docs: close o3 inst type alias plan"
git push origin main
```

Expected: both commits succeed and `origin/main` advances to the closeout
commit.
