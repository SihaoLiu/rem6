# O3 Instruction-Type Alias Authority Design

## Context

The 18 non-memory O3 functional-unit classes are already represented by
`O3RuntimeFuLatencyClass`, but their instruction-type names are independently
re-enumerated across three crates.

- `rem6-system` repeats canonical `issued_inst_type` and
  `committed_inst_type` stems plus gem5 op-class aliases while registering
  counters.
- top-level runtime, core-summary, event-summary, and debug JSON paths each
  repeat the canonical stem mapping.
- text and JSON alias output each repeat the full gem5 op-class table.
- debug trace totals keep another pair of 18-way matches solely to obtain
  static IQ and commit suffixes.

The mappings currently agree, but they encode several non-obvious rules:
scalar integer multiply and divide use `int_mul` and `int_div` for
instruction-type paths while their functional-unit latency paths use
`integer_mul` and `integer_div`; gem5 spells the classes `IntMult`, `IntDiv`,
`FloatMultAcc`, and `SimdFloatMultAcc`; and JSON aliases suppress zero-valued
floating-point and vector classes unless the caller requests zero extension.
Keeping those rules in local match expressions makes new O3 class work prone
to silent output drift.

## Ledger Boundary

This cleanup strengthens both
`CPU Execution Models - 74% representative` and
`Stats, Debug, Trace, and Checkpoint - 59% single-axis`. It consolidates
existing O3 IQ, commit, event-summary, debug, text, JSON, and m5 dump/reset
evidence. It does not add general IQ/wakeup/select behavior, broader O3
execution semantics, or a new stats/debug capability row, so
`docs/architecture/gem5-to-rem6-migration.md` remains unchanged and exactly
1,200 lines.

## Approaches

### Retain local mappings and add equality tests

Cross-crate tests could detect drift after the fact, but every new class would
still require coordinated edits in registration, JSON, text, debug, and core
summary code. The duplicate authority would remain.

### Put an alias table in the top-level `rem6` crate

This would simplify output code but leave `rem6-system` with an independent
mapping because lower-level crates cannot depend on the CLI crate. It also
places instruction-class metadata above the type that owns the class set.

### Attach one descriptor table to `O3RuntimeFuLatencyClass`

This is the selected design. `rem6-cpu` owns the enum and therefore owns one
ordered descriptor for every enum variant. `rem6-system` and top-level `rem6`
consume that exported metadata without introducing a dependency cycle.

## Descriptor Authority

Add `O3RuntimeInstTypeDescriptor` beside `O3RuntimeFuLatencyClass` with these
immutable fields:

- `class: O3RuntimeFuLatencyClass`;
- `source_stem: &'static str` for canonical IQ and commit paths;
- `gem5_alias: &'static str` for gem5 dot and bucket aliases;
- `event_iq_stat_suffix: &'static str` for debug trace snapshots;
- `event_commit_stat_suffix: &'static str` for debug trace snapshots; and
- `zero_extended_alias: bool` for the JSON zero-value extension policy.

`O3_RUNTIME_INST_TYPE_DESCRIPTORS` contains exactly 18 entries in enum index
order. `O3RuntimeFuLatencyClass::inst_type_descriptor()` returns
`&'static O3RuntimeInstTypeDescriptor` by indexing that table. The descriptor
exposes const accessors rather than public fields, matching the crate's typed
API style.

The first two descriptors use canonical stems `int_mul` and `int_div`; all
remaining stems equal the existing `stat_stem()`. Their gem5 aliases preserve
the current spellings:

`IntMult`, `IntDiv`, `FloatAdd`, `FloatCmp`, `FloatMisc`, `FloatMult`,
`FloatMultAcc`, `FloatDiv`, `FloatSqrt`, `SimdMult`, `SimdDiv`,
`SimdFloatAdd`, `SimdFloatCmp`, `SimdFloatMisc`, `SimdFloatMult`,
`SimdFloatMultAcc`, `SimdFloatDiv`, and `SimdFloatSqrt`.

Only scalar integer multiply and divide have `zero_extended_alias == false`.
Floating-point and vector descriptors have it set to true so the JSON writer
continues to omit their zero-valued aliases unless its existing
`include_zero_extended_aliases` input is true.

`O3RuntimeFuLatencyClass::stat_stem()` remains the authority for
`fu_latency_class` metrics. It is deliberately not replaced by the
instruction-type stem because the integer spellings differ.

## Consumer Migration

`rem6-system` iterates `O3_RUNTIME_INST_TYPE_DESCRIPTORS` when registering IQ,
commit, and gem5 alias counters. Counter arrays remain indexed by
`descriptor.class().index()`. Delete the local stem and alias helpers.

Top-level runtime stats, core-summary JSON, debug summary JSON, and event
summary JSON obtain canonical stems from
`class.inst_type_descriptor().source_stem()`.

Text output iterates the shared descriptors for both IQ and commit aliases.
IQ source values continue to derive from
`fu_{class.stat_stem()}_instructions`; commit source values continue to derive
from `commit.committed_inst_type.{source_stem}`. Memory aliases remain separate
because `MemRead` and `MemWrite` derive from LSQ totals rather than an
`O3RuntimeFuLatencyClass`.

JSON output keeps the existing memory aliases separate, then iterates the
descriptors for IQ and commit. Scalar integer aliases are always copied when
their source samples exist. A descriptor marked for zero extension is copied
with the existing `include_zero_extended_aliases` policy, preserving the
current active-hart suppression behavior and bucket aliases.

Debug trace totals use the descriptor's static event suffixes directly. Delete
`debug_output/o3_event_inst_type_stats.rs`, its module declaration, and its
imports. `debug_output/o3_fu_latency_stats.rs` remains unchanged because it
owns a different five-metric debug descriptor per functional-unit class.

## Compatibility Boundary

This refactor changes no execution, counter, checkpoint, or CLI schema.

- canonical IQ and commit stat paths remain byte-for-byte identical;
- gem5 dot and bucket alias paths remain byte-for-byte identical;
- event-summary and debug trace suffixes remain byte-for-byte identical;
- memory aliases remain LSQ-derived;
- scalar integer aliases remain present whenever source samples exist;
- zero-valued floating-point and vector aliases retain their existing
  suppression/extension policy; and
- `O3RuntimeFuLatencyClass::ALL`, `COUNT`, `index`, `as_str`, and `stat_stem`
  remain available.

## Source Policy

The root source-policy suite must enforce the final authority shape:

- `rem6-cpu` defines and publicly exports the descriptor type and exact
  18-entry table;
- descriptor order matches `O3RuntimeFuLatencyClass::index()`;
- source stems, gem5 aliases, and both event suffix families are unique;
- only scalar integer multiply and divide opt out of zero extension;
- production source no longer defines the obsolete local mapping helpers;
- top-level debug no longer contains or declares
  `o3_event_inst_type_stats.rs`; and
- system registration plus top-level runtime, JSON, text, and debug consumers
  reference the shared descriptor authority.

The focused source-policy test is the observed RED boundary before production
implementation. A `rem6-cpu` integration test validates descriptor contents
and invariants without parsing source text.

## Evidence Matrix

Representative executable rows cover:

- top-level O3 runtime JSON IQ, commit, and extended floating-point classes;
- text gem5 aliases for integer, floating-point, and vector classes;
- O3 debug event output and static event-summary suffixes;
- m5 dump/reset snapshots of nested O3 class stats and aliases;
- multicore active-hart output for float-misc aliases;
- reset-scoped multicore alias output; and
- the timing-mode negative row that omits O3 runtime aliases entirely.

Focused descriptor tests cover all 18 rows, order/index agreement, uniqueness,
integer stem exceptions, exact gem5 spellings, event suffixes, and the
zero-extension partition.

## Files

- `crates/rem6-cpu/src/o3_runtime_trace.rs`: define the descriptor authority.
- `crates/rem6-cpu/src/public_api.rs`: export the descriptor and table.
- `crates/rem6-cpu/tests/o3_runtime_inst_type.rs`: validate all descriptor
  rows and invariants.
- `crates/rem6-system/src/riscv_o3_runtime_stats/helpers.rs`: register counters
  from shared descriptors and delete local mappings.
- `crates/rem6/src/core_summary_json.rs`,
  `crates/rem6/src/stats_output/o3_runtime.rs`,
  `crates/rem6/src/stats_output/text_o3.rs`, and
  `crates/rem6/src/stats_output/json_aliases.rs`: derive canonical and gem5
  paths from shared descriptors.
- `crates/rem6/src/debug_output/o3_summary_json.rs`,
  `crates/rem6/src/debug_output/o3_event_summary_json.rs`,
  `crates/rem6/src/debug_output/o3_trace_totals.rs`, and
  `crates/rem6/src/debug_output/o3.rs`: consume descriptor metadata and remove
  the obsolete debug suffix module.
- `crates/rem6/src/debug_output/o3_event_inst_type_stats.rs`: delete.
- `crates/rem6/tests/source_policy.rs`: enforce the shared authority and
  removed legacy helpers.

## Verification

Verification includes an observed RED/GREEN source-policy test, the focused
`rem6-cpu` descriptor test, `rem6-system` runtime-stat tests, representative
top-level JSON/text/debug/m5/active-hart/suppression CLI rows, all targets for
the three affected crates, the full workspace, formatting, protected-path and
1,200-line ledger checks, and an independent read-only review before push.
