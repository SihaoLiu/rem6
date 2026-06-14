# gem5 to rem6 Migration

This document is the current migration ledger from gem5 concepts and tests to
rem6.

The gem5 tree under `temp/reference_designs/gem5` is read-only audit input. Do
not copy test binaries, generated outputs, or build products into rem6.

## Document Boundary

`docs/architecture/rem6-architecture.md` owns the stable architecture story:
runtime shape, ownership model, invariants, and design motivation.

This migration ledger owns changing state:

- component scores and score calculations;
- markdown checklists for migrated and missing behavior;
- concise migrated, not migrated, evidence, and next-evidence notes;
- gem5 test-anchor crosswalks and external-adapter rows.

Do not duplicate the architecture document's invariant list here. Do not put
current percentages or proof logs in the architecture document.

## Scoring Rubric

A percentage is a behavior-boundary score, not a count of related files.
Checklist items use markdown checkboxes:

- `[x]` means current rem6 has executable evidence for that item.
- `[ ]` means the item is not migrated or lacks executable rem6 evidence.

Checklist source: the component-local markdown checkbox list is the auditable
source for the component percentage. The gem5 test-anchor table uses row scores
only as compact crosswalk markers; those row scores do not override component
checklist calculations.

For each component, the score uses this formula:

1. Count completed checklist items and divide by all listed checklist items.
2. Round that raw ratio to the nearest whole percent.
3. Apply the evidence-breadth bucket as an upper bound.

The component's score calculation must state both the checklist fraction and
any bucket cap. The component checklists are the auditable source for migration
progress.

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
Audit it in this order:

1. Count `[x]` items and total checklist items.
2. Confirm the section-local `Score calculation` states the same fraction and
   rounded raw percentage.
3. Confirm the section-local `Score calculation` states the bucket cap label.
4. Confirm the section header score is the raw percentage after the cap is
   applied.
5. Use `Not migrated` and `Next evidence` as the blocking-boundary summary.

Do not add a second component-score table here; duplicated mutable scores drift
from the checklist source.

## Component Progress

### RISC-V ISA and Privileged Substrate - 59% single-axis

**Score calculation:** 7 of 11 items have executable evidence, or 64% raw,
capped to 59% by the single-axis bucket.
The bucket cap is single-axis because non-RISC-V ISAs and full RV64GC/vector
parity are not present.

- [x] RV64 integer, atomic, CSR, trap, counter, WFI, fence, PMP/PMA slices have tests.
- [x] RV64C integer/load-store/control-flow slices have tests.
- [x] RV64F/RV64D scalar load/store, arithmetic, comparisons, conversions, legal FP arithmetic and integer-to-float rounding-mode decode, exact static non-RNE integer-to-float conversion execution, inexact integer-to-float accrued flag updates, rounding-insensitive static arithmetic execution, NaN-boxing, and accrued flag slices have tests.
- [x] RV64F/RV64D integer-to-float conversions execute inexact static directed rounding and valid dynamic `frm` modes with accrued inexact flags.
- [x] RV64C double-precision FP load/store decode and compressed FP load CPU data-access slices have tests.
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
execution, compressed double FP load/store decoding, compressed FP load CPU data
access, traps, translation helpers, and SE ecall plumbing.

**Not migrated:** Full RV64GC/vector breadth, other major ISAs, directed
rounding breadth beyond the covered integer-to-float slice, and complete Linux
privileged behavior.

**Evidence:** `RiscvInstruction::decode_with_length`,
`decode_float_op`, `decode_compressed`, `walk_sv39_page_table_with_context`,
tests `rv64i`, `rv64m`, `rv64f`, `rv64d`, `riscv_frontend`, `sv39`, and
privileged RISC-V tests.

**Next evidence:** Generated or imported RV64GC/vector instruction tests plus
privileged Linux trap and interrupt smoke tests.

### CPU Execution Models - 30% unit-slice

**Score calculation:** 3 of 10 items have executable evidence, or 30% raw. The
bucket cap is unit-slice because RISC-V core timing has direct completed-fetch
overlap and a bounded normal-driver fetch-ahead slice, but not full top-level
stalls/squashes, and O3 state is not yet an executable cycle-visible engine.

- [x] RISC-V atomic execution and parallel clusters execute real instructions.
- [x] Data access issue/response and store-conditional progress diagnostics have tests.
- [x] Basic, GShare, and Tournament branch predictors are trained from retired control flow.
- [ ] Minor-like fetch/decode/execute/commit timing is wired into normal CPU execution.
- [ ] Branch predictors steer fetch with speculation snapshots, squash, and rollback.
- [ ] A running O3 engine owns ROB, LSQ, rename map, commit, store-to-load forwarding, and FU latency.
- [ ] Checker CPU support exists.
- [ ] CPU mode switching transfers live architectural and timing authority.
- [ ] KVM or equivalent fast-forward execution exists.
- [ ] CPU instruction/data traffic uses the full cache, NoC, and DRAM hierarchy by default.

**Migrated:** Atomic RISC-V execution, frontend/data slices, branch predictor
state and retired training, directly issued completed 4-byte fetches occupying
the in-order timing state before retire, normal `drive_next_action`, cluster,
and data-translation drivers issuing a bounded fetch-ahead for completed
straight-line 4-byte integer instructions before retire, per-retired-instruction
in-order stage advancement with runtime stats, data-response wait cycles folded
into in-order retire timing, per-core data-wait cycle stats, retired branch
prediction and redirect summaries in normal in-order timing records, and O3
policy helpers.

**Not migrated:** Full Minor-like in-order timing with realistic stalls and
squashes, executable O3 timing, fetch speculation, checker, and KVM equivalents.

**Evidence:** `RiscvCore::execute_next_completed_fetch`,
`RiscvClusterRun`, `record_data_retire_cycle`, `InOrderPipelineState`,
`O3DistributedIssueScheduler`, `O3SourceRenamePlan`, CPU frontend and O3 tests.
CLI run stats include per-core in-order pipeline cycle and retired counters
from executed RISC-V instructions, and CLI data stats show load/store response
wait changing the in-order pipeline cycle counter and data-wait cycle stat.
RISC-V in-order timing tests include retired taken and fall-through branch
prediction redirect evidence from the normal execution path, plus direct
completed-fetch overlap evidence before retire. CPU frontend and cluster tests
also cover normal-driver fetch-ahead before retiring straight-line integer
instructions while preserving trap and data-access ordering.

**Next evidence:** Broader per-cycle in-order stalls/squashes, branch-predicted
fetch with squash/rollback, then a ROB/LSQ-backed O3 run test.

### Memory, Cache, Coherence, Fabric, and DRAM - 54% single-axis

**Score calculation:** 7 of 13 items have executable evidence, or 54% raw. The
bucket cap is single-axis because full CPU-facing L1/L2/L3 plus NoC and DRAM is
not the default instruction/data path.

- [x] Memory stores, page maps, translation queues, and TLB state have tests.
- [x] Cache banks model replacement, MSHRs, write queues, maintenance, sector and compressed tags.
- [x] MSI/MESI/MOESI/CHI line and bank models have protocol tests.
- [x] DRAM/NVM profiles expose bank, timing, QoS, low-power, and routed execution slices.
- [x] Fabric and transport expose multi-hop routing, credits, virtual networks, and activity records.
- [x] Named DDR4, DDR5, and HBM JEDEC-style refresh presets validate `tREFI`/`tRFC` cycles through profile constructors, controller refresh scheduling, and `rem6 run --dram-memory-profile` stats.
- [x] Cache-local prefetch queues expose enqueue, issue, drop, and translation counters from queued prefetch and translation flows.
- [ ] Normal CPU instruction/data traffic uses a complete L1/L2/L3 hierarchy.
- [ ] Ruby-scale protocol transactions and topology races are migrated.
- [ ] NoC router, flit, virtual-channel, and routing detail match gem5-class coverage.
- [ ] Broader DRAM refresh modes and full JEDEC timing tables are complete.
- [ ] Prefetch translation consumers and queue-level stats are complete.
- [ ] Bank/cache/fabric/DRAM resource counters are complete enough for full-system studies.

**Migrated:** Typed memory primitives, cache banks, protocol harnesses, DRAM
profiles, controller-level refresh timing slices, routed topology slices,
CLI-selectable JEDEC refresh presets,
cache-local queued-prefetch and missing-translation queue counters, and trace
replay consumers; optional single-core CLI RISC-V data traffic can drive
MSI-bank, MESI-line, MOESI-line, and CHI-line data-cache runs and emit CPU
response, directory decision, and DRAM access counters from those runs; optional
single-core CLI RISC-V instruction fetch traffic can drive an MSI
instruction-cache run path with separate instruction-cache counters; volatile
CLI external-memory profiles carry refresh interval/recovery timing, DDR4,
DDR5, and HBM preset constructors validate `tREFI`/`tRFC` cycles through
existing timing checks and controller refresh scheduling, and a DDR CLI RISC-V
DRAM execution path emits nonzero refresh counters from real fetch traffic.
DRAM target activity exposes per-bank resource counters for access/read/write
counts, byte counts, row hits/misses, commands, and refresh cycles.

**Not migrated:** Broad CPU-facing hierarchy, Ruby-scale protocol networks,
flit-level NoC, broad JEDEC preset validation, system-level prefetch
translation consumers, and broad DRAM refresh/preset breadth.

**Evidence:** `MsiCacheBank`, `MsiCacheController`, protocol directory
harnesses, `DramController`, `DramMemoryController`, `FabricModel`,
`MemoryTransport`, and tests `riscv_topology_msi_data`,
`riscv_topology_chi_data`, `memory_controller`, `timing`, `fabric_timing`,
`system_run_resource_activity`, `prefetch_queue_stats`,
`prefetch_queue_translation`, `refresh_presets`, and CLI `run` data-cache smoke
coverage with resource-activity stats plus instruction-cache fetch smoke
coverage. DRAM memory-profile tests cover bank-level resource counters and
activity-window counter deltas.
CLI `run` also has DDR profile refresh smoke coverage that exposes refresh
timing fields and nonzero refresh stats from RISC-V DRAM execution.

**Next evidence:** RISC-V instruction/data execution through a coherent
multi-level cache and DRAM path with unified resource accounting, plus
validated DDR4/DDR5/HBM refresh presets.

### RISC-V SE, Workloads, and Linux Boot - 45% single-axis

**Score calculation:** 5 of 11 items have executable evidence, or 45% raw. The
bucket cap is single-axis because static newlib smokes are high-value but
tool-detected, and broad workload coverage is not present.

- [x] User-mode ecalls reach `RiscvSyscallTable`.
- [x] Startup stack, argv/envp/auxv, `brk`, `mmap`, in-place `mremap` slice, `mprotect`, mapped-page `mincore` present-vector reporting, `madvise` known-advice and mapped-range validation, `msync` flags and mapped-range validation, `mlock`/`munlock` `mmap`/`brk` range validation, stdio, file create/truncate/`ftruncate`/read/write/append, positioned I/O, vector I/O, `statx`, `statfs`/`fstatfs`, `sysinfo`, value-mode `riscv_hwprobe` base key reporting, `ppoll`, `sched_getscheduler`, `sched_getparam`, `sched_get_priority_max/min`, `sched_rr_get_interval`, single-word `sched_setaffinity`/`sched_getaffinity`, single CPU/node `getcpu`, single-process `membarrier` slice, zero-duration `nanosleep` and `clock_nanosleep` validation, `clock_getres`, `CLOCK_TAI` `clock_gettime`, `kill(..., 0)`, `tkill(..., 0)`, and `tgkill(..., 0)` existence checks, current-process scoped process-group/session `setpgid`/`getpgid`/`getsid`/`setsid` slices, gem5-style advisory `setuid`/`setrlimit` success returns, legacy `getrlimit` stack/data/NPROC limits, basic `rt_sigaction`/`rt_sigprocmask`, empty `rt_sigpending` mask reporting, no-pending zero-timeout `rt_sigtimedwait`, futex mismatch and wake-bitset count/bitset behavior, `umask` masking for `mkdirat` directories and `openat(O_CREAT)` regular files, time, cwd, `chdir`/`fchdir`, random, resource, and wait slices have tests.
- [x] Unknown syscall returns `ENOSYS` and records a typed diagnostic.
- [x] Static no-libc and newlib smoke binaries can be generated and compared with qemu when tools exist; tool-detected newlib directory-open and `O_NOCTTY`/`O_NOFOLLOW` coverage runs through the legacy `open` syscall and registered guest files, while `/proc/self/exe` readlink and pipe roundtrip coverage run through direct ecalls.
- [x] Linux at-family hard-link, `renameat2` flags=0, unlink, `mkdirat`, `unlinkat` with `AT_REMOVEDIR`, and registered-directory `getdents64` syscalls mutate or expose registered guest files and directories and have qemu-compared raw smoke evidence.
- [ ] Process/thread lifecycle, signals, permissions, and blocking wait/futex semantics are broad enough for distro-like programs.
- [ ] Broad Linux syscall table parity exists.
- [ ] Host filesystem policy matches the needed gem5 SE cases.
- [ ] SBI/OpenSBI-class firmware behavior exists.
- [ ] A real Linux kernel boots to userspace or clean shutdown.
- [ ] PARSEC or comparable workload programs run through ROI/stat hooks.

**Migrated:** RISC-V SE ecall path; startup stack and auxv setup; `brk`,
`mmap`, in-place `mremap` shrink and tail-free expansion, `mprotect`,
mapped-page `mincore` present-vector reporting, `madvise` known-advice and
mapped-range validation, `msync` flags and mapped-range validation,
`mlock`/`munlock` `mmap`/`brk` range validation, stdio, guest-backed file create/truncate/`ftruncate`/read/write/append/positioned read-write/readback and open
fd/link visibility, vector I/O, `ppoll`, `sched_getscheduler`, `sched_getparam`,
`sched_get_priority_max/min`, `sched_rr_get_interval`,
single-word `sched_setaffinity` and `sched_getaffinity`, `statx` basic stat buffer writes,
`statfs`/`fstatfs` deterministic guest-namespace filesystem statistics,
`sysinfo` uptime and configured SE-visible memory-capacity writes,
value-mode `riscv_hwprobe` base key reporting, single CPU/node
`getcpu`, `membarrier` single-process command query, registration, and
private-expedited barrier slices, zero-duration `nanosleep` and
`clock_nanosleep` validation, `clock_getres`, `CLOCK_TAI` `clock_gettime`,
time, cwd, random, resource, wait, unknown syscall, `kill(..., 0)`
process-existence checks, `tkill(..., 0)` and `tgkill(..., 0)` current-thread
existence checks, process-group/session `setpgid`/`getpgid`/`getsid`/`setsid`
query, current-leader rejection, and nonleader transition slices, basic signal action/mask state for `rt_sigaction` and `rt_sigprocmask`,
gem5-style advisory success returns for `setuid` and `setrlimit`,
empty `rt_sigpending` mask reporting, no-pending zero-timeout `rt_sigtimedwait`,
futex wait mismatch and wake-bitset count/bitset behavior,
`umask` state applied to `mkdirat` directory modes and `openat(O_CREAT)`
regular-file modes, cwd-aware registered-path lookup, at-family
hard-link/`renameat2` flags=0/unlink/`mkdirat`/`AT_REMOVEDIR`, and
registered-directory `getdents64` slices; supervisor SBI base read-only
identity/probe calls, minimal TIME `set_timer` STIP scheduling, IPI
`send_ipi` SSIP pending-bit injection for registered harts, standard SRST
shutdown stop requests and invalid-param returns, RFENCE probe reporting, and
remote SFENCE.VMA finite-range and ASID-scoped data TLB flushes through
translated execution, remote SFENCE.VMA scheduled completion events, explicit
unsupported HFENCE.GVMA/VVMA validation for invalid hart masks and ASID/VMID
width, plus HSM probe, `hart_get_status`, `hart_start`
secondary-hart `START_PENDING` reporting before the scheduled entry event,
secondary-hart release with supervisor entry state, `satp=0`, `sstatus.SIE=0`,
`a0=hartid`, and `a1=opaque`, current-hart `hart_stop` as a no-return stop
that does not write `sbiret`, `STOP_PENDING` reporting until the scheduled stop
event completes, retentive current-hart `hart_suspend` through
`SUSPEND_PENDING` until the scheduled suspend event reaches the CPU execution
gate, default non-retentive current-hart `hart_suspend` reporting
`RESUME_PENDING` until the scheduled resume event re-enters at `resume_addr`
with the same supervisor entry-state contract, stale non-retentive resume events
ignored after intervening state changes, checkpoint roundtrip coverage for all
modeled hart run states, and SBI IPI pending interrupts waking retentive
suspended harts; typed
unknown-syscall records; static smoke coverage; a static newlib
`fopen("w+")` create, write, seek, readback, and exit-code roundtrip; and a
static newlib program that reads `/proc/self/exe` through a direct
`readlinkat` ecall and compares the exit path with qemu; direct and vector I/O
unit coverage for pipe endpoints plus a static newlib program that roundtrips
bytes through `pipe2`, `write`, `read`, and `close` direct ecalls; and a static
newlib `open(".", O_DIRECTORY | O_CLOEXEC)` directory traversal smoke plus
`open` with `O_NOCTTY`, `O_NOFOLLOW`, and `O_SYNC` through the legacy `open`
syscall with newlib/libgloss flags.

**Not migrated:** Broad Linux SE parity, process/thread lifecycle, broad SBI
timer/IPI/reset power-state behavior, remaining HSM wake semantics beyond the
`hart_start`, `hart_stop`, retentive `hart_suspend`, and default
non-retentive `hart_suspend` slices, RFENCE hypervisor-fence execution
semantics and broader completion coverage, full Linux boot, and real benchmark
workloads.

**Evidence:** `RiscvSyscallTable::handle_with_guest_memory_io_at_tick`,
`RiscvSyscallEmulation::handle_pending_core_trap`, CLI static newlib tests,
`riscv_syscall_getrusage`, `riscv_se_resource`, `riscv_se_chdir`,
`riscv_se_links`, `riscv_se_mkdir`, `riscv_se_rename`, `riscv_se_getdents`,
`riscv_se_fd`, `riscv_se_open_flags`, `riscv_se_permissions`, `riscv_se_proc`,
`riscv_se_stdio`, `riscv_syscall_pipe`, `riscv_syscall_readv`,
`riscv_syscall_writev`, `riscv_syscall::tests::cpu_tests`,
`riscv_syscall::tests::hwprobe_tests`,
`riscv_syscall::tests::mlock_tests`,
`riscv_syscall::tests::mmap_tests`,
`riscv_syscall::tests::msync_tests`,
`riscv_syscall::tests::positioned_io_tests`,
`riscv_syscall::tests::mkdir_tests`,
`riscv_syscall::tests::truncate_tests`,
`riscv_syscall::tests::stat_tests`,
`riscv_syscall::tests::statfs_tests`,
`riscv_syscall::tests::process_tests`,
`riscv_syscall::tests::scheduler_tests`,
`riscv_syscall::tests::signal_tests`,
`riscv_syscall::tests::futex_tests`,
`riscv_syscall::tests::nanosleep_tests`,
`riscv_syscall::tests::sysinfo_tests`,
`riscv_se_statx`,
`riscv_se_sysinfo`,
`riscv_se_time`,
`riscv_sbi_firmware`, `riscv_system_translation`, `riscv_se_process`,
`riscv_se_signal`,
`riscv_sbi::tests::remote_hfence_gvma_rejects_missing_target_before_reporting_unsupported`,
`riscv_sbi::tests::remote_hfence_gvma_rejects_invalid_range_before_reporting_unsupported`,
`riscv_sbi::tests::remote_hfence_gvma_reports_not_supported_after_valid_target_validation`,
`riscv_sbi::tests::remote_hfence_gvma_vmid_rejects_invalid_vmid_before_reporting_unsupported`,
`riscv_sbi::tests::remote_hfence_vvma_asid_rejects_invalid_asid_before_reporting_unsupported`,
`RiscvLinuxBootHandoffConfig`, and RISC-V DTB handoff tests.

**Next evidence:** Broader static libc program coverage, followed by RFENCE
hypervisor-fence execution tests, then a real Linux boot smoke.

### Devices and Platforms - 50% single-axis

**Score calculation:** 5 of 10 items have executable evidence, or 50% raw. The
bucket cap is single-axis because real Linux driver interaction, host
networking, non-RISC-V boards, and coherent DMA timing are not complete.

- [x] MMIO bus, UART, PL011, CLINT, PLIC, RTC, timers, and interrupt routes have tests.
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
tests, PCI/VirtIO/storage/network checkpoint tests, CLINT/PLIC/UART tests.

**Next evidence:** Board-level Linux boot with console, timer, storage, and
network evidence.

### Stats, Probes, Debug, Host Actions, and Checkpointing - 59% single-axis

**Score calculation:** 10 of 15 items have executable evidence, or 67% raw,
capped to 59% by the single-axis bucket. The bucket cap is single-axis because
probe, debug, power, and checkpoint evidence is not yet integrated across O3
pipeline and cache/DRAM runtime state.

- [x] Hierarchical stats, reset/dump history, and CLI stats artifacts exist.
- [x] Probe registry plus real RISC-V retired-instruction and data-access producers exist.
- [x] m5 exit/fail/stats/checkpoint/work markers reach typed host actions.
- [x] Decode-first checkpoint capture/restore exists across scheduler, memory, device, storage, VirtIO, timer, interrupt, RISC-V hart run-state, platform, workload, and manifest owners.
- [x] GDB remote packet/session parsing and RISC-V integer/PC register paths exist.
- [x] Power and thermal models plus external power-analysis exports exist.
- [x] Host actions and guest events are typed and checkpoint-aware.
- [x] First-class histogram stats have registry snapshots, deltas, resets,
  CLI JSON/text bucket output, and real data-access stack-distance producer
  output.
- [x] RISC-V in-order pipeline timing state is captured and restored by core checkpoints.
- [x] GDB packet byte streams drive typed step/resume and break/watch control state in memory-backed sessions.
- [ ] Stricter gem5 text-stat compatibility exists.
- [ ] Cache/bank/fabric/DRAM hierarchy counters are complete.
- [ ] GDB socket loop, step/resume/break/watch integration, and full FP/vector/CSR register cache exist.
- [ ] Power and thermal models are calibrated against real component activity.
- [ ] O3 pending-state checkpoints exist.

**Migrated:** Structured stats, real RISC-V probe producers, checkpoint banks,
m5ops, host actions, GDB packet/session parsing, RISC-V integer/PC debug
register paths, RISC-V software breakpoint patch/restore through the system GDB
memory handler, gem5-style final-tick stat aliases, target/port/bank-level DRAM
runtime resource counters, RISC-V in-order pipeline checkpoint capture/restore,
GDB byte-stream packet handling, debug execution-control state for packet-stream
step/resume and break/watch requests, fixed-width register-cache seeding, O3
writeback transfer deferred-completion checkpoint payloads, and custom plus
McPAT-shaped and DSENT-shaped power-analysis exports.

**Not migrated:** Complete gem5 text-stat parity, full debug execution control,
runtime resource counters, runtime-calibrated power/thermal, and broad O3
ROB/LSQ/rename checkpoint ownership.

**Evidence:** `StatsRegistry`, `ProbeRegistry`, `RiscvInstructionStats`,
`RiscvDataAccessStats`, `SystemActionExecutor`, `GdbRemoteSession`,
checkpoint tests including RISC-V hart run-state and in-order pipeline restore,
O3 writeback transfer deferred-state payload tests, GDB byte-stream and
control-state tests, `gdb_remote_packet` execution-control tests,
power-analysis export tests, system GDB software breakpoint patch/restore
tests, CLI data-access probe tests, and histogram registry/output tests.
The CLI data-access probe tests include stack-distance histogram stats emitted
from executed RISC-V loads. CLI DRAM-backed execution tests include
target/port/bank-level DRAM resource counters emitted from executed RISC-V
instruction fetches. CLI text stats include gem5-style final-tick aliases
emitted from an executed RISC-V run.

**Next evidence:** Broader gem5 text-stat compatibility, remaining
cache/bank/fabric runtime resource counters, GDB execution-control tests, and
O3 checkpoint capture/restore.

### Configuration, Resources, Suites, GPU, and Accelerators - 50% single-axis

**Score calculation:** 6 of 12 items have executable evidence, or 50% raw. The
bucket cap is single-axis because GPU memory behavior is visible inside the GPU
execution path but does not yet drive cache/DRAM, and broad acquisition and
benchmark orchestration remain absent. The library-level in-memory acquisition
executor is tracked as scoped evidence but is not counted as top-level
workload-resource migration until a CLI or runtime workload path consumes it.

- [x] CLI `run`, `gups`, and `trace-replay` plus TOML configuration have tests; `gups` emits traffic profile summaries from the executed controller.
- [x] Workload manifests, resource identity, disk-image construction records, and suite planning exist.
- [ ] CLI/runtime workload-resource acquisition consumes a resource executor for manifest and suite required artifacts.
- [x] GPU and accelerator command routing, DMA routes, topology validation, and replay evidence exist.
- [x] Dispatch plans and execution summaries expose typed parallel evidence.
- [ ] gem5-style ergonomic experiment definitions cover broad full-system sweeps.
- [ ] External workload-resource acquisition executors cover host, network, archive, and broader artifact kinds.
- [x] GPU ISA-level execution exists.
- [x] GPU queued workgroups expose compute-unit assignment and coalesced memory access records from scalar ISA memory intents.
- [ ] GPU CU scheduling, memory coalescing, and cache/DRAM interactions are representative.
- [ ] Multi-run simulator orchestration and artifact compatibility are complete.
- [ ] PARSEC or comparable workload suites run end to end.

**Migrated:** Typed configuration, manifests, suite dispatch, resource identity
and suite-level required-resource declarations, library-level in-memory
acquisition records for declared artifacts, GPU/accelerator shells, DMA routing,
and a minimal GPU
scalar ISA program execution path with completion, queued-workgroup snapshot
evidence, visible compute-unit assignment, coalesced memory access records, and
top-level GUPS traffic profile JSON/stats output.

**Not migrated:** Full gem5 stdlib ergonomics, host/network/archive resource
acquisition executors, CLI workload-resource acquisition, broad GPU ISA
semantics, GPU cache/DRAM interaction, and broad benchmark orchestration.

**Evidence:** `Rem6RunConfig`, `run_from_config`, `WorkloadManifest`,
`WorkloadResource`, `WorkloadSuiteReplayPlan`,
`WorkloadInMemoryResourceAcquisitionExecutor`, suite tests, resource
acquisition executor tests, `rem6 gups` profile-summary CLI tests, GPU and
accelerator topology tests, and GPU compute tests covering scalar ISA execution,
coalesced memory records, and snapshot restore of queued ISA programs.

**Next evidence:** Host-backed and archive-backed workload acquisition,
data-driven full-system workload declarations, and GPU memory requests through
cache/DRAM.

## Test Migration Ledger

This table is a crosswalk from gem5 test anchors to rem6 owners. Its estimates
are compact row-level status markers, not component scores. `Row score` entries
use the same percentage and bucket vocabulary as component scores, but the
checklist-backed component sections above define the auditable percentages.

| gem5 test anchor | rem6 owner | Row score | Migrated boundary | Next evidence |
| --- | --- | --- | --- | --- |
| `tests/gem5/arm_boot_tests` | future ARM ISA crate, `rem6-platform` | 0% open | ARM device slices exist, but this row requires Arm ISA boot. | Add Arm ISA, board handoff, device tree, and kernel boot tests. |
| `tests/gem5/asmtest` | ISA crates, `rem6` CLI | 50% single-axis | RISC-V no-libc and ISA unit tests cover selected instruction, ecall, and scalar FP directed integer-to-float paths. | Split RV32/RV64 and extension families with architectural-state comparison. |
| `tests/gem5/checkpoint_tests` | `rem6-checkpoint`, subsystem checkpoint banks | 65% representative | Scheduler, memory, devices, storage, VirtIO, timer, interrupt, RISC-V started/stopped/suspended hart run-state, RISC-V in-order pipeline state, platform, workload, and manifest checkpoints exist. | Add O3 and non-quiescent restore evidence. |
| `tests/gem5/chi_protocol` | `rem6-coherence`, protocol crates, `rem6-cache` | 40% single-axis | CHI-like line, controller, bank, dirty peer sourcing, reservation, and Evict-hazard tests exist. | Add Ruby-scale CHI transactions, topology networks, directory races, and workload checks. |
| `tests/gem5/chi_tlm_tests` | `rem6-proto`, future adapter crates, `rem6-coherence` | 19% scoped | A library-level co-simulation boundary can register TLM endpoints, validate transaction shape, hand off events, and checkpoint clean adapter state in self-tests. | Add runtime TLM bridge tests with coherence traffic. |
| `tests/gem5/config_output_files` | `rem6` CLI, `rem6-workload` | 45% single-axis | CLI output paths, stats-output paths, JSON artifacts, and text stats output tests exist. | Add config-driven file layouts for full-system manifests and multi-artifact workloads. |
| `tests/gem5/cpu_tests` | `rem6-cpu`, `rem6-system` | 30% unit-slice | Atomic RISC-V execution, frontend slices, retired predictor training, direct completed-fetch overlap in in-order timing, bounded normal-driver straight-line fetch-ahead, per-retired-instruction in-order stage timing stats, and O3 policies exist. | Add broader in-order stalls/squashes and ROB/LSQ-backed O3 execution tests. |
| `tests/gem5/dram_lowp` | `rem6-dram`, `rem6-power` | 40% single-axis | DRAM/NVM profile counters and low-power constants are surfaced. | Add executable low-power state transition tests through routed requests. |
| `tests/gem5/example_configs`, `tests/gem5/learning_gem5` | `rem6` CLI, `rem6-platform`, `rem6-workload` | 40% single-axis | CLI and TOML tests cover several execution and trace-replay paths. | Add rem6 examples that run from data files without recompilation. |
| `tests/gem5/fdp_tests` | `rem6-cache` | 45% single-axis | Fetch-directed prefetcher state, errors, and cache-local queue/translation counters have cache tests. | Add FDP execution through cache-bank and CPU/frontend consumers. |
| `tests/gem5/fs` | `rem6-platform`, `rem6-system`, device crates | 15% scoped | Generic device and handoff slices exist, but the gem5 row is mainly full-system boot. | Add full-system Linux boot with SBI, console, storage, network, timer, and shutdown evidence. |
| `tests/gem5/gem5_resources` | `rem6-workload` | 35% unit-slice | Resource declarations, identity, provenance, disk-image construction records, and library-level in-memory acquisition executor records exist. | Add CLI/runtime acquisition, host-backed, archive-backed, and broader artifact-kind acquisition coverage. |
| `tests/gem5/gpu` | `rem6-gpu`, `rem6-accelerator`, `rem6-transport` | 35% unit-slice | GPU and accelerator topology, command, DMA route, scalar ISA, CU assignment, and coalesced memory-record tests exist. | Add representative CU scheduling and cache/DRAM interactions. |
| `tests/gem5/insttest_se` | future SPARC owner, ISA crates | 10% scoped | Current RISC-V evidence belongs under `asmtest`; this gem5 anchor is SPARC SE focused. | Add SPARC or explicitly retire the row as out of scope. |
| `tests/gem5/kvm_fork_tests`, `tests/gem5/kvm_switch_tests` | `rem6-system`, future host adapters | 10% scoped | Host-assisted takeover admission rejects unsafe switch shapes. | Add explicit fast-forward adapter and KVM-like switch/fork tests. |
| `tests/gem5/m5_util`, `tests/test-progs/m5-exit` | `rem6-isa-riscv`, `rem6-system`, `rem6-workload` | 50% single-axis | RISC-V m5 exit, fail, stats, checkpoint, and work markers reach typed host actions. | Add payload breadth, repeat scheduling, other ISA entries, and clock-domain behavior. |
| `tests/gem5/m5threads_test_atomic` | `rem6-isa-riscv`, `rem6-cpu`, `rem6-coherence` | 40% single-axis | RISC-V LR/SC and AMO plus coherence reservation invalidation tests exist. | Add multi-threaded SE or full-system atomic tests through shared memory. |
| `tests/gem5/se_mode` | `rem6-system`, `rem6` CLI | 50% single-axis | RISC-V SE startup, ecalls, static newlib smokes including `fopen("w+")` create/write/readback, `/proc/self/exe` readlink through direct `readlinkat` ecall, pipe roundtrip through direct `pipe2`/`write`/`read`/`close` ecalls, `open` directory traversal with `O_DIRECTORY` and `O_CLOEXEC`, and `open` regular-file access with `O_NOCTTY` and `O_NOFOLLOW` through legacy `open` with newlib/libgloss flags, selected syscalls including `statx`, `statfs`/`fstatfs`, `sysinfo`, value-mode `riscv_hwprobe`, `ppoll`, in-place `mremap`, `mprotect`, `madvise` known-advice and mapped-range validation, `msync` flags and mapped-range validation, `mlock`/`munlock` `mmap`/`brk` range validation, `ftruncate`, `pread64`, `pwrite64`, `sched_getscheduler`, `sched_getparam`, `sched_get_priority_max/min`, `sched_rr_get_interval`, single-word `sched_setaffinity`/`sched_getaffinity`, single CPU/node `getcpu`, single-process `membarrier` slice, zero-duration `nanosleep` and `clock_nanosleep` validation, `clock_getres`, `CLOCK_TAI` `clock_gettime`, `kill(..., 0)`, `tkill(..., 0)`, and `tgkill(..., 0)` existence checks, current-process scoped process-group/session `setpgid`/`getpgid`/`getsid`/`setsid` slices, gem5-style advisory `setuid`/`setrlimit` success returns, legacy `getrlimit` stack/data/NPROC limits, basic `rt_sigaction`/`rt_sigprocmask`, empty `rt_sigpending` mask reporting, no-pending zero-timeout `rt_sigtimedwait`, futex mismatch and wake-bitset count/bitset behavior, `umask` masking for `mkdirat` directories and `openat(O_CREAT)` regular files, cwd-aware registered paths, guest-backed file output/readback and open visibility, at-family file and directory mutation, registered-directory enumeration, `ENOSYS` records, and guest writes exist. | Split hello, multicore SE, RVV intrinsic, and other-ISA subrows; add broader libc and lifecycle behavior. |
| `tests/gem5/memory` | `rem6-memory`, `rem6-cache`, `rem6-dram`, `rem6-fabric` | 56% single-axis | Stores, page maps, cache banks, topology slices, optional single-core CLI RISC-V MSI-bank, MESI-line, MOESI-line, and CHI-line data-cache routing, DRAM/NVM counters, CLI-selectable JEDEC-style refresh presets, prefetch queue counters, and fabric activity exist. | Add CPU-facing multi-level cache, NoC, broader DRAM refresh breadth, and full preset coverage. |
| `tests/gem5/multisim`, `tests/gem5/suite_tests` | `rem6-workload`, `rem6-kernel` | 45% single-axis | Suite planning, dispatch, execution summaries, and occupancy contracts exist. | Split multisim checkpoint restore from suite dispatch and add multi-run orchestration. |
| `tests/gem5/parsec_benchmarks` | `rem6-workload`, `rem6-system`, ISA crates | 0% open | Workload suites exist, but PARSEC-class programs do not run. | Add static or dynamic user workload support and ROI/stat hooks. |
| `tests/gem5/processor_switch_tests` | `rem6-system`, `rem6-cpu` | 20% unit-slice | Host-assisted switch admission and execution-mode metadata exist. | Add executable CPU model switching with quiescence and state transfer. |
| `tests/gem5/py_port` | `rem6` CLI, `rem6-workload` | 0% open | No Python embedding port exists. | Decide on a typed external control adapter or document a Rust/CLI replacement. |
| `tests/gem5/pyunit` | rem6 test crates, `rem6-workload`, `rem6-stats` | 35% unit-slice | Rust tests cover selected typed stats, workload, config, and helper behavior. | Map each pyunit helper family to a Rust owner. |
| `tests/gem5/readfile_tests` | `rem6-platform`, `rem6-system`, `rem6` CLI | 25% unit-slice | DTB/initrd handoff and CLI input-file plumbing exist. | Add guest-visible readfile device or replacement semantics. |
| `tests/pyunit` | `rem6-stats`, `rem6-workload`, future utility owners | 35% unit-slice | Selected pystats and stdlib semantics are covered by typed Rust tests. | Split HDF5, pystats, registry/probes, stdlib helpers, and parsing rows. |
| `tests/gem5/regression_tests` | all rem6 crates | 35% unit-slice | Workspace tests act as the current regression suite. | Add migration tags or per-family regression rows. |
| `tests/gem5/replacement_policies` | `rem6-cache` | 60% representative | Multiple replacement, indexing, dueling, compressed, and sector tag tests exist. | Add remaining policies and exact trace/reference parity where useful. |
| `tests/gem5/riscv_boot_tests` | `rem6-platform`, `rem6-system`, `rem6-isa-riscv`, `rem6-cpu`, `rem6-kernel` | 35% unit-slice | DTB/initrd handoff, CLINT/PLIC, traps, CSRs, page-fault causes, translated faults, SBI base read-only ecalls, minimal TIME `set_timer` STIP scheduling, IPI `send_ipi` SSIP pending-bit injection, standard SRST shutdown stop requests, RFENCE remote SFENCE.VMA data TLB flushes with finite-range, ASID scope, and scheduled completion events, unsupported HFENCE validation, and HSM start entry-state, `START_PENDING`, status, no-return stop, retentive-suspend, default-non-retentive `RESUME_PENDING`/resume, and IPI-wake slices are tested. | Add broader SBI timer/IPI/reset power-state behavior, remaining HSM wake semantics, RFENCE hypervisor-fence execution semantics and broader completion coverage, and a real Linux boot smoke. |
| `tests/gem5/stats` | `rem6-stats`, `rem6` CLI, `rem6-power` | 62% representative | Hierarchical counters, reset/dump histories, deltas, first-class histogram buckets, real probe producers, power bindings, instruction/data cache counters, cache-local prefetch queue counters, CLI stat output, and library-level McPAT/DSENT-shaped export self-tests exist. | Add more hierarchy counters, power-export CLI/runtime wiring, and stricter text-stat compatibility. |
| `tests/gem5/stdlib` | `rem6-workload`, `rem6-platform`, `rem6` CLI | 40% single-axis | Workload manifests, resource payloads, library-level in-memory resource acquisition records, suite dispatch plans, Linux handoff intent, and TOML/CLI tests exist. | Add broader stdlib object coverage and ergonomic topology/workload definitions. |
| `tests/test-progs` | `rem6-system`, `rem6` CLI, ISA crates | 35% unit-slice | Static RISC-V no-libc, newlib, and raw syscall smoke binaries, including `statx`, `sysinfo`, newlib file-create roundtrip, newlib `/proc/self/exe` readlink coverage, newlib pipe2 roundtrip coverage, newlib directory-open coverage, and newlib open-flag coverage, are generated when tools exist. | Add durable generated fixtures for hello, threads, and m5 utility shapes across ISAs. |
| `tests/gem5/traffic_gen` | `rem6-traffic`, `rem6-system`, `rem6-workload`, `rem6` CLI | 55% single-axis | Text config parsing, GUPS, packet trace replay, flags, maintenance, HTM, responses, workload summaries, typed generator/memory-profile summaries, and top-level GUPS profile JSON/stats output exist. | Add cache hierarchy matrix and broader trusted stats. |
| `tests/gem5/x86_boot_tests` | `rem6-isa-x86`, future platform work | 0% open | Narrow x86 prefix and interrupt-flag semantics exist, but no x86 boot path exists. | Add x86 ISA execution, paging, interrupt, platform, and boot-image tests. |

## External Adapter Migration

### SystemC and TLM Adapters - 19% scoped

**Score calculation:** 1 of 4 items have executable evidence, or 25% raw. The
bucket cap is scoped because the current code is a library-level typed boundary
with self-tests; no runtime SystemC/TLM model adapter executes through `rem6`.

- [x] A typed co-simulation adapter boundary exists.
- [ ] Adapter event handoff executes from a runtime SystemC/TLM bridge.
- [ ] Adapter checkpoint capture and restore are consumed by a runtime adapter.
- [ ] Runtime SystemC/TLM model integration executes through the adapter.

**Migrated:** `CoSimAdapterBoundary` registers SystemC/TLM endpoints, hands off
typed events with required transaction shape, records acknowledgements, rejects
ambiguous handoff, and snapshots or restores only clean boundaries in
`rem6-proto` self-tests.

**Not migrated:** Runtime `src/systemc`, `util/tlm`, and `ext/systemc`
behavior.

**Evidence:** `cosim_adapter` tests in `rem6-proto`.

**Next evidence:** Runtime model integration tests through the adapter.

### SST Adapter - 19% scoped

**Score calculation:** 1 of 4 items have executable evidence, or 25% raw. The
bucket cap is scoped because SST evidence is the same library-level adapter
boundary, not a runtime SST execution path.

- [x] A typed SST adapter boundary exists.
- [ ] SST traffic handoff executes from a runtime SST bridge.
- [ ] SST adapter checkpoint capture and restore are consumed by runtime SST state.
- [ ] Runtime SST execution uses the adapter.

**Migrated:** `CoSimAdapterBoundary` registers SST endpoints and accepts typed
traffic-packet handoff with required transaction shape through the same
external adapter contract in `rem6-proto` self-tests.

**Not migrated:** SST-specific checkpoint tests and runtime `ext/sst` plus
`configs/example/sst` behavior.

**Evidence:** `cosim_adapter` tests in `rem6-proto`.

**Next evidence:** SST-specific checkpoint and runtime handoff tests.

### Power and Physical-Design Export Adapters - 50% single-axis

**Score calculation:** 3 of 6 items have executable evidence, or 50% raw. The
bucket cap is single-axis because McPAT-shaped and DSENT-shaped exports exist,
but ingestion, full schema parity, and NoMali evidence remain absent.

- [x] rem6-power can export typed power-analysis records.
- [x] McPAT-shaped XML export serializes power, thermal, and residency records.
- [x] DSENT-shaped CSV export serializes power, thermal, and residency records.
- [ ] McPAT-compatible ingestion/export parity is complete.
- [ ] DSENT-compatible ingestion/export parity is complete.
- [ ] NoMali-compatible GPU adapter evidence exists.

**Migrated:** Typed power-analysis export records and deterministic custom XML
smoke coverage for totals, components, and residency entries, plus deterministic
McPAT-shaped XML and DSENT-shaped CSV exports.

**Not migrated:** Complete `ext/nomali`, `ext/mcpat`, and `ext/dsent` parity,
plus top-level CLI/runtime export wiring from real simulation activity.

**Evidence:** rem6-power power-analysis export self-tests including custom XML,
McPAT-shaped XML, and DSENT-shaped CSV output.

**Next evidence:** Adapter ingestion and stricter external schema parity tests.

### Native Loader and Math Replacement - 50% single-axis

**Score calculation:** 2 of 4 items have executable evidence, or 50% raw. The
bucket cap is single-axis because loader and softfloat matrix breadth is not
complete.

- [x] Native ELF loading reaches executable RISC-V SE smoke paths.
- [x] Native DTB handoff records exist.
- [ ] libelf replacement breadth covers the needed gem5 loader matrix.
- [ ] softfloat replacement breadth covers all FP rounding and exception paths.

**Migrated:** Native Rust loader and DTB handoff slices plus RV64F/RV64D
scalar load/store, arithmetic, comparison, conversion, NaN-boxing, and accrued
flag slices.

**Not migrated:** Complete `ext/libelf`, `ext/libfdt`, and `ext/softfloat`
parity.

**Evidence:** CLI static RISC-V smoke tests, RISC-V DTB handoff tests, and
RV64F/RV64D tests.

**Next evidence:** Expand loader breadth and soft-float parity.

## Open Migration Gaps

1. Connect in-order and O3 CPU state to executable engines.
2. Run more real static-libc SE programs, then expand SBI runtime coverage and real Linux boot.
3. Route CPU instruction/data traffic through cache, coherence, NoC, and DRAM.
4. Promote hierarchy resource counters to first-class stats.
5. Split broad migration rows as evidence grows, especially `se_mode`,
   `cpu_tests`, `traffic_gen`, `stats`, and `tests/test-progs`.

## Update Rules

- Update percentages only when executable rem6 evidence changes.
- Keep the checklist beside each component so the score can be audited.
- Do not count unknown-syscall diagnostics as implemented syscall coverage.
- Do not count tool-detected static smokes as broad workload parity.
- Do not cite exact line ranges from gem5 or rem6.
- Keep detailed proof in tests, artifacts, traces, checkpoints, or manifests
  instead of expanding this document into a proof log.
