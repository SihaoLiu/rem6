# RISC-V O3 Live-Window Depth Design

## Goal

Break the current four-row detailed-O3 scalar-window ceiling without widening
memory concurrency beyond its existing proven boundary. Add an independently
configurable total live-window depth, make the scoped issue scheduler the
authority for data and control dependency classification, and prove six- and
eight-row execution through the top-level `rem6 run --execute` path.

The increment must preserve current defaults, existing
`--riscv-o3-scalar-memory-depth` behavior, direct and cache/fabric/DRAM route
semantics, ordered retirement, and timing-mode suppression. It does not claim
unbounded ROB depth, more than four outstanding scalar memory operations,
general vector or floating-point issue, restorable transport ownership, or a
general O3 engine.

The CPU checklist remains 8 of 10, 80% raw, capped at 74% representative.

## Current Boundary

One field, `scalar_memory_window_limit`, currently owns two different limits:

1. the number of live scalar memory operations admitted to the LSQ-backed
   memory prefix; and
2. the total number of ROB rows admitted after a memory or long-latency FU
   head, including younger scalar and control rows.

Both the CLI parser and the CPU runtime independently hard-code a maximum of
four. `RiscvScalarIntegerLiveWindow` also clamps every constructor to the same
four-row constant. This conflation prevents a one-load/six-younger-scalar
window even though it requires only one LSQ row and the existing ROB, rename,
issue, writeback, and commit structures are dynamically sized.

The issue path has a second ownership problem. It computes dependency
readiness in `o3_runtime_issue.rs`, partitions candidates into ready and
blocked vectors, then passes only ready rows to `O3ScopedIssueScheduler` with
an empty resolved-scope set. The scheduler's typed `waits_on` and `produces`
model is therefore bypassed by the live runtime, while parallel manual logic
owns dependency-blocked accounting and next-ready-tick selection.

## Chosen Architecture

Split the existing limit into two typed runtime authorities:

- **Scalar memory depth:** maximum live scalar memory operations. It remains
  bounded to 1 through 4 and continues to control LSQ-backed memory-prefix
  admission.
- **Scalar live-window depth:** maximum total ROB rows in an untranslated
  scalar-memory-prefix window. It is bounded to 1 through 8 and controls
  younger scalar-integer fetch, staging, dependency scheduling, and ROB
  residency.

Add the public CLI/TOML setting `riscv_o3_scalar_live_window_depth` and CLI
flag `--riscv-o3-scalar-live-window-depth`. When it is absent, the scalar
live-window depth equals the selected scalar-memory depth, preserving all
current behavior. Existing branch-lookahead-derived defaults remain
unchanged. The selected scalar live-window depth must be at least the selected
scalar-memory depth.

The CPU crate owns and exports the minimum and maximum constants for both
settings. CLI validation imports those constants rather than retaining a
second private copy.

## Scope Boundaries

The new eight-row bound applies only to scalar-integer live windows opened by
an untranslated cacheable scalar load/store prefix. Existing fixed-FU-head,
translated scalar-load, memory-result scalar-suffix, predicted-control,
linked-control, and producer-forwarded descendant windows remain capped at
four rows. Increasing the scalar live-window depth must not authorize:

- a fifth outstanding scalar load or store;
- broader FP, vector, atomic-result, or MMIO result suffixes;
- unsupported translated or device shapes;
- additional branch speculation beyond the existing branch-lookahead bound;
- a fifth or later control/producer-forwarded descendant row;
- a second issue, writeback, rename, or retirement authority; or
- checkpoint capture of live transport-owned state.

This separation avoids turning a ROB-depth feature into an accidental memory
ordering expansion.

## Configuration Semantics

The selected depths follow these rules:

| Scalar memory depth | Scalar live-window depth | Meaning |
| --- | --- | --- |
| omitted | omitted | Preserve the current branch-lookahead-derived depth. |
| explicit | omitted | Preserve compatibility: both limits use the scalar-memory value. |
| omitted | explicit | Keep the existing memory default and use the explicit scalar-row limit when it is not smaller than that memory default. |
| explicit | explicit | Accept only when scalar live-window depth is at least memory depth. |

Values outside 1 through 4 for scalar memory depth or 1 through 8 for scalar
live-window depth fail during configuration parsing. The new flag follows the
same `--execute`, RISC-V-only, CLI-over-TOML precedence, and diagnostics rules
as the existing O3 issue/writeback/depth controls.

`Rem6RunConfig` retains both raw selections as `Option<usize>`. A focused
config owner resolves them into one `RiscvO3WindowDepths` value containing the
selected scalar-memory and scalar-live depths. Resolution first applies CLI
over TOML, then derives the memory default from branch lookahead, then derives
the scalar-live default from the selected memory depth, and finally validates
both ranges and `scalar_live >= scalar_memory`. This also rejects an explicit
scalar-live depth below the implicit memory default.

Core startup calls one atomic `set_o3_window_depths(memory, scalar_live)`
method after branch-lookahead setup. That method marks both runtime limits
explicit, so a later branch-lookahead change cannot rewrite CLI-resolved
values. The existing single-depth CPU helper keeps compatibility by setting
both limits to the same value. A default `O3RuntimeState` keeps both limits at
two and derives both from branch lookahead only while the depth pair is not
explicit.

No deprecated alias is added. The old scalar-memory setting keeps its existing
name and meaning; internal identifiers are renamed where needed so memory
capacity and total-row capacity cannot be confused.

## Runtime Ownership

`O3RuntimeState` stores `scalar_memory_window_limit` and
`scalar_live_window_limit` as separate fields plus one
`window_depths_explicit` bit. Depths are configured atomically; there is no
runtime state in which only one member of the pair is explicit. Their use is
fixed by owner:

`O3DataAccessWindowPolicy` distinguishes `ScalarMemoryPrefix` from
`UntranslatedScalarMemoryPrefix`. Data issue selects the untranslated variant
only when no RISC-V data-translation owner is configured. Translated scalar
loads retain `ScalarMemoryPrefix` and its four-row behavior. Shared predicate
helpers treat both variants as scalar-memory policies for memory ordering,
while only the untranslated variant may consume scalar live depth above four.

| Owner/path | Scalar memory depth | Scalar live depth |
| --- | --- | --- |
| `has_scalar_memory_window_capacity` and `stage_live_data_access_issue` | Limits live scalar memory operations to four. | Also prevents total rows from exceeding the scalar live bound. |
| `can_consider_scalar_memory_younger` and scalar-memory-prefix admission | Limits an additional load/store. | Limits total ROB rows. |
| `scalar_memory_window_candidate` | Limits the memory-prefix row count. | Limits memory plus scalar rows after the prefix. |
| `data_access_integer_window` for `UntranslatedScalarMemoryPrefix` | Supplies existing occupied memory rows. | Supplies the total scalar-row limit up to eight. |
| `data_access_integer_window` for translated `ScalarMemoryPrefix` | Supplies existing occupied memory rows. | Retains the fixed four-row limit. |
| `stage_o3_data_access_younger_window` | Does not widen memory capacity. | Stages untranslated scalar successors up to the selected total depth. |
| fixed-FU, translated-load, memory-result, control, and producer-forwarded paths | Existing behavior. | Uses a fixed four-row bound, not the new setting. |

`scalar_memory_window_candidate` peeks at the next completed/candidate fetch
before applying the memory cap. If the next row is another load/store, it is
admitted only when both the memory-row count is below scalar memory depth and
the total-row count is below scalar live depth. If the memory prefix is full
but the next row is a supported scalar integer instruction, the path may
transition into the scalar suffix and continue until scalar live depth. Thus
four memory rows followed by four scalar rows are legal at depths 4/8, while a
fifth memory row is rejected without consuming its fetch identity.

The window classifier gains distinct constructor bounds. Untranslated
scalar-memory-prefix windows accept the configured total depth up to eight.
Fixed-FU-head, translated scalar-load, and memory-result constructors retain
four-row bounds. Control classification inside a scalar prefix also retains a
four-total-row ceiling, so a scalar row beyond four cannot open or extend a
control/producer-forwarded chain. The common constructor no longer silently
reclamps every caller to one global constant; each typed entry point supplies
its own validated maximum.

All rows continue to use the existing live ROB, rename overlay, speculative
execution records, shared writeback reservation calendar, and oldest-first
commit path. No new queue or response callback is introduced.

## Scoped Dependency Scheduling

Split the current combined candidate into two representations:

- `O3LiveIssueSchedulingCandidate` contains sequence, request index,
  operation class, destination shape, data-producer sequences, control
  dependency, and fetch identity. It is constructible from live ROB/rename
  metadata even when producer values are not yet forwardable.
- `O3LiveSpeculativeIssueCandidate` contains the executable instruction plus
  forwarded register values. It is constructed only after the scheduler
  selects a row whose dependency scopes are resolved.

Each scheduling candidate is converted into one `O3ScopedReadyInstruction`
carrying:

- `produces`: its own typed data and control dependency scopes;
- `waits_on`: typed data scopes for register producers and a typed control
  scope for serializing-control lineage; and
- the existing issue queue and operation class.

Data and control scopes for one sequence are distinct because data consumers
may issue at admitted writeback while serializing-control descendants retain
the existing writeback-plus-one release rule. A per-plan
`O3LiveIssueDependencyTable` assigns deterministic ephemeral
`O3DependencyScopeId` values to `(Data, sequence)` and `(Control, sequence)`
keys; no bit packing or persistent checkpoint identifier is added.

At each modeled cycle, the dependency table derives resolved scopes and the
earliest future resolution tick from admitted writeback timing and
control-lineage state. Data scopes resolve at the recorded dependency ready
tick. Control scopes resolve at admitted writeback plus one. Unknown control
producers remain unresolved. The runtime passes all scheduling candidates,
including rows without currently forwardable values, to
`O3ScopedIssueScheduler`. The returned issued, resource-blocked, and
dependency-blocked sets become the sole classification source for issue
statistics and row selection.

`O3ScopedIssueScheduler` gains reservation-aware planning: callers provide the
number of globally reserved issue slots in addition to operation-class
capacity already consumed by the head and previously issued rows. Existing
unreserved planning delegates with zero reserved slots. The plan retains the
configured total `issue_width`, records `reserved_width`, and derives
`available_width = issue_width - reserved_width`. When reservations consume
the full issue width, the scheduler issues no row while still separating
dependency-blocked candidates from resource-blocked candidates. A resolved
row left unissued by global width or operation-class capacity is resource
blocked; an unresolved row is dependency blocked regardless of remaining
capacity.

The runtime still owns execution and time advancement. Before mutating runtime
state, it builds executable candidates and forwarded values for the complete
selected batch and validates every exact fetch identity. If any selected row
cannot become executable despite resolved scheduling metadata, the whole
batch fails closed for that cycle without recording a partial issue. It then
executes the validated batch in sequence order. The runtime advances one tick
for resource pressure, jumps to the earliest known dependency resolution when
every remaining row is dependency blocked, and stops when a required scope is
unresolved. Issuing a producer does not resolve its scope in the same cycle;
consumers wait for admitted writeback exactly as they do today.

Issue-cycle stats use scheduler output directly. `issued_rows` counts newly
executed rows, blocked counts come from the corresponding plan lists, and
`max_rows_per_cycle` observes `reserved_width + issued_rows`. Existing plan
callers with zero reservations retain their current values.

The existing full-width head-reservation case uses this reservation-aware plan
so the manual ready predicate is removed entirely.

## Representative Matrix

The primary program contains this exact dependency shape, with distinct
nonzero registers selected by the test fixture:

| Row | Shape | Dependency purpose |
| --- | --- | --- |
| 1 | delayed `LD` | One untranslated scalar-memory head and one LSQ row. |
| 2 | independent `MUL` | Long-latency producer A. |
| 3 | independent `MUL` | Competes with row 2 for the multiply resource. |
| 4 | `ADDI` from row 2 | Transitive wakeup from producer A. |
| 5 | `ADD` from rows 2 and 3 | Two-source fan-in. |
| 6 | independent `ADDI` | Cross-resource issue witness. |
| 7 | `ADD` from rows 4 and 5 | Deeper fan-in chain. |
| 8 | `ADD` from row 7 and the load result | Terminal load-dependent wakeup. |

It contains no control, translated, MMIO, FP, vector, atomic, or additional
memory successor, keeping the new depth authority scalar-only. Depth six uses
the first six rows of the same shape; depth four uses the first four and proves
that row five is not staged before the head response.

Top-level evidence covers these axes:

| Axis | Rows |
| --- | --- |
| Route | direct; cache/fabric/DRAM |
| Scalar live-window depth | 4 suppression; 6 representative; 8 full-depth |
| Issue width | 1, 2, and 4 |
| Blocking | dependency-blocked, ALU/MUL resource-blocked, and width-blocked |
| Wakeup | independent, transitive, and fan-in |
| Lifecycle | successful drain, rollback/fault cleanup, mode switch, checkpoint boundary |
| Mode | detailed positive; timing suppression |

The six-row case proves the feature is not an exact eight-row special case.
The depth-four row proves the fifth row is still suppressed under the existing
configuration. The depth-eight row proves exact eight-ROB/one-LSQ residency
before the delayed load response, followed by ordered issue, admitted
writeback, and commit.

## Negative And Lifecycle Boundaries

The increment must prove:

- scalar-memory depth 5 and scalar-live depth 0 or 9 are rejected;
- selected scalar-live depth below selected scalar-memory depth is rejected,
  including an explicit live value below the implicit memory default;
- the new setting is rejected without `--execute` and for non-RISC-V runs;
- depth four suppresses fifth-and-later rows without stale fetch or rename
  identity;
- a fault, retry, or older redirect discards the complete younger suffix and
  all future writeback reservations;
- live checkpoint capture remains rejected;
- detailed-to-timing transfer preserves already-owned timing until drain but
  does not create new timing-mode O3 rows; and
- timing mode preserves architectural results while omitting O3 runtime,
  trace, and gem5-style O3 surfaces.

The direct route must remain transport-only. The hierarchy route must show
cache, transport, fabric, and DRAM activity. Neither route may exceed four LSQ
rows, and the representative deep rows must use exactly one LSQ row.

## Executable Evidence

Focused CPU tests must prove:

- independent memory and total-row limits, including compatibility defaults;
- scalar-prefix construction at depths six and eight while fixed-FU and
  memory-result constructors remain capped at four;
- candidate `waits_on` and `produces` scopes for data, control, transitive, and
  fan-in dependencies;
- scheduler-owned issued/resource-blocked/dependency-blocked classification;
- earliest dependency tick advancement and unresolved-scope termination;
- exact cleanup of deep-window ROB, rename, execution, lineage, and writeback
  state; and
- existing four-row tests remain unchanged.

Top-level CLI tests must invoke `env!("CARGO_BIN_EXE_rem6")` and prove:

- CLI and TOML selection plus CLI precedence for scalar live-window depth;
- exact pre-response ROB/LSQ residency for depth six and eight;
- depth-four fifth-row suppression;
- issue-width 1/2/4 timing and blocked-row counters;
- independent, transitive, and fan-in issue/writeback ordering;
- final register and memory witnesses;
- direct versus cache/fabric/DRAM activity;
- JSON, text, and `m5_dump_stats` issue counters;
- checkpoint and mode-switch boundaries, with focused CPU tests owning
  retry/failure rollback cleanup; and
- timing-mode architectural equivalence without O3 surfaces.

Config parsing, range validation, pair resolution, and flag dispatch belong in
the focused `config/riscv_timing.rs` owner. `config.rs` receives only the new
raw field in each existing config struct, focused delegation, and final struct
initialization, and must remain below its existing strict 1,700-line cap.
Source-policy evidence must also keep `o3_runtime_issue.rs` and large test
dispatchers below their existing caps. New dependency helpers and deep-window
tests belong in focused child modules rather than extending already large
roots.

## Documentation Boundary

After executable evidence passes, update the CPU Migrated, Not migrated, and
Next evidence text to record configurable untranslated six/eight-row
scalar-memory-prefix live windows and scheduler-owned dependency scopes.
Remove only the exact four-row ceiling for this scalar matrix.

Keep arbitrary broader mixed windows, fifth-and-deeper FP/vector/result/device
shapes, more than four outstanding memory operations, broader control depth,
restorable transport ownership, and a general O3 engine open. Preserve the
migration ledger at exactly 1,200 lines and do not change the component score.
