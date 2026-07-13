# Execution Mode Lane Authority Design

## Status

Approved for implementation on 2026-07-13.

This document is an implementation design, not a migration-progress ledger. The
only progress authority remains
`docs/architecture/gem5-to-rem6-migration.md`.

## Context

The `rem6` CLI represents the three `rem6_system::ExecutionMode` variants as the
ordered public strings `functional`, `timing`, and `detailed`. Production code
currently repeats that vocabulary and order across configuration parsing, host
action summaries, O3 debug JSON, O3 trace stats, runtime stats, checker stats,
and host-action stats.

Five modules declare the exact same string array. Three debug paths also repeat
lane-index lookup logic, and `debug_output/o3_execution_mode_stats.rs` embeds
the same names in a separate tuple table. The configuration parser and host
action serializer independently repeat the enum-to-name mapping. Those copies
are alternate authorities: a new mode or spelling change can compile while
different CLI surfaces disagree about parsing, serialization, lane order, or
unknown-mode handling.

The duplicated strings are a CLI representation concern. The system crate owns
execution behavior, checkpoint encoding, state transfer, and the
`ExecutionMode` enum itself. This cleanup must not move those responsibilities
or treat checkpoint numeric codes as output-lane indexes.

## Alternatives Considered

### Shared string array and index helper

Expose only `EXECUTION_MODE_LANES: [&str; 3]` and a name-to-index helper. This
removes the five exact array copies, but leaves enum parsing, enum serialization,
and static O3 trace suffixes as independent mappings.

This approach is rejected because it reduces duplication without establishing
one complete CLI representation authority.

### Macro-generated descriptor authority

Create one crate-private macro declaration containing each system enum variant
and CLI name once. Generate the descriptor table, parsing, serialization,
iteration order, lane count, name-to-index lookup, and an exhaustive enum match
from that declaration.

This is the selected approach. It removes all production copies of the
three-lane mapping, makes a new system enum variant fail compilation until the
CLI projection is updated, and keeps output mechanics in their current
consumers.

### Move names into `rem6-system`

Add `as_str`, parsing, and lane indexes to `rem6_system::ExecutionMode`.

This approach is rejected. The strings and O3 trace suffixes are CLI output and
configuration policy, while `rem6-system` owns simulator behavior and
checkpoint compatibility. Moving the representation down would widen the
public API and couple the system crate to current CLI spellings.

### Generic execution-mode output engine

Centralize JSON objects, stat paths, counters, target grouping, and registry
emission together with the vocabulary.

This approach is rejected as unnecessary. The consumers intentionally preserve
different prefixes, units, reset policies, target grouping, unknown-mode
fallbacks, and output record types.

## Goals

1. Establish one production authority for CLI execution-mode names and order.
2. Derive CLI parsing and enum serialization from the same descriptor table.
3. Make additions to `rem6_system::ExecutionMode` fail compilation until the
   CLI authority handles the new variant.
4. Reuse one name-to-index lookup for fixed-size lane counters.
5. Derive every fixed-size execution-mode counter dimension from the authority.
6. Preserve every existing JSON field, stat path, unit, value, reset policy,
   ordering rule, and unknown-mode fallback.
7. Add mechanical source policy preventing local three-lane authorities from
   returning.

## Non-Goals

1. Do not change `rem6_system::ExecutionMode` or expose new system APIs.
2. Do not change checkpoint encoding, manifest decoding, handoff state, or
   execution-mode switching behavior.
3. Do not add, remove, or rename command-line values, JSON fields, or stats.
4. Do not unify the debug, stats, configuration, and host-action output engines.
5. Do not reject unknown runtime summary strings that current stats paths emit
   through their existing dynamic fallback.
6. Do not replace `rem6-workload`'s separate `WorkloadExecutionMode`
   representation or the system crate's workload-to-system conversion.
7. Do not change migration checklist state, score, bucket, or ledger prose.

## Production Authority

Add `crates/rem6/src/execution_mode_lanes.rs` and declare it from `src/lib.rs`.

The module owns a private `define_execution_mode_lanes!` macro invoked once with
the three `ExecutionMode` variant and CLI-name pairs. The macro generates an
`ExecutionModeLane` descriptor table with:

1. The corresponding `rem6_system::ExecutionMode` value.
2. The public CLI name.
3. The static O3 trace stat suffix required by `Rem6O3TraceStat`.
4. The static O3 checkpoint-restore trace suffix required by
   `Rem6O3TraceStat`.

The suffix fields are generated with `concat!` from each row's single name
literal. No row repeats its name inside a full path.

`EXECUTION_MODE_LANES` contains exactly three generated descriptors in the
existing functional, timing, detailed order. The same macro generates an
exhaustive `match` for enum-to-name serialization. If `rem6-system` adds an
`ExecutionMode` variant, this match becomes non-exhaustive and the `rem6` crate
cannot compile until its CLI projection is defined.

`EXECUTION_MODE_LANE_COUNT` is generated from the macro rows, and the descriptor
array uses that count as its dimension. Every mode and target-mode counter type
and initializer in consumers uses this constant rather than a literal `3`.

The module exposes focused descriptor accessors plus:

1. `execution_mode_from_name` for CLI and host-event parsing.
2. `execution_mode_name` for host-action summary serialization.
3. `execution_mode_lane_index` for fixed-size JSON and stat counters.

The descriptor type remains crate-private. It does not construct full stat
paths, JSON objects, checkpoint data, or host actions.

## Consumer Changes

### Configuration parsing

`config.rs` and `config/host_event.rs` import the shared parser under their
existing local `parse_execution_mode` name. The duplicate parser is removed
from `config/riscv_timing.rs`. Existing invalid-value errors and option handling
remain unchanged.

### Host-action summaries

`host_actions.rs` imports the shared enum-to-name function and removes its local
match. Summary construction and all execution-mode state transfer data remain
unchanged.

### O3 debug output

`debug_output/o3_execution_mode_stats.rs` iterates descriptors for trace
suffixes, authority stat paths, JSON fields, and indexes. Its local tuple table
and index helper are removed.

`debug_output/o3_checkpoint_restore_json.rs` and
`debug_output/host_action.rs` use the shared descriptors and index lookup for
fixed-size mode and target-mode counters. The local checkpoint-specific
`(suffix, index)` table is removed; checkpoint trace suffixes come from the
shared descriptor. JSON grouping mechanics remain local.

### Stats output

`stats_output/o3_runtime.rs`, `stats_output/o3_runtime_snapshot_restore.rs`,
`stats_output/host_actions.rs`, and `stats_output/cpu.rs` iterate shared
descriptors and use the shared lookup for known-lane checks. Dynamic stats for
unknown summary strings remain exactly as they are today.

### Run execution summaries

`run_execution_summary.rs` uses `execution_mode_name` for the functional
checker default and the detailed checkpoint-restore filter. It retains the
current typed mode choices and all summary construction behavior; only the
string projection moves to the shared authority.

## TDD And Policy

Add a focused source-policy module at
`crates/rem6/tests/source_policy/execution_mode_lanes.rs`, registered by
`crates/rem6/tests/source_policy.rs`.

The first implementation step is a red source-policy test named
`execution_mode_cli_lanes_have_one_representation_authority`. Before the
authority exists, it must fail because `src/lib.rs` lacks the declaration and
production consumers retain local mappings.

The policy test requires:

1. `src/lib.rs` declares `mod execution_mode_lanes;`.
2. The authority module defines the generated lane count, descriptor table, and
   three focused lookup functions.
3. The authority source contains one macro invocation and an exhaustive
   generated enum match.
4. Configuration, host-action, run-summary, debug, and stats consumers import
   the shared module.
5. The policy recursively inspects every production Rust source under
   `crates/rem6/src`, excluding the authority and test-only source or embedded
   test modules. Exact standalone lane-name literals and either generated
   static trace-suffix family are forbidden outside the authority.
6. Named consumers must not retain local lane constants, index helpers, the
   checkpoint `(suffix, index)` table, a local parser, or a local enum-to-name
   serializer. Execution-mode counter types and initializers must not retain a
   literal dimension of `3`.
7. The scan is deliberately crate-local. `rem6-system` checkpoint sources and
   `rem6-workload` execution-mode sources remain outside this CLI authority.

Authority unit tests prove:

1. The exact public name, O3 trace suffix, and O3 checkpoint-restore trace
   suffix order remains stable.
2. Names and both trace-suffix families are unique.
3. Every currently declared `ExecutionMode` variant round-trips through name
   serialization and parsing; compile-time exhaustiveness protects future enum
   additions.
4. Name-to-index lookup matches descriptor order.
5. Unknown names are rejected by parsing and index lookup.
6. The generated lane count matches the descriptor table length.

## Behavioral Verification

Existing tests remain the independent behavior oracle. Focused verification
must cover:

1. Authority unit tests.
2. The source-policy integration test, including its observed red state before
   implementation.
3. Configuration parsing and host-event parsing tests.
4. Checker default-mode and detailed checkpoint-restore summary tests.
5. Host-action, O3 debug JSON, O3 trace, runtime stat, checkpoint-restore stat,
   and checker stat tests.
6. Focused O3-runtime and checker stat tests prove unknown summary strings still
   produce their existing sanitized dynamic lanes in addition to the three
   zero-valued known lanes.
7. Full `rem6` crate tests and workspace tests required by the repository
   contract.

No migration-ledger edit is warranted because executable capability and
coverage claims do not change. The ledger must remain exactly 1,200 lines.
