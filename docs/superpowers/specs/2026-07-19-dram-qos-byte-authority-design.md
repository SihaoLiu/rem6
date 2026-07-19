# DRAM QoS Byte Authority Design

## Goal

Remove the byte-count copy stored in `DramQosAccess` so `DramAccess::byte_count()`
is the sole authority for bytes consumed by one completed DRAM access.

The cleanup must preserve DRAM QoS arbitration, escalation metadata, aggregate
activity counters, and the real trace-replay CLI stats surface. Because
`DramQosAccess::bytes()` cannot derive a parent `DramAccess` value, the public
accessor is removed together with the duplicate field rather than retained as
a misleading compatibility alias.

## Ledger Boundary

The target is the `Memory, Cache, Coherence, Fabric, and DRAM` component, which
is currently `73% representative`. This increment is structural cleanup and
does not add a new executable matrix or change the score.

The relevant existing evidence is DRAM QoS priority/escalation accounting and
the top-level `rem6 trace-replay` profiled-DRAM QoS stats path. The broader open
ledger items remain Ruby-scale protocol networks, gem5-class NoC detail, named
full JEDEC timing tables, and remaining refresh and low-power matrices.

## Current Problem

`DramAccess` stores the completed access byte count in `byte_count`.
`DramQosAccess`, nested inside the same access, stores a second `bytes` value
copied from the originating request. Activity aggregation reads the nested
copy for total, priority, and requestor QoS byte counters.

The two values are populated from the same request today, but the type system
does not require them to remain equal. Future construction or restore changes
could therefore produce internally inconsistent access and QoS summaries.

## Considered Approaches

### Keep both fields and assert equality

This would detect some mismatches but retain two authorities and require every
constructor or restore path to maintain the invariant. It does not satisfy the
cleanup goal.

### Remove the field but retain `DramQosAccess::bytes()`

`DramQosAccess` is a copied nested value and has no reference to its parent
`DramAccess`. Retaining the accessor would require a back-reference, a new
borrowed view type, or another stored byte value. Each option adds complexity
only to preserve a redundant API.

### Make `DramAccess` the sole byte authority

Remove `DramQosAccess::bytes` and its accessor. Keep QoS metadata limited to
requestor, assigned priority, and effective priority. Aggregate QoS byte
counters from the enclosing `DramAccess::byte_count()` value. This is the
chosen approach.

## Design

`DramQosAccess` retains only:

- requestor identity;
- assigned priority;
- effective priority; and
- the derived escalation predicate.

`DramQosAccess::from_request` stops copying request size. `DramAccess` remains
the owner of `byte_count`, populated once from the request during access
construction.

`DramActivitySummary::record` uses `access.byte_count()` for all QoS byte
families:

- aggregate QoS bytes;
- effective-priority QoS bytes; and
- requestor QoS bytes.

Access-count and escalation-count behavior remains unchanged.

## Compatibility Boundary

The public `DramQosAccess::bytes()` method is removed. The workspace has one
direct test caller and three production aggregation calls, all of which move
to `DramAccess::byte_count()`.

No serialized checkpoint, JSON artifact, CLI flag, or stats key changes. The
observable `sim.trace_replay.dram.qos.bytes`, priority byte, and requestor byte
values remain identical.

## Source Policy

Add a focused `rem6-dram` source-policy test that parses `DramQosAccess` and
requires its fields to be exactly:

- `requestor`;
- `assigned_priority`; and
- `effective_priority`.

The policy also rejects a public `bytes` method on `DramQosAccess`. This makes
regrowth of a renamed byte-count cache or compatibility accessor fail closed.

## Executable Evidence

TDD begins with the source-policy test, which must fail against the existing
`bytes` field and accessor.

Focused behavior then proves:

- a priority-escalated DRAM request still records the correct access byte
  count through `DramAccess`;
- heterogeneous 4-, 8-, and 16-byte QoS accesses produce exact aggregate,
  priority, and requestor byte summaries; and
- the real trace-replay CLI emits unchanged DRAM QoS access, byte, priority,
  and requestor stats.

The representative path is:

```text
trace request -> profiled DRAM QoS arbitration -> DramAccess ->
DramActivitySummary -> WorkloadParallelExecutionSummary -> StatsRegistry
```

## Negative And Suppression Evidence

Existing non-QoS DRAM paths continue to produce no QoS activity. Source policy
proves there is no second byte authority. Existing QoS validation and priority
range failures remain unchanged because the cleanup does not alter arbitration
or request construction.

## Expected Verification

Run the exact source-policy test through RED and GREEN, the focused DRAM QoS
timing and activity tests, the top-level trace-replay DRAM QoS CLI test,
`rem6-dram` all-targets, and the full workspace. The migration ledger remains
exactly 1,200 lines and is not edited.
