# gem5 to rem6 Migration

This document is the current migration ledger from gem5 concepts and tests to
rem6. The gem5 tree under `temp/reference_designs/gem5` is read-only audit
input; do not copy test binaries, generated outputs, or build products into
rem6.

## Document Boundary

`docs/architecture/rem6-architecture.md` owns the stable architecture story:
runtime shape, ownership model, invariants, and design motivation.

This migration ledger owns changing state: component scores and calculations,
checklists, migrated and missing behavior, evidence notes, gem5 test-anchor
crosswalks, and external-adapter rows. Do not duplicate the architecture
document's invariant list here or put percentages/proof logs there.

## Scoring Rubric

A percentage is a behavior-boundary score, not a count of related files.
Checklist items use markdown checkboxes:

- `[x]` means current rem6 has executable evidence for that item.
- `[ ]` means the item is not migrated or lacks executable rem6 evidence.

Checklist source: the component-local markdown checkbox list is the auditable
source for the component percentage. The gem5 test-anchor table uses row scores
only as compact crosswalk markers; those row scores do not override component
checklist calculations.

For each component, the score calculation must count completed checklist items,
round that ratio to the nearest whole percent, apply the evidence-breadth
bucket as an upper bound, and state both the checklist fraction and any bucket
cap. The component checklists are the auditable source for migration progress.

| Score | Bucket | Meaning |
| --- | --- | --- |
| 0% | open | No rem6 owner, or the row is scoped to the wrong gem5 target. |
| 1-19% | scoped | Owner or data types exist, but executable behavior evidence is absent or tiny. |
| 20-39% | unit-slice | Narrow unit behavior exists, with major integration axes absent. |
| 40-59% | single-axis | At least one executable ISA, device, config, or workload path works. |
| 60-74% | representative | Representative integration exists with typed artifacts, stats, or checkpoints. |
| 75-89% | matrix-gapped | Major gem5 matrix axes are covered, with explicit remaining gaps. |
| 90-99% | near-covered | All subtargets are covered except accepted non-goals or small gaps. |
| 100% | covered | Equivalent or stronger rem6 evidence exists for the named boundary. |

Detection-gated static newlib or qemu smoke tests count only for the narrow
path they execute. Unknown-syscall records count as diagnostics and
observability, not as implemented syscall coverage.

## Score Audit Format

Each component section below is the single source for that component's score.
Audit it by counting `[x]` items and total checklist items, confirming the
section-local `Score calculation` states the same fraction, rounded raw
percentage, and bucket cap label, confirming the section header score is the
raw percentage after the cap is applied, and using `Not migrated` plus `Next
evidence` as the blocking-boundary summary.

Do not add a second component-score table here; duplicated mutable scores drift
from the checklist source.

## Component Progress

### RISC-V ISA and Privileged Substrate - 59% single-axis

**Score calculation:** 21 of 25 items have executable evidence, or 84% raw,
capped to 59% by the single-axis bucket.
The bucket cap is single-axis because non-RISC-V ISAs and full RV64GC/vector
parity are not present.

- [x] RV64 integer, atomic, CSR, trap, counter, WFI, fence, PMP/PMA slices have tests.
- [x] RV64C integer/load-store/control-flow slices have tests.
- [x] RV64F/RV64D scalar load/store, arithmetic, comparisons, conversions, legal FP arithmetic and integer-to-float rounding-mode decode, exact static non-RNE integer-to-float conversion execution, inexact integer-to-float accrued flag updates, rounding-insensitive static arithmetic execution, `fadd.s`/`fsub.s` exact-wide-sum directed rounding, `fmul.s` exact-product and `fmul.d` normal finite exact-product plus directed-overflow and directed-underflow subnormal rounding, `fdiv.s` and `fdiv.d` finite-quotient directed rounding with NX, `fmadd.s` exact-window directed rounding, narrow `fmadd.d` exact-window static/dynamic directed rounding slices, `fadd.s`/`fsub.s`/`fmul.s`/`fmadd.s` invalid, overflow, underflow, and inexact accrued flags, and NaN-boxing have tests.
- [x] RV64F/RV64D integer-to-float conversions execute inexact static directed rounding and valid dynamic `frm` modes with accrued inexact flags.
- [x] RV64C double-precision FP load/store decode and compressed FP load/store CPU data-access slices have tests.
- [x] RVV vector-configuration instruction family (`vsetvli`, `vsetivli`, `vsetvl`) decode, hart execution, and CPU fetch-stream execution have tests.
- [x] RVV unmasked integer `vadd.vv` LMUL=1 and m2 register-group slices, invalid-`vtype` and unaligned-group traps, and CPU fetch-stream execution have tests.
- [x] RVV unmasked integer `vadd.vx` and signed-immediate `vadd.vi`, plus masked integer `vadd.vv`/`vadd.vx`/`vadd.vi` slices, decode, hart execution, and CPU fetch-stream execution have tests.
- [x] RVV unmasked integer `vsub.vv` LMUL=1 plus m2 register-group slices and `vsub.vx` decode, hart execution, and CPU fetch-stream execution have tests.
- [x] RVV unmasked integer bitwise `vand`, `vor`, and `vxor` vv/vx/vi forms decode, hart execution, and CPU fetch-stream execution have tests.
- [x] RVV unmasked integer shift `vsll`, `vsrl`, and `vsra` vv/vx/vi forms decode, hart execution, and CPU fetch-stream execution have tests.
- [x] RVV unmasked integer min/max `vminu`, `vmin`, `vmaxu`, and `vmax` vv/vx forms decode, hart execution, and CPU fetch-stream execution have tests.
- [x] RVV unmasked integer multiply `vmul`, `vmulhu`, `vmulhsu`, and `vmulh` vv/vx forms decode, hart execution, and CPU fetch-stream execution have tests.
- [x] RVV unmasked integer divide and remainder `vdivu`, `vdiv`, `vremu`, and `vrem` vv/vx forms decode, hart execution, and CPU fetch-stream execution have tests.
- [x] RVV unmasked integer mask compare `vmseq` and `vmsne` vv/vx/vi forms decode, hart execution, and CPU fetch-stream execution have tests.
- [x] RVV unmasked integer ordered mask compare `vmsltu`, `vmslt`, `vmsleu`, `vmsle`, `vmsgtu`, and `vmsgt` supported vv/vx/vi forms decode, hart execution, and CPU fetch-stream execution have tests.
- [x] RVV integer unmasked `vslideup`/`vslidedown` vx/vi forms plus `vslide1up`/`vslide1down` vx and `vrgather` vv/vx/vi forms, mask reductions `vcpop.m`/`vfirst.m`, mask prefixes `vmsbf.m`/`vmsof.m`/`vmsif.m`, mask indexes `viota.m`/`vid.v`, `vmerge` masked vv/vx/vi forms, `vmv.v` unmasked v/x/i, `vmv.x.s`/`vmv.s.x` scalar/lane-zero forms, and `vmv<nr>r.v` whole-register moves, `vcompress.vm`, `vzext`/`vsext` masked/unmasked extension-family decode, `vf2` extension hart execution, unmasked fixed-point saturating `vsaddu`/`vsadd`/`vssubu`/`vssub` vv/vx plus `vsaddu`/`vsadd` vi forms, unmasked fixed-point averaging `vaaddu`/`vaadd`/`vasubu`/`vasub` vv/vx forms, unmasked fixed-point signed fractional multiply `vsmul` vv/vx forms, integer carry/borrow `vadc`/`vmadc`/`vsbc`/`vmsbc` vv/vx plus add-immediate forms with reserved-encoding coverage, representative ISA execution, and representative CPU fetch-stream/fetch-ahead execution, single-width integer multiply-add `vmadd`/`vnmsub`/`vmacc`/`vnmsac` vv/vx decode with representative masked/unmasked ISA execution and representative unmasked CPU fetch-stream execution, base widening integer add/subtract `vwaddu`/`vwadd`/`vwsubu`/`vwsub` vv/vx/wv/wx decode with representative masked/unmasked ISA execution and representative unmasked CPU fetch-stream execution, widening integer multiply `vwmulu`/`vwmulsu`/`vwmul` vv/vx and widening integer multiply-add `vwmaccu`/`vwmacc`/`vwmaccsu` plus vx-only `vwmaccus` decode with representative masked/unmasked ISA execution and representative unmasked CPU fetch-stream execution, integer reductions `vredsum`, `vredand`, `vredor`, `vredxor`, `vredminu`, `vredmin`, `vredmaxu`, and `vredmax` plus widening reductions `vwredsumu` and `vwredsum` masked/unmasked vs forms at ISA level with unmasked CPU fetch-stream execution, and unmasked slide/gather, mask reductions, mask prefixes, mask indexes, plus `vzext.vf2` CPU fetch-stream execution have tests.
- [x] RVV mask logical `vmand`, `vmnand`, `vmandn`, `vmxor`, `vmor`, `vmnor`, `vmorn`, and `vmxnor` `.mm` forms decode, hart execution, and CPU fetch-stream execution have tests.
- [x] RVV floating-point `vfadd`, `vfsub`, `vfmin`, `vfmax`, `vfmul`, and `vfdiv` vv/vf forms plus `vfrsub.vf`, `vfrdiv.vf`, `vfmacc`, `vfnmacc`, `vfmsac`, and `vfnmsac` vv/vf forms and `vfsqrt.v` exact finite SEW=32 lane execution, E64 `vfadd.vv/vf`, `vfsub.vv/vf`, `vfrsub.vf`, `vfmul.vv/vf`, `vfdiv.vv/vf`, and `vfrdiv.vf` exact finite CPU fetch-stream execution, E64 `vfmin.vv/vf` and `vfmax.vv/vf` signed-zero and NaN CPU fetch-stream execution, `vfclass.v` SEW=32 classification lane execution, `vfmv.v.f` SEW=32 scalar splat lane execution, `vfmerge.vfm` SEW=32 masked scalar merge execution, `vfmv.s.f` SEW=32 scalar-to-lane-zero execution, `vfmv.f.s` SEW=32 lane-zero-to-scalar execution, integer-to-float `vfcvt.f.xu.v` and `vfcvt.f.x.v` plus float-to-integer `vfcvt.xu.f.v`, `vfcvt.x.f.v`, `vfcvt.rtz.xu.f.v`, and `vfcvt.rtz.x.f.v` dynamic/directed SEW=32 lane execution with NX accrual, E64 `vfcvt.f.xu.v`, `vfcvt.f.x.v`, `vfcvt.xu.f.v`, `vfcvt.x.f.v`, `vfcvt.rtz.xu.f.v`, and `vfcvt.rtz.x.f.v` CPU fetch-stream execution, FP mask compare `vmfeq`, `vmfne`, `vmflt`, and `vmfle` vv/vf SEW=32 mask results, `vfmin`, `vfsqrt.v`, and FP compare invalid-flag accrual, `vfsqrt.v` non-exact finite rejection, and `vfsgnj`, `vfsgnjn`, and `vfsgnjx` vv/vf sign-bit lane execution have tests, with `vfadd.vv` fetch-ahead coverage and the remaining listed forms CPU fetch-stream coverage.
- [x] Sv39 helpers and CPU memory-walker paths have tests.
- [x] RISC-V SE ecalls reach the system syscall table.
- [ ] Full RV64GC including vector execution and directed rounding coverage is complete.
- [ ] Linux-grade privileged CSR, interrupt, and exception breadth is complete.
- [ ] ARM, x86, Power, SPARC, MIPS, and GPU ISA execution have gem5-class owners.
- [ ] Hardware fetch translation and full boot-time privileged behavior are complete.

**Migrated:** RISC-V architectural state, large RV64 scalar slices, FP slices
including fused multiply-add special-case exception flags and legal static FP
arithmetic plus integer-to-float conversion rounding-mode decoding, exact static
non-RNE integer-to-float conversion execution, integer-to-float inexact
`fflags`/`fcsr` updates, inexact static directed and valid dynamic `frm`
integer-to-float execution, rounding-insensitive static FP arithmetic
execution, `fadd.s`/`fsub.s` exact-wide-sum static/dynamic directed rounding
with NX accrual, `fmul.s` exact-product static directed rounding with NX,
UF, and directed-boundary overflow accrual, `fmul.d` normal finite exact-product
static directed rounding with NX, directed overflow boundaries with OF/NX, and
directed subnormal underflow boundaries with UF/NX, `fdiv.s` and `fdiv.d`
finite-quotient static directed rounding with NX accrual, `fmadd.s` exact-window
static/dynamic directed rounding with RMM tie-away, NX, and directed-boundary
overflow accrual, narrow `fmadd.d`
static RoundUp and dynamic RoundDown exact-window inexact rounding with
directed zero-sign and overflow-flag boundary slices, plus
`fadd.s`/`fsub.s` signaling-NaN, infinity invalid, overflow, and wider finite
NX flag slices, compressed double FP load/store decoding, compressed FP load/store
CPU data access, privileged interrupt fixed-priority machine and delegated
supervisor trap entry, RVV vector-configuration decode and unmasked integer
`vadd.vv` LMUL=1 plus m2 register-group execution, unmasked integer
`vadd.vx`, signed-immediate `vadd.vi`, masked `vadd.vv`/`vadd.vx`/`vadd.vi`,
unmasked integer `vsub.vv` LMUL=1 plus
m2 register-group execution, `vsub.vx`, `vrsub.vx`/`vrsub.vi`, and unmasked integer bitwise
`vand`/`vor`/`vxor`, shift `vsll`/`vsrl`/`vsra`, and min/max
`vminu`/`vmin`/`vmaxu`/`vmax` plus multiply
`vmul`/`vmulhu`/`vmulhsu`/`vmulh` and divide/remainder
`vdivu`/`vdiv`/`vremu`/`vrem`, integer carry/borrow `vadc`/`vmadc`/`vsbc`/`vmsbc` vv/vx plus add-immediate forms with reserved-encoding coverage and representative execution, single-width integer multiply-add `vmadd`/`vnmsub`/`vmacc`/`vnmsac` vv/vx decode with representative execution, integer reductions `vredsum`/`vredand`/`vredor`/`vredxor`/`vredminu`/`vredmin`/`vredmaxu`/`vredmax` plus widening reductions `vwredsumu`/`vwredsum`, base widening add/subtract `vwaddu`/`vwadd`/`vwsubu`/`vwsub` vv/vx/wv/wx decode with representative execution, widening multiply `vwmulu`/`vwmulsu`/`vwmul` vv/vx and widening multiply-add `vwmaccu`/`vwmacc`/`vwmaccsu` plus vx-only `vwmaccus` decode with representative execution, and mask compare
`vmseq`/`vmsne` plus ordered mask compare
`vmsltu`/`vmslt`/`vmsleu`/`vmsle`/`vmsgtu`/`vmsgt`, merge `vmerge`, move
`vmv.v`, `vmv.x.s`/`vmv.s.x`, and `vmv<nr>r.v`, unmasked slide `vslideup`/`vslidedown`/`vslide1up`/`vslide1down`, gather `vrgather`, mask reductions `vcpop.m`/`vfirst.m`, mask prefixes `vmsbf.m`/`vmsof.m`/`vmsif.m`, mask indexes `viota.m`/`vid.v`, compress `vcompress.vm`, zero/sign extension `vzext`/`vsext` masked/unmasked decode with `vf2` hart execution and unmasked slide/gather, mask reductions, mask prefixes, mask indexes, plus `vzext.vf2` CPU fetch-stream execution, unmasked fixed-point saturating `vsaddu`/`vsadd`/`vssubu`/`vssub` vv/vx plus `vsaddu`/`vsadd` vi forms, unmasked fixed-point averaging `vaaddu`/`vaadd`/`vasubu`/`vasub` vv/vx forms, unmasked fixed-point signed fractional multiply `vsmul` vv/vx forms, unmasked fixed-point scaling shifts `vssrl`/`vssra` vv/vx/vi, fixed-point narrowing shifts `vnsrl.wv`/`vnsra.wv`/`vnsrl.wx`/`vnsra.wx`/`vnsrl.wi`/`vnsra.wi` and narrow clip `vnclipu.wv`/`vnclip.wv`/`vnclipu.wx`/`vnclip.wx`/`vnclipu.wi`/`vnclip.wi`, and unmasked floating-point `vfadd`, `vfsub`, `vfmin`, `vfmax`, `vfmul`, and
`vfdiv` vv/vf forms plus `vfrsub.vf`, `vfrdiv.vf`, `vfmacc`,
`vfnmacc`, `vfmsac`, and `vfnmsac` vv/vf exact finite SEW=32 lanes,
E64 `vfadd.vv/vf`, `vfsub.vv/vf`, `vfrsub.vf`, `vfmul.vv/vf`, `vfdiv.vv/vf`, and `vfrdiv.vf` exact finite CPU fetch-stream lanes, E64 `vfmin.vv/vf` and `vfmax.vv/vf` signed-zero and NaN CPU fetch-stream lanes, `vfclass.v` SEW=32 classification lanes, `vfmv.v.f`
SEW=32 scalar splat lanes, `vfmerge.vfm` SEW=32 masked scalar merge,
`vfmv.s.f` SEW=32 scalar-to-lane-zero, `vfmv.f.s` SEW=32
lane-zero-to-scalar, integer-to-float `vfcvt.f.xu.v` and `vfcvt.f.x.v`, and
float-to-integer `vfcvt.xu.f.v`, `vfcvt.x.f.v`, `vfcvt.rtz.xu.f.v`,
and `vfcvt.rtz.x.f.v` dynamic/directed SEW=32 lanes with NX accrual plus
E64 `vfcvt.f.xu.v`, `vfcvt.f.x.v`, `vfcvt.xu.f.v`, `vfcvt.x.f.v`,
`vfcvt.rtz.xu.f.v`, and `vfcvt.rtz.x.f.v` CPU fetch-stream execution, FP
mask compare `vmfeq`, `vmfne`, `vmflt`, and `vmfle` vv/vf SEW=32 mask results,
`vfmin` signaling-NaN invalid-flag accrual, `vfsqrt.v` negative and
signaling-NaN invalid-flag accrual, `vmfeq`/`vmfne` quiet-compare
signaling-NaN invalid-flag accrual, `vmflt`/`vmfle` signaling-compare invalid-flag accrual,
`vfsqrt.v` non-exact finite rejection, `vfsgnj`, `vfsgnjn`, and `vfsgnjx`
vv/vf sign-bit lanes, reserved-`frm` and NaN-boxing trap coverage,
`vfadd.vv` fetch-ahead coverage, and CPU fetch-stream coverage for the remaining listed forms, plus
`vxrm`/`vxsat`/`vcsr` CSR visibility through the CPU fetch stream
or hart execution; mask logical `.mm` forms through decode, hart execution, and
CPU fetch-stream tests; unmasked RVV unit-stride `vle8`/`vle16`/`vle32`/`vle64`
and `vse8`/`vse16`/`vse32`/`vse64` m1 CPU data-path execution, plus aligned
`e32,m2` full 32-byte register-group unit-stride load/store CPU data-path
execution and misaligned register-group rejection; and traps, translation
helpers, and SE ecall plumbing.

**Not migrated:** Full RV64GC/vector data-operation breadth, other major ISAs, directed
rounding breadth beyond the covered integer-to-float, single-precision
add/sub/multiply/divide/fused multiply-add, double-precision multiply/divide, and narrow
double-precision `fmadd.d` slices, and complete Linux privileged behavior.

**Evidence:** `RiscvInstruction::decode_with_length`,
`decode_float_op`, `decode_compressed`, `walk_sv39_page_table_with_context`,
tests `rv64i`, `rv64m`, `rv64f`, `rv64f_add`, `rv64f_sub`, `rv64f_fma`,
`rv64d`, `rv64d_fma`, `riscv_scalar_float_frontend`,
`vector_config_prediction`, `vector_compress`,
`vector_extend`, `vector_fixed_point`, `vector_integer`, `vector_merge_move`,
`vector_mask_compare`, `vector_mask_logical`, `riscv_frontend`,
`riscv_vector_extend_frontend`, `riscv_vector_float_frontend`, `sv39`, and privileged RISC-V tests.

**Next evidence:** Generated or imported RV64GC/vector instruction tests plus
privileged Linux trap and interrupt smoke tests.

### CPU Execution Models - 39% unit-slice

**Score calculation:** 5 of 10 items have executable evidence, or 50% raw,
capped to 39% by the unit-slice bucket. The bucket cap is unit-slice because
RISC-V core timing has direct completed-fetch overlap, bounded normal-driver
fetch-ahead, and narrow pending-fetch and data-access resource-stall slices,
but broad stalls/squashes remain incomplete and O3 state is not yet an
executable cycle-visible engine.

- [x] RISC-V atomic execution and parallel clusters execute real instructions.
- [x] Data access issue/response and store-conditional progress diagnostics have tests.
- [x] Basic, GShare, BiMode, Tournament, TAGE-SC-L, and multiperspective perceptron branch predictors are trained from retired control flow.
- [x] Checker CPU support exists as a RISC-V retire-path reference hart with CLI-visible counts.
- [ ] Minor-like fetch/decode/execute/commit timing is wired into normal CPU execution.
- [ ] Branch predictors steer fetch with speculation snapshots, squash, and rollback.
- [ ] A running O3 engine owns ROB, LSQ, rename map, commit, store-to-load forwarding, and FU latency.
- [ ] CPU mode switching transfers live architectural and timing authority.
- [ ] KVM or equivalent fast-forward execution exists.
- [x] CPU instruction/data traffic uses the top-level L1/L2/L3 cache, fabric, and DRAM hierarchy by default.

**Migrated:** Atomic RISC-V execution, frontend/data slices, branch predictor state and retired training, directly issued completed 4-byte fetches occupying the in-order timing state before retire, normal `drive_next_action`, cluster, and data-translation drivers issuing bounded fetch-ahead for completed straight-line 4-byte integer instructions, predictor-selected conditional branches including top-level `rem6 run --riscv-branch-predictor gshare`, `bimode`, `tage-sc-l`, and `multiperspective-perceptron` fetch-steering evidence, and direct RISC-V `JAL`/`JALR` targets before retire, with direct `JAL` front-end predictions preserved through retire in the normal driver; compressed straight-line instructions reaching the same normal parallel-cluster fetch-ahead path, branch and direct-jump fetch-ahead speculation history recorded and resolved at retire, and explicit `--riscv-branch-lookahead 2` two-deep conditional-branch fetch-ahead through the normal `rem6 run` path with an older misprediction repairing predictor history and removing younger branch speculation before wrong-path registers commit,
selected TAGE-SC-L fetch-ahead using pending branch-speculation and direct-jump history overlays, selected multiperspective-perceptron fetch-ahead using pending thread-history snapshots, selected GShare, BiMode, and Tournament fetch-ahead using live selected-predictor speculative history, and direct selected TAGE-SC-L plus multiperspective-perceptron fetch-ahead recording live mutable selected-family history with snapshot rollback,
completed younger fetches consumed by in-order branch flushes, wrong-path branch fetch-ahead requests from fall-through and predicted-target paths squashed even when their memory response is still outstanding, bounded normal-driver fetch-ahead requests inserted into the in-order timing state when issued and before their memory response, per-retired instruction in-order stage advancement with runtime stats, non-retire fetch and intermediate retire-loop cycle records retained in per-core in-order pipeline history and consumed by top-level stats, data-response wait cycles emitted as explicit non-retire resource-stall in-order cycle records before data-instruction retire, per-stage occupied-cycle stats, per-stage resource-blocked and ordering-blocked instruction plus cycle summaries emitted from in-order cycle records, explicit data-stall cycle stats, retired Tournament predictor local/global selection counters plus GShare, BiMode, Tournament, TAGE-SC-L, and multiperspective-perceptron predictor snapshot lookup/update/repair counters emitted by top-level `rem6 run`, fixed scalar integer M-extension, vector integer multiply and multiply-accumulate including fixed-point signed fractional multiply and widening variants, divide, remainder, fixed-point scaling shifts, narrowing shifts, narrow clip, and integer/widening reductions, scalar FP arithmetic, compare, conversion, and misc, plus decoded vector FP add/sub/min/max, mask compare, conversion/move/merge, sign/class, multiply/FMA/divide/sqrt execute-stage latency emitted as additional in-order resource-stall cycles from top-level CLI runs,
unmasked RVV unit-stride load/store for matching e8/e16/e32/e64 m1 vector
configurations plus aligned e32/m2 full 32-byte register-group vector memory
moving bytes through direct and cache-backed normal data access paths and adding
fixed vector LSU execute-stage latency before data wait,
selected GShare, BiMode, and Tournament fetch-ahead now record live selected-predictor speculative history at issue and squash/repair it at retire for wrong-path nested branch runs, direct selected TAGE-SC-L and multiperspective-perceptron fetch-ahead records live selected-family histories and restores pre-update snapshots on stale redirect cleanup, with top-level per-family rollback counters covering aggregate branch-speculation repairs, selected GShare/BiMode/Tournament younger cleanup, and explicit selected TAGE-SC-L plus multiperspective-perceptron snapshot rollback counts, and CPU checkpoint tests proving committed selected-family history snapshots and restore-time selected-record invalidation,
top-level conditional multiperspective-perceptron and indirect TAGE-SC-L plus
multiperspective-perceptron wrong-path selected fetch-ahead runs now prove the
same selected-family rollback counters and branch-kind lookup stats through
`rem6 run`,
per-core fetch-response and data-response wait cycle stats, retired branch
prediction, speculation repair, and redirect summaries in normal in-order timing records, RISC-V
normal parallel-cluster pending-fetch resource-stall accounting consumed by CLI run stats, core checkpoints preserving the basic fetch-steering branch predictor/BTB payload, including live fetch-ahead pending branch speculation state, plus GShare, BiMode, Tournament, TAGE-SC-L, and multiperspective perceptron predictor checkpoint payloads, a RISC-V checker CPU option that runs an
independent reference hart at retire, records structured execution/state
mismatches, and exposes checked/mismatch counts through `rem6 run --checker-cpu`
stats, per-stage in-order flush and branch-prediction flush attribution stats,
plus aggregate and per-stage flush-cycle and branch-prediction flush-cycle stats
from real branch redirect cycle records,
top-level `rem6 run --riscv-in-order-width` configuring the live
five-stage in-order pipeline width, allowing wider fetch-stage overlap in real
executed state without false younger retire, and exposing width plus max
in-flight occupancy through executed-run JSON/stat output, top-level default RISC-V instruction/data traffic through L1/L2/L3 MSI
cache/fabric/DRAM, explicit `--memory-system direct` timing-test coverage, and
O3 policy helpers.

**Not migrated:** Full Minor-like in-order timing with realistic stalls and
squashes, executable O3 timing, broader predictor-family speculation snapshots
and rollback beyond the current direct, conditional, and indirect selected-family
top-level slices, gem5-class checker coverage across future timing CPU modes, and
KVM equivalents.

**Evidence:** `RiscvCore::execute_next_completed_fetch`,
`RiscvClusterRun`, `record_data_retire_cycle`, `RiscvCheckerCpu`,
`InOrderPipelineState`, `O3DistributedIssueScheduler`, `O3SourceRenamePlan`,
CPU frontend, checker, and O3 tests.
CLI run stats include per-core in-order pipeline cycle, retired, and
fetch-response wait counters from executed RISC-V instructions, and CLI data
stats show load/store response wait changing the in-order pipeline cycle
counter, data-stall cycle stat, and data-wait cycle stat; top-level CLI
latency tests also cover vector fixed-point scaling shifts, narrowing shifts,
narrow clip, and integer/widening reduction execute-stage stall cycles from
executed RISC-V runs; CLI run stats also expose per-core
in-order cycle-plan advance, general flush, resource-blocked,
ordering-blocked, branch-prediction, branch-misprediction,
branch-prediction flush, and redirect counters from the same executed RISC-V
timing records, including nonzero branch-prediction flush evidence for a real
taken RISC-V branch; they also expose branch-speculation prediction, repair, younger-removal, and maximum-pending counters for a real nested conditional branch run where the corrected older branch prevents younger wrong-path register writes from committing. CLI `rem6 run --riscv-branch-predictor gshare`, `bimode`, `tage-sc-l`, and `multiperspective-perceptron` drive the normal RISC-V fetch-ahead path with the selected predictor, clean stale wrong-path in-order state across loop backedges, and change deterministic fetch-steering timing or branch-steering counters while preserving the final architectural register state; CLI stats now expose per-family GShare, BiMode, Tournament, TAGE-SC-L, and multiperspective-perceptron predictor counters from the live core snapshots, including selected GShare, BiMode, and Tournament rollback counters from the live top-level nested-branch repair path plus explicit selected TAGE-SC-L and multiperspective-perceptron rollback counters from top-level selected snapshot cleanup runs.
The selected snapshot cleanup runs include conditional multiperspective-perceptron
wrong-path branches and indirect JALR wrong-path fetch-ahead for TAGE-SC-L plus
multiperspective-perceptron, with indirect-unconditional branch-kind lookup
stats proving the top-level path.
Branch redirect CLI runs now emit `Branch` debug records and `sim.debug.branch_trace.records`,
`sim.debug.branch_trace.mispredictions`, `sim.debug.branch_trace.flushed`, and CPU/kind/outcome scoped stats from executed in-order branch-prediction cycle records, with flushed counts checked against emitted flushed-sequence lists.
These runs also emit per-stage in-order flush counts, aggregate and per-stage flush-cycle and branch-prediction flush-cycle counts, proving squash attribution and cycle-level squash presence from the same executed cycle records.
Pipeline debug runs emit per-cycle `Pipeline` debug records with `sim.debug.pipeline_trace.records`, `sim.debug.pipeline_trace.branch_prediction_flushed`, `sim.debug.pipeline_trace.flush_cause.branch_prediction.records`, `sim.debug.pipeline_trace.stall_cause.fetch_wait.records`, `sim.debug.pipeline_trace.stall_cause.data_wait.stall_cycles`, `sim.debug.pipeline_trace.stall_cause.execute_wait.records`, and CPU/state scoped stats from the executed in-order cycle plans; normal run stats expose `sim.cpu0.pipeline.in_order.execute_wait_cycles` and the `system.cpu.pipeline.inOrder.executeWaitCycles` text alias from the same executed pipeline records.
CLI `rem6 run --riscv-in-order-width 2` applies a uniform stage-width config to the live `RiscvCore` in-order pipeline snapshot and emits per-stage width plus max-in-flight occupancy stats from the executed run, with width 2 reaching two in-flight instructions in affected stages. Pending-fetch runs expose per-stage resource-blocked instruction counts whose total matches aggregate resource-blocked counts, plus per-stage resource-blocked and ordering-blocked cycle stat families derived from the same cycle records.
RISC-V in-order timing tests cover retired taken/fall-through redirect, completed-fetch overlap, serial/translated/parallel fetch-ahead, direct `JAL` and register-target `JALR` fetch-ahead preservation, compressed fetch-ahead, wrong-path repair, pending occupancy before response completion, serial trap repair, stream-reset discard, and normal-driver fetch-response wait resource-stall records that preserve in-flight fetch state without breaking completed-fetch overlap, selected GShare, BiMode, and Tournament mutable speculative-history repair for younger fetch-ahead branch prediction plus TAGE-SC-L and multiperspective-perceptron speculative-history overlays, direct selected TAGE-SC-L and multiperspective-perceptron mutable history rollback on stale redirect cleanup, committed checkpoint snapshots, and restore-time selected-record invalidation, top-level retired Tournament predictor local/global selection stats, top-level predictor-family counter stats, selected GShare/BiMode/Tournament rollback counter stats, and selected TAGE-SC-L plus multiperspective-perceptron rollback counter stats, scalar integer M-extension multiply/divide/remainder, vector integer multiply and multiply-accumulate including fixed-point signed fractional multiply and widening variants, divide, remainder, fixed-point scaling shifts, narrowing shifts, narrow clip, and integer/widening reductions, scalar FP add/multiply/FMA/divide/sqrt, compare, conversion, and misc, decoded vector FP add/sub/min/max, mask compare, conversion/move/merge, sign/class, multiply/FMA/divide/sqrt execute-stage latency, and e8/e16/e32/e64 m1 plus aligned e32/m2 full-register-group unmasked RVV unit-stride load/store byte movement through direct/cache-backed top-level CLI paths and fixed vector LSU execute latency through execute-wait stats, with representative e32/m1 vector memory cycle and stall deltas, plus normal parallel-cluster pending-fetch resource stalls emitted through top-level CLI aggregate, per-stage instruction, and per-stage cycle stats. CLI tick-limit runs expose pending fetch occupancy, per-stage occupied-cycle stats, non-retire cycle advancement, and direct `JAL` target-fetch-in-flight behavior without committing the fall-through register update. These paths preserve branch speculation, pending-interrupt redirect, and data-access ordering.
CLI `rem6 run --checker-cpu` executes the same RISC-V retire path and emits per-core checked-instruction and mismatch counters from the checker snapshot;
checker tests cover retire comparison, data writeback sync, environment-call
completion sync, public register writes, and HTM abort rollback.

**Next evidence:** Broader per-cycle in-order squashes and remaining stalls beyond current
resource/ordering blocked-cycle, fetch/data/execute wait, and branch-redirect flush-cycle/flush-cause attribution, remaining
masked, strided, indexed, segment, fault-only, arbitrary multi-line, and broader LMUL
register-group vector load/store forms beyond the current unmasked unit-stride
m1 plus aligned e32/m2 full-register-group slice, broader TAGE-SC-L and multiperspective-perceptron mutable predictor-family speculation snapshots
and rollback beyond the current top-level conditional and indirect selected fetch-ahead slices, full checker integration for future timing CPU modes, then a
ROB/LSQ-backed O3 run test.

### Memory, Cache, Coherence, Fabric, and DRAM - 59% single-axis

**Score calculation:** 10 of 15 items have executable evidence, or 67% raw. The bucket cap is single-axis because Ruby-scale protocol networks, router/virtual-channel NoC behavior beyond top-level shared-link/per-VN lane arbitration and credit-delay attribution, broader DRAM refresh modes beyond per-bank/all-bank policy plus named full JEDEC timing-table breadth beyond custom timing rows, and broader hierarchy-wide prefetch translation consumers remain incomplete.

- [x] Memory stores, page maps, translation queues, and TLB state have tests.
- [x] Cache banks model replacement, MSHRs, write queues, maintenance, sector and compressed tags.
- [x] MSI/MESI/MOESI/CHI line and bank models have protocol tests.
- [x] DRAM/NVM profiles expose bank, timing, QoS, refresh-policy, low-power, and routed execution slices, including custom CLI DRAM timing rows in executed JSON and stats.
- [x] Fabric and transport expose multi-hop routing, credits, virtual networks, credit-delay attribution, and activity records.
- [x] Named DDR4, DDR5, HBM, and LPDDR4 JEDEC-style refresh presets validate `tREFI`/`tRFC` cycles through profile constructors, controller refresh scheduling, and `rem6 run --dram-memory-profile` stats.
- [x] Cache-local prefetch queues expose enqueue, issue, drop, and translation counters from queued prefetch and translation flows.
- [x] Top-level CLI instruction-cache and data-cache prefetch consume identity-mapped page-crossing translation requests and emit queue-level stats from executed RISC-V runs.
- [x] Top-level RISC-V instruction/data traffic uses the L1/L2/L3 cache hierarchy and fabric-routed DRAM by default, with `rem6 run --memory-system cache-fabric-dram` selecting the same MSI hierarchy and `--memory-system direct` preserving direct-memory runs when explicitly requested.
- [x] Top-level CLI data-cache page-crossing prefetch preserves `PrefetchRead` lower-fill intent through an MSI L1/L2/L3 hierarchy and emits per-level prefetch-fill counters.
- [ ] Ruby-scale protocol transactions and topology races are migrated.
- [ ] NoC router, flit, virtual-channel, and routing detail match gem5-class coverage.
- [ ] Broader DRAM refresh modes beyond per-bank/all-bank policy and named full JEDEC timing tables beyond custom timing rows are complete.
- [ ] Broader instruction-cache and non-MSI prefetch translation consumers and queue-level stats are complete across hierarchy paths.
- [ ] Bank/cache/fabric/DRAM resource counters are complete enough for full-system studies.

**Migrated:** Typed memory primitives, cache banks, protocol harnesses, DRAM
profiles, controller-level refresh timing slices, routed topology slices,
CLI-selectable JEDEC refresh presets,
cache-local queued-prefetch and missing-translation queue counters, top-level
CLI RISC-V tagged next-line data-cache and instruction-cache prefetch issue and
identity-mapped page-crossing translation stats, and trace replay
consumers; optional CLI RISC-V data traffic can drive MSI-bank,
MESI-line, MOESI-line, and CHI-line data-cache runs and emit CPU response and
directory decision counters from those runs; CLI RISC-V MSI-bank data-cache
runs also emit bank accepted, immediate-hit, scheduled-miss, and coalesced-miss
counters from the executed cache-bank cycles; three-core CLI RISC-V runs can share
MSI, MESI, MOESI, and CHI data-cache runtimes, observe cross-core data
coherence, and emit per-core data route stats; three-core CLI RISC-V instruction
fetch traffic can drive MSI, MESI, MOESI, and CHI instruction-cache runtimes
and emit per-core fetch route stats; DRAM-backed MSI data-cache line fills emit
backing DRAM read counters from a CLI run using `--dram-memory` and
`--data-cache-protocol msi`, and read-only cache fetch/load responses do not
emit false DRAM writes; an explicit CLI run using `--data-cache-protocol msi`
and `--data-cache-l2-protocol msi` with optional
`--data-cache-l3-protocol msi` routes data misses through L1/L2 or L1/L2/L3
before DRAM; equivalent instruction-cache flags do the same for fetches, and
both can share an explicit fabric link with request/response virtual networks,
shared physical-link and FIFO/LIFO/least-recently-granted QoS queue arbitration, credit depth, and aggregate, per-virtual-network, per-link, and per-link/per-VN lane
fabric JSON/stat counters including flit and credit-delay counts, optional router input-VC/output-port stage latency and queue-delay timing through executed run and replay summaries, and
`--debug-flags Fabric` exposes aggregate lane/hop transfer, byte, flit, occupied-tick, queue-delay,
max-queue-delay, credit-delay, and max-credit-delay stats from emitted fabric trace records while `--debug-flags Dram` exposes aggregate target/port/bank activity, row, refresh, command, and ready-latency stats, target/port turnaround stats, and bank byte stats from emitted DRAM trace records. Volatile CLI external-memory
profiles carry refresh/recovery, per-bank/all-bank refresh policy, and custom timing-table rows; DDR4, DDR5, and
HBM presets validate `tREFI`/`tRFC`; DDR and LPDDR CLI RISC-V paths emit
configurable refresh/recovery and low-power timing stats from routed fetches. DRAM target activity
exposes per-bank access/read/write, byte, row, command, refresh, controller all-bank sibling-bank refresh accounting, HBM2 JEDEC terminal all-scheduler-bank refresh counters from a top-level CLI run that crosses `tREFI`, and LPDDR terminal low-power plus multi-bank exit counters.
CLI RISC-V runs aggregate executed instruction-cache, data-cache,
memory-transport, and DRAM activity into unified memory-resource activity and
active-resource JSON/stat counters, including unified cache CPU-response,
directory-decision, backing-DRAM access, cache-bank accepted, immediate-hit,
scheduled-miss, and coalesced-miss counters plus L1/L2/L3 cache hierarchy
resource breakdowns. CLI trace
replay can route packet traces through an explicit workload fabric link and
configure request/response virtual networks, link credit depth, and router-stage timing from CLI or
TOML config while emitting active-lane, active-virtual-network, transfer, byte, flit,
occupancy, queue-delay, credit-delay, contention, per-virtual-network, per-link, per-link/per-VN lane pressure, and per-hop activity stats from
the resulting workload parallel summary, plus aggregate resource-activity and
active-resource stats from the same summary; the same explicit-fabric path emits fabric wait-for edge counts, queue/credit kind windows, blocked packet windows, target lane/credit resource windows, and stable `sim.trace_replay.fabric.wait_for.*` stats. CLI trace replay with an explicit
data-cache protocol emits data-cache run accounting, per-protocol run stats,
CPU-response and directory-decision counters, scheduler epoch, dispatch, batch,
active-partition, worker-count, profiled backing-DRAM access,
target/port/bank, row, command, and ready-latency stats in artifact JSON, plus
QoS stats in `StatsRegistry`; a combined CLI trace-replay run proves explicit MSI data-cache plus explicit fabric routing
compose in one execution with cache-maintenance and fabric activity counters.
CLI `run` with `--data-cache-prefetcher tagged-next-line` consumes real RISC-V
data loads, issues prefetch reads through the selected data-cache protocol,
routes page-crossing next-line candidates through the queued missing-translation
flow with identity completion, counts a later demand hit on an issued prefetch
line as useful, preserves useful-but-miss, useful-span-page, unused, and late-hit subcounts, records demand MSHR misses for coverage denominators, and emits prefetch
identified/enqueued/issued/useful plus page-span, in-cache drop, translation queue, and fixed-point `accuracy_ppm`/`coverage_ppm` stats from the executed run, including configured single-core gem5-style
`system.cpu.dcache.prefetcher.pfIdentified`/`pfIssued`/`pfUseful`/`pfUsefulButMiss`/`pfUnused`/`pfHitInCache`/`pfHitInMSHR`/`pfHitInWB`/`pfLate`/`pfSpanPage`/`pfUsefulSpanPage`/`pfInCache`/`demandMshrMisses`
aliases plus derived `accuracy`/`coverage` aliases. CLI
`run` with `--instruction-cache-prefetcher tagged-next-line` consumes real
RISC-V instruction fetches, issues prefetch reads through the selected
instruction-cache protocol, routes page-crossing next-line candidates through
the same identity-completed queue flow, suppresses repeated next-line issues,
and emits the same prefetch queue and translation queue stats from the executed run, including configured single-core gem5-style
`system.cpu.icache.prefetcher.pfIdentified`/`pfIssued`/`pfUseful`/`pfUsefulButMiss`/`pfUnused`/`pfHitInCache`/`pfHitInMSHR`/`pfHitInWB`/`pfLate`/`pfSpanPage`/`pfUsefulSpanPage`/`pfInCache`/`demandMshrMisses`
aliases plus derived `accuracy`/`coverage` aliases. Bare top-level `rem6 run --isa riscv --execute` now selects the cache/fabric/DRAM hierarchy by default; `--memory-system direct` remains an explicit direct-memory configuration for timing-focused runs.
Top-level `rem6 run` with data-cache tagged next-line prefetch plus MSI L2/L3 preserves the `PrefetchRead` operation for lower hierarchy fills and exposes L1, L2, and L3 `prefetch_fills` artifact JSON and `prefetch.fills` stats from the translated page-crossing prefetch path.

**Not migrated:** Ruby-scale protocol networks, gem5-class router, flit, routing, and virtual-channel NoC detail beyond the current optional per-hop router-stage and QoS queue-policy slices, named full JEDEC timing-table breadth beyond custom timing rows, broader instruction-cache and non-MSI hierarchy-level prefetch translation consumers, and additional DRAM refresh modes beyond per-bank/all-bank policy.

**Evidence:** `MsiCacheBank`, `MsiCacheController`, protocol directory harnesses, `DramController`, `DramMemoryController`, `FabricModel`, `MemoryTransport`, and tests `riscv_topology_msi_data`, `riscv_topology_chi_data`, `memory_controller`, `timing`, `fabric_timing`, `workload_replay_fabric_hop_activity`, `system_run_resource_activity`, `prefetch_queue_stats`, `prefetch_queue_translation`, `refresh_presets`, and CLI `run` data-cache smoke coverage with three-core MSI/MESI/MOESI/CHI data-cache coherence routing, three-core MSI/MESI/MOESI/CHI instruction-cache fetch routing, DRAM-backed MSI data-cache fill read accounting, MSI-bank accepted, immediate-hit, scheduled-miss, and coalesced-miss counters from an executed RISC-V data-cache run, explicit MSI L1-to-L2-to-DRAM plus L1-to-L2-to-L3-to-DRAM data/instruction-cache fill smoke coverage, and a combined top-level RISC-V cache/fabric/DRAM smoke exposing aggregate, per-virtual-network, per-link, and per-link/per-VN lane `sim.memory.fabric.*` counters including flits and credit-delay ticks, fabric roll-up inside aggregate memory-resource stats, and `--memory-system cache-fabric-dram` preset coverage. DRAM memory-profile tests cover bank-level resource counters, resource-activity stats, and activity-window counter deltas.
CLI `run` default-memory-system smoke coverage proves bare RISC-V load/store execution drives L2/L3 cache activity, fabric transfers, DRAM accesses, and unified stats; `--memory-system direct` proves explicit direct-memory execution remains available.
CLI `run` also has DDR profile refresh smoke coverage that exposes default and custom refresh timing fields, all-bank refresh-policy JSON/stats, and nonzero refresh stats from RISC-V DRAM execution, HBM custom timing-table coverage for activate/read/write/precharge, bus-turnaround, burst-spacing, same-bank-group spacing, and command-window rows in executed JSON and stats, HBM2 JEDEC terminal refresh coverage that emits one `tRFC` window across all 16 scheduler banks after a top-level run crosses `tREFI`, plus controller sibling-bank accounting for all-bank refresh, LPDDR default and custom low-power timing, active-powerdown, precharge-powerdown, self-refresh, target/port/bank hierarchy low-power stats from routed RISC-V fetch requests, all profiled LPDDR scheduler banks after terminal idle residency, and repeated-access low-power exits across multiple scheduler banks; CLI `run` cache/DRAM resource smoke coverage exposes unified memory-resource cache CPU-response, directory-decision, backing-DRAM access, cache-bank, L1/L2/L3, instruction/data cache hierarchy, and hierarchy-scoped cache prefetch queue/translation-queue counters in artifact JSON and `StatsRegistry`.
CLI `run` cache/fabric/DRAM resource smoke coverage also exposes fetch/data memory-transport resource activity, active-resource, arrival/response, round-trip, and max-round-trip counters in artifact JSON and `StatsRegistry`, derived from real fetch/data `MemoryTrace` summaries. The same smoke exposes memory-resource fabric active-lane, active-virtual-network, transfer, byte, flit, occupied-tick, queue-delay, credit-delay, contention, configured router stage, and router latency/queue counters in artifact JSON and `StatsRegistry`, plus configured QoS queue policy in artifact JSON, link/lane/hop artifact JSON, and virtual-network/per-link/per-VN/per-hop resource stats derived from the executed run fabric summary and matched back to emitted lane and hop records. The Fabric debug smoke also exposes aggregate lane/hop transfer, byte, flit, occupied-tick, queue-delay, max-queue-delay, credit-delay, and max-credit-delay `sim.debug.fabric_trace.*` counters derived from emitted records; the DRAM debug smoke exposes aggregate target/port/bank activity, row, refresh, command, and ready-latency counters, target/port turnaround counters, and bank byte counters derived from emitted records.
Top-level `rem6 run` cache/DRAM resource smoke coverage exposes memory-resource DRAM active target/port/bank, access, read/write, byte, row plus split read/write row-hit, refresh, low-power residency, command, turnaround, ready-latency, and read-ready-latency counters in artifact JSON and `StatsRegistry`, derived from the executed DRAM summary. The same run path exposes gem5-style memory-controller request, burst, and byte aliases under `system.mem_ctrl.*` plus DRAM-interface burst, per-bank burst, row-hit, byte, memory-access-latency, and NVM-profile `nvmBytesRead`/`nvmBytesWritten` aliases under `system.mem_ctrl.dram.*`, backed by executed DRAM/NVM summary request counts, bank request counts, split row-hit counts, bank byte totals, and read ready-latency ticks, plus shared L2/L3 text-output overall hit/miss/access/miss-rate and MSHR aliases such as `system.l2.overallMshrHits`/`system.l3.overallMshrMissRate` backed by executed instruction/data L2/L3 cache-bank summaries. Top-level `rem6 run --dram-memory` smoke coverage also exposes executed DRAM target, port, and bank hierarchy counters as structured `/dram/targets` and `/memory_resources/dram/targets` artifact JSON, including port active-bank counts, target/port byte totals, port row, refresh, and ready-latency counters, bank read/write counts, and target/port/bank low-power residency, plus matching `sim.memory.resources.dram.target*.port*.bank*` stats.
CLI
trace-replay fabric-route smoke coverage exposes nonzero active-lane,
active-virtual-network, transfer, byte, and flit stats plus request/response virtual
network and credit-depth config fields plus per-virtual-network, per-link, and per-link/per-VN lane pressure
`sim.trace_replay.fabric.*` counters, per-lane flit counts, and per-hop activity JSON from the top-level replay command, including aggregate, per-VN, per-lane, and per-hop
credit-delay fields, router config JSON, and per-link/per-VN/per-hop trace-replay stat families for transfer, byte, flit, occupied-tick, queue-delay, max-queue-delay, credit-delay, router-latency, and router-queue counters plus per-link/per-VN lane backpressure counters; CLI trace-replay fabric-route wait-for coverage exposes queue/credit edge-kind windows, blocked fabric packet windows, target lane/credit resource windows, and matching `sim.trace_replay.fabric.wait_for.*` stats from a top-level replay with fabric credit depth; CLI
trace-replay data-cache protocol smoke coverage exposes data-cache run,
protocol, scheduler, and profiled backing-DRAM resource stats from the top-level
replay command. CLI `run` data-cache and instruction-cache prefetch smoke
coverage exposes tagged next-line queue enqueue/issue/useful, useful-but-miss, useful-span-page, unused, late-hit fields, demand MSHR miss, fixed-point accuracy/coverage, and
gem5-style prefetcher alias stats during real RISC-V loads and fetches, plus identity-mapped page-crossing translation queue stats from real RISC-V loads and fetches.
Data-cache prefetch hierarchy smoke coverage also proves a translated page-crossing prefetch is serviced through MSI L1/L2/L3 with per-level `prefetch_fills` artifact JSON and `sim.data_cache.l2/l3.prefetch.fills` plus `sim.memory.resources.cache.data.l2/l3.prefetch.fills` stats.
**Next evidence:** Ruby-scale protocol-network execution, router and virtual-channel NoC arbitration beyond top-level shared-link/per-VN lane pressure and the optional router-stage slice, additional DRAM refresh modes beyond per-bank/all-bank policy and named full JEDEC timing-table coverage beyond custom timing rows, broader low-power state matrices, and instruction-cache/non-MSI hierarchy-level prefetch translation consumers.

### RISC-V SE, Workloads, and Linux Boot - 45% single-axis

**Score calculation:** 5 of 11 items have executable evidence, or 45% raw. The
bucket cap is single-axis because static newlib smokes are high-value but
tool-detected, and broad workload coverage is not present.

- [x] User-mode ecalls reach `RiscvSyscallTable`.
- [x] Startup stack, argv/envp/auxv, `brk`, `mmap`, in-place `mremap` slice, `mprotect`, mapped-page `mincore` present-vector reporting, `madvise` known-advice, mapped-range validation, anonymous `MADV_DONTNEED` page-zeroing, and file-backed `MADV_DONTNEED` backing restore, `msync` flags and mapped-range validation, `sync`/`fsync`/`fdatasync`/`sync_file_range`/`syncfs` validation, `readahead` regular-file hint validation, `mlock`/`munlock` `mmap`/`brk` range validation, `mlock2` range/flag validation, `mlockall` flag validation, single-node `mbind` mapped-range and nodemask validation slice, default and set/readback `get_mempolicy`/`set_mempolicy` mode/nodemask slices, stdio, file create/path `truncate`/`ftruncate`/read/write/append, `mknodat` regular-file creation, `unshare` single-process resource-flag slices, `setns` fd and namespace-type validation, positioned I/O, vector I/O, positional vector I/O, flags-zero `preadv2`/`pwritev2` positional vector I/O, path/fd xattr set/get/list/remove lifecycle, pipe `ioctl(FIONREAD)` unread-byte reporting, terminal `ioctl(TIOCGWINSZ/TCGETS)` reporting, pipe `fcntl(F_GETPIPE_SZ/F_SETPIPE_SZ)` capacity reporting/resizing and nonblocking full-pipe `EAGAIN`, guest-only `socket(AF_UNIX, SOCK_STREAM)` fd creation, unconnected read/write error boundaries, and abstract-name bind/listen/connect/accept4 stream handoff plus bound/listener/client/accepted `getsockname`/`getpeername` local/peer-name, `socklen` truncation/writeback, and accept/accept4 peer-address `sockaddr_un`/`addrlen` writeback slices, guest-only `socketpair` stream fd allocation/read/write/null-address sendto/recvfrom/sendmsg/recvmsg with common no-network flags/name/shutdown/options/poll/close and pipe-identity exclusion slices, `sendfile`, `splice`, `tee`, `vmsplice`, `statx`, `faccessat2`, `utimensat`, advisory `flock`, `fadvise64`, mode-zero `fallocate`, `fcntl` byte-range advisory lock no-conflict slices, legacy and typed `fcntl` signal-owner state plus signal-number state (`F_SETOWN`/`F_GETOWN`/`F_SETOWN_EX`/`F_GETOWN_EX`/`F_SETSIG`/`F_GETSIG`) with dup aliasing, `fchmodat2` permission updates and flag validation, and `fchownat`/`fchown` validation, `statfs`/`fstatfs`, `sysinfo`, `syslog` and admin syscall deterministic error boundaries including chroot, value-mode `riscv_hwprobe` base key reporting, `prctl` process-name set/get, no-new-privs, dumpable, and parent-death-signal state, `personality` query/set state, `ppoll` timeout/sigmask validation, finite-timeout expiration/writeback, indefinite blocking, and fd readiness slices, `pselect6` fd-set readiness slices, `eventfd2` counter/semaphore/nonblock/close-on-exec/poll slices, `epoll_create1`/`epoll_ctl`/`epoll_pwait` eventfd-readiness slices, `sched_setparam`, `sched_setscheduler`, `sched_getscheduler`, `sched_getparam`, `sched_get_priority_max/min`, `sched_rr_get_interval`, single-word `sched_setaffinity`/`sched_getaffinity`, single CPU/node `getcpu`, single-process `membarrier` slice, current-thread `rseq` register/unregister with guest struct initialization and validation, `set_tid_address` exit clear-child-tid write and futex wake behavior, `set_robust_list` head-size validation, current-thread `get_robust_list`, unknown-thread rejection, and guest-write fault handling, `waitid` no-child and siginfo writeback plus `wait4` option/status filtering slices, zero-duration `nanosleep` and `clock_nanosleep` validation, `gettimeofday`, `clock_getres`, `clock_settime` validation/denial, `clock_gettime64`, `CLOCK_TAI` `clock_gettime`, interval timer query/disarm state, `kill(..., 0)`, `tkill(..., 0)`, `tgkill(..., 0)`, and current-process `pidfd_open`, `pidfd_send_signal(..., 0)` fd-lifecycle existence checks, and `pidfd_getfd` current-process fd duplication, installed `SIG_IGN` and default-ignored nonzero `kill`/`tkill`/`tgkill` plus `rt_sigqueueinfo` non-delivery success, blocked ignored/default-ignored pending mask reporting plus disposition and unblock clearing, current-process scoped process-group/session `setpgid`/`getpgid`/`getsid`/`setsid` slices, `getresuid`/`getresgid` identity triples, current-credential `setresuid`/`setresgid` validation and identity updates, current-credential `setreuid`/`setregid` real/effective identity updates, current-credential `setuid`/`setgid` validation and effective-identity updates, file-system `setfsuid`/`setfsgid` return/update semantics, empty supplementary `getgroups` reporting and `setgroups` `EPERM`, `capget` zero-capability reporting/version probe and `capset` zero-capability/`EPERM` validation, stateful `setrlimit`, legacy `getrlimit` stack/data/NPROC/NOFILE limits, `prlimit64` stack/data/NPROC/NOFILE reporting and limit updates, NOFILE fd-allocation enforcement, plus unknown-pid rejection, `sigaltstack` query/set/disable state, basic `rt_sigaction`/`rt_sigprocmask`, `rt_sigpending` mask reporting, no-pending zero-timeout and blocked pending-signal `rt_sigtimedwait` with `siginfo_t` writeback, `rt_sigqueueinfo` target and guest `siginfo_t` validation with non-delivery `ENOSYS` records, futex mismatch, zero-timeout wait, wait-bitset zero and elapsed absolute timeout validation, wake-bitset count/bitset behavior, requeue wake/move behavior, compare-requeue mismatch handling, and `FUTEX_WAKE_OP` guest-word update plus conditional two-address wake behavior, `close_range` close and `CLOSE_RANGE_CLOEXEC` slices, `openat2` `open_how` parsing, mode validation, and close-on-exec slice, `umask` masking for `mkdirat` directories and `openat(O_CREAT)` regular files, time, cwd, `chdir`/`fchdir`, random, resource, and wait slices have tests.
  Current-process `getpriority`/`setpriority`, `ioprio_get`/`ioprio_set`, `pidfd_open`/`pidfd_send_signal(..., 0)`, and `pidfd_getfd` raw slices are covered by syscall table tests and static raw CLI/qemu smokes.
  `FUTEX_WAKE_OP` guest-word update and conditional two-address wake behavior are covered by syscall table tests and a static raw CLI/qemu smoke.
  `splice` pipe/file movement, `tee` pipe duplication, `vmsplice` guest-iovec pipe writes, `copy_file_range` guest-file copy, `mknodat` regular-file creation, `unshare` resource flags, `setns` validation, and `riscv_flush_icache` global/local no-op plus reserved-flag rejection are covered by syscall table tests and static raw CLI/qemu smokes.
  Signal pending coverage includes blocked ignored, default-ignored, and nonignored deliveries,
  with unsupported nonignored delivery recorded when the pending signal is unblocked.
  `signalfd4` coverage consumes blocked pending signals through a guest fd with
  read, nonblock, write-rejection, poll-readiness, and close-cleanup behavior.
  `inotify_init1`/`inotify_add_watch`/`inotify_rm_watch` coverage tracks guest-backed
  `IN_CREATE` from `openat(O_CREAT)` through read, poll, rm-watch, close, and CLI/qemu smoke.
  `waitid` no-child behavior, option validation, and `siginfo_t` writeback plus `wait4` Linux option-mask and no-child behavior are covered by syscall table tests and no-libc static CLI/qemu smokes.
  `capget`/`capset` zero-capability, version-probe, pid, null-pointer, and
  nonzero-set error paths are covered by syscall table tests and a no-libc
  static CLI/qemu smoke.
  `execve` missing-path/fault and flags-zero `execveat` missing-path slices have
  table coverage; unsupported `execveat` flag/fd-relative boundaries and existing
  guest exec paths stay typed unknown, while missing-path CLI smokes have no unknown diagnostics.
  `memfd_create` anonymous regular-file fd allocation and file-seal state are
  covered by syscall table tests plus a static raw CLI/qemu smoke; `setfsuid` and `setfsgid` return previous file-system identity values, update allowed
  credentials, reject unprivileged new identities without `errno`, and are
  covered by table tests plus a no-libc static CLI/qemu smoke.
  `socket(AF_UNIX, SOCK_STREAM)` creates deterministic guest-only fds with `SO_TYPE`, `getsockname`, close-on-exec/nonblock flags, unconnected `read`/`write` error boundaries, abstract-name bind/listen/connect/accept4 stream handoff, bound listener/client/accepted local/peer name queries, `socklen` truncation plus actual-length writeback, and accept/accept4 peer-address `sockaddr_un`/`addrlen` writeback; `socketpair(AF_UNIX, SOCK_STREAM)` has deterministic guest-only stream queues for bidirectional `read`/`write`, `getsockname`/`getpeername`, `shutdown`, selected `SOL_SOCKET` `getsockopt`/`setsockopt`, null-address `sendto`/`recvfrom`/`sendmsg`/`recvmsg` with `MSG_NOSIGNAL`/`MSG_DONTWAIT`, `poll`, close cleanup, and pipe-only fcntl exclusion, with syscall table coverage plus a static raw CLI/qemu smoke.
- [x] Unknown syscalls and unsupported signal-frame restore (`rt_sigreturn`) return `ENOSYS` with typed diagnostics, while known no-implementation entries (`lookup_dcookie`/`nfsservctl`) return `ENOSYS` without unknown records.
- [x] Static no-libc and newlib smoke binaries can be generated and compared with qemu when tools exist; shared CLI smoke support detects RISC-V tools from `PATH` and the local module toolchain path, tool-detected newlib directory-open, `O_NOCTTY`/`O_NOFOLLOW`, and path-backed `--riscv-se-file` writeback coverage run through the legacy `open` syscall and registered guest files, while `/proc/self/exe`, `/proc/self/cwd` after `chdir`, file-backed and pipe-backed `/proc/self/fd/<fd>` readlink coverage, `/proc/self/maps` open/read after raw `mmap`, `/proc/self/comm` open/read from the `PR_SET_NAME` process name, `/proc/self/status` open/read from modeled identity and process state, pipe roundtrip, unconnected `socket` creation, abstract listener connect/accept4 stream handoff with listener/client/accepted name-query and accept4 peer-address checks, socketpair name/shutdown/option plus null-address sendto/recvfrom/sendmsg/recvmsg flag roundtrip coverage run through direct ecalls, and a top-level `--riscv-se` m5 ROI/stat-hook smoke exposes no unknown syscalls plus host-action stats.
- [x] Linux at-family hard-link, `symlinkat`, `renameat`, `renameat2` flags=0, `RENAME_NOREPLACE`, and regular-file `RENAME_EXCHANGE`, unlink, legacy `mkdir`, `mkdirat`, `unlinkat` with `AT_REMOVEDIR`, legacy `time`, raw `clock_gettime64`, and registered-directory `getdents64` syscalls mutate or expose registered guest files/directories or tick-derived time and have raw smoke evidence. Current qemu-riscv64 reports `ENOSYS` for raw `renameat`, raw `clock_gettime64`, raw `get_mempolicy`/`set_mempolicy`, and raw legacy `mkdir`/`open`/`time`, so those smoke tests record the qemu boundary and verify rem6 registered-file, registered-directory, memory-policy, and deterministic-time behavior directly.
- [ ] Process/thread lifecycle, signals, permissions, and blocking wait/futex semantics are broad enough for distro-like programs.
- [ ] Broad Linux syscall table parity exists.
- [ ] Host filesystem policy matches the needed gem5 SE cases.
- [ ] SBI/OpenSBI-class firmware behavior exists.
- [ ] A real Linux kernel boots to userspace or clean shutdown.
- [ ] PARSEC or comparable workload programs run through ROI/stat hooks.

**Migrated:** RISC-V SE ecall path; startup stack and auxv setup; `brk`,
`mmap`, in-place `mremap` shrink and tail-free expansion, `mprotect`,
mapped-page `mincore` present-vector reporting, `madvise` known-advice,
mapped-range validation, anonymous `MADV_DONTNEED` page-zeroing, and
file-backed `MADV_DONTNEED` backing restore,
`msync` flags and mapped-range validation,
`sync` success plus fd-validating `fsync`/`fdatasync`/`sync_file_range`/`syncfs`,
regular-file `readahead` hint success and bad-fd validation with static raw
CLI/qemu smoke coverage,
`mlock`/`munlock` `mmap`/`brk` range validation, `mlock2` range/flag validation, `mlockall`
valid/invalid flag validation with direct ecall coverage, single-node `mbind`
mapped-range, nodemask, privilege, and unsupported-flag validation slice,
default and set/readback `get_mempolicy`/`set_mempolicy` mode/nodemask slices, stdio, guest-backed file create/path `truncate`/`ftruncate`/read/write/append/positioned read-write/readback, xattr lifecycle, and open
fd/link visibility, vector I/O, positional vector I/O, flags-zero `preadv2`/`pwritev2` positional vector I/O, guest-file `sendfile`,
`splice`, `tee`, `vmsplice`, mode-zero `fallocate`, and `copy_file_range`, `waitid`, `ppoll` timeout,
sigmask, expiration/writeback, indefinite blocking, and readiness behavior, `pselect6`,
`eventfd2` counter, semaphore, nonblock, close-on-exec, poll-readiness, and
actual ecall read-to-exit smoke slices, `epoll_create1`, `epoll_ctl`,
`epoll_pwait`, and `epoll_pwait2` eventfd-readiness slices,
`signalfd4` blocked pending-signal read, nonblock, write-rejection,
poll-readiness, and close-cleanup slices with
direct ecall smoke coverage, `inotify_init1`/`inotify_add_watch`/`inotify_rm_watch`
guest-backed `IN_CREATE` event production from `openat(O_CREAT)`, read,
nonblock, poll-readiness, ignored-watch cleanup, and direct ecall smoke
coverage, plus `timerfd` create, set/gettime, read,
nonblock, write-rejection, and poll-readiness slices with direct ecall smoke coverage,
`sched_setparam`, `sched_setscheduler`, `sched_getscheduler`, `sched_getparam`,
`sched_get_priority_max/min`, `sched_rr_get_interval`, single-word
`sched_setaffinity`/`sched_getaffinity`, current-process `getpriority`/`setpriority`
raw priority reporting, nice-state updates, `ioprio_get`/`ioprio_set` state, and
current-process `pidfd_open`/`pidfd_send_signal(..., 0)`/`pidfd_getfd` fd-lifecycle checks,
`statx` basic stat buffer writes,
`faccessat2` registered-path, missing-path, directory-fd relative path,
`AT_SYMLINK_NOFOLLOW`, flag-validation, and `AT_EMPTY_PATH` fd access checks,
`utimensat` registered-path, missing-path, `AT_EMPTY_PATH` fd,
`AT_SYMLINK_NOFOLLOW`, flag-validation, and timespec validation checks,
advisory `flock` fd and operation validation with direct ecall smoke coverage,
`fadvise64` fd/advice validation with static raw CLI/qemu smoke coverage,
pipe `fcntl(F_GETPIPE_SZ/F_SETPIPE_SZ)` capacity reporting/resizing with
nonblocking full-pipe `EAGAIN` behavior,
anonymous `memfd_create` backed by guest file descriptions and consumed by
existing read/write/seek/truncate/stat syscalls, including basic file-seal
state and enforcement,
`fcntl` byte-range advisory `F_GETLK` no-conflict reporting and
`F_SETLK`/`F_SETLKW` validation with guest `struct flock` ABI tests plus direct
ecall memory-write coverage, unknown-command `EINVAL` validation without
unknown-syscall record pollution, shared-description legacy and typed
`F_SETOWN`/`F_GETOWN`/`F_SETOWN_EX`/`F_GETOWN_EX` signal-owner state plus
`F_SETSIG`/`F_GETSIG` signal-number state with dup aliasing and
invalid-signal validation, and static raw CLI/qemu smokes,
advisory `fchownat`/`fchown` registered-path, missing-path, bad-flag, fd,
`AT_EMPTY_PATH`, bad-fd, no-op owner forms, non-no-op `EPERM`, and
`AT_SYMLINK_NOFOLLOW` plus normalized symlink-target checks,
`fchmodat2` registered-path permission updates, invalid-flag rejection without
unknown-syscall records, and raw chmod-family CLI/qemu smoke coverage,
`symlinkat` guest-link creation consumed by `readlinkat` and followed by
`faccessat` in a static raw CLI/qemu smoke,
`statfs`/`fstatfs` deterministic guest-namespace filesystem statistics,
`sysinfo` uptime and configured SE-visible memory-capacity writes, `uname`
`new_utsname` writes including the cleared domain-name field,
value-mode `riscv_hwprobe` base key reporting, `riscv_flush_icache`
global/local no-op and reserved-flag validation, `prctl` process-name set/get, no-new-privs, dumpable, and parent-death-signal state slices, `personality` query/set state, single CPU/node
`getcpu`, `membarrier` single-process command query, registration, and
private-expedited barrier slices, current-thread `rseq` register/unregister
with guest struct initialization and validation, `set_tid_address` exit clear-child-tid
write and futex wake behavior, `set_robust_list` head-size validation,
current-thread `get_robust_list`, unknown-thread rejection, and guest-write
fault handling, zero-duration `nanosleep` and
`clock_nanosleep` validation, `gettimeofday`, `clock_getres`, `clock_settime` validation/denial, `clock_gettime64`, `CLOCK_TAI` `clock_gettime`,
interval timer query/disarm state,
time, cwd, random, resource, wait, `close_range`, unknown syscall diagnostics,
unsupported `rt_sigreturn` `ENOSYS` records, `kill(..., 0)`
process-existence checks, `tkill(..., 0)` and `tgkill(..., 0)` current-thread
existence checks, installed `SIG_IGN` and default-ignored nonzero
`kill`/`tkill`/`tgkill` plus `rt_sigqueueinfo` non-delivery success without
unknown-syscall records, blocked ignored/default-ignored/nonignored pending
mask reporting plus disposition/unblock clearing and unsupported nonignored
delivery records, process-group/session
`setpgid`/`getpgid`/`getsid`/`setsid`
query, current-leader rejection, and nonleader transition slices,
`getresuid` and `getresgid` real/effective/saved identity triple writes,
current-credential `setresuid` and `setresgid` validation and identity updates,
current-credential `setuid` and `setgid` validation and effective-identity
updates, file-system `setfsuid` and `setfsgid` return/update semantics,
empty supplementary `getgroups` reporting and `setgroups` `EPERM`,
`capget` zero-capability reporting and version-probe behavior, `capset`
zero-capability acceptance plus `EPERM` nonzero/missing-pid rejection,
`sigaltstack` query/set/disable state, basic signal action/mask state for
`rt_sigaction` and `rt_sigprocmask`,
stateful `setrlimit`,
legacy `getrlimit` and `prlimit64` resource-limit reporting and updates
including stack/data/NPROC/NOFILE limits, NOFILE fd-allocation enforcement, and unknown-pid rejection, `rt_sigpending` mask reporting,
no-pending zero-timeout and blocked pending-signal `rt_sigtimedwait` with
`siginfo_t` writeback, `rt_sigsuspend` sigset validation
and blocking wait, `rt_sigqueueinfo` target and guest `siginfo_t` validation
with non-delivery `ENOSYS` records,
futex wait mismatch, zero-timeout wait, wait-bitset zero and elapsed absolute timeout validation,
wake-bitset count/bitset behavior,
requeue wake/move behavior, compare-requeue mismatch handling, and
`FUTEX_WAKE_OP` guest-word update plus conditional two-address wake behavior,
`umask` state applied to legacy `mkdir`/`mkdirat` directory modes and `openat(O_CREAT)`
regular-file modes, cwd-aware registered-path lookup, at-family
hard-link/`renameat`/`renameat2` flags=0, `RENAME_NOREPLACE`, and regular-file `RENAME_EXCHANGE`/unlink/legacy `mkdir`/`mkdirat`/`AT_REMOVEDIR`, and
registered-directory `getdents64` slices; supervisor SBI base read-only
identity/probe calls, SBI 2.0 spec-version reporting, conservative standard
extension probes plus legacy console putchar/getchar probes, top-level CLI
legacy console putchar, legacy console empty-getchar, and DBCN shared-memory write, top-level CLI TIME
`set_timer` deadline artifact/stat reporting plus boot-hart and HSM-started secondary-hart supervisor-handler interrupt delivery and retentive HSM wake by SBI TIME/STIP, library/workload
replay DBCN shared-memory read/write, direct write-byte debug-console output,
DBCN advertisement when functional guest memory I/O is configured, and workload replay Linux handoff DBCN read
input from a manifest `Input` resource into backing-store and data-cache-resident
guest memory using functional line replacement, without topology-routed firmware
memory transactions, minimal TIME `set_timer` STIP
scheduling and CLI S-mode interrupt-handler delivery on boot and HSM-started secondary harts, IPI
`send_ipi` scheduled completion events, SSIP pending-bit injection for
registered harts, and top-level CLI HSM-started secondary-hart S-mode handler
delivery with IPI artifact/stat records, with scheduler-error rejection before
partial target delivery,
standard SRST shutdown stop requests, invalid-param returns, top-level
shutdown, cold-reboot, warm-reboot, and system-failure reset artifact/stat
records, RFENCE probe reporting plus
top-level CLI RFENCE remote FENCE.I artifact/stat records plus remote
SFENCE.VMA completion artifact/stat records, and
remote SFENCE.VMA finite-range and ASID-scoped data TLB flushes through
translated execution, remote SFENCE.VMA scheduled completion events, explicit
RFENCE remote FENCE.I target fetch-stream reset on scheduled completion,
HFENCE.GVMA conservative whole modeled data TLB invalidation through translated
execution, HFENCE.VVMA range-scoped data TLB invalidation, HFENCE.VVMA.ASID
scoped data TLB invalidation that preserves other address spaces, and HFENCE
invalid hart mask, ASID-width, VMID-width, and range validation, plus HSM probe,
`hart_get_status`, `hart_start` secondary-hart `START_PENDING` reporting before
the scheduled entry event, secondary-hart release with supervisor entry state,
`satp=0`, `sstatus.SIE=0`, `a0=hartid`, and `a1=opaque`, top-level CLI HSM
`hart_start` secondary-hart release through `rem6 run --riscv-sbi --cores 2`
with HSM start artifact/stat records,
top-level CLI boot-hart already-started `hart_start` `-6` return, and
library `hart_start` already-started and suspended-target `-6` error boundaries, top-level and library current-hart `hart_stop` as a no-return stop that
does not write `sbiret`, `STOP_PENDING` reporting until the scheduled stop
event completes, with HSM stop artifact/stat and idle-stop records, retentive current-hart `hart_suspend` through
`SUSPEND_PENDING` until the scheduled suspend event reaches the CPU execution
gate, default non-retentive current-hart `hart_suspend` reporting
`RESUME_PENDING` until the scheduled resume event re-enters at `resume_addr`
with the same supervisor entry-state contract plus top-level HSM suspend
artifact/stat records, stale non-retentive resume events
ignored after intervening state changes, checkpoint roundtrip coverage for all
modeled hart run states, and retentive HSM wake by SBI IPI and TIME/STIP with top-level HSM wake artifact/stat records; workload replay Linux boot handoff starts the boot hart in
supervisor mode with `a0=hartid` and `a1=dtb_addr`, keeps secondary harts
stopped before HSM start, and routes replay SBI base ecalls through firmware;
typed unknown-syscall records, including raw `rt_sigreturn` CLI records instead of
silent success; `execve` missing-path/fault and flags-zero `execveat` missing-path
errors avoid unknown pollution; unsupported exec boundaries stay typed unknown;
direct ecall CLI coverage exists for missing-path cases; static smoke coverage; a static newlib
`fopen("w+")` create, write, seek, readback, and exit-code roundtrip; and a
static newlib program that reads `/proc/self/exe` through a direct `readlinkat`
ecall and compares the exit path with qemu; a static raw program that creates and enters `work`, reads `/proc/self/cwd` through `readlinkat`, and checks the `/work` suffix against qemu; static raw programs that read file-backed and pipe-backed `/proc/self/fd/<fd>` links through `readlinkat`, check guest-path, equal `pipe:[...]`, missing-fd `ENOENT`, and pipe-link trailing-slash `ENOTDIR` behavior, and compare the exit path with qemu; a static raw program
that opens and reads `/proc/self/maps` after raw `mmap` and checks the mapped
guest address against qemu; a static raw program that sets `PR_SET_NAME`, opens
and reads `/proc/self/comm`, and checks the formatted task name against qemu;
static raw `/proc/self/status` open/read coverage validates the modeled process
name and single-process identity lines through guest writes;
static raw `prctl`
programs that roundtrip the Linux process name, no-new-privs, and
parent-death-signal state through direct ecalls and compare the exit paths with qemu; a static raw `personality` program that
matches qemu query/set return values; a static raw `sigaltstack` program that
matches qemu query/set/disable state; static raw ignored-signal programs that
drive `rt_sigaction(SIG_IGN)`, default ignored `SIGCHLD`/`SIGURG`/`SIGWINCH`,
blocked default-ignored `rt_sigpending`/unblock behavior, and blocked
nonignored pending/unblock unsupported-delivery behavior through `rem6 run
--riscv-se`, with table-level pending disposition clearing coverage; direct
and vector I/O unit coverage for pipe
endpoints, positional vector I/O unit coverage for guest files, pipe `ioctl(FIONREAD)` unread-byte reporting, terminal `ioctl(TIOCGWINSZ/TCGETS)` reporting, deterministic guest-only unconnected `socket` creation/error-boundary coverage, abstract listener connect/accept4 stream handoff with listener/client/accepted name queries, `socklen` writeback, and accept/accept4 peer-address `sockaddr_un`/`addrlen` writeback, and `socketpair` stream queue coverage for bidirectional direct ecall I/O, name queries, shutdown, selected `SOL_SOCKET` options, null-address `sendto`/`recvfrom`/`sendmsg`/`recvmsg` with no-network flags, and poll readiness without pipe identity, plus a static newlib program that roundtrips bytes through `pipe2`,
`write`, `read`, and `close` direct ecalls; and a static
raw `preadv`/`pwritev` program that matches qemu for split-offset
positional scatter/gather file access without changing the file offset; a static
raw `preadv2`/`pwritev2` program that verifies flags-zero split-offset positional scatter/gather file access through `rem6 run --riscv-se` without unknown-syscall records; a static
raw `truncate` program that matches qemu for guest-file shrink, missing-path
`ENOENT`, reread contents, guest writes, and an empty unknown-syscall list; a static raw `mknodat` program that matches qemu for regular-file creation, duplicate `EEXIST`, open/read/write consumption, and an empty unknown-syscall list; a static
raw xattr program that matches qemu for user xattr set/get/list membership/remove
and an empty unknown-syscall list when the host temp filesystem supports xattrs; a static
newlib `open(".", O_DIRECTORY | O_CLOEXEC)` directory traversal smoke plus
`open` with `O_NOCTTY`, `O_NOFOLLOW`, and `O_SYNC` through the legacy `open`
syscall with newlib/libgloss flags; `openat2` table coverage for `open_how`
parsing, zero extension, size/fault errors, mode constraints, and unsupported
resolve rejection plus direct `openat2` ecall coverage for close-on-exec fd
state; a static raw `faccessat2` program that
matches qemu for existing and missing registered guest files; and a static raw
`utimensat` program that matches qemu for registered guest files, null times,
missing paths, bad flags, invalid nanoseconds, `AT_EMPTY_PATH`, and bad fds;
a static raw `sendfile` program that matches qemu for explicit input-offset
copying, output reads, and unchanged input fd offsets; a static raw `splice`
program that matches qemu for pipe/file movement, offset writeback, and file-to-file rejection; a static raw `tee` program that matches qemu for non-consuming pipe duplication; a static raw `vmsplice` program that matches qemu for guest-iovec pipe writes; a static raw `copy_file_range` program that matches qemu for explicit input/output offset
writeback, guest-file contents, unchanged fd offsets, bad flags, and bad fds;
a static raw `flock` program that matches qemu for advisory lock, unlock, and
bad-fd returns; a static raw `fcntl` byte-range advisory lock program that
matches qemu for no-conflict reporting, lock/unlock success, bad lock type, and
bad-fd returns; a static raw pipe-size `fcntl` program that matches qemu for
positive pipe-size queries, set/get consistency, non-pipe `EBADF`, and bad-fd
returns; a static raw `fcntl` owner program that matches qemu for
`F_SETOWN`/`F_GETOWN`, `F_SETOWN_EX`/`F_GETOWN_EX`, `F_SETSIG`/`F_GETSIG`, dup aliasing, invalid-signal validation, and close; a static raw
`fchownat`/`fchown` program that matches qemu
for no-op owner changes, missing paths, bad flags, fd-based ownership calls,
`AT_EMPTY_PATH`, and bad fds; a static raw chmod-family program that drives
`fchmod`, `fchmodat`, and `fchmodat2` through `rem6 run --riscv-se`; a static raw
`prlimit64` programs that set and requery `RLIMIT_STACK`, set `RLIMIT_NOFILE`, and observe fd-allocation `EMFILE` through `rem6 run --riscv-se`; static raw
`prctl` no-new-privs, dumpable, and parent-death-signal programs that match qemu; static raw `get_mempolicy`/`set_mempolicy` programs that record the qemu `ENOSYS` boundary and verify rem6 memory-policy writeback; a static raw
interval-timer program that matches qemu for zero-state query/disarm and Linux
argument errors; static raw `getpriority`/`setpriority` and `ioprio_get`/`ioprio_set`
programs that match qemu for deterministic current-process priority state and invalid target returns; and a no-libc static raw capability program that
matches qemu for `capget`/`capset` zero-capability and Linux error paths.

**Not migrated:** Broad Linux SE parity, process/thread lifecycle, signal
handler/frame delivery beyond ignored-action non-delivery, broad SBI
timer/IPI/reset power-state behavior, remaining HSM wake semantics beyond IPI/TIME retentive wake and the
`hart_start`, `hart_stop`, retentive `hart_suspend`, and default
non-retentive `hart_suspend` slices, VMID/G-stage/range-precise HFENCE.GVMA
completion coverage beyond conservative modeled data TLB invalidation, full
Linux boot, and real benchmark workloads.

**Evidence:** `RiscvSyscallTable::handle_with_guest_memory_io_at_tick`,
`RiscvSyscallEmulation::handle_pending_core_trap`, CLI static newlib tests,
shared RISC-V CLI tool detection in `cli_run::support`,
`riscv_syscall_getrusage`, `riscv_syscall::tests::wait4_tests`, `riscv_syscall::tests::exit_tests`,
`riscv_se_resource`, `riscv_se_chdir`, `riscv_se_poll`,
`riscv_se_links`, `riscv_se_mkdir`, `riscv_se_rename`, `riscv_se_getdents`,
`riscv_se_fd`, `riscv_se_open_flags`, `riscv_se_permissions`, `riscv_se_proc`,
`riscv_se_stdio`, `riscv_se_pvec`, `riscv_se_sync`, `riscv_se_tee`, `riscv_se_truncate`, `riscv_se_mknod`, `riscv_se_vmsplice`, `riscv_se_socket`, `riscv_se_xattr`, `riscv_syscall_mknod`, `riscv_syscall_pipe`, `riscv_syscall_socket`, `riscv_syscall_eventfd`, `riscv_syscall_ioctl`, `riscv_syscall_readv`,
`riscv_syscall_writev`, `riscv_syscall::tests::cpu_tests`,
`riscv_syscall::tests::fcntl_tests`,
`riscv_syscall::tests::memfd_tests`, `riscv_syscall::tests::namespace_tests`,
`riscv_syscall_emulation::user_ecall_fcntl_getlk_writes_no_conflict_lock_before_exit`,
`riscv_se_memfd`,
`riscv_se_fd::rem6_run_riscv_se_runs_static_raw_fcntl_locks_against_qemu`,
`riscv_se_fd::rem6_run_riscv_se_runs_static_raw_pipe_size_fcntl_against_qemu`,
`riscv_se_fd::rem6_run_riscv_se_runs_static_raw_fcntl_owner_against_qemu`, `riscv_se_setns`,
`riscv_syscall::tests::hwprobe_tests`, `riscv_syscall_riscv_flush_icache`, `riscv_se_riscv`,
`riscv_syscall_close_range`,
`riscv_syscall_openat2`,
`riscv_syscall_eventfd`, `riscv_syscall::tests::timerfd_tests`, `riscv_se_timerfd`,
`riscv_syscall_signalfd`,
`riscv_se_signalfd`,
`riscv_syscall_inotify`, `riscv_se_inotify`,
`riscv_syscall_epoll`, `riscv_se_epoll`,
`riscv_syscall_pselect`,
`riscv_syscall::tests::mlock_tests`, `riscv_se_mmap`,
`riscv_syscall::tests::memory_policy_tests`,
`riscv_syscall::tests::mmap_tests`,
`riscv_syscall::tests::msync_tests`,
`riscv_syscall_prlimit64`,
`riscv_syscall::tests::sync_tests`,
`riscv_syscall::tests::positioned_io_tests`,
`riscv_syscall::tests::mkdir_tests`,
`riscv_syscall::tests::truncate_tests`,
`riscv_syscall::tests::xattr_tests`,
`riscv_syscall::tests::stat_tests`,
`riscv_syscall::tests::statfs_tests`, `riscv_syscall::tests::admin_tests`,
`riscv_syscall::tests::process_tests`, `riscv_syscall::tests::unshare_tests`,
`riscv_syscall::tests::capability_tests`,
`riscv_se_permissions::rem6_run_riscv_se_runs_static_raw_capability_syscalls_against_qemu`, `riscv_se_admin`, `riscv_se_known_ni`, `riscv_se_ioprio`,
`riscv_se_process::rem6_run_riscv_se_runs_static_raw_futex_wake_op_against_qemu`,
`riscv_se_process::rem6_run_riscv_se_runs_static_raw_futex_wait_bitset_elapsed_timeout_against_qemu`,
`riscv_syscall::tests::robust_tests`,
`riscv_syscall::tests::scheduler_tests`,
`riscv_syscall::tests::signal_tests`,
`riscv_syscall::tests::futex_tests`,
`riscv_syscall::tests::nanosleep_tests`,
`riscv_syscall::tests::sysinfo_tests`, `riscv_syscall::tests::syslog_tests`,
`riscv_syscall::tests::utsname_tests`,
`riscv_se_statx`,
`riscv_se_sysinfo`, `riscv_se_syslog`,
`riscv_se_time`,
`riscv_sbi_base`, `riscv_sbi_debug_console`, `riscv_sbi_firmware`,
`riscv_system_translation`, `riscv_se_process`, `riscv_se_unshare`,
`riscv_se_signal`,
`riscv_sbi::tests::remote_hfence_gvma_rejects_missing_target_before_scheduling_flush`,
`riscv_sbi::tests::remote_hfence_gvma_rejects_invalid_range_before_scheduling_flush`,
`riscv_sbi::tests::remote_hfence_gvma_flushes_target_tlb_when_completion_event_runs`,
`riscv_sbi::tests::remote_hfence_gvma_vmid_conservatively_flushes_all_modeled_tlb_entries`,
`riscv_sbi::tests::remote_hfence_gvma_vmid_rejects_invalid_vmid_before_scheduling_flush`,
`riscv_sbi::tests::remote_hfence_vvma_asid_rejects_invalid_asid_before_scheduling_flush`,
`riscv_sbi::tests::remote_hfence_vvma_range_flushes_overlapping_pages_only`,
`riscv_sbi::tests::remote_hfence_vvma_asid_preserves_other_address_spaces`,
`riscv_sbi::tests::remote_fence_i_resets_target_fetch_stream_when_completion_event_runs`,
`riscv_sbi::tests::handle_pending_core_trap_schedules_ipi_completion_event`,
`riscv_sbi::tests::parallel_handle_pending_core_trap_schedules_ipi_completion_event`,
`riscv_sbi::tests::send_ipi_scheduler_error_leaves_no_partial_target_events`, `riscv_sbi::rem6_run_riscv_sbi_starts_secondary_hart_through_hsm`, `riscv_sbi::rem6_run_riscv_sbi_secondary_hart_receives_ipi_interrupt_after_hsm_start`, `riscv_sbi::rem6_run_riscv_sbi_ipi_wakes_retentive_hart_suspend`, `riscv_sbi::rem6_run_riscv_sbi_timer_wakes_retentive_hart_suspend`, `riscv_sbi::rem6_run_riscv_sbi_remote_fence_i_records_rfence_request`, `riscv_sbi::rfence_completion::rem6_run_riscv_sbi_remote_sfence_vma_records_completion`, `riscv_sbi::rem6_run_riscv_sbi_shutdown_reset_records_shutdown_stat`, `riscv_sbi::rem6_run_riscv_sbi_reboot_resets_record_reboot_type_stats`, `riscv_sbi::rem6_run_riscv_sbi_system_reset_records_reset_request`, `riscv_sbi::rem6_run_riscv_sbi_hart_start_reports_already_available_for_boot_hart`,
`workload_replay_linux_boot::workload_replay_linux_boot_handoff_enters_supervisor_sbi`,
`riscv_sbi_remote_hfence_gvma_flushes_translated_data_tlb`,
`RiscvLinuxBootHandoffConfig`, and RISC-V DTB handoff tests.

**Next evidence:** Broader static libc program coverage, broader SBI completion
coverage, then a real Linux boot smoke.

### Devices and Platforms - 50% single-axis

**Score calculation:** 5 of 10 items have executable evidence, or 50% raw. The
bucket cap is single-axis because real Linux driver interaction, host
networking, non-RISC-V boards, and coherent DMA timing are not complete.

- [x] MMIO bus, UART, PL011, CLINT, PLIC, RTC, timers, interrupt routes, platform readfile checkpoint attachment, and CLI readfile MMIO have tests.
- [x] PCI, VirtIO MMIO/PCI, block, console, RNG, storage image, SimpleDisk, and IDE slices exist.
- [x] Ethernet switching and SINIC MMIO/PCI/DMA/checkpoint paths exist.
- [x] RISC-V DTB and initrd handoff APIs exist.
- [x] Device and topology validation reject malformed ownership and routes.
- [ ] PS/2, QEMU bridge, and broader board-specific devices exist.
- [ ] Real TUN/TAP or host networking adapters exist.
- [ ] Non-RISC-V board trees and boot paths exist.
- [ ] Devices are validated by real Linux boot and driver interaction.
- [ ] Device timing and DMA paths are complete across cache/coherence/DRAM.

**Migrated:** Broad typed device slices and topology validation.

**Not migrated:** Several board devices, real host networking adapters,
non-RISC-V boards, and Linux-driver validation.

**Evidence:** `PlatformBuilder`, `PlatformRiscvDeviceTreeConfig`, topology
tests, PCI/VirtIO/storage/network checkpoint tests, CLINT/PLIC/UART tests,
`readfile_device` platform-MMIO tests, `riscv_topology_readfile`, and CLI
`run --readfile` guest-load tests covering host-file and resource payloads.

**Next evidence:** Board-level Linux boot with console, timer, storage, and
network evidence.

### Stats, Probes, Debug, Host Actions, and Checkpointing - 59% single-axis

**Score calculation:** 23 of 26 items have executable evidence, or 88% raw,
capped to 59% by the single-axis bucket. The bucket cap is single-axis because
probe, debug, power, and checkpoint evidence is not yet complete across broad
O3 engine ownership, cache/DRAM runtime state, and calibrated power paths.

- [x] Hierarchical stats, reset/dump history, and CLI stats artifacts exist.
- [x] Probe registry plus real RISC-V retired-instruction, retired-PC target,
  and data-access producers exist.
- [x] m5 exit/fail/stats/checkpoint/work markers reach typed host actions,
  with direct `rem6 run` JSON evidence for repeated RISC-V work-marker
  payloads, store-backed and DRAM-backed checkpoint memory component/chunk
  metadata with checkpoint chunk checksum changes, m5 hypercall
  selector/argument/response metadata, m5 return-value `sum` execution, periodic m5
  stats dump/reset and checkpoint tick repeats, and
  `m5_switch_cpu` as a top-level injected `switchcpu` command plus host stop,
  and run-level `sim.host_actions.*` stats including SE ROI/stat hooks.
- [x] Decode-first checkpoint capture/restore exists across scheduler, memory, device, storage, VirtIO, timer, interrupt, RISC-V hart run-state, platform, workload, and manifest owners.
- [x] GDB remote packet/session parsing and RISC-V integer/PC register paths exist.
- [x] GDB RV64D floating-point, including XML-aligned `fflags`/`frm`/`fcsr` and placeholder numbering, advertised RV64 CSR target descriptions including supervisor `sscratch`, supervisor environment `senvcfg`, translation `satp`, interrupt aliases `sie`/`sip`, machine identity `mhartid`/`mvendorid`/`marchid`/`mimpid`, machine ISA `misa`, counter `cycle`/`time`/`instret`, machine-counter aliases `mcycle`/`minstret`, packed PMP config `pmpcfg0`/`pmpcfg2`, and PMP address registers `pmpaddr0` through `pmpaddr15`, and RV64 machine status, interrupt, trap, identity, ISA, environment-config, counter, and PMP CSR register-cache paths exist, with `cycle`/`mcycle`, `time`, and `instret`/`minstret` remaining GDB-visible across real GDB single-step execution and PMP0 through PMP15 writes updating the same `RiscvCore` PMP state consumed by CPU access checks.
- [x] GDB RV64 vector fixed-point and vector-configuration CSR target descriptions and register-cache read/write paths exist for `vxsat`, `vxrm`, `vcsr`, `vl`, `vtype`, and `vlenb`.
- [x] GDB RV32D floating-point plus RV32 CSR target descriptions and register-cache read/write paths exist for FP registers, `fflags`/`frm`/`fcsr`, supervisor, machine, interrupt, translation, vector fixed-point CSRs, RV32 `mstatush`, and XLEN-mapped vector-configuration CSRs; RV32 CSR target descriptions also advertise packed PMP config CSRs `pmpcfg0` through `pmpcfg3`, with `pmpcfg1`/`pmpcfg3` core packet-handler writes updating live PMP entries 4 through 7 and 12 through 15.
- [x] GDB RV32/RV64 vector data register target descriptions and register-cache read/write paths exist for `v0` through `v31`.
- [x] Power and thermal models plus external power-analysis exports exist.
- [x] Host actions and guest events are typed and checkpoint-aware.
- [x] First-class histogram stats have registry snapshots, deltas, resets,
  CLI JSON/text bucket output, and real data-access stack-distance producer
  output.
- [x] RISC-V in-order pipeline timing state and fetch-steering branch predictor
  state are captured and restored by core checkpoints.
- [x] GDB packet byte streams drive typed step/resume and break/watch control state in memory-backed sessions.
- [x] CLI `run --gdb-listen` applies pre-execution RISC-V register writes,
  memory writes, and software breakpoint packets before the normal run consumes
  the mutated core and memory state, including vector data registers, vector-configuration CSRs, and supervisor/machine trap CSRs; it also serves RV32 target descriptions from RV32 ELF metadata and accepts RV32-only packed PMP config CSR read/write packets.
- [x] CLI `run --gdb-listen` single-step packets drive one real RISC-V
  instruction, return a GDB stop reply, and leave the stepped state visible to
  subsequent register reads and detach-time execution.
- [x] CLI `run --gdb-listen` continue and `vCont;c` packets drive the normal
  RISC-V run driver to a guest stop, return a GDB stop reply, and feed the
  completed run into CLI execution summaries, including cache-protocol runtime
  stats when instruction and data caches are selected.
- [x] CLI `run --gdb-listen` write watchpoints accept `Z2`/`z2`, stop after a
  real RISC-V store data access completes with `T05watch:<addr>;`, and leave later guest instructions for detach-time execution.
- [x] CLI `run --gdb-listen` read/access watchpoints accept `Z3`/`z3` and
  `Z4`/`z4`, stop after real RISC-V load or store data accesses complete with `T05rwatch:<addr>;`/`T05awatch:<addr>;`, and leave later guest instructions for detach-time execution.
- [x] CLI `run --gdb-listen` hardware breakpoints accept `Z1`/`z1`, stop before
  a matching RISC-V instruction retires, and continue after removal without
  patching guest memory.
- [x] CLI `run --execute --stats-format json --debug-flags Exec,Fetch,Data,Cache,Dram,Fabric,Memory,Syscall` emits
  deterministic instruction, fetch, data-access, cache hierarchy, DRAM hierarchy including low-power target/port/bank state counters, fabric lane/hop, memory-transport with response-latency ticks, and RISC-V SE syscall trace records from real RISC-V execution paths.
- [x] Stricter gem5 text-stat compatibility exists.
- [ ] Cache/bank/fabric/DRAM hierarchy counters are complete.
- [ ] Broader GDB CSR register-cache coverage exists.
- [ ] Power and thermal models are calibrated against real component activity.
- [x] O3 pending-state checkpoints exist.

**Migrated:** Structured stats, real RISC-V probe producers, checkpoint banks,
m5ops, host actions, run-level `sim.host_actions.*` stats, GDB packet/session parsing, RISC-V integer/PC register
paths, RV64D floating-point target descriptions including XML-aligned
`fflags`/`frm`/`fcsr` and placeholder numbering,
advertised RV64 CSR target descriptions including supervisor `sscratch`, supervisor environment `senvcfg`, translation `satp`, interrupt aliases `sie`/`sip`, machine identity
`mhartid`/`mvendorid`/`marchid`/`mimpid`, machine ISA `misa`, counter `cycle`/`time`/`instret`, packed PMP config `pmpcfg0`/`pmpcfg2`, and PMP address registers `pmpaddr0` through `pmpaddr15`,
RV64 machine status, interrupt, trap, identity, ISA, environment-config, counter, and PMP CSR register-cache paths, including independent
GDB-visible `cycle` and `time` state across CSR writes and single-step execution and PMP0 through PMP15 writes reflected in the live `RiscvCore` PMP table, and
RV64 vector fixed-point and vector-configuration CSR register-cache paths for
`vxsat`, `vxrm`, `vcsr`, `vl`, `vtype`, and `vlenb`, plus RV32D floating-point target descriptions and register-cache
read/write paths for FP registers and `fflags`/`frm`/`fcsr`, plus RV32 CSR
target descriptions and register-cache read/write paths for supervisor,
machine, interrupt, translation, vector fixed-point CSRs, and XLEN-mapped vector-configuration CSRs, plus RV32/RV64
vector data register target descriptions and register-cache read/write paths
for `v0` through `v31`,
RISC-V software breakpoint
patch/restore through the
system GDB memory handler, gem5-style final-tick, committed-instruction, and
sim-frequency stat aliases, text output with gem5-compatible no-leading-blank
headers plus `simOps`, deterministic `simSeconds`, single-core
`system.cpu.numInsts`/`system.cpu.numOps`/`system.cpu.commitStats0.numInsts`/`system.cpu.commitStats0.numOps`/`system.cpu.numCycles` aliases plus text-output derived `system.cpu.ipc`/`system.cpu.cpi` and `system.cpu.commitStats0.ipc`/`system.cpu.commitStats0.cpi`, and multicore
`system.cpuN.numInsts`/`system.cpuN.numOps`/`system.cpuN.commitStats0.numInsts`/`system.cpuN.commitStats0.numOps`/`system.cpuN.numCycles` aliases plus text-output derived `system.cpuN.ipc`/`system.cpuN.cpi` and `system.cpuN.commitStats0.ipc`/`system.cpuN.commitStats0.cpi`,
target/port/bank-level DRAM runtime resource counters, RISC-V in-order pipeline
per-stage current/max in-flight and occupied-cycle stats, fetch-steering branch predictor checkpoint
capture/restore including live fetch-ahead pending speculation, and GShare, BiMode, Tournament, and multiperspective perceptron predictor checkpoint capture/restore, GDB byte-stream packet handling, debug
execution-control state for packet-stream step/resume and break/watch requests,
CLI run aggregate memory-resource activity, active-resource, fabric active-link/active-hop resource counters, instruction/data cache-resource hierarchy, prefetch queue/translation-queue, DRAM byte, split row-hit, and read-ready-latency, refresh/low-power stats, custom LPDDR low-power timing, gem5-style `system.mem_ctrl` request/burst/byte aliases plus `system.mem_ctrl.dram` burst/per-bank burst/row-hit/byte and text-output `avgRdBW`/`avgWrBW`/`readRowHitRate`/`writeRowHitRate`/`pageHitRate` aliases, and target/port/bank DRAM low-power hierarchy stats derived from executed cache, transport, and DRAM paths,
CLI `run --gdb-listen` attach-before-execute socket handoff for RISC-V
stop-reason, register, memory, and detach packets, pre-execution register and
memory writes, and pre-execution software breakpoints consumed by the following
run, plus pre-detach single-step execution through the normal RISC-V run driver,
runtime GDB continue and `vCont;c` execution through the normal RISC-V run
driver with completed-run CLI summary handoff, including instruction-cache and
data-cache runtime summaries from cache-backed GDB sessions,
runtime GDB data-watchpoint execution through real RISC-V load and store
data-access completions with reason/address stop replies and detach-time continuation of later guest
instructions, runtime GDB hardware breakpoints that stop before the matching
RISC-V instruction retires without patching guest memory,
top-level `rem6 run --execute --stats-format json --debug-flags Exec,Fetch,Data,Cache,Dram,Fabric,Memory,Syscall`
execution-trace JSON emitted from real RISC-V instruction execution events with
CPU/retirement-scoped retired-record and instruction-byte stats, fetch-trace JSON emitted from real
RISC-V fetch issue records with CPU/endpoint-scoped issue-record and byte stats, and data-trace JSON emitted from
completed RISC-V data-access records with CPU/kind-scoped load/store/atomic count and byte stats, cache-trace JSON emitted from executed instruction/data L1/L2/L3 memory-resource cache summaries with activity, CPU-response, directory-decision, backing-DRAM, bank, and hierarchy-scoped prefetch queue/translation/ratio stats,
DRAM-trace JSON
emitted from real top-level DRAM target, port, and bank activity with target
access/read/write, port-command, low-power state/residency/exit-latency fields, hierarchy-scoped target/port/bank activity, row, refresh, command, and ready-latency stats, target/port turnaround stats, and bank byte stats, fabric-trace JSON
emitted from real top-level fabric lane and hop activity with aggregate and hierarchy-scoped lane/hop transfer, byte, flit, and timing stats, memory-transport trace
JSON emitted from real fetch/data `MemoryTrace` events with top-level debug trace
record-count, channel-scoped route/endpoint/request-agent, event-kind, response-status, and response-latency tick stats, plus data-trace load/store/atomic classification stats,
RISC-V SE syscall-trace JSON/outcome plus hierarchy-scoped syscall-number, call-site, CPU, and argument stats emitted from real syscall trap handling, and power-trace JSON plus target-hierarchy/state/residency/temperature/microwatt/microwatt-tick stats emitted from executed-run activity including shared L2/L3 cache and fabric/NoC power records derived from memory-resource summaries,
top-level debug trace record/category/active-flag/payload-byte roll-up stats emitted from the same debug summary,
target-description-aligned register-cache seeding, top-level trace-replay
fabric-route activity and flit counters, top-level run fabric resource roll-up, top-level trace-replay aggregate resource
activity counters, top-level trace-replay data-cache run and protocol
counters, top-level trace-replay data-cache scheduler resource and backing DRAM
target/port/bank, row, command, latency, and QoS counters,
top-level `rem6 run` retired-instruction probe summaries and CLI-configured
retired-PC target counter summaries,
RISC-V core checkpoint capture/restore of O3 runtime payloads covering
ROB entries, LSQ entries, rename maps, pending scopes, and deferred writeback completions, and custom plus library-level and
`rem6 run --power-output`, `rem6 trace-replay --power-output`, and `rem6 gpu-run --power-output` McPAT-shaped and DSENT-shaped power-analysis exports, including shared L2/L3 cache plus run, trace-replay, and GPU fabric/NoC power records from executed hierarchy, replay, and GPU route activity.

**Not migrated:** Complete gem5 text-stat parity, full debug execution control,
remaining broad CSR GDB register-cache coverage, broader debug-run stat parity beyond current trace, classification, and selected aggregate counters, runtime resource counters,
broader runtime-calibrated power/thermal, and running O3 execution beyond checkpointed runtime state.

**Evidence:** `StatsRegistry`, `ProbeRegistry`, `RiscvInstructionStats`,
`RiscvDataAccessStats`, `SystemActionExecutor`, `GdbRemoteSession`,
`cli_run::pc_count_probes::rem6_run_emits_riscv_pc_count_probe_stats`,
checkpoint tests including RISC-V hart run-state, in-order pipeline restore,
fetch-steering branch predictor restore with live fetch-ahead pending
speculation, and GShare, BiMode, Tournament, and multiperspective perceptron predictor checkpoint restore,
RISC-V core O3 runtime checkpoint restore tests plus O3 pending/writeback payload tests, GDB byte-stream,
RV64D FP, FP CSR, RV64 CSR register-cache including supervisor `sscratch` and `senvcfg`,
translation `satp`, interrupt aliases `sie`/`sip`, counter
`cycle`/`time`/`instret`, and machine-counter aliases `mcycle`/`minstret`, RV64 machine CSR register-cache including `mscratch`
write/readback plus machine identity `mhartid`/`mvendorid`/`marchid`/`mimpid`
and machine ISA `misa` readback through guest CSR execution and top-level GDB,
RV32 CSR target/register-cache coverage for `mstatush` plus packed `pmpcfg0` through `pmpcfg3`
with core packet-handler writes reflected in the live `RiscvCore` PMP table,
PMP CSR target/register-cache coverage for packed `pmpcfg0`/`pmpcfg2` and `pmpaddr0` through `pmpaddr15` in both the core packet handler and top-level `rem6 run --gdb-listen`,
top-level RV32 ELF-selected `rem6 run --gdb-listen` target-description service plus `mstatush` and `pmpcfg1`/`pmpcfg3` register read/write packets,
live `cycle`/`time`/`instret` plus `mcycle`/`minstret` reads after top-level GDB single-step execution,
and independent counter writes consumed by subsequent GDB single-step
execution,
RV64 vector fixed-point CSR register-cache tests
covering `vxsat`, `vxrm`, and `vcsr`, RV32D FP target/register-cache tests,
RV32 CSR target and register-cache tests, RV32/RV64 vector target and
register-cache tests, and control-state tests,
`gdb_remote_packet` execution-control tests,
CLI `run --gdb-listen` smoke tests for RISC-V pre-execution state and
pre-execution writes consumed by the subsequent run, including RV64 vector
data register target-description and register-cache smoke coverage and RV32
target-description plus packed PMP config smoke coverage,
CLI `run --gdb-listen`
single-step, continue, `vCont;c`, hardware-breakpoint, and data-watchpoint
stop-reply smoke tests, trap CSR pre-execution smoke tests, GDB cache-runtime smoke tests,
power-analysis export tests, CLI power-output tests, system GDB software
breakpoint patch/restore tests, CLI data-access probe tests, CLI `Branch`, `Pipeline`, `Exec`, `Fetch`, `Data`, `Cache`, `Dram`, `Fabric`, `Memory`, `Syscall`, and `Power` debug-flag tests, including `Branch` in-order branch-prediction JSON with `sim.debug.branch_trace.records`, `sim.debug.branch_trace.cpu.cpu0.records`, and `sim.debug.branch_trace.mispredictions`, `Pipeline` in-order cycle-plan JSON with `sim.debug.pipeline_trace.records`, `sim.debug.pipeline_trace.branch_prediction_flushed`, and `sim.debug.pipeline_trace.cpu.cpu0.records`, `Exec` CPU/retirement-scoped retired-record and byte stats, `Fetch` CPU/endpoint-scoped issue-record and byte stats, `Data` CPU/kind-scoped load/store/atomic classification and byte stats, `Cache` instruction/data L1/L2/L3 activity, CPU-response, directory-decision, backing-DRAM, bank, and hierarchy-scoped prefetch queue/translation/ratio stats, `Memory` channel-scoped route/endpoint/request-agent, event-kind, response-status, and response-latency tick stats, `Syscall` return/exit/blocked outcome plus hierarchy-scoped syscall-number, call-site, CPU, and argument stats, `Power` target-hierarchy/state/residency/temperature/microwatt/microwatt-tick stats, `Memory` debug trace record-count stats, `Dram` debug trace record-count plus hierarchy-scoped target/port/bank access, row, refresh, command, latency, low-power state/residency/exit-latency, and byte stats, `Fabric` debug trace record-count plus hierarchy-scoped lane/hop transfer, byte, flit, queue-delay, credit-delay, max-credit-delay, and occupancy stats, and histogram registry/output tests.
CLI `Exec,Fetch` debug-flag evidence covers top-level debug trace record, category, active-flag, and payload-byte roll-up stats against the emitted debug JSON.
CLI retired-instruction probe tests expose InstTracker-backed event counts and
tracked-instruction counts from executed RISC-V instructions.
The CLI data-access probe tests include stack-distance histogram stats emitted
from executed RISC-V loads. CLI DRAM-backed execution tests include
target/port/bank-level DRAM resource counters emitted from executed RISC-V
instruction fetches. CLI text stats include gem5-style final-tick,
committed-instruction, sim-ops, sim-seconds, sim-frequency, single-core CPU
`numInsts`/`numOps`/`commitStats0.numInsts`/`commitStats0.numOps`/`numCycles` aliases plus CPU and `commitStats0` `ipc`/`cpi` rates, single-core no-prefetch L1 I-cache/D-cache demand and overall hit/miss/access/miss-rate aliases plus `demandMshrHits`/`overallMshrHits`/`demandMshrMisses`/`overallMshrMisses`/`demandMshrMissRate`/`overallMshrMissRate`, configured single-core L1 I-cache/D-cache prefetcher `pfIdentified`/`pfIssued`/`pfUseful`/`pfUsefulButMiss`/`pfUnused`/`pfHitInCache`/`pfHitInMSHR`/`pfHitInWB`/`pfLate`/`pfSpanPage`/`pfUsefulSpanPage`/`pfInCache`/`demandMshrMisses` aliases plus derived `accuracy`/`coverage` aliases and artifact/stat `accuracy_ppm`/`coverage_ppm` fields, shared L2/L3 overall hit/miss/access/miss-rate plus MSHR aliases backed by executed instruction/data L2/L3 cache-bank summaries, multicore CPU `numInsts`/`numOps`/`commitStats0.numInsts`/`commitStats0.numOps`/`numCycles` aliases plus CPU and `commitStats0` `ipc`/`cpi` rates, and in-order text aliases including `system.cpu.pipeline.inOrder.stallCycles`, `system.cpu.pipeline.inOrder.fetchWaitCycles`, `system.cpu.pipeline.inOrder.resourceBlocked`, `system.cpu.pipeline.inOrder.orderingBlocked`, `system.cpu.pipeline.inOrder.flushCycles`, `system.cpu.pipeline.inOrder.branchPredictionFlushes`, `system.cpu.pipeline.inOrder.branchPredictionFlushCycles`, `system.cpu.pipeline.inOrder.stage.fetch1.resourceBlocked`, `system.cpu.pipeline.inOrder.stage.fetch1.resourceBlockedCycles`, `system.cpu.pipeline.inOrder.stage.decode.branchPredictionFlushed`, multicore `system.cpu0.pipeline.inOrder.stallCycles`, `system.cpu0.pipeline.inOrder.resourceBlocked`, `system.cpu1.pipeline.inOrder.stallCycles`, and `system.cpu1.pipeline.inOrder.resourceBlocked`
plus memory-controller `readReqs`/`writeReqs`/`readBursts`/`writeBursts`/`bytesReadSys`/`bytesWrittenSys`/`avgRdBWSys`/`avgWrBWSys` and DRAM/NVM-interface `readBursts`/`writeBursts`/`perBankRdBursts`/`perBankWrBursts`/`readRowHits`/`writeRowHits`/`dramBytesRead`/`dramBytesWritten`/NVM-profile `nvmBytesRead`/`nvmBytesWritten`/`totMemAccLat` plus text-output `avgRdBW`/`avgWrBW`/`avgMemAccLat`/`readRowHitRate`/`writeRowHitRate`/`pageHitRate`
aliases emitted from an executed RISC-V run. CLI
trace-replay fabric-route tests include
nonzero fabric active-lane, active-virtual-network, transfer, byte, and flit stats
from an executed packet replay, request/response virtual-network and
credit-depth config fields from CLI and TOML entry points, aggregate
resource-activity and active-resource stats, while occupancy, queue-delay,
credit-delay, contention, per-virtual-network and per-lane flit counts, and per-hop activity details are emitted
from the same workload parallel summary. Top-level RISC-V cache/fabric/DRAM runs and trace-replay fabric routes now also emit per-link/per-VN/per-hop transfer, byte, flit, occupied-tick, queue-delay, max-queue-delay, and credit-delay stats matched to the same emitted hop activity records. CLI trace-replay data-cache protocol
tests include data-cache run, protocol, CPU-response, directory-decision, and
scheduler resource counters emitted from the executed workload parallel
summary.

**Next evidence:** Broader gem5 text-stat compatibility, remaining
cache/bank/fabric runtime resource counters, remaining CSR register-cache
families, and ROB/LSQ-backed O3 execution beyond checkpointed state.

### Configuration, Resources, Suites, GPU, and Accelerators - 59% single-axis

**Score calculation:** 16 of 21 items have executable evidence, or 76% raw,
capped to 59% by the single-axis bucket. The bucket cap is single-axis because GPU memory behavior now has narrow
top-level cache/DRAM/fabric micro-runs but is not representative, manifest and suite
acquisition have top-level local-artifact paths, narrow run and trace-replay
resource handoffs exist, and benchmark orchestration has only narrow top-level
mixed-command multi-run slices including GPU and accelerator micro-runs.

- [x] CLI `run`, `gups`, `trace-replay`, `gpu-run`, and `accelerator-run` plus TOML configuration have tests; repository `gups`, `gpu-run`, `accelerator-run`, and multi-run example configs run through the top-level CLI without recompilation, TOML-driven `run` output layouts create nested artifact directories for run JSON, stats JSON, and power analysis output, `gups` emits traffic profile summaries from the executed controller, TOML-driven `gpu-run` executes recorded GPU global memory traffic through cache/DRAM and writes activity-derived power-analysis and NoMali-compatible adapter output, and TOML-driven `accelerator-run` emits child JSON/stats artifacts.
- [x] Workload manifests, resource identity, disk-image construction records, and suite planning exist.
- [x] CLI workload-resource acquisition consumes a resource executor for manifest required artifacts.
- [x] CLI workload-resource acquisition consumes a resource executor for suite required artifacts.
- [x] CLI `run` consumes a manifest-acquired kernel resource at runtime.
- [x] CLI `run` consumes unique or selected suite-acquired kernel resources at runtime.
- [x] CLI `run` consumes acquired input/initrd resources as guest readfile/load-blob payloads and RISC-V SE stdin/guest-file inputs, including `suite-resource:<workload>/<resource>` selectors for same-name suite resources, RISC-V SE input-source artifact summaries, and runtime rejection of non-initrd load-blob resources.
- [x] CLI `trace-replay` consumes manifest and suite-acquired trace resources at runtime, including TOML/CLI `suite-resource:<workload>/<resource>` selectors for same-name suite traces and runtime rejection of non-input trace resources.
- [x] GPU and accelerator command routing, DMA routes, topology validation, replay evidence, and top-level `accelerator-run` NPU/GPU command execution with stats exist.
- [x] Dispatch plans and execution summaries expose typed parallel evidence.
- [ ] gem5-style ergonomic experiment definitions cover broad full-system sweeps.
- [ ] External workload-resource acquisition executors cover host, network, archive, and broader artifact kinds.
- [x] GPU ISA-level execution exists.
- [x] GPU queued workgroups expose compute-unit assignment and coalesced memory access records from scalar ISA memory intents.
- [x] CLI `gpu-run` routes recorded coalesced scalar GPU global memory requests through direct memory, explicit fabric, or MSI data-cache and DRAM-backed runtime stats after GPU workgroup completion, with bank-level cache counters, tagged next-line data-cache prefetch counters, fabric aggregate/per-VN/per-link/per-hop stats, shared-link queue contention, and link/lane/hop activity exposed in the top-level artifact.
- [x] CLI `gpu-run` reports per-compute-unit workgroup completion, queue-wait, busy-cycle, activity-window, and coalesced global-memory read/write stats from real scheduled completions and recorded memory accesses; GPU library tests cover queued-workgroup restore for the same queue-wait summary boundary.
- [x] CLI `multi-run` sequentially orchestrates multiple real `run --config`, `gups --config`, `gpu-run --config`, `accelerator-run --config`, and `trace-replay --config` entries from TOML and command-qualified `--run` entries through the top-level binary, preserves and reports child-configured output and extra artifacts, rolls up run-child checkpoint action counts and accelerator command/completion counts into suite JSON/stats output, and emits aggregate JSON/stats artifacts plus explicit failure summaries when configured to continue after child errors.
- [x] CLI `trace-replay` accepts planned direct-memory and data-cache line-synchronized, same-tick trace-overlap-guarded host checkpoint and restore events from TOML or flags, executes them through `RiscvWorkloadReplay` with a nonempty memory checkpoint payload captured and restored, exposes trace-replay checkpoint and restore counts plus checkpoint payload metadata in JSON and host-action counts in stats, and lets `multi-run` roll up trace-replay child checkpoint and restore counts.
- [ ] GPU CU scheduling, memory coalescing, and cache/DRAM interactions are representative.
- [ ] Multi-run simulator orchestration and artifact compatibility are complete.
- [ ] PARSEC or comparable workload suites run end to end.

**Migrated:** Typed configuration, manifests, suite dispatch, resource identity
and suite-level required-resource declarations, top-level manifest resource
acquisition through local artifacts and the in-memory executor, runtime-consumable
resolved resource construction for acquired manifest payloads, suite replay-plan
resource acquisition through the same top-level local-artifact executor path,
top-level `preloaded` acquisition locators for already materialized local artifacts,
top-level host-file acquisition through config-relative host paths and the same
executor validation flow, top-level uncompressed tar archive entry acquisition
including USTAR prefix member paths and gzip-compressed tar archive entry acquisition, standalone gzip artifact
acquisition, plus stored and deflated ZIP archive entry acquisition through the same executor validation flow,
deterministic generated zero-fill artifact acquisition through the same executor
validation flow with tagged locators for multiple generated resources,
top-level HTTP `remote-uri` acquisition requiring an
`artifact_digest` content SHA-256 and validating response bodies for basic and
chunked transfer responses plus absolute-URL, absolute-path, and relative HTTP redirects through the same
executor validation flow for explicit pre-simulation `resource-acquire`,
runtime `run` and `trace-replay` resource handoffs rejecting `remote-uri`
resources before artifact reads to keep simulation entry points network-free,
top-level `rem6 run` handoff of a manifest-acquired
kernel resource into the normal ELF load and execution path, top-level
`rem6 run` handoff of unique and `suite-resource:<workload>/<resource>`
selected suite-acquired kernel resources into the normal ELF load and execution
path, top-level `rem6 run` handoff of unique and
`suite-resource:<workload>/<resource>` suite-acquired input/initrd resources
into the guest readfile and load-blob paths, manifest-acquired and suite-selected
RISC-V SE stdin bytes, and suite-selected RISC-V SE guest-file bytes consumed by guest syscalls,
including generated zero-fill initrd payloads validated by memory dump, top-level
`trace-replay` handoff of an acquired
trace manifest resource into `RiscvWorkloadReplay`, top-level `trace-replay`
handoff of unique and TOML/CLI `suite-resource:<workload>/<resource>`
suite-acquired trace resources into `RiscvWorkloadReplay` with the selected
trace resource exposed in the JSON artifact, GPU/accelerator
shells, DMA routing, top-level `accelerator-run` NPU/GPU command dispatch and completion stats with TOML artifact output, and a
minimal GPU scalar ISA program execution path with completion, queued-workgroup
snapshot evidence, visible compute-unit assignment, coalesced memory access
records, top-level `gpu-run` cache/DRAM micro-run evidence routing recorded
coalesced global memory requests through direct memory or MSI data-cache and
DRAM-backed runtime stats after GPU workgroup completion, plus top-level
`gpu-run` explicit fabric route execution with request/response virtual-network
selection, shared-link queue contention, credit-depth validation, aggregate/per-VN/per-link/per-hop fabric stats, and link/lane/hop activity,
TOML-driven `gpu-run` cache/DRAM/fabric execution and config-value validation,
top-level
`gpu-run` per-CU completion, busy-cycle, activity-window, and coalesced
global-memory read/write stats derived from scheduled workgroup completions and
recorded memory accesses, checked-in GUPS, GPU, accelerator, and multi-run
example TOML files that run through the top-level CLI without recompilation,
top-level `rem6 run --config` creation of nested run, stats, and power-analysis artifact directories from TOML-relative paths, top-level `rem6 trace-replay` planned direct-memory and data-cache line-synchronized, same-tick trace-overlap-guarded host checkpoint and restore execution through `RiscvWorkloadReplay` with nonempty memory checkpoint payload capture/restore, JSON checkpoint counts and payload metadata, and stats checkpoint counts, top-level `rem6 multi-run` sequential orchestration of real `run --config`, `gups --config`, `gpu-run --config`, `accelerator-run --config`, and `trace-replay --config` TOML and command-qualified `--run` entries with child-configured output and power/GPU extra-artifact path summaries, run-child, accelerator-child, and trace-replay-child count roll-up, aggregate JSON/stats artifacts, and explicit child-failure summaries, and top-level GUPS traffic profile JSON/stats output.

**Not migrated:** Full gem5 stdlib ergonomics, host/network/archive resource
acquisition beyond the host-file, tar-entry, gzip-tar-entry, stored/deflated
ZIP-entry, standalone gzip payload, preloaded-local, generated zero-fill, basic
HTTP, chunked HTTP, and broader HTTP redirect slices, HTTPS, cache/policy controls, broader archive and artifact kinds, broad
runtime handoff of acquired suite resources beyond the
unique run-kernel, `suite-resource:<workload>/<resource>` readfile/load-blob,
RISC-V SE stdin/guest-file, and selected trace-resource replay slices, broad GPU
ISA semantics, representative GPU cache/DRAM interaction, complete multi-run artifact compatibility, full cache-controller checkpoint payload/state preservation beyond trace-replay cache-line synchronization and same-tick trace-overlap guards, broader checkpoint restore orchestration beyond the current trace-replay host-event slices, and broad benchmark orchestration.

**Evidence:** `Rem6RunConfig`, `run_config`, `WorkloadManifest`,
`WorkloadResource`, `WorkloadSuiteReplayPlan`,
`WorkloadInMemoryResourceAcquisitionExecutor`, `WorkloadResolvedResources`,
`rem6 resource-acquire` CLI tests, `rem6 run` manifest and suite
resource-config kernel handoff tests including preloaded-local acquisition locator handoff, `rem6 run` suite resource-config
readfile, load-blob, selected RISC-V SE stdin source-summary, and RISC-V SE guest-file handoff tests
including `suite-resource:<workload>/<resource>` same-name suite resource
selection and generated zero-fill load-blob memory dump coverage,
`rem6 resource-acquire` remote-uri
content-digest, content-address requirement, chunked-transfer, absolute/relative HTTP redirect, USTAR-prefix tar
archive-entry, ZIP archive-entry, standalone gzip artifact, and generated zero-fill artifact tests, `rem6
run` remote-uri runtime rejection tests, `rem6 trace-replay` manifest and suite
resource-config handoff tests including TOML and CLI
`suite-resource:<workload>/<resource>` same-name trace selection, `rem6
trace-replay` remote-uri runtime rejection
tests, suite tests, resource acquisition executor tests, repository GUPS, GPU,
accelerator, and multi-run example-config CLI tests, `rem6 run --config` nested multi-artifact output
layout CLI test, `rem6 trace-replay` planned direct-memory and data-cache line-synchronized, same-tick trace-overlap-guarded host checkpoint and restore TOML plus flag CLI tests with nonempty memory checkpoint payload metadata, `rem6 multi-run` mixed TOML and command-qualified run/GUPS/GPU/accelerator/trace-replay aggregate artifact/stat, run-child checkpoint, accelerator-child command/completion, and trace-replay-child checkpoint action count roll-up, child-output and power/GPU extra-artifact path, and continue-on-failure CLI tests, `rem6 gups` profile-summary CLI tests, `rem6 gpu-run --config`
cache/DRAM execution and config validation tests, GPU and
accelerator topology tests, GPU compute tests covering scalar ISA execution,
coalesced memory records, and snapshot restore of queued ISA programs, `rem6 accelerator-run` CLI/TOML smoke coverage plus multi-run child coverage for scheduled NPU inference and GPU kernel commands with command, completion, trace-event, scheduler, child-output, and child-stats evidence, and
`rem6 gpu-run` CLI smoke coverage with TOML and flag-driven
direct-memory store/dump evidence plus MSI cache-run, bank-level cache counter,
DRAM read, explicit fabric route link/lane/hop activity and per-link/per-hop fabric stats, and
transport stats from recorded coalesced GPU global memory requests, plus per-CU
completion, queue-wait, busy-cycle, activity-window, and coalesced global-memory
read/write JSON/stat evidence from the top-level scheduled GPU run.

**Next evidence:** Broader suite-level workload replay beyond selected run-kernel,
`suite-resource:<workload>/<resource>` readfile/load-blob, RISC-V SE guest-file,
and selected trace-resource handoffs, network-backed workload acquisition,
broader archive and artifact kinds, data-driven
full-system workload declarations, and representative GPU CU scheduling plus
cache/DRAM interactions.

## Test Migration Ledger

This table is a crosswalk from gem5 test anchors to rem6 owners. Its estimates
are compact row-level status markers, not component scores. `Row score` entries
use the same percentage and bucket vocabulary as component scores, but the
checklist-backed component sections above define the auditable percentages.

| gem5 test anchor | rem6 owner | Row score | Migrated boundary | Next evidence |
| --- | --- | --- | --- | --- |
| `tests/gem5/arm_boot_tests` | future ARM ISA crate, `rem6-platform` | 0% open | ARM device slices exist, but this row requires Arm ISA boot. | Add Arm ISA, board handoff, device tree, and kernel boot tests. |
| `tests/gem5/asmtest` | ISA crates, `rem6` CLI | 50% single-axis | RISC-V no-libc, ISA unit tests, and CPU fetch-stream tests cover selected instruction, ecall, scalar FP directed integer-to-float, `fmul.s` exact-product and `fmul.d` normal finite exact-product plus directed-overflow and directed-underflow subnormal rounding, and `fdiv.s`/`fdiv.d` finite-quotient directed-rounding paths, RVV vector configuration, unmasked integer `vadd.vv` LMUL=1 plus m2, `vadd.vx`, `vadd.vi`, masked integer `vadd.vv`/`vadd.vx`/`vadd.vi`, `vsub.vv` LMUL=1 plus m2, `vsub.vx`, `vrsub.vx`/`vrsub.vi`, bitwise `vand`/`vor`/`vxor` vv/vx/vi, shift `vsll`/`vsrl`/`vsra` vv/vx/vi, min/max `vminu`/`vmin`/`vmaxu`/`vmax` vv/vx, multiply `vmul`/`vmulhu`/`vmulhsu`/`vmulh` vv/vx, divide/remainder `vdivu`/`vdiv`/`vremu`/`vrem` vv/vx, integer carry/borrow `vadc`/`vmadc`/`vsbc`/`vmsbc` vv/vx plus add-immediate forms with reserved-encoding, representative ISA, and CPU fetch-stream/fetch-ahead slices, single-width integer multiply-add `vmadd`/`vnmsub`/`vmacc`/`vnmsac` vv/vx decode plus representative masked/unmasked ISA and unmasked CPU fetch-stream slices, integer reductions `vredsum`/`vredand`/`vredor`/`vredxor`/`vredminu`/`vredmin`/`vredmaxu`/`vredmax` plus widening reductions `vwredsumu`/`vwredsum` and base widening add/subtract `vwaddu`/`vwadd`/`vwsubu`/`vwsub` vv/vx/wv/wx decode plus representative masked/unmasked ISA and unmasked CPU fetch-stream slices, widening multiply `vwmulu`/`vwmulsu`/`vwmul` vv/vx and widening multiply-add `vwmaccu`/`vwmacc`/`vwmaccsu` plus vx-only `vwmaccus` decode plus representative masked/unmasked ISA and unmasked CPU fetch-stream slices, equality mask compare `vmseq`/`vmsne` vv/vx/vi, ordered mask compare `vmsltu`/`vmslt`/`vmsleu`/`vmsgtu`/`vmsgt` supported vv/vx/vi, unmasked slide `vslideup`/`vslidedown`/`vslide1up`/`vslide1down`, gather `vrgather`, mask reductions `vcpop.m`/`vfirst.m`, mask prefixes `vmsbf.m`/`vmsof.m`/`vmsif.m`, mask indexes `viota.m`/`vid.v`, merge/move/compress `vmerge`/`vmv.v`/`vmv.x.s`/`vmv.s.x`/`vmv<nr>r.v`/`vcompress.vm`, zero/sign extension `vzext`/`vsext` masked/unmasked decode plus `vf2` hart execution and unmasked slide/gather, mask reductions, mask prefixes, mask indexes, plus `vzext.vf2` CPU fetch-stream execution, unmasked fixed-point saturating `vsaddu`/`vsadd`/`vssubu`/`vssub` vv/vx plus `vsaddu`/`vsadd` vi forms, unmasked fixed-point averaging `vaaddu`/`vaadd`/`vasubu`/`vasub` vv/vx forms, unmasked fixed-point signed fractional multiply `vsmul` vv/vx forms, unmasked fixed-point scaling shifts `vssrl`/`vssra` vv/vx/vi, fixed-point narrowing shifts `vnsrl.wv`/`vnsra.wv`/`vnsrl.wx`/`vnsra.wx`/`vnsrl.wi`/`vnsra.wi` and narrow clip `vnclipu.wv`/`vnclip.wv`/`vnclipu.wx`/`vnclip.wx`/`vnclipu.wi`/`vnclip.wi`, `vxrm`/`vxsat`/`vcsr`, mask logical `.mm`, and floating-point `vfadd`, `vfsub`, `vfmin`, `vfmax`, `vfmul`, and `vfdiv` vv/vf forms plus `vfrsub.vf`, `vfrdiv.vf`, `vfmacc`, `vfnmacc`, `vfmsac`, and `vfnmsac` vv/vf exact finite SEW=32 lane slices, E64 `vfadd.vv/vf`, `vfsub.vv/vf`, `vfrsub.vf`, `vfmul.vv/vf`, `vfdiv.vv/vf`, and `vfrdiv.vf` exact finite CPU fetch-stream slices, E64 `vfmin.vv/vf` and `vfmax.vv/vf` signed-zero and NaN CPU fetch-stream slices, `vfclass.v` SEW=32 classification slices, `vfmv.v.f` SEW=32 scalar splat slices, `vfmerge.vfm` SEW=32 masked scalar merge slice, `vfmv.s.f` SEW=32 scalar-to-lane-zero slice, `vfmv.f.s` SEW=32 lane-zero-to-scalar slice, integer-to-float `vfcvt.f.xu.v` and `vfcvt.f.x.v` plus float-to-integer `vfcvt.xu.f.v`, `vfcvt.x.f.v`, `vfcvt.rtz.xu.f.v`, and `vfcvt.rtz.x.f.v` dynamic/directed SEW=32 slices with NX accrual, E64 `vfcvt.f.xu.v`, `vfcvt.f.x.v`, `vfcvt.xu.f.v`, `vfcvt.x.f.v`, `vfcvt.rtz.xu.f.v`, and `vfcvt.rtz.x.f.v` CPU fetch-stream slices, FP mask compare `vmfeq`, `vmfne`, `vmflt`, and `vmfle` vv/vf SEW=32 mask slices, `vfmin`, `vfsqrt.v`, and FP compare invalid-flag accrual, `vfsqrt.v` non-exact finite rejection, and `vfsgnj`, `vfsgnjn`, and `vfsgnjx` vv/vf sign-bit slices with reserved-`frm` and NaN-boxing trap coverage. | Split RV32/RV64 and extension families with architectural-state comparison. |
| `tests/gem5/checkpoint_tests` | `rem6-checkpoint`, subsystem checkpoint banks | 65% representative | Scheduler, memory, devices, storage, VirtIO, timer, interrupt, RISC-V started/stopped/suspended hart run-state, RISC-V in-order pipeline state, RISC-V fetch-steering branch predictor state including live fetch-ahead pending speculation, platform, workload, manifest, and O3 pending-state plus ROB/LSQ/rename runtime checkpoints exist. | Add running O3 execution and non-quiescent restore evidence. |
| `tests/gem5/chi_protocol` | `rem6-coherence`, protocol crates, `rem6-cache` | 40% single-axis | CHI-like line, controller, bank, dirty peer sourcing, reservation, and Evict-hazard tests exist. | Add Ruby-scale CHI transactions, topology networks, directory races, and workload checks. |
| `tests/gem5/chi_tlm_tests` | `rem6-proto`, future adapter crates, `rem6-coherence` | 19% scoped | A library-level co-simulation boundary can register TLM endpoints, validate transaction shape, hand off events, and checkpoint clean adapter state in self-tests. | Add runtime TLM bridge tests with coherence traffic. |
| `tests/gem5/config_output_files` | `rem6` CLI, `rem6-workload` | 45% single-axis | CLI output paths, stats-output paths, JSON artifacts, text stats output, TOML-driven nested run/stats/power artifact directory creation, TOML-driven GPU run config tests, and TOML-driven GPU power-output plus NoMali adapter artifact creation exist. | Add config-driven file layouts for full-system manifests and broader multi-artifact workloads. |
| `tests/gem5/cpu_tests` | `rem6-cpu`, `rem6-system` | 30% unit-slice | Atomic RISC-V execution, frontend slices, retired predictor training, direct completed-fetch overlap in in-order timing, bounded normal-driver straight-line and conditional-branch fetch-ahead including compressed straight-line fetch-ahead through the normal parallel-cluster path, explicit `--riscv-branch-lookahead 2` two-deep conditional-branch fetch-ahead rollback through `rem6 run`, CLI-selected `gshare`, `bimode`, `tage-sc-l`, and `multiperspective-perceptron` branch-predictor fetch steering changing deterministic timing or branch-steering counters, direct RISC-V `JAL` and `JALR` target fetch-ahead preserved through retire in driver tests plus direct `JAL` CLI run coverage, pending GShare, BiMode, Tournament, TAGE-SC-L, and multiperspective-perceptron fetch-ahead history for older conditional-branch and direct-jump speculation, selected GShare/BiMode/Tournament mutable speculative-history rollback counters through top-level nested-branch runs, top-level retired Tournament predictor local/global selection stats and live predictor-family counter stats, pending-fetch retire overlap for older completed straight-line fetches, issued fetch-ahead occupancy in in-order timing before response completion, normal parallel-cluster pending-fetch resource-stall stats including top-level per-stage blocked instruction and cycle counters, retained non-retire in-order cycle history consumed by top-level stats, branch speculation history repair/commit with younger-speculation removal stats, completed younger fetch squash, per-retired-instruction in-order stage timing stats, top-level configured stage width changing executed max in-flight occupancy, top-level per-stage width, in-flight, occupied-cycle, blocked-cycle, and branch-redirect flush stats, top-level fetch/data wait stats, top-level in-order cycle-plan advance/block/flush stats, top-level scalar integer M-extension, vector integer multiply and multiply-accumulate including fixed-point signed fractional multiply and widening variants, divide, remainder, fixed-point scaling shifts, narrowing shifts, narrow clip, and integer/widening reductions, scalar FP arithmetic, compare, conversion, and misc, and decoded vector FP add/sub/min/max, mask compare, conversion/move/merge, sign/class, multiply/FMA/divide/sqrt execute-stage latency stats, top-level branch redirect/misprediction/branch-prediction-flush stats, and O3 policies exist. | Add broader in-order stalls/squashes and ROB/LSQ-backed O3 execution tests. |
| `tests/gem5/dram_lowp` | `rem6-dram`, `rem6-power` | 40% single-axis | DRAM/NVM profile counters, low-power constants, top-level LPDDR routed-request active-powerdown, precharge-powerdown, self-refresh, custom timing, terminal all-bank residency, and repeated-access multi-bank exit-latency stats are surfaced. | Add broader executable low-power state-transition matrices across profiles, banks, and power states. |
| `tests/gem5/example_configs`, `tests/gem5/learning_gem5` | `rem6` CLI, `rem6-platform`, `rem6-workload` | 40% single-axis | CLI and TOML tests cover several execution, trace-replay, GPU, accelerator, and multi-run micro-suite paths, including checked-in GUPS, GPU, accelerator, and multi-run example configs that run without recompilation. | Add broader example suites spanning run, trace replay, resources, and full-system handoff. |
| `tests/gem5/fdp_tests` | `rem6-cache` | 45% single-axis | Fetch-directed prefetcher state, errors, and cache-local queue/translation counters have cache tests. | Add FDP execution through cache-bank and CPU/frontend consumers. |
| `tests/gem5/fs` | `rem6-platform`, `rem6-system`, device crates | 15% scoped | Generic device and handoff slices exist, but the gem5 row is mainly full-system boot. | Add full-system Linux boot with SBI, console, storage, network, timer, and shutdown evidence. |
| `tests/gem5/gem5_resources` | `rem6-workload`, `rem6` CLI | 58% single-axis | Resource declarations, identity, provenance, disk-image construction records, library-level in-memory acquisition executor records, manifest/suite-level `rem6 resource-acquire` execution with local-artifact, host-file, uncompressed/gzip tar-entry including USTAR prefix paths, standalone gzip payload, stored/deflated ZIP-entry, generated zero-fill artifact, and content-checked basic, chunked, and redirected HTTP remote inputs, explicit pre-simulation HTTP `remote-uri` acquisition through `rem6 resource-acquire`, while `run` and `trace-replay` reject `remote-uri` resources before artifact reads so simulation/replay entry points remain network-free, plus manifest run-kernel, unique/selected-suite run-kernel, `suite-resource:<workload>/<resource>` suite readfile/load-blob payloads, manifest and selected-suite RISC-V SE stdin handoff, suite-selected RISC-V SE guest-file handoff, generated zero-fill load-blob memory dump coverage, and manifest plus unique/selected-suite trace-replay resource-config handoff through TOML or CLI selector exist. | Add broader network-backed, broader archive/artifact acquisition, and suite runtime handoff beyond the current selector-based slices. |
| `tests/gem5/gpu` | `rem6-gpu`, `rem6-accelerator`, `rem6-transport`, `rem6` CLI | 40% single-axis | GPU and accelerator topology, command, DMA route, scalar ISA, CU assignment, coalesced memory-record tests, flag/TOML top-level `gpu-run` recorded-memory cache/DRAM/fabric micro-runs with aggregate/per-VN/per-link/per-hop fabric counters, shared-link queue contention, and link/lane/hop activity, tagged next-line data-cache prefetch counters from GPU global loads, per-CU queue-wait and coalesced read/write stats, top-level `accelerator-run` scheduled NPU/GPU command/completion stats including TOML and multi-run child output, and top-level NoMali-compatible adapter artifacts with GPU/MMU/job-slot command-table, MMU AS0 config-register, ignored-command, IRQ-clear, shader/tiler/L2 low/high plus L3 zero-present power-domain, register-fault, job-slot next-register/start-next, and job/MMU interrupt-block PIO evidence exist. | Add representative CU scheduling, broader cache/DRAM interactions, and register-level GPU device modeling. |
| `tests/gem5/insttest_se` | future SPARC owner, ISA crates | 10% scoped | Current RISC-V evidence belongs under `asmtest`; this gem5 anchor is SPARC SE focused. | Add SPARC or explicitly retire the row as out of scope. |
| `tests/gem5/kvm_fork_tests`, `tests/gem5/kvm_switch_tests` | `rem6-system`, future host adapters | 10% scoped | Host-assisted takeover admission rejects unsafe switch shapes. | Add explicit fast-forward adapter and KVM-like switch/fork tests. |
| `tests/gem5/m5_util`, `tests/test-progs/m5-exit` | `rem6-isa-riscv`, `rem6-system`, `rem6-workload` | 50% single-axis | RISC-V m5 exit, fail, stats, checkpoint, hypercall, and work markers reach typed host actions; direct `rem6 run` JSON records repeated work-marker payloads, stats reset/dump id and tick metadata, periodic stats dump/reset and checkpoint tick repeats, nonempty RISC-V core checkpoint metadata including branch-predictor state plus store-backed and DRAM-backed memory checkpoint component/chunk metadata, checkpoint chunk checksum changes after guest stores, hypercall selector/argument/response metadata, m5 `sum` return-value execution, and `--riscv-se` ROI/stat hook counts in `sim.host_actions.*`. | Add broader payload breadth, other ISA entries, and calibrated clock-domain behavior. |
| `tests/gem5/m5threads_test_atomic` | `rem6-isa-riscv`, `rem6-cpu`, `rem6-coherence` | 40% single-axis | RISC-V LR/SC and AMO plus coherence reservation invalidation tests exist. | Add multi-threaded SE or full-system atomic tests through shared memory. |
| `tests/gem5/se_mode` | `rem6-system`, `rem6` CLI | 50% single-axis | RISC-V SE startup, ecalls, static newlib smokes including `fopen("w+")` create/write/readback and path-backed `--riscv-se-file` host writeback, `/proc/self/exe`, `/proc/self/cwd` after `chdir`, and file-backed plus pipe-backed `/proc/self/fd/<fd>` readlink through direct `readlinkat` ecall, `/proc/self/maps` open/read after raw `mmap`, `/proc/self/comm` open/read from `PR_SET_NAME`, `/proc/self/status` open/read from modeled identity state, pipe roundtrip through direct `pipe2`/`write`/`read`/`close` ecalls, unconnected `socket(AF_UNIX, SOCK_STREAM)` creation/error-boundary behavior, abstract-name bind/listen/connect/accept4 stream handoff plus listener/client/accepted name queries, `socklen` writeback, and accept/accept4 peer-address `sockaddr_un`/`addrlen` writeback, `socketpair(AF_UNIX, SOCK_STREAM)` guest-only bidirectional stream read/write, name/shutdown/selected `SOL_SOCKET` option behavior, null-address sendto/recvfrom/sendmsg/recvmsg with no-network flags, poll/close through direct ecalls without pipe identity, raw terminal `ioctl(TIOCGWINSZ/TCGETS)`, pipe-size `fcntl(F_GETPIPE_SZ/F_SETPIPE_SZ)`, `open` directory traversal with `O_DIRECTORY` and `O_CLOEXEC`, and `open` regular-file access with `O_NOCTTY` and `O_NOFOLLOW` through legacy `open` with newlib/libgloss flags, selected syscalls including `sendfile`, `splice`, `tee`, `vmsplice`, `copy_file_range`, `memfd_create`, `mknodat`, `unshare`, `setns`, mode-zero `fallocate`, `readahead`, `statx`, `faccessat2`, `fchmodat2`, `utimensat`, advisory `flock`, `fadvise64`, `fcntl` byte-range advisory lock no-conflict slices, legacy and typed `fcntl` signal-owner state plus signal-number state (`F_SETOWN`/`F_GETOWN`/`F_SETOWN_EX`/`F_GETOWN_EX`/`F_SETSIG`/`F_GETSIG`), `symlinkat` creation consumed by `readlinkat` and followed by `faccessat`, `fchownat`/`fchown`, path/fd xattr set/get/list/remove, `statfs`/`fstatfs`, `sysinfo`, `syslog` and admin syscall deterministic error boundaries including chroot, `uname` `new_utsname`, value-mode `riscv_hwprobe`, `riscv_flush_icache`, `prctl` process-name set/get, no-new-privs, dumpable, and parent-death-signal state, `personality` query/set state, `getresuid`/`getresgid` identity triples, current-credential `setresuid`/`setresgid` validation and identity updates, current-credential `setreuid`/`setregid` real/effective identity updates, current-credential `setuid`/`setgid` validation and effective-identity updates, file-system `setfsuid`/`setfsgid` return/update semantics, empty supplementary `getgroups` reporting and `setgroups` `EPERM`, `capget`/`capset` zero-capability and error-path slices, `sigaltstack` query/set/disable state, `ppoll` timeout/sigmask validation, finite-timeout expiration, timeout writeback, and fd-readiness static smoke, `pselect6` fd-set readiness, `eventfd2` counter/semaphore/nonblock/close-on-exec/poll behavior plus direct ecall readback, `epoll_create1`/`epoll_ctl`/`epoll_pwait` eventfd readiness plus direct ecall smoke, `inotify_init1`/`inotify_add_watch`/`inotify_rm_watch` guest-backed `IN_CREATE` event readiness/readback plus direct ecall smoke, `timerfd_create`/`timerfd_settime`/`timerfd_gettime` tick-derived expiration, read, nonblocking, write-rejection, and poll-readiness slices plus direct ecall smoke, in-place `mremap`, `mprotect`, `madvise` known-advice, mapped-range validation, anonymous `MADV_DONTNEED` page-zeroing, and file-backed `MADV_DONTNEED` backing restore, `msync` flags and mapped-range validation, `sync`/`fsync`/`fdatasync`/`sync_file_range`/`syncfs`, `mlock`/`munlock` `mmap`/`brk` range validation, `mlock2` range/flag validation, `mlockall` flag validation, single-node `mbind` mapped-range/nodemask validation slice, default and set/readback `get_mempolicy`/`set_mempolicy` mode/nodemask slices, `truncate`, `ftruncate`, `pread64`, `pwrite64`, `preadv`, `pwritev`, flags-zero `preadv2`, flags-zero `pwritev2`, `sched_setparam`, `sched_setscheduler`, `sched_getscheduler`, `sched_getparam`, `sched_get_priority_max/min`, `sched_rr_get_interval`, `ioprio_get`/`ioprio_set`, single-word `sched_setaffinity`/`sched_getaffinity`, single CPU/node `getcpu`, single-process `membarrier` slice, current-thread `rseq` register/unregister with guest struct initialization and validation, `set_tid_address` exit clear-child-tid write and futex wake behavior, `waitid` no-child behavior and `siginfo_t` writeback, `wait4` Linux option-mask no-child behavior, zero-duration `nanosleep` and `clock_nanosleep` validation, `gettimeofday`, legacy `time=1062`, `clock_gettime64`, `clock_getres`, `clock_settime` validation/denial, `CLOCK_TAI` `clock_gettime`, interval timer query/disarm state, `kill(..., 0)`, `tkill(..., 0)`, `tgkill(..., 0)`, and current-process `pidfd_open`/`pidfd_send_signal(..., 0)` existence checks, current-process scoped process-group/session `setpgid`/`getpgid`/`getsid`/`setsid` slices, stateful `setrlimit`, legacy `getrlimit` stack/data/NPROC/NOFILE limits, `prlimit64` stack/data/NPROC/NOFILE reporting and limit updates, NOFILE fd-allocation enforcement, plus unknown-pid rejection, basic `rt_sigaction`/`rt_sigprocmask`, `rt_sigpending` mask reporting, no-pending zero-timeout and blocked pending-signal `rt_sigtimedwait` with `siginfo_t` writeback, `rt_sigqueueinfo` target and guest `siginfo_t` validation with non-delivery `ENOSYS` records, futex mismatch, zero-timeout wait, wait-bitset zero and elapsed absolute timeout validation, wake-bitset count/bitset behavior, `FUTEX_WAKE_OP` guest-word update plus conditional two-address wake behavior, `close_range` close and `CLOSE_RANGE_CLOEXEC` behavior, `openat2` `open_how` parsing, mode validation, and close-on-exec behavior, `umask` masking for legacy `mkdir`/`mkdirat` directories and `openat(O_CREAT)` regular files, cwd-aware registered paths, guest-backed file output/readback and open visibility, at-family file and directory mutation including `renameat2` `RENAME_NOREPLACE` plus regular-file `RENAME_EXCHANGE`, registered-directory enumeration, `ENOSYS` records including unsupported `rt_sigreturn`, known no-implementation entries, and guest writes exist. | Split hello, multicore SE, RVV intrinsic, and other-ISA subrows; add broader libc and lifecycle behavior. |
| `tests/gem5/memory` | `rem6-memory`, `rem6-cache`, `rem6-dram`, `rem6-fabric` | 59% single-axis | Stores, page maps, cache banks, topology slices, optional CLI RISC-V MSI-bank, MESI-line, MOESI-line, and CHI-line data-cache routing, top-level CLI RISC-V MSI-bank cache-cycle bank counters, three-core shared MSI/MESI/MOESI/CHI data-cache coherence routing, three-core MSI/MESI/MOESI/CHI instruction-cache fetch routing, top-level `RiscvTopologySystem` MSI instruction-cache fetch plus MSI data-cache load routing, explicit CLI RISC-V MSI data-cache and instruction-cache L1-to-L2-to-DRAM plus L1-to-L2-to-L3-to-DRAM fills, explicit run cache/fabric/DRAM aggregate/per-VN, per-link, and per-link/per-VN lane fabric stats including flit and credit-delay counters plus fabric memory-resource roll-up, default bare `rem6 run --isa riscv --execute` cache/fabric/DRAM smoke coverage, explicit `--memory-system cache-fabric-dram` preset smoke coverage, explicit `--memory-system direct` direct-transport coverage, DRAM-backed MSI fill accounting, DRAM/NVM counters, CLI-selectable JEDEC-style refresh presets, custom refresh timing, and per-bank/all-bank refresh policy, cache-local prefetch queue counters, top-level CLI RISC-V tagged next-line data-cache and instruction-cache prefetch issue/useful/useful-but-miss/useful-span-page/unused/late-hit/demand-miss and fixed-point accuracy/coverage stats, top-level instruction-cache and data-cache identity-mapped prefetch translation queue stats, top-level CLI run aggregate cache/transport/DRAM resource activity, top-level trace-replay fabric-route aggregate/per-VN, per-link, and per-link/per-VN lane activity with backpressure, aggregate resource-activity, virtual-network config stats, and fabric wait-for queue/credit edge-window JSON/stats, top-level trace-replay data-cache run/protocol/scheduler/profiled backing-DRAM and DRAM QoS stats, and combined trace-replay data-cache plus explicit-fabric route stats exist. | Add Ruby-scale protocol networks, router/virtual-channel NoC detail beyond counted flits, broader DRAM refresh breadth beyond per-bank/all-bank policy, and hierarchy-level prefetch translation consumers. |
| `tests/gem5/multisim`, `tests/gem5/suite_tests` | `rem6-workload`, `rem6-kernel`, `rem6` CLI | 52% single-axis | Suite planning, dispatch, execution summaries, occupancy contracts, and top-level `rem6 multi-run` sequential orchestration of real `run --config`, `gups --config`, `gpu-run --config`, `accelerator-run --config`, and `trace-replay --config` TOML and command-qualified `--run` entries with child-configured output, power/GPU extra-artifact path summaries, run-child checkpoint, accelerator-child command/completion, direct-memory trace-replay child checkpoint/restore count roll-up from nonempty memory checkpoint payload metadata, checked-in multi-run example aggregate/child artifact layout, aggregate artifacts, and explicit child-failure summaries exist. | Add broader checkpoint restore orchestration and suite-compatible multi-artifact workload semantics. |
| `tests/gem5/parsec_benchmarks` | `rem6-workload`, `rem6-system`, ISA crates | 0% open | Workload suites exist, but PARSEC-class programs do not run. | Add PARSEC-class static or dynamic user workload support and suite-scale ROI/stat validation. |
| `tests/gem5/processor_switch_tests` | `rem6-system`, `rem6-cpu` | 20% unit-slice | Host-assisted switch admission and execution-mode metadata exist. | Add executable CPU model switching with quiescence and state transfer. |
| `tests/gem5/py_port` | `rem6` CLI, `rem6-workload` | 0% open | No Python embedding port exists. | Decide on a typed external control adapter or document a Rust/CLI replacement. |
| `tests/gem5/pyunit` | rem6 test crates, `rem6-workload`, `rem6-stats` | 35% unit-slice | Rust tests cover selected typed stats, workload, config, and helper behavior. | Map each pyunit helper family to a Rust owner. |
| `tests/gem5/readfile_tests` | `rem6-platform`, `rem6-system`, `rem6` CLI | 55% single-axis | DTB/initrd handoff, CLI input-file plumbing, `PlatformBuilder` read-only readfile MMIO-window tests, topology host-checkpoint capture of attached readfile payloads, and `rem6 run --readfile` host-file plus manifest and `suite-resource:<workload>/<resource>` suite resource-config input payload binding into guest-visible MMIO loads exist. | Validate Linux consumption and board-level boot handoff. |
| `tests/pyunit` | `rem6-stats`, `rem6-workload`, future utility owners | 35% unit-slice | Selected pystats and stdlib semantics are covered by typed Rust tests. | Split HDF5, pystats, registry/probes, stdlib helpers, and parsing rows. |
| `tests/gem5/regression_tests` | all rem6 crates | 35% unit-slice | Workspace tests act as the current regression suite. | Add migration tags or per-family regression rows. |
| `tests/gem5/replacement_policies` | `rem6-cache` | 60% representative | Multiple replacement, indexing, dueling, compressed, and sector tag tests exist. | Add remaining policies and exact trace/reference parity where useful. |
| `tests/gem5/riscv_boot_tests` | `rem6-platform`, `rem6-system`, `rem6-isa-riscv`, `rem6-cpu`, `rem6-kernel` | 35% unit-slice | DTB/initrd handoff, workload replay Linux boot handoff into supervisor boot-hart state with secondary harts stopped before HSM start, CLINT/PLIC, traps, CSRs, page-fault causes, translated faults, SBI base read-only ecalls, top-level `rem6 run --riscv-sbi` supervisor base ecall smoke with SBI 2.0 base version reporting, conservative standard extension probes plus legacy console putchar probe, top-level CLI legacy console putchar and DBCN shared-memory write, top-level CLI TIME `set_timer` deadline artifact/stat reporting and boot-hart plus HSM-started secondary-hart S-mode interrupt-handler delivery, top-level CLI HSM `hart_start` secondary-hart release and HSM start artifact/stat records through `rem6 run --riscv-sbi --cores 2`, top-level CLI boot-hart already-started `hart_start` `-6` return, top-level CLI current-hart `hart_stop` no-return idle completion with HSM stop artifact/stat records, top-level CLI non-retentive `hart_suspend` resume-address completion with HSM suspend artifact/stat records, top-level CLI retentive HSM wake by IPI and SBI TIME/STIP with artifact/stat records, library/workload replay DBCN shared-memory read/write, direct write-byte debug-console output, DBCN advertisement when functional guest memory I/O is configured, and workload replay Linux handoff DBCN read input from a manifest `Input` resource into backing-store and data-cache-resident guest memory using functional line replacement, without topology-routed firmware memory transactions, minimal TIME `set_timer` STIP scheduling including `u64::MAX` timer disable semantics, IPI `send_ipi` scheduled completion, SSIP pending-bit injection, top-level CLI HSM-started secondary-hart S-mode handler delivery with IPI artifact/stat records, and no partial event delivery on scheduler errors, standard SRST shutdown stop requests plus top-level shutdown, cold-reboot, warm-reboot, and system-failure reset artifact/stat records, RFENCE remote FENCE.I fetch-stream reset plus top-level artifact/stat records, remote SFENCE.VMA data TLB flushes with finite-range, ASID scope, scheduled completion events, and top-level completion artifact/stat records, HFENCE.GVMA conservative whole modeled data TLB flush execution, HFENCE.VVMA range-scoped flush, HFENCE.VVMA.ASID scoped flush preservation, HFENCE validation, and HSM start entry-state, `START_PENDING`, status, no-return stop, retentive-suspend, default-non-retentive `RESUME_PENDING`/resume, and IPI/TIME-wake slices are tested. | Add broader SBI timer/IPI/reset power-state behavior, topology-routed firmware shared-memory reads/stores, remaining HSM wake semantics, VMID/G-stage/range-precise HFENCE.GVMA completion coverage beyond conservative modeled data TLB invalidation, and a real Linux boot smoke. |
| `tests/gem5/stats` | `rem6-stats`, `rem6` CLI, `rem6-power` | 66% representative | Hierarchical counters, reset/dump histories, deltas, first-class histogram buckets, real probe producers, top-level retired-instruction probe stats and retired-PC target probe summaries, power bindings, instruction/data cache counters, top-level CLI RISC-V MSI-bank cache-cycle bank counters, cache-local and top-level data-cache and instruction-cache prefetch queue counters, top-level instruction-cache and data-cache identity-mapped prefetch translation queue counters, top-level run and trace-replay aggregate/per-VN, per-link, and per-link/per-VN lane fabric-route counters including active virtual networks, flits, credit-delay ticks, and aggregate resource activity, plus trace-replay per-link/per-VN lane backpressure ticks, top-level trace-replay data-cache run/protocol/scheduler, CPU-response, directory-decision, and profiled backing-DRAM target/port/bank, row, command, latency, and QoS counters, top-level gpu-run data-cache bank and prefetch counters plus fabric aggregate/per-VN active-lane and contention counters and per-VN/per-link/per-hop transfer, byte, flit, occupancy, queue-delay, and credit-delay counters in stats and artifact JSON, top-level SBI console byte, HSM start/stop/suspend/wake, IPI request/target, RFENCE request/target/completion, and SRST reset request/type/failure stats from executed `rem6 run --riscv-sbi`, CLI text-stat output with gem5-style final-tick, committed-instruction, sim-ops, sim-seconds, sim-frequency, single-core CPU `numInsts`/`numOps`/`commitStats0.numInsts`/`commitStats0.numOps`/`numCycles` aliases plus CPU and `commitStats0` `ipc`/`cpi` rates, single-core no-prefetch text-output L1 I-cache/D-cache `demandHits`/`demandMisses`/`demandAccesses`/`demandMissRate` plus `overallHits`/`overallMisses`/`overallAccesses`/`overallMissRate` and MSHR `demandMshrHits`/`overallMshrHits`/`demandMshrMisses`/`overallMshrMisses`/`demandMshrMissRate`/`overallMshrMissRate` aliases, configured single-core L1 I-cache/D-cache prefetcher `pfIdentified`/`pfIssued`/`pfUseful`/`pfUsefulButMiss`/`pfUnused`/`pfHitInCache`/`pfHitInMSHR`/`pfHitInWB`/`pfLate`/`pfSpanPage`/`pfUsefulSpanPage`/`pfInCache`/`demandMshrMisses` aliases plus derived `accuracy`/`coverage` aliases and artifact/stat `accuracy_ppm`/`coverage_ppm` fields, single-core `conditional_branch_predictions`/`conditional_branch_predicted_taken`/`conditional_branch_mispredictions` counters feeding `system.cpu.branchPred.condPredicted`/`system.cpu.branchPred.condPredictedTaken`/`system.cpu.branchPred.condIncorrect` conditional-branch text aliases, multicore per-core `conditional_branch_predictions`/`conditional_branch_predicted_taken`/`conditional_branch_mispredictions` counters feeding `system.cpu0.branchPred.condPredicted`/`system.cpu0.branchPred.condPredictedTaken`/`system.cpu0.branchPred.condIncorrect` and `system.cpu1.branchPred.condPredicted`/`system.cpu1.branchPred.condPredictedTaken`/`system.cpu1.branchPred.condIncorrect` text aliases, top-level RISC-V Tournament predictor local/global selection counters under `sim.cpu*.branch_predictor.tournament.*` and artifact JSON, modeled RISC-V fetch-ahead BTB probe lookup/hit counters, including conditional and direct-jump probes whose target may not steer fetch, feeding `system.cpu.branchPred.BTBLookups`/`system.cpu.branchPred.BTBHits` and multicore `system.cpu0.branchPred.BTBLookups`/`system.cpu0.branchPred.BTBHits` plus `system.cpu1.branchPred.BTBLookups`/`system.cpu1.branchPred.BTBHits` text aliases, and shared L2/L3 text-output `overallHits`/`overallMisses`/`overallAccesses`/`overallMissRate` plus `overallMshrHits`/`overallMshrMisses`/`overallMshrMissRate` aliases backed by executed cache-bank summaries including `system.l2.overallMshrHits` and `system.l3.overallMshrMissRate`, multicore CPU `numInsts`/`numOps`/`commitStats0.numInsts`/`commitStats0.numOps`/`numCycles` aliases plus CPU and `commitStats0` `ipc`/`cpi` rates, gem5-style memory-controller request/burst/byte aliases plus text-output `avgRdBWSys`/`avgWrBWSys` bandwidth aliases and DRAM/NVM-interface burst/per-bank burst/row-hit/byte/memory-access-latency plus text-output `avgRdBW`/`avgWrBW`/`avgMemAccLat`/`readRowHitRate`/`writeRowHitRate`/`pageHitRate` aliases, including NVM-profile `nvmBytesRead`/`nvmBytesWritten`, and per-stage in-order pipeline width, resource/ordering blocked instruction and cycle, flush, and branch-prediction flush attribution stats from executed RISC-V runs, CLI GDB attach-before-execute register/memory smoke coverage exists; pre-execution writes cover RV64 vector data registers, RV64 supervisor CSR `sscratch`, supervisor environment CSR `senvcfg`, supervisor interrupt CSR `sie`, translation CSR `satp`, machine CSR `mscratch`, RV64 vector fixed-point CSR `vxsat`/`vxrm`/`vcsr`, RV64 vector-configuration CSR `vl`/`vtype`, and packed PMP CSR `pmpcfg0`/`pmpcfg2` plus `pmpaddr0` through `pmpaddr15` writes into live `RiscvCore` PMP state; RV64 vector-configuration CSR `vlenb`, machine identity CSR `mhartid`/`mvendorid`/`marchid`/`mimpid`, and machine ISA CSR `misa` readback through top-level GDB and matching guest CSR execution exist; RV64 counter-enable CSR `scounteren`/`mcounteren` GDB writes are read back through the register cache and consumed by matching guest CSR execution, with counter-read privilege gating; RV32 counter-enable CSR `scounteren`/`mcounteren` GDB writes through `p9e`/`p9f` are read back through the RV32 register cache and consumed by matching guest CSR execution; RV64 GDB-written `mstatus`, `mtvec`, `mie`, and `mip` machine-interrupt CSRs vector before the first guest instruction, and GDB-written `mepc`, `stvec`, `mideleg`, `sie`, and `sip` delegated-interrupt state survives `mret` into supervisor-vector execution; RV32 CSR register-cache read/write coverage, RV32/RV64 vector data register-cache read/write coverage, RV32/RV64 vector-configuration CSR coverage, and counter CSR `cycle`/`time`/`instret` plus machine-counter alias `mcycle`/`minstret` readback with counter writes after top-level GDB single-step execution exist, with selected CSR writes consumed by guest execution and counter CSR reads consumed after real execution, plus software and hardware breakpoints, single-step execution consumed by the following run, continue and `vCont;c` execution with completed-run summary handoff, cache-backed GDB run-control summary stats, data watchpoints stopping after real RISC-V load/store data-access completion with reason/address stop replies, top-level `Exec` debug-flag instruction trace records with CPU/retirement-scoped retired-record and byte stats, `Fetch` debug-flag issue trace records with CPU/endpoint-scoped issue-record and byte stats, `Data` debug-flag data-access trace records with CPU/kind-scoped load/store/atomic classification and byte stats, `Cache` debug-flag instruction/data L1/L2/L3 cache hierarchy trace records with activity, CPU-response, directory-decision, backing-DRAM, bank, and prefetch queue/translation/ratio stats, `Dram` debug-flag target/port/bank DRAM hierarchy trace records with record-count plus hierarchy-scoped access, row, refresh, command, latency, and byte stats, `Fabric` debug-flag lane/hop fabric activity trace records with record-count plus hierarchy-scoped transfer, byte, flit, occupancy, queue-delay, and credit-delay stats, `Memory` debug-flag fetch/data transport trace records with record-count, channel-scoped route/endpoint/request-agent, event-kind, response-status, and response-latency tick stats, `Syscall` debug-flag return/exit/blocked outcome plus hierarchy-scoped syscall-number, call-site, CPU, and argument stats, and `Power` debug-flag activity-derived power trace records with target/state/residency/microwatt stats from executed RISC-V runs, and library-level plus run-CLI and gpu-run-CLI McPAT/DSENT-shaped export tests exist. | Add more hierarchy counters, calibrated power/thermal activity, broader branch-predictor vector/thread stat families, executable unsupported branch-kind coverage, and broader stat naming breadth, broader debug-flag categories beyond current cache/execution/fetch/data/DRAM/fabric/memory/syscall/power coverage, and remaining broader CSR GDB register-cache coverage. |
| `tests/gem5/stdlib` | `rem6-workload`, `rem6-platform`, `rem6` CLI | 54% single-axis | Workload manifests, resource payloads, manifest/suite-level CLI resource acquisition including host-file, uncompressed/gzip tar-entry with USTAR prefix paths, and standalone gzip inputs, manifest-acquired and unique-suite run kernel handoff, `suite-resource:<workload>/<resource>` suite readfile/load-blob/trace runtime handoff, resource-backed RISC-V SE stdin and guest-file handoff, suite dispatch plans, Linux handoff intent, and TOML/CLI tests including GPU micro-run config exist. | Add broader stdlib object coverage, remote/cache policy acquisition, and ergonomic topology/workload definitions. |
| `tests/test-progs` | `rem6-system`, `rem6` CLI, ISA crates | 35% unit-slice | Static RISC-V no-libc, newlib, and raw syscall smoke binaries, including `sendfile`, `splice`, `tee`, `vmsplice`, `copy_file_range`, `memfd_create`, raw `mknodat`, raw `unshare`, raw `setns`, mode-zero `fallocate`, raw `readahead`, raw `socket` unconnected fd creation/error boundaries and abstract-name listener connect/accept4 handoff with listener/client/accepted name queries plus accept4 peer-address writeback, raw `socketpair` plus name/shutdown/selected option and null-address `sendto`/`recvfrom`/`sendmsg`/`recvmsg` flags, raw `timerfd`, raw `inotify`, raw terminal `ioctl(TIOCGWINSZ/TCGETS)`, raw `pidfd_open`/`pidfd_send_signal(..., 0)`, raw `riscv_flush_icache`, raw `setfsuid`/`setfsgid`, raw `ioprio_get`/`ioprio_set`, raw `mlock2`, `madvise(MADV_DONTNEED)` anonymous page-zeroing and file-backed restore, raw `get_mempolicy` default query and raw `set_mempolicy` set/get/reset query, `truncate`, xattr lifecycle, `statx`, `faccessat2`, `fchmodat2`, `utimensat`, advisory `flock`, `fadvise64`, `fcntl` byte-range advisory lock, pipe-size `fcntl`, legacy and typed `fcntl` owner and signal-number state (`F_SETOWN`/`F_GETOWN`/`F_SETOWN_EX`/`F_GETOWN_EX`/`F_SETSIG`/`F_GETSIG`), legacy `mkdir`, `preadv`/`pwritev`, flags-zero raw `preadv2`/`pwritev2`, `symlinkat`, raw `renameat2` `RENAME_NOREPLACE` and regular-file `RENAME_EXCHANGE`, and `fchownat`/`fchown`, `waitid`, raw `wait4` option-mask no-child behavior, `ppoll` timeout/sigmask/finite-expiration/readiness, futex wake-op, `sysinfo`, raw `syslog`, raw admin syscall rejection boundaries including chroot, raw known no-implementation ENOSYS boundaries, raw `prctl` no-new-privs, dumpable, and parent-death-signal state, raw `gettimeofday`, raw `clock_gettime64`, raw `clock_settime` validation/denial, raw legacy `time=1062`, raw interval timer query/disarm, raw `capget`/`capset` capability checks, newlib file-create roundtrip, newlib `/proc/self/exe` readlink coverage, raw `/proc/self/cwd` after `chdir` readlink coverage, raw file-backed and pipe-backed `/proc/self/fd/<fd>` readlink coverage, raw `/proc/self/maps` after `mmap` coverage, raw `/proc/self/comm` after `PR_SET_NAME` coverage, raw `/proc/self/status` identity-state coverage, newlib pipe2 roundtrip coverage, newlib directory-open coverage, and newlib open-flag coverage, are generated when tools exist. | Add durable generated fixtures for hello, threads, and m5 utility shapes across ISAs. |
| `tests/gem5/traffic_gen` | `rem6-traffic`, `rem6-system`, `rem6-workload`, `rem6` CLI | 55% single-axis | Text config parsing, GUPS, packet trace replay including manifest and unique or TOML/CLI `suite-resource:<workload>/<resource>` suite resource-config trace handoff with non-input trace resource rejection, flags, maintenance, HTM, responses, workload summaries, typed generator/memory-profile summaries, top-level trace-replay fabric-route aggregate/per-VN, per-link, and per-link/per-VN lane pressure/backpressure stats with aggregate resource-activity, virtual-network, credit-depth config, credit-delay fields, and fabric wait-for queue/credit windows, top-level trace-replay data-cache run/protocol/scheduler, CPU-response, and directory-decision stats, and top-level GUPS profile JSON/stats output exist. | Add cache hierarchy matrix and broader trusted stats. |
| `tests/gem5/x86_boot_tests` | `rem6-isa-x86`, future platform work | 0% open | Narrow x86 prefix and interrupt-flag semantics exist, but no x86 boot path exists. | Add x86 ISA execution, paging, interrupt, platform, and boot-image tests. |

Stats ledger note: the BTB, lookup, committed, and mispredict-cause slice now exposes `sim.cpu*.branch_predictor.btb.{lookups,hits,misses,updates,evictions,mispredictions,predicted_taken_misses}`, branch-kind `sim.cpu*.branch_predictor.btb.mispredict_due_to_btb_miss.{no_branch,return,call_direct,call_indirect,direct_conditional,direct_unconditional,indirect_conditional,indirect_unconditional,total}`, branch-kind `sim.cpu*.branch_predictor.lookups.{no_branch,return,call_direct,call_indirect,direct_conditional,direct_unconditional,indirect_conditional,indirect_unconditional,total}`, branch-kind `sim.cpu*.branch_predictor.committed.{no_branch,return,call_direct,call_indirect,direct_conditional,direct_unconditional,indirect_conditional,indirect_unconditional,total}`, branch-kind `sim.cpu*.branch_predictor.mispredicted.{no_branch,return,call_direct,call_indirect,direct_conditional,direct_unconditional,indirect_conditional,indirect_unconditional,total}`, branch-kind `sim.cpu*.branch_predictor.corrected.{no_branch,return,call_direct,call_indirect,direct_conditional,direct_unconditional,indirect_conditional,indirect_unconditional,total}`, and branch-kind `sim.cpu*.branch_predictor.mispredict_due_to_predictor.{no_branch,return,call_direct,call_indirect,direct_conditional,direct_unconditional,indirect_conditional,indirect_unconditional,total}` counters in top-level run JSON and stats. It also maps real BTB update, hit-ratio, no-target/wrong-target, predicted-taken BTB-miss, branch-predictor lookup-by-branch-kind, committed-branch-by-branch-kind, committed-misprediction-by-branch-kind, committed taken-misprediction BTB-miss-by-branch-kind, and committed predictor-cause misprediction-by-branch-kind values into gem5-style text aliases: `system.cpu.branchPred.BTBUpdates`, `system.cpu.branchPred.BTBHitRatio`, `system.cpu.branchPred.BTBMispredicted`, `system.cpu.branchPred.predTakenBTBMiss`, `system.cpu.branchPred.lookups_0::DirectCond`, `system.cpu.branchPred.lookups_0::DirectUncond`, `system.cpu.branchPred.lookups_0::IndirectUncond`, `system.cpu.branchPred.committed_0::DirectCond`, `system.cpu.branchPred.committed_0::IndirectUncond`, `system.cpu.branchPred.mispredicted_0::DirectCond`, `system.cpu.branchPred.mispredicted_0::IndirectUncond`, `system.cpu.branchPred.mispredictDueToBTBMiss_0::DirectCond`, `system.cpu.branchPred.mispredictDueToBTBMiss_0::IndirectUncond`, `system.cpu.branchPred.mispredictDueToPredictor_0::DirectCond`, `system.cpu.branchPred.mispredictDueToPredictor_0::IndirectUncond`, multicore `system.cpu0.branchPred.BTBUpdates`, `system.cpu0.branchPred.BTBHitRatio`, `system.cpu0.branchPred.BTBMispredicted`, `system.cpu0.branchPred.predTakenBTBMiss`, `system.cpu0.branchPred.lookups_0::DirectCond`, `system.cpu0.branchPred.lookups_0::DirectUncond`, `system.cpu0.branchPred.lookups_0::IndirectUncond`, `system.cpu0.branchPred.committed_0::DirectCond`, `system.cpu0.branchPred.committed_0::IndirectUncond`, `system.cpu0.branchPred.mispredicted_0::DirectCond`, `system.cpu0.branchPred.mispredicted_0::IndirectUncond`, `system.cpu0.branchPred.mispredictDueToBTBMiss_0::DirectCond`, `system.cpu0.branchPred.mispredictDueToBTBMiss_0::IndirectUncond`, `system.cpu0.branchPred.mispredictDueToPredictor_0::DirectCond`, `system.cpu0.branchPred.mispredictDueToPredictor_0::IndirectUncond`, `system.cpu1.branchPred.BTBUpdates`, `system.cpu1.branchPred.BTBHitRatio`, `system.cpu1.branchPred.BTBMispredicted`, `system.cpu1.branchPred.predTakenBTBMiss`, `system.cpu1.branchPred.lookups_0::DirectCond`, `system.cpu1.branchPred.lookups_0::DirectUncond`, `system.cpu1.branchPred.lookups_0::IndirectUncond`, `system.cpu1.branchPred.committed_0::DirectCond`, `system.cpu1.branchPred.committed_0::IndirectUncond`, `system.cpu1.branchPred.mispredicted_0::DirectCond`, `system.cpu1.branchPred.mispredicted_0::IndirectUncond`, `system.cpu1.branchPred.mispredictDueToBTBMiss_0::DirectCond`, `system.cpu1.branchPred.mispredictDueToBTBMiss_0::IndirectUncond`, `system.cpu1.branchPred.mispredictDueToPredictor_0::DirectCond`, and `system.cpu1.branchPred.mispredictDueToPredictor_0::IndirectUncond`. The current RISC-V execution path uses gem5-style link-register classification for direct calls, indirect calls, returns, and direct/indirect unconditional jumps, with real lookup and committed coverage for direct-conditional and direct-unconditional lanes, BTB-miss attribution coverage for direct-conditional, indirect-call, return, and indirect-unconditional lanes, and predictor-cause coverage for direct-conditional wrong-direction and wrong-target repairs; direct `jal`/`jal ra` targets are statically known, so correct cold-BTB jumps remain scalar BTB misses rather than `mispredictDueToBTBMiss`. Remaining gaps are broader branch-predictor vector/thread stat families beyond the current lookup, target-provider, committed, corrected, target-wrong, and mispredict-cause lanes plus executable coverage for branch-kind lanes that are not yet produced by RISC-V fetch-ahead scenarios.
The lookup total text aliases are also covered as `system.cpu.branchPred.lookups_0::total`, `system.cpu0.branchPred.lookups_0::total`, and `system.cpu1.branchPred.lookups_0::total`.
The corrected branch-kind slice exposes `sim.cpu*.branch_predictor.corrected.{no_branch,return,call_direct,call_indirect,direct_conditional,direct_unconditional,indirect_conditional,indirect_unconditional,total}` counters and maps them to `system.cpu.branchPred.corrected_0::DirectCond`, `system.cpu.branchPred.corrected_0::IndirectUncond`, `system.cpu.branchPred.corrected_0::total`, multicore `system.cpu0.branchPred.corrected_0::DirectCond`, `system.cpu0.branchPred.corrected_0::IndirectUncond`, `system.cpu0.branchPred.corrected_0::total`, `system.cpu1.branchPred.corrected_0::DirectCond`, `system.cpu1.branchPred.corrected_0::IndirectUncond`, and `system.cpu1.branchPred.corrected_0::total`.
The target-provider slice exposes `sim.cpu*.branch_predictor.target_provider.{no_target,btb,ras,indirect,total}` counters and maps current real BTB-selected, no-target, single/multicore RAS return, and single/multicore indirect JALR providers to `system.cpu.branchPred.targetProvider_0::BTB`, `system.cpu.branchPred.targetProvider_0::NoTarget`, `system.cpu.branchPred.targetProvider_0::RAS`, `system.cpu.branchPred.targetProvider_0::Indirect`, `system.cpu.branchPred.targetProvider_0::total`, multicore `system.cpu0.branchPred.targetProvider_0::BTB`, `system.cpu0.branchPred.targetProvider_0::NoTarget`, `system.cpu0.branchPred.targetProvider_0::RAS`, `system.cpu0.branchPred.targetProvider_0::Indirect`, `system.cpu0.branchPred.targetProvider_0::total`, `system.cpu1.branchPred.targetProvider_0::BTB`, `system.cpu1.branchPred.targetProvider_0::NoTarget`, `system.cpu1.branchPred.targetProvider_0::RAS`, `system.cpu1.branchPred.targetProvider_0::Indirect`, and `system.cpu1.branchPred.targetProvider_0::total`; RAS is produced by top-level call/return fetch-ahead execution, indirect is produced by top-level register-sourced `jalr` fetch-ahead execution, and remaining target-provider gaps are broader provider/thread/vector scenarios beyond these executable RISC-V paths.
The target-wrong branch-kind slice exposes `sim.cpu*.branch_predictor.target_wrong.{no_branch,return,call_direct,call_indirect,direct_conditional,direct_unconditional,indirect_conditional,indirect_unconditional,total}` counters and maps them to `system.cpu.branchPred.targetWrong_0::DirectCond`, `system.cpu.branchPred.targetWrong_0::IndirectUncond`, `system.cpu.branchPred.targetWrong_0::total`, multicore `system.cpu0.branchPred.targetWrong_0::DirectCond`, `system.cpu0.branchPred.targetWrong_0::IndirectUncond`, `system.cpu0.branchPred.targetWrong_0::total`, `system.cpu1.branchPred.targetWrong_0::DirectCond`, `system.cpu1.branchPred.targetWrong_0::IndirectUncond`, and `system.cpu1.branchPred.targetWrong_0::total`.

## External Adapter Migration

### SystemC and TLM Adapters - 59% single-axis

**Score calculation:** 3 of 4 items have executable evidence, or 75% raw. The
bucket cap is single-axis because `trace-replay` drives the typed SystemC/TLM
adapter boundary, but no external SystemC simulator or TLM model executes through it.

- [x] A typed co-simulation adapter boundary exists.
- [x] Adapter event handoff executes from the top-level trace-replay runtime adapter path.
- [x] Adapter checkpoint capture and restore are consumed by the top-level trace-replay runtime adapter path.
- [ ] Runtime SystemC/TLM model integration executes through the adapter.

**Migrated:** `CoSimAdapterBoundary` SystemC/TLM endpoint tests exist, and `rem6 trace-replay --external-adapter-kind systemc|tlm --external-adapter-endpoint <id>` hands packet-trace requests into that boundary, acknowledges them, captures a runtime checkpoint when `--external-adapter-checkpoint-after-events` is set, restores it, completes later packet requests through the restored boundary, and emits `external_adapter` JSON plus `sim.trace_replay.external_adapter.*` stats.

**Not migrated:** External SystemC simulator or TLM model execution, external model-owned state, and `src/systemc`, `util/tlm`, and `ext/systemc` behavior.

**Evidence:** `cosim_adapter`; `rem6_trace_replay_hands_off_packet_requests_to_systemc_and_tlm_adapters`; TOML/CLI external-adapter validation tests.

**Next evidence:** External SystemC/TLM bridge execution and external model-owned checkpoint restore tests.

### SST Adapter - 59% single-axis

**Score calculation:** 3 of 4 items have executable evidence, or 75% raw. The
bucket cap is single-axis because `trace-replay` drives the typed SST adapter
boundary, but no external SST simulator runtime executes through it.

- [x] A typed SST adapter boundary exists.
- [x] SST traffic handoff executes from the top-level trace-replay runtime adapter path.
- [x] SST adapter checkpoint capture and restore are consumed by the top-level runtime adapter path.
- [ ] Runtime SST execution uses an external SST simulator bridge.

**Migrated:** `CoSimAdapterBoundary` SST endpoint tests exist, and `rem6 trace-replay --external-adapter-kind sst --external-adapter-endpoint <id>` hands packet-trace requests into that boundary, acknowledges them, captures a runtime checkpoint when `--external-adapter-checkpoint-after-events` is set, restores it, completes later packet requests through the restored boundary, and emits `external_adapter` JSON plus `sim.trace_replay.external_adapter.*` stats.

**Not migrated:** External SST simulator bridge execution, external SST-owned runtime state, and `ext/sst` plus `configs/example/sst` behavior.

**Evidence:** `cosim_adapter`; `rem6_trace_replay_hands_off_packet_requests_to_sst_adapter`; TOML/CLI external-adapter validation tests in the `rem6` CLI suite.

**Next evidence:** External SST bridge execution and external SST-owned checkpoint restore tests.

### Power and Physical-Design Export Adapters - 59% single-axis

**Score calculation:** 5 of 7 items have executable evidence, or 71% raw. The bucket cap is single-axis because top-level
McPAT/DSENT/NoMali artifacts and rem6-shaped McPAT/DSENT typed ingestion
round-trips exist, but full external-tool ingestion, schema parity, calibrated activity, and complete NoMali PIO remain absent.

- [x] rem6-power can export typed power-analysis records.
- [x] McPAT-shaped XML export serializes power, thermal, and residency records.
- [x] DSENT-shaped CSV export serializes power, thermal, and residency records.
- [x] `rem6 run --power-output`, `rem6 trace-replay --power-output`, and `rem6 gpu-run --power-output` write executed-run activity-derived power-analysis artifacts.
- [ ] McPAT-compatible ingestion/export parity is complete.
- [ ] DSENT-compatible ingestion/export parity is complete.
- [x] NoMali-compatible GPU adapter evidence exists.

**Migrated:** Typed power-analysis export records, deterministic custom XML smoke coverage, McPAT-shaped XML, DSENT-shaped CSV, and
typed McPAT/DSENT adapter ingestion round-trips. Top-level `rem6 run --power-output` emits an activity-derived McPAT-shaped or
DSENT-shaped artifact from executed CPU, instruction-cache, data-cache, shared L2/L3 cache, fabric/NoC, and memory-transport/DRAM
summaries, and executed McPAT/DSENT run artifacts parse back into typed power-analysis records. Top-level `rem6 run --debug-flags Power`
emits the same executed-run activity-derived CPU, cache, fabric/NoC, memory-transport, and DRAM power records in deterministic run JSON.
Top-level `rem6 trace-replay --power-output` emits activity-derived trace replay data-cache, fabric/NoC, and DRAM power records from executed packet replay summaries and explicit-fabric route activity. Top-level `rem6 gpu-run --power-output` emits activity-derived GPU compute-unit, GPU
fabric/NoC, data-cache, and DRAM power records, with the artifact path reported in the run JSON or CLI output envelope.
Top-level `rem6 power-import` ingests McPAT-shaped XML, McPAT text reports, DSENT-shaped CSV, and gem5 DSENT Python-tuple reports into deterministic typed JSON summaries and optional output artifacts; report imports map printed power while deriving residency and temperature because those reports omit them.
Top-level `rem6 gpu-run --nomali-output` emits a deterministic
NoMali-compatible T760 adapter artifact from executed GPU run summaries,
including register-file checkpoint evidence, simple GPU command-table evidence for reset, no-effect, perf-sample, clean-cache, and clean-invalidate commands, MMU address-space no-effect and ignored-command evidence, ignored
unsupported-command, MMU AS0 `TRANSTAB`/`MEMATTR`/`LOCKADDR` storage evidence, job-slot next-register storage and start-next transfer plus completion IRQ evidence, IRQ-clear state, shader/tiler/L2 low/high-word and L3 zero-present power-state IRQ evidence, misaligned/out-of-range register fault records, job/MMU interrupt-block raw/mask/status and IRQ status-register read evidence, callback identifiers, GPU/job/MMU interrupt callback transition records, and observed workgroup
plus memory activity; TOML and flag-driven runs both report the artifact path.

**Not migrated:** Complete `ext/nomali`, `ext/mcpat`, and `ext/dsent` parity, NoMali PIO command/register breadth beyond the existing reset,
simple GPU/MMU/job-slot command-table, MMU AS0 config-register, job-slot next-register, ignored-command, IRQ-clear, shader/tiler/L2 low/high-word and L3 zero-present power, deterministic fault-record, job/MMU interrupt-block, and callback-transition slices, broader command/power/register fault behavior, real interrupt delivery into a simulated interrupt controller, broader/full external-tool ingestion beyond current McPAT/DSENT report and rem6-shaped adapter artifacts,
full external schema parity, and broader calibrated power/thermal activity.

**Evidence:** rem6-power export self-tests for custom XML, McPAT XML, DSENT CSV, and McPAT/DSENT import round-trips; `rem6 run` CLI
tests for `--power-output`, executed McPAT/DSENT artifact ingestion, instruction-cache, data-cache, shared L2/L3 cache, fabric/NoC,
memory-transport, and DRAM activity records, `rem6 trace-replay --power-output` data-cache, explicit-fabric, and DRAM activity records,
`rem6 gpu-run --power-output` compute-unit plus explicit-fabric activity records, `rem6 power-import` McPAT XML, McPAT report, DSENT CSV, and DSENT tuple-report ingestion summaries plus output artifacts, `--debug-flags Power` run-JSON records, envelope reporting,
and load-only rejection; `rem6 gpu-run` CLI/TOML McPAT/DSENT output plus `--nomali-output` T760 register-file, hard-reset, soft-reset,
no-effect commands, perf-sample, clean-cache, clean-invalidate, MMU address-space config registers and command table, job-slot next-register/start-next and command table, ignored-command, IRQ-clear, shader/tiler/L2 low/high and L3 zero-present power, register-fault, job/MMU interrupt-block, IRQ status-register reads, GPU/job/MMU interrupt-callback-transition, multi-artifact envelope, and output-path conflict evidence.

**Next evidence:** Broader external McPAT/DSENT tool ingestion, broader NoMali PIO command/register/power/fault/interrupt behavior, calibrated activity models, and
stricter external schema parity tests.

### Native Loader and Math Replacement - 50% single-axis

**Score calculation:** 2 of 4 items have executable evidence, or 50% raw. The
bucket cap is single-axis because loader and softfloat matrix breadth is not
complete.

- [x] Native ELF loading reaches executable RISC-V SE smoke paths.
- [x] Native DTB handoff records exist.
- [ ] libelf replacement breadth covers the needed gem5 loader matrix.
- [ ] softfloat replacement breadth covers all FP rounding and exception paths.

**Migrated:** Native Rust loader and DTB handoff slices, top-level ELF32/ELF64
extended program-header-count (`PN_XNUM`) loading, `PT_PHDR`/`PT_NOTE` metadata,
`PT_INTERP` interpreter reporting/rejection, `.tbss`/`PT_TLS` TLS metadata,
`PT_GNU_STACK` stack-exec, `PT_GNU_RELRO`, `PT_GNU_EH_FRAME`, and `PT_GNU_PROPERTY` metadata,
symbol/dynamic-symbol counts, section-header/name/flag/storage/address metadata, dynamic needed/path/loader-string/table/lifecycle-scalar/array/relocation/hash/version/linker/flag and ABI-note OS metadata,
plus RV64F/RV64D scalar FP slices.

**Not migrated:** Complete `ext/libelf`, `ext/libfdt`, and `ext/softfloat` parity.

**Evidence:** CLI static RISC-V smoke tests cover ELF64 extended program-header
counts, `PT_PHDR`/`PT_NOTE` JSON/stats, `PT_INTERP` rejection/reporting, `.tbss`/`PT_TLS`
TLS metadata, `PT_GNU_STACK`, `PT_GNU_RELRO`, `PT_GNU_EH_FRAME`, and `PT_GNU_PROPERTY` metadata,
symbol/dynamic-symbol counts, section-header/name/flag/storage/address JSON/stats, dynamic needed/path/loader-string/table/lifecycle-scalar/array/relocation/hash/version/linker/flag JSON/stats,
ELF32 extended counts, ABI-note OS, DTB, and RV64F/RV64D tests.

**Next evidence:** Expand loader breadth beyond current extended-numbering
slices and soft-float parity.

## Update Rules

- Update percentages only when executable rem6 evidence changes; keep the checklist beside each component so the score can be audited.
- Do not count unknown-syscall diagnostics as implemented syscall coverage or tool-detected static smokes as broad workload parity.
- Do not cite exact line ranges from gem5 or rem6.
- Keep detailed proof in tests, artifacts, traces, checkpoints, or manifests.
