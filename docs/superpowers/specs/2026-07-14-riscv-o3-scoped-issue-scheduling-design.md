# RISC-V O3 Scoped Issue Scheduling Design

## Status

Approved for implementation on 2026-07-14 under the active
`temp/improve-rem6-0.md` continuation contract.

This document defines one bounded execution increment. The migration progress
authority remains `docs/architecture/gem5-to-rem6-migration.md`.

## Goal

Make `O3ScopedIssueScheduler` the real cycle-visible issue authority for the
younger scalar and direct-conditional rows in the existing detailed RISC-V O3
live window. Configured issue width, operation-class capacity, and dependency
readiness must change observed issue ticks through the top-level `rem6` binary.

This slice advances the CPU ledger's explicit scoped issue-width/resource-
contention boundary. It does not claim an arbitrary issue queue, an unbounded
ROB, arbitrary mixed memory/control windows, restorable live transport, or a
general O3 engine. The CPU score remains at 74% representative.

## Current Boundary

The repository contains two disconnected scheduling models:

- `O3ScopedIssueScheduler` and `O3DistributedIssueScheduler` implement
  sequence-ordered width, queue-capacity, and dependency selection in
  `crates/rem6-cpu/src/o3_pipeline.rs`.
- The live RISC-V path in
  `crates/rem6-cpu/src/riscv_live_retire_window.rs` walks every available
  younger row serially and records each execution against the same supplied
  base tick. Register dependencies can raise an individual tick through
  forwarding readiness, but no shared issue-width or operation-class resource
  owns the batch.

The scheduler APIs are exported publicly and covered by isolated unit tests,
but they do not govern a real `rem6 run --execute` path. Leaving them detached
would violate the active goal's prohibition on orphan APIs and weak static-only
evidence.

## Considered Approaches

### 1. Four-deep direct-conditional control window

Raise branch lookahead and scalar-memory depth to admit a load plus four direct
conditional controls.

Advantages:

- small implementation delta;
- existing generic rollback and fetch paths likely scale;
- removes duplicated cap literals.

Cost: this is another depth point immediately after the two- and three-control
increments. It does not change issue arbitration or remove the orphan
scheduler boundary.

### 2. Mixed direct, unconditional, and indirect controls

Admit a direct conditional followed by `JAL` and `JALR` under one live window.

Advantages:

- broadens control-kind coverage;
- exercises link-register and indirect-target behavior.

Cost: redirect ownership, speculative link writes, and indirect target
validation are a separate semantic expansion. It still leaves issue width and
resource contention synthetic.

### 3. Bounded scoped issue scheduling

Wire the existing scoped scheduler into the current live scalar/control window
and expose one issue-width configuration.

Advantages:

- changes real cycle behavior instead of only increasing a cap;
- makes an existing public scheduler abstraction operational;
- directly advances the strongest remaining CPU execution-model boundary;
- composes with the current ROB, LSQ, rename, forwarding, FU, rollback,
  handoff, checkpoint, and trace authorities.

Cost: issue timing ownership must remain deterministic across partial fetch
completion and execution-mode transfer.

Approach 3 is selected.

## Configuration

Add `--riscv-o3-issue-width <1..=4>` and the matching TOML field
`riscv_o3_issue_width`.

The default is four. The current bounded window has at most three younger rows,
so the default width avoids introducing width-only serialization into existing
matrices. Resource-class capacity can still expose contention for specialized
functional units.

The valid range is owned by `rem6-cpu` and reused by the CLI parser and direct
`RiscvCore` setter. Zero and five fail before execution. The option requires
`--execute` and `--isa riscv`, matching the existing scalar-memory-depth
configuration boundary.

This increment does not add per-operation-class CLI knobs. One centralized
RISC-V issue profile is sufficient to prove ownership without creating a broad
machine-description surface prematurely.

## Issue Profile

The bounded live scheduler uses one queue and the existing operation classes:

- scalar memory head reservation: `Memory`, capacity one;
- scalar integer multiply/divide family: `IntMult`, capacity one;
- direct conditional control: `Branch`, capacity one;
- other admitted scalar integer descendants: `IntAlu`, capacity equal to the
  configured issue width.

The global issue width limits total issue in a cycle. Per-class capacity then
limits rows that share a specialized resource. This provides three distinct
observable cases:

1. width one places every younger row after the load-head issue cycle;
2. width two admits an independent integer ALU and multiply together after the
   load-head reservation;
3. width two still serializes two independent multiplies.

Memory-head issue remains owned by the existing data path. This slice schedules
the younger rows that currently bypass a shared issue authority; it does not
move scalar memory requests into the generic scheduler. The head's recorded
issue tick nevertheless reserves one global issue slot and the memory-class
capacity for that cycle, so configured width describes the whole bounded
window rather than only its suffix.

## Runtime Ownership

A focused `o3_runtime_issue.rs` module will own live issue classification and
per-cycle arbitration. `riscv_live_retire_window.rs` will delegate the younger
batch instead of recording rows itself.

For each eligible tick, the issue module will:

1. collect load-head and already-issued younger reservations for that tick;
2. build sequence-ordered candidates from currently fetched live ROB rows;
3. classify each candidate into the centralized issue profile;
4. map register producer sequences to scoped dependencies;
5. treat a producer scope as resolved only when its forwarding value is ready
   at the current tick;
6. call `O3ScopedIssueScheduler::try_plan` with the remaining width and class
   capacity for that cycle;
7. execute and record only the plan's issued rows;
8. retain resource- or dependency-blocked rows for the next eligible cycle.

Candidate construction and execution remain deterministic. Already issued rows
in `live_speculative_executions` are the persistent source of issue history, so
the scheduler does not introduce a second ROB or duplicate rename map.

Control-window dependency edges continue to protect rollback ownership, but a
predicted descendant is not treated as data-dependent on branch resolution.
Candidate construction may therefore use the already staged predicted-path
ownership before the controlling branch has an issue record. Issued rows are
still executed and recorded in sequence order within a cycle. The existing
prediction record remains the path authority. Register data dependencies,
including multiply forwarding and completed scalar-load wakeup, remain timing
dependencies.

## Timing Semantics

Issue ticks are scheduler outputs, not arithmetic assigned after every row has
already been accepted.

- A row cannot issue before its fetch and live-ROB identity are available.
- A row cannot issue before every register producer's forwarding-ready tick.
- A cycle cannot issue more rows than the configured width.
- A cycle cannot exceed the selected operation-class capacity.
- Writeback remains issue tick plus the existing FU latency.
- Commit remains oldest-first through the existing live retirement path.

No speculative register or memory effect becomes architectural at issue or
writeback. Normal retirement remains the only publication authority.

## Partial Fetch And Re-entry

The live-window recorder may be called when only a prefix of younger fetches is
complete and again when more fetches arrive. Arbitration must account for rows
already issued at the same or later tick so repeated calls cannot overbook a
cycle.

The implementation will derive occupied width and operation-class slots from
recorded live speculative executions rather than maintain an unrelated mutable
calendar. A newly available row starts at the current callback tick and moves
forward until both global width and class capacity are available.

This keeps transfer recomputation deterministic and avoids checkpointing a
separate scheduler queue.

## Execution-Mode Transfer And Checkpointing

The current schema-v7 live-data handoff remains unchanged. The handoff already
preserves resident ROB ordering and scalar-memory timing while transient
younger speculative executions are recomputed. With identical issue config and
recorded fetch ordering, the scoped scheduler must reproduce baseline issue,
writeback, and commit ticks after a detailed-to-timing switch.

Live transport-backed windows remain non-restorable and checkpoint capture
continues to reject them. A drained checkpoint/restore must expose zero ROB and
LSQ occupancy and no pending scheduler work.

The O3 runtime checkpoint payload advances from version 21 to version 22 so the
new arbitration counters round-trip with existing runtime stats. Versions 1
through 21 remain decodable and initialize the new counters to zero. No pending
issue queue, reservation calendar, or live scheduler state is serialized.

## Observable Evidence

The existing O3 event trace remains the primary timing artifact. New behavior
is proved by issue, writeback, and commit tick relationships plus typed ROB,
LSQ, rename, branch, FU, memory-route, and execution-mode artifacts.

Add a compact issue-arbitration summary derived from the executed plans:

- issue cycles;
- issued rows;
- resource-blocked row-cycles;
- dependency-blocked row-cycles;
- maximum rows issued in one cycle.

The summary resets with existing O3 runtime stats and is absent in timing mode.
It is exposed through the current JSON and text O3 runtime surfaces. The fields
must come from executed scheduler plans, not inferred after the run from final
instruction counts.

The counters participate in O3 runtime checkpoint compatibility exactly like
the existing FU, LSQ, branch, and live-retire-gate stats. Version 22 preserves
them; older payloads decode them as zero.

## TDD Matrix

### Focused scheduler/runtime tests

- width one reserves the load-head cycle, then issues only the oldest ready
  younger row and reports the next row as resource blocked;
- width two issues independent `ADDI` and `MUL` rows in one cycle;
- width two serializes two independent `MUL` rows through `IntMult` capacity;
- a dependent `ADDI` remains dependency blocked until its producer writeback;
- a later partial-fetch callback cannot overbook an already occupied issue
  cycle;
- rollback removes blocked and issued younger authority without publishing
  rename state;
- stats reset clears the issue-arbitration summary;
- checkpoint version 22 round-trips arbitration stats while version 21 decodes
  them as zero.

### Top-level CLI matrix

- direct-memory width-one serialization with exact issue ordering;
- direct-memory width-two cross-resource co-issue;
- cache/fabric/DRAM width-two same-resource multiply contention with hierarchy
  activity;
- dependency-blocked multiply-to-add wakeup and ordered commit;
- detailed-to-timing switch preserving baseline issue/writeback/commit ticks;
- live checkpoint rejection and drained restore cleanup;
- timing-mode suppression of O3 issue trace and arbitration stats;
- CLI and TOML acceptance for widths one and four;
- CLI and TOML rejection for zero and five;
- rejection without `--execute` and with non-RISC-V ISA.

One hierarchy-backed contention representative is enough. Duplicating every
width/resource row across both memory routes would add runtime without testing
a different scheduling rule.

## Negative Boundaries

- Scalar memory heads remain data-path owned but reserve global width and
  memory-class capacity in the live issue calendar.
- `JAL`, `JALR`, traps, interrupts, system events, floating-point descendants,
  vector descendants, and descendant memory operations remain outside this
  bounded live-window scheduler.
- The four-row ROB window and branch lookahead maximum remain unchanged.
- No arbitrary IQ occupancy, wakeup/select network, writeback-port contention,
  or general FU topology is claimed.
- Timing execution mode remains free of the detailed O3 issue surface.

## Source Boundaries

- `o3_runtime_issue.rs` owns classification, scoped-plan construction,
  arbitration, and plan-derived counters.
- `o3_runtime_control_window.rs` continues to own candidate validation,
  forwarding values, and rollback metadata.
- `o3_runtime_checkpoint.rs` owns version-22 arbitration-stat compatibility;
  older payloads default the new fields to zero.
- `riscv_live_retire_window.rs` delegates and remains below its source cap.
- `o3_pipeline.rs` retains the generic scheduler implementation; integration
  should not add RISC-V-specific behavior there.
- CLI parsing follows focused config modules rather than expanding facade logic.
- Top-level tests live in a new focused O3 child module, not in an existing
  giant test file.
- Source-policy anchors protect the scheduler owner and real runtime consumer.

No file under `temp/` is committed.

## Slop And Legacy Cleanup

This slice removes the high-confidence orphan status of
`O3ScopedIssueScheduler` by connecting it to real execution. It also updates
stale migration prose that still describes bounded predicted-control coverage
as only one- and two-branch after the three-deep matrix landed.

The guest-visible `uname` strings that still identify rem6 as gem5 are valid
cleanup work but belong to a separate syscall-focused increment with their own
behavioral compatibility decision. They are not bundled into this CPU timing
change.

## Verification

Required gates:

- every TDD red test must be observed failing for the intended missing
  behavior before production implementation;
- focused `rem6-cpu` scheduler, control-window, live-retire, and stats tests;
- focused config validation and every new CLI row;
- complete O3 predicted-control and scalar-memory/FU CLI modules;
- full `rem6-cpu` suite;
- full `rem6` CLI suite;
- workspace all-targets suite;
- `rem6` and `rem6-cpu` source-policy suites;
- rustfmt, `git diff --check`, exact 1,200-line ledger count, and clean status;
- high-intensity read-only review before push.

## Migration Ledger

The CPU heading stays at 74% representative, the raw score stays 8/10, and both
unchecked checklist items remain unchecked. The migrated evidence will record
the bounded scheduler-owned width, operation-class capacity, dependency,
handoff, checkpoint, timing-suppression, and route matrix.

The `Next evidence` boundary may remove only scoped issue-width/resource
contention. Arbitrary mixed memory/control windows, writeback-port contention,
general IQ/wakeup/select behavior, restorable transport ownership, indirect or
unconditional nested controls, fourth/deeper branch chains, and a general O3
engine remain open.
