# Branch Predictor Legacy Checkpoint Fixtures Design

## Goal

Remove test-time writers for retired branch-predictor checkpoint schemas while
preserving explicit decode compatibility for versions 1 through 5 and the sole
version-6 production writer.

Compatibility evidence must come from frozen literal payloads rather than
truncating and rewriting bytes emitted by the current encoder. Each decoded
legacy payload must migrate through `BranchPredictorCheckpointPayload::encode`
to version 6 and decode again without losing the state supported by its source
schema.

## Ledger Boundary

The cleanup supports the CPU execution-model and checkpoint/debug evidence.
CPU execution remains `74% representative`; this structural increment does not
change its score or claim a new timing matrix.

The current writer already emits version 6 only, and the decoder intentionally
accepts versions 1 through 6. The open CPU boundaries in the migration ledger,
including general O3 execution, restorable transport ownership, broader result
windows, and wider stall/squash matrices, remain unchanged.

## Current Problem

`crates/rem6-cpu/tests/branch_predictor.rs` currently manufactures valid
version-1 through version-4 payloads from current version-6 bytes. It knows
historical header sizes, BTB layout sizes, active-speculation record widths,
and field-removal offsets. The tests then modify the version byte and splice or
truncate the current writer output.

This creates a second retired-schema writer in tests. A current-writer layout
change can silently rewrite the supposed historical fixture instead of proving
the decoder still accepts bytes that were fixed when the old schema existed.

Version 5 is accepted by production decode but has no focused frozen valid-row
test. It shares the v5 active branch-kind shape with v6 but predates v6 BTB
branch-kind counters.

## Considered Approaches

### Keep the synthetic downgrade helpers

This preserves compact tests but leaves the current writer as the authority for
historical bytes. It cannot detect accidental drift in retired schemas.

### Move the downgrade helpers into a focused child module

This improves file ownership but retains a test-only writer for versions that
production no longer emits. It moves the duplicate authority rather than
removing it.

### Freeze literal version-1 through version-5 payloads

Store the exact valid payload bytes in a focused test fixture module. Final
compatibility tests decode those constants, compare typed state, encode the
decoded value through the current writer, require version 6, and decode again.
This is the chosen approach.

## Fixture Ownership

Create:

`crates/rem6-cpu/tests/branch_predictor/legacy_checkpoint_fixtures.rs`

The module owns exactly five byte slices:

- `LEGACY_V1_DEFAULT_PAYLOAD`;
- `LEGACY_V2_ACTIVE_MAPPING_PAYLOAD`;
- `LEGACY_V3_TARGET_PREDICTION_PAYLOAD`;
- `LEGACY_V4_RAS_PAYLOAD`; and
- `LEGACY_V5_BRANCH_KIND_PAYLOAD`.

`branch_predictor.rs` imports these constants through a normal path-owned child
module. No fixture encoder, byte-splicing helper, version-offset mutation, or
historical record-size constant remains in the final test source.

During implementation only, the existing synthetic helpers may be used to
print and cross-check the literal bytes. That transient generator must be
deleted before the final source-policy and hygiene gates.

## Compatibility Matrix

### Version 1

Freeze a default eight-entry predictor snapshot with no active speculation.
Decode must preserve predictor state and synthesize the historical default BTB
and empty RAS/active metadata.

### Version 2

Freeze two active sequence-to-speculation mappings with an explicit compact
eight-entry/two-way BTB. Decode must preserve the mapping and BTB shape while
leaving target predictions, RAS operations, and branch kinds empty.

### Version 3

Freeze the same two active mappings with one taken target and one no-target
prediction. Decode must preserve target predictions and leave RAS operations
and branch kinds empty.

### Version 4

Freeze the active mappings, target predictions, and two live RAS operations.
Decode must preserve RAS snapshot and active operation ownership while leaving
branch kinds empty.

### Version 5

Freeze active mappings, target predictions, RAS operations, and branch kinds
using a nontrivial eight-entry/two-way BTB. Decode must preserve all v5-owned
state and default the v6-only BTB per-kind counters to zero.

## Migration To Current

Every fixture test performs the same migration boundary:

1. decode the literal legacy bytes;
2. assert the expected typed state for that schema;
3. encode the decoded payload;
4. assert byte four is version 6;
5. assert the current bytes differ from the legacy fixture; and
6. decode the version-6 bytes and require equality with the migrated payload.

The existing current version-6 round-trip tests remain the writer authority.
No production encode or decode branch changes are required.

## Source Policy

Add a focused `rem6-cpu` source-policy test that requires the fixture child and
all five constants. The final branch-predictor test family must reject:

- `current_payload_prefix_without_btb_kind_counters`;
- historical active-record byte constants;
- valid legacy tests rewriting `VERSION_OFFSET`; and
- valid legacy tests truncating current `payload.encode()` output.

Malformed-payload tests may continue mutating bytes because they exercise
decoder rejection rather than create valid compatibility evidence.

## Executable Evidence

TDD begins with the source-policy test and must fail on the existing synthetic
downgrade helper.

Focused compatibility tests then prove all five retired versions decode and
migrate to version 6. Existing malformed checkpoint tests remain green. The
current version-6 branch predictor/RAS round trips remain green.

A real top-level `rem6 run --execute` selected-branch-predictor CLI row remains
part of regression verification so the cleanup is not justified only by codec
unit tests.

## Negative And Suppression Evidence

Existing invalid magic, unsupported version, malformed size, invalid mapping,
BTB, RAS, branch-kind, and duplicate-sequence tests remain unchanged. The
source-policy test ensures valid historical compatibility cannot regress to a
generated retired writer.

## Documentation Boundary

The migration ledger is not changed because no new simulator capability is
added. It must remain exactly 1,200 lines. `temp/**` remains untouched and
untracked.
