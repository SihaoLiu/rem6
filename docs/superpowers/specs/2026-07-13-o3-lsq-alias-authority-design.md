# O3 LSQ Gem5 Alias Authority Design

## Status

Approved for implementation on 2026-07-13.

This document is an implementation design, not a migration-progress ledger. The
only progress authority remains
`docs/architecture/gem5-to-rem6-migration.md`.

## Context

The `rem6` CLI exposes the same O3 load/store queue compatibility vocabulary
through four production paths:

1. Runtime `StatsRegistry` emission creates the dotted gem5-compatible operation,
   ordering, byte, and data-response samples.
2. Final JSON output derives gem5 histogram-style count bucket aliases.
3. Final text output derives the same count bucket aliases.
4. `m5_dump_stats` and `m5_dump_reset_stats` translate reset-scoped canonical O3
   samples into dotted, bucket, and data-response aliases.

Those paths currently repeat the operation, ordering, bucket, and latency metric
spellings. The repeated matches and tuple tables are an alternate authority in
each consumer. Adding or renaming an LSQ lane can therefore leave JSON, text, or
host-action output inconsistent while each local implementation still compiles.

The integration-test helper repeats the same snake-case to lower-camel and
upper-camel conversion. That reduces the independence of the black-box oracle:
the test helper can drift in the same way as production instead of stating the
expected public paths directly.

The repository already solved the equivalent problem for O3 IEW aliases through
`src/o3_iew_aliases.rs`. This design applies that focused pattern to LSQ aliases
without introducing a general output abstraction.

## Alternatives Considered

### Descriptor-only production authority

Create one crate-private module containing typed LSQ alias descriptors and
lookups. Runtime, JSON, text, and host-action consumers retain their local
serialization and ordering logic.

This is the selected approach. It removes the duplicated vocabulary while
minimizing behavior risk.

### Shared path-projection helpers

Centralize construction of complete dotted and bucket paths as well as the
vocabulary. This removes more local code, but it couples consumers that have
different prefix, ordering, duplicate-suppression, id, kind, unit, and reset
semantics.

This approach is rejected because a naming cleanup should not change output
mechanics.

### Generic multi-format alias sink

Introduce one engine that emits aliases into `StatsRegistry`, JSON records, text
lines, and host-action summaries.

This approach is rejected as premature. The four sinks intentionally consume
different source shapes and preserve different metadata. A generic sink would
increase abstraction cost and blast radius beyond the duplicated LSQ names.

## Goals

1. Make one production module authoritative for O3 LSQ operation names,
   ordering names, count bucket names, and data-response metric names.
2. Keep gem5 compatibility policy in the `rem6` CLI crate rather than
   `rem6-cpu`.
3. Preserve every existing public alias path, source unit, reset policy, value,
   prefix rule, append order, and active-hart filter.
4. Preserve support for both flat and nested host-action operation-latency
   source paths.
5. Keep `O3RuntimeLsqOperation::None` and `O3RuntimeLsqOrdering::None` outside
   emitted count and latency lanes.
6. Replace reusable test-side conversion helpers with explicit independent
   expected aliases.
7. Add mechanical policy that prevents production consumers from recreating
   local LSQ mapping tables.

## Non-Goals

1. Do not add, remove, or rename any public stat or JSON field.
2. Do not change O3 runtime counters, checkpoint schemas, reset behavior, or
   CPU execution.
3. Do not unify JSON, text, registry, and host-action serialization.
4. Do not change alias ordering, synthetic JSON ids, text stat kinds, or
   host-action duplicate suppression.
5. Do not expose gem5 compatibility names from `rem6-cpu`.
6. Do not make integration tests consume production descriptors.
7. Do not change migration checklist state, score, bucket, or ledger prose.

## Production Authority

Add `crates/rem6/src/o3_lsq_aliases.rs` and declare it from `src/lib.rs` beside
the existing O3 alias modules.

The module owns three descriptor families.

### Operation aliases

Each operation descriptor contains:

1. Its `O3RuntimeLsqOperation` value.
2. The lower-camel gem5 dotted alias, such as `storeConditional`.
3. The upper-camel gem5 bucket alias, such as `StoreConditional`.

The canonical snake-case source name remains owned by
`O3RuntimeLsqOperation::as_str()`. Descriptor accessors expose it rather than
copying it into another string field.

The descriptor list follows `O3RuntimeLsqOperation::TRACKED` order exactly:
load, store, load-reserved, store-conditional, atomic, float load, float store,
vector load, and vector store. It contains no `None` descriptor.

### Ordering aliases

Each ordering descriptor contains:

1. Its `O3RuntimeLsqOrdering` value.
2. The lower-camel gem5 dotted alias.
3. The upper-camel gem5 bucket alias.

The descriptor list follows `O3RuntimeLsqOrdering::TRACKED` order exactly:
acquire, release, and acquire-release. It contains no `None` descriptor.

### Data-response metrics

Each metric descriptor contains:

1. A small typed metric kind used by the runtime emitter to select the value.
2. The canonical source suffix: `samples`, `ticks`, `max_ticks`, `min_ticks`,
   or `avg_ticks`.
3. The gem5 alias suffix: `samples`, `totalLatency`, `maxLatency`, `minLatency`,
   or `avgLatency`.
4. The public unit: `Count` for samples and `Tick` for latency metrics.

The module provides only the lookups current consumers require: operation by
canonical source name or dotted alias, ordering by dotted alias, and metric by
canonical source suffix. Runtime emitters iterate the typed descriptor arrays
directly, so separate enum-value and ordering-source lookup APIs would be unused.
The module does not construct full paths, inspect snapshots, append records, or
emit stats.

## Consumer Changes

### Runtime registry emission

`stats_output/o3_runtime_gem5_lsq.rs` iterates operation, ordering, and metric
descriptors. It retains all value extraction, total accumulation, path
construction, unit application, reset policy, and registry error handling.

The descriptor order must preserve the current operation and ordering sample
order. Operation totals remain after all operation counts, and ordering totals
remain after all ordering counts. Aggregate data-response aliases remain before
operation-scoped data-response aliases.

### Final JSON aliases

`stats_output/json_aliases.rs` iterates the operation and ordering descriptors to
derive histogram-style bucket aliases from the already-emitted dotted aliases.
It retains synthetic id assignment, source metadata copying, single-core and
multicore prefixes, and append order.

### Final text aliases

`stats_output/text_o3.rs` uses the same descriptors for count bucket paths. It
retains derived-stat formatting, `Count` units, duplicate suppression, prefix
selection, and placement before raw snapshot samples.

### Host-action stats dumps

`host_actions/o3_stats_dump_aliases.rs` replaces local operation, ordering, and
metric matches with descriptor lookup. It retains:

1. Original samples before appended aliases.
2. Family append order.
3. Active-O3-CPU filtering.
4. `system.cpu` versus `system.cpuN` prefix rules.
5. Source `kind`, unit, value, and reset policy.
6. Both flat `operation_latency_*` and nested `operation.latency.*` parsing.
7. Existing duplicate-path suppression.

## Independent Test Oracles

The black-box CLI tests must not import or expose the production descriptor
module. The shared `m5_host_actions.rs` helper will stop converting snake-case
field names into gem5 spellings.

Call-site matrices will state the expected family, lower-camel alias, and bucket
alias explicitly beside each structured runtime field and value. A small helper
may still assert the two concrete paths, but it receives the expected names as
data and performs no name translation.

This intentionally retains expected public strings in tests. They are test
oracles, not production authorities, and they will fail if the production
descriptor changes unexpectedly.

## TDD And Policy

`crates/rem6/tests/source_policy.rs` is already at its 1,500-line facade
boundary. The existing IEW alias-authority policy and the new LSQ policy will
therefore live in a focused
`crates/rem6/tests/source_policy/o3_alias_authority.rs` module registered by the
root integration-test driver. The extraction is mechanical and must pass the
existing IEW policy test before the LSQ red test is added.

The first behavior-changing test step is a red source-policy test named
`o3_lsq_gem5_aliases_have_one_projection_authority`. Before the authority module
exists, it must fail for the missing declaration and local production mappings.

The policy test requires:

1. The source-policy root delegates O3 alias authority checks to the focused
   policy module and remains below its line limit.
2. `src/lib.rs` declares `mod o3_lsq_aliases;`.
3. The focused production module exists and exposes operation, ordering, and metric
   descriptor constants.
4. Runtime, JSON, text, and host-action consumers import the module.
5. Distinctive LSQ mapping tokens do not remain in the production portions of
   those consumers.
6. The old test-side conversion functions are absent.

Descriptor unit tests then prove:

1. Operation and ordering descriptor sequences match the CPU `TRACKED` arrays.
2. No descriptor represents `None`.
3. Canonical names, dotted aliases, and bucket aliases are unique within their
   families.
4. Metric source suffixes and gem5 aliases are unique.
5. The exact public operation, ordering, and metric vocabulary remains stable.

## Behavioral Verification

Existing executable tests remain the behavioral authority. The focused matrix
must cover:

1. Ordered atomic and float/vector runtime JSON operation and ordering aliases.
2. Final JSON and text store-conditional failure aliases.
3. Reset-scoped LSQ matrix dumps and operation byte/count bucket aliases.
4. Multicore active-hart LSQ forwarding and data-response aliases with generic
   alias suppression.
5. Checkpoint-restored data-response dump aliases.
6. Cross-epoch reset behavior for an outstanding LSQ response.
7. Timing-mode suppression of canonical and gem5 O3 aliases.
8. Full `rem6` source policy and CLI regression coverage.

No migration-ledger edit is warranted because the executable behavior and
coverage claims do not change. The ledger must remain exactly 1,200 lines.

## Commit Boundaries

1. Commit this design document.
2. Commit a detailed implementation plan.
3. Commit the mechanical O3 alias source-policy extraction.
4. Observe the LSQ source-policy test fail without committing the failing state.
5. Commit the descriptor authority and production consumer migration as one
   mechanical refactor.
6. Commit independent test-oracle cleanup and any focused regression additions.
7. Run full verification and a high-intensity read-only whole-diff review before
   pushing the implementation increment.
