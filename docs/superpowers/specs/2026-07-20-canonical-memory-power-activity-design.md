# Canonical Memory Power Activity Design

## Context

The Stats, Probes, Debug, Host Actions, and Checkpointing component is currently
`59% single-axis`, despite broad power export and debug evidence. Its score is
capped because calibrated component activity remains incomplete. The separate
Power and Physical-Design Export Adapters component is also `59% single-axis`;
its explicit next evidence includes calibrated activity models.

The normal `rem6 run` path already builds one canonical
`Rem6MemoryResourceSummary` from executed instruction-cache, data-cache,
transport, fabric, and DRAM summaries. Artifact JSON, stats, and several power
records consume that resource summary. CPU L1 cache power and DRAM power are the
exceptions: `run_power_analysis_records_from_parts` also accepts the original
`CliDataCacheSummary` values and the original `Rem6DramSummary`, then applies
separate activity checks and byte estimates.

That split creates duplicate authority:

- L1 cache power has a raw-cache activity predicate while L2/L3 power has a
  resource-summary predicate;
- run DRAM power ignores the canonical DRAM resource activity calculation;
- low-power-only DRAM activity can be visible in resource stats while the power
  record is suppressed; and
- DRAM dynamic power estimates bytes as `(reads + writes) * 64` instead of using
  the executed `read_bytes` and `write_bytes` counters already preserved in the
  resource summary.

The existing CLI power tests are individual presence checks in the 4,000-line
`load.rs` root. They do not reconcile exported power records with the JSON/stats
activity that selected those records.

## Ledger Boundary

This increment adds representative calibration evidence without claiming
physical-tool parity.

For Stats, the existing power/thermal checklist item becomes complete and the
component moves to `74% representative`: 24 of 26 checklist items, or 92% raw,
capped at 74%. The representative cap is justified by a real route matrix that
correlates canonical cache, transport, fabric, and DRAM resource activity with
McPAT-shaped and DSENT-shaped records, plus inactive and low-power boundaries.

For Power and Physical-Design Export Adapters, a new calibrated run-memory
activity checklist item raises the raw score to 6 of 8, or 75%, capped at
`74% representative`. Full McPAT/DSENT schema parity, external-tool breadth,
broader GPU/trace-replay calibration, and calibrated physical coefficients
remain explicit gaps.

The increment does not claim that the current coefficients match a fabricated
implementation or a vendor power model. "Calibrated" here means that record
selection, residency, bytes, operations, temperature, and dynamic power are
deterministically derived from the same executed typed activity exposed by the
run artifact and stats.

## Approaches

### Patch run DRAM power only

The DRAM record could switch from `Rem6DramSummary` to
`Rem6DramResourceSummary` and use real byte counters. This fixes the clearest
drift but leaves L1 cache power on a separate raw authority and does not create
one auditable run-memory boundary.

### Centralize all power models across run, GPU, and trace replay

Every source could be converted immediately to common cache, fabric, transport,
and DRAM activity records. This would reduce more duplication, but it crosses
three command owners with different summary types and would turn a bounded run
calibration task into an adapter redesign.

### Make the normal run path consume only canonical memory resources

This is the selected design. `run_power_analysis_records_from_parts` receives
cores plus `Rem6MemoryResourceSummary`. CPU L1, shared L2/L3, transport, fabric,
and DRAM power all derive from that one summary. GPU and trace-replay retain
their current adapters and output behavior.

## Runtime Data Flow

`build_run_execution_summary` continues to:

1. build core summaries;
2. build fetch and data transport summaries;
3. snapshot the DRAM summary;
4. construct `Rem6MemoryResourceSummary`; and
5. construct power records before debug and final artifact assembly.

Step 5 changes from passing three raw memory summaries plus the resource summary
to passing only the resource summary. The run power owner then selects:

- `cache_instruction.l1` for `cpu.instruction_cache`;
- `cache_data.l1` for `cpu.data_cache`;
- `cache_l2` for `memory.cache.l2`;
- `cache_l3` for `memory.cache.l3`;
- `transport` for `memory.transport`;
- `fabric` for `memory.fabric`; and
- `dram` for `memory.dram`.

Records remain sorted by target. CPU core records remain independent because
their activity is not part of the memory resource summary.

## Activity Projections

Focused internal activity projections keep policy out of the output assembly:

- cache activity uses canonical run, response, directory, bank, prefetch, and
  backing-DRAM counters;
- transport activity uses requests, arrivals, responses, and latency;
- fabric activity keeps the existing transfer, lane, VN, link, hop, byte, flit,
  occupancy, queue-delay, credit-delay, and contention inputs; and
- DRAM activity uses accesses, commands, refreshes, low-power entries/exits,
  actual read/write bytes, active topology, and latency/residency counters.

The existing coefficients and target names are preserved. Cache target-specific
temperature/static-power constants remain parameters. Run DRAM dynamic bytes
change to the actual byte total. Run DRAM activity and residency also recognize
refresh-only and low-power-only summaries. These are intentional correctness
changes because the resource summary already treats those states as real
activity.

GPU and trace-replay continue to adapt their existing raw summaries into the
same focused projection where practical, but their record selection and
external behavior are not expanded in this increment.

## Representative Matrix

A table-driven top-level CLI test executes one load/store RISC-V program through
representative memory routes and writes both a run artifact and a power artifact.

| Row | Memory route | Power format | Required active targets | Required suppressed targets |
| --- | --- | --- | --- | --- |
| direct | direct | McPAT XML | CPU core, transport | L1 caches, shared caches, fabric, DRAM |
| dram | direct transport plus DRAM | DSENT CSV | CPU core, transport, DRAM | L1 caches, shared caches, fabric |
| cache | MSI L1/L2/L3 | McPAT XML | CPU core, instruction/data L1, shared L2/L3, transport | fabric |
| hierarchy | cache-fabric-DRAM | DSENT CSV | CPU core, instruction/data L1, shared L2/L3, transport, fabric, DRAM | none of the modeled run-memory targets |

Each row imports the emitted power artifact with `rem6-power`, parses the run
artifact, and proves that target presence follows canonical
`memory_resources.*.active` evidence. Active records require positive dynamic
power, positive residency, and a temperature at or above the component base.
Suppressed targets must be absent rather than emitted as zero-activity records.

The DRAM rows additionally reconcile the exported dynamic-power byte term with
the artifact's `read_bytes + write_bytes`; this fails against the legacy
fixed-64-byte estimate whenever executed byte counts differ.

Focused unit tests cover boundaries that are awkward to create through a CPU
run:

- a refresh-only DRAM resource emits a DRAM power record;
- a low-power-only DRAM resource emits a DRAM power record;
- an all-zero memory resource suppresses every memory component; and
- cache records use the canonical resource activity predicate.

## Test Ownership

Existing run power component tests move from
`crates/rem6/tests/cli_run/load.rs` to
`crates/rem6/tests/cli_run/load/power_activity_matrix.rs`. The root keeps shared
helpers and declares the focused child with an explicit `#[path]` attribute.
The extracted tests retain their names unless the new table-driven matrix
supersedes them.

A source-policy test enforces that the normal run power path receives no raw
`CliDataCacheSummary` or `Rem6DramSummary` parameters and that the focused CLI
module owns the representative matrix. It does not forbid those summary types
from GPU or trace-replay adapters.

## Compatibility Boundary

The increment preserves:

- CLI flags, TOML fields, defaults, validation, and artifact paths;
- McPAT XML and DSENT CSV schemas;
- power target names and deterministic target ordering;
- CPU core, GPU, and trace-replay power ownership;
- stats and run-artifact schemas; and
- existing error behavior.

Expected output-value changes are limited to normal-run memory records that were
previously derived from duplicate or estimated activity: canonical L1 selection,
actual DRAM bytes, and refresh/low-power DRAM activity.

## Files

- `crates/rem6/src/power_output.rs`: consume canonical run memory resources,
  consolidate activity projections, and remove duplicate raw-summary checks.
- `crates/rem6/src/run_execution_summary.rs`: pass only cores and canonical
  memory resources to normal-run power assembly.
- `crates/rem6/tests/cli_run/load.rs`: declare the focused power matrix module
  and remove extracted component tests.
- `crates/rem6/tests/cli_run/load/power_activity_matrix.rs`: own representative
  route/format, suppression, and activity-correlation evidence.
- `crates/rem6/tests/source_policy.rs` and a focused child policy module: lock
  canonical ownership and test placement.
- `docs/architecture/gem5-to-rem6-migration.md`: update Stats, Power, and test
  ledger evidence while preserving exact 1,200-line policy.

## Verification

Verification requires observed RED/GREEN focused unit and CLI tests, the full
load CLI module, power import/export tests, source policy, all `rem6` targets,
formatting, exact migration-ledger validation, the full workspace, protected
path and diff review, and independent read-only review before commit and push.
