# gem5 to rem6 Alignment

This document is the compact module map between the read-only gem5 reference
tree at `temp/reference_designs/gem5` and rem6. It tracks current ownership,
coverage, evidence boundaries, and the remaining work needed for rem6 to become
a gem5-class, cycle-accurate, heterogeneous full-system simulator with stronger
typing, parallel execution, reproducibility, and maintainability.

The gem5 tree is audit input only. rem6 production code must not import it,
execute it, depend on its build outputs, or require it at runtime.

## Research Anchors

The long-term design choices are driven by recurring gem5 pain points:

- Global and cross-queue event scheduling remains hard to reason about. rem6
  makes partition ownership, lookahead, deterministic merge order, remote
  delivery windows, scheduler snapshots, wait-for diagnostics, and actual or
  planned worker-lane occupancy typed runtime data.
- Python SimObject composition is flexible but weakly checked. rem6 keeps the
  composability goal through typed builders, TOML/CLI inputs, workload
  manifests, resource payloads, and identity-hashed provenance.
- Ruby and protocol generation are coupled to build-time infrastructure. rem6
  keeps protocols as ordinary Rust crates with typed snapshots, explicit
  controller errors, and tested trace/workload consumers.
- Several gem5 issue classes come from broad mutable objects or hidden
  side-effects. rem6 prefers decode-first validation, staged restore/capture,
  explicit error variants, and runtime artifacts that expose the evidence used
  by tests and replay contracts.

## Audit Method

Each row below records:

- the stable gem5 source anchor;
- the rem6 crate or crates that own the equivalent behavior;
- the current coverage state;
- the evidence boundary and the remaining alignment target.

Coverage levels:

- `covered`: rem6 has typed runtime behavior and tests for the corresponding
  capability.
- `partial`: rem6 has a narrower equivalent or early model, but not yet the
  full gem5-class surface.
- `planned`: the gem5 capability is in scope but no production rem6 model owns
  it yet.
- `external-adapter`: gem5 integrates an external simulator or library; rem6
  should keep the interoperability value behind typed adapters, not make the
  external package part of the simulator core.

Local gem5 file counts are sizing hints only. Acceptance evidence must be a
rem6 test, typed trace, runtime summary, checkpoint record, CLI artifact, or
explicit error.

## Module Map

| gem5 source anchor | rem6 owner | Coverage | Current state and remaining target |
| --- | --- | --- | --- |
| `src/arch` | `rem6-isa-riscv`, future ISA crates | partial | RISC-V owns RV64I/M/A/C scalar coverage, CSR/trap foundations, Sv39 helpers and CPU walker integration, PMP/PMA, vector state foundations, initial RV64F/RV64D scalar load/store, arithmetic, fused multiply-add/subtract, square root, comparisons, conversions, NaN-boxing, and accrued FP flag paths. Remaining work includes full F/D coverage, broader compressed edge cases, complete vector execution, hardware Sv39 fetch coverage, and ARM, x86, Power, SPARC, MIPS, and AMDGPU ISA ownership. |
| `src/cpu` | `rem6-cpu`, `rem6-kernel`, `rem6-system`, `rem6-traffic` | partial | RISC-V atomic execution, parallel clusters, data access issue/response, store-conditional progress diagnostics, HTM core checkpoints, basic/GShare/Tournament branch-predictor training from real retired control-flow, typed in-order pipeline state, and O3 policy pieces exist. The large gap is executable cycle-visible CPU integration: in-order fetch/decode/execute timing, branch predictor steering and rollback, checker support, and O3 ROB/LSQ/rename/FU execution are still open. |
| `src/mem` | `rem6-memory`, `rem6-transport`, `rem6-cache`, `rem6-directory`, `rem6-coherence`, `rem6-dram`, `rem6-fabric`, protocol crates | partial | Memory stores, translation queues, TLB/page-map state, transport routes, replacement policies, MSHR resources, MSI/MESI/MOESI/CHI line and bank models, MSI CacheBank-backed RISC-V and accelerator-DMA topology data paths, DRAM/NVM profiles, packet/fabric activity, trace replay consumers, and workload data-cache attribution exist. Remaining targets include fuller CHI transactions, CPU-facing multi-level cache integration, bank scheduler/resource summary parity, prefetch translation consumers, NoC router/flit detail, DRAM refresh and JEDEC preset breadth, and Ruby-scale protocol/test coverage. |
| `src/sim`, `src/python` | `rem6`, `rem6-platform`, `rem6-system`, `rem6-workload` | partial | CLI `run`, `gups`, and `trace-replay` paths plus TOML configuration, typed platform builders, workload manifests, suite dispatch plans, resource payload validation, and durable JSON/text stats artifacts exist. Remaining work is a broader gem5-style experiment surface for full-system topology sweeps, richer platform declarations, and external workload-resource acquisition. |
| `src/dev` | `rem6-mmio`, `rem6-uart`, `rem6-timer`, `rem6-interrupt`, `rem6-pci`, `rem6-virtio`, `rem6-storage`, `rem6-net`, `rem6-platform`, `rem6-system` | partial | Typed models exist for MMIO, UARTs, CLINT/PLIC, RTCs, ARM timers/watchdogs, PL011, PCI, VirtIO MMIO/PCI, block/console/RNG devices, storage images, SimpleDisk, IDE, Ethernet switching, and SINIC. Storage and SINIC are wired into topology/system paths. Remaining devices include PS/2, QEMU bridge, real TUN/TAP, non-RISC-V board trees, and broader platform-specific devices. |
| `src/gpu-compute` | `rem6-gpu`, `rem6-accelerator`, `rem6-transport` | partial | GPU and accelerator shells, command routing, DMA routes, topology validation, and replay evidence exist. ISA-level GPU execution, richer CU scheduling, memory coalescing, and full gem5 GPU test parity remain open. |
| `src/kern` | `rem6-system`, `rem6-platform`, `rem6-workload` | partial | RISC-V Linux handoff can install DTB/initrd payloads and set A0/A1 through typed APIs. True Linux boot still needs SBI firmware/runtime behavior, remaining privileged CSR/trap/device coverage, and kernel-facing platform breadth. |
| `src/base` | `rem6-kernel`, `rem6-stats`, `rem6-debug`, shared utilities | partial | Typed statistics, reset/dump history, probes, GDB packet/session parsing, scheduler clocks, wait-for diagnostics, and checkpoint helpers exist without a broad utility crate. Remaining work is gem5 unittest migration breadth and additional typed helper coverage where tests demand it. |
| `src/debug` and `src/gdbremote` | `rem6-debug`, `rem6-system`, ISA crates | partial | GDB remote packet parsing, session state, register read/write for RISC-V integer/PC state, target XML serving, memory access, break/watch requests, detach/kill, and retransmission behavior exist. Remaining work includes complete ISA register-cache breadth, socket loop integration, richer debug flags, and disassembly/exec trace surfaces. |
| `src/proto`, packet traces, traffic generators | `rem6-proto`, `rem6-traffic`, `rem6-system` | partial | gem5 packet trace framing, gzip wrapper detection, many memory commands, LLSC, HTM, maintenance, response/error matching, sideband consumers, deterministic replay, shard planning, and workload trace evidence exist. Remaining work is broader packet flags, additional traffic generator modes, and end-to-end consumers across more CPU/cache/memory paths. |
| `src/systemc`, `util/tlm`, `ext/systemc` | future adapter crates | external-adapter | Interoperability is useful, but the rem6 scheduler must stay the timing authority. |
| `ext/sst`, `configs/example/sst` | future SST adapter | external-adapter | Co-simulation should be explicit, checkpoint-aware, and isolated from core runtime contracts. |
| `ext/nomali`, `ext/mcpat`, `ext/dsent` | `rem6-power`, future optional adapters | external-adapter | rem6-power already emits typed external power-analysis exports. Optional external adapters should consume typed records rather than become core dependencies. |
| `ext/libelf`, `ext/libfdt`, `ext/softfloat` | `rem6-boot`, `rem6-platform`, ISA crates | partial | ELF loading and RISC-V FDT generation are native typed Rust paths. Remaining work includes loader breadth for more ISAs, soft-float parity, and broader bootloader handoff coverage. |
| `util/gem5art`, resources tooling, disk image tooling | `rem6-workload`, future artifact tooling | partial | Workload manifests carry resource provenance, disk-image construction records, resolved payload identity, DTB/initrd intent, and replay validation. Acquisition and construction executors for more artifact kinds remain open. |
| `tests`, `tests/test-progs`, `util/statetrace` | rem6 tests and trace tooling | partial | gem5 tests are migration input. rem6 acceptance is Rust tests plus typed trace, artifact, stats, and checkpoint evidence. `docs/architecture/gem5-test-migration.md` now tracks the durable test-anchor migration ledger. |

## Detailed Anchor Map

| gem5 source anchor | rem6 owner | Coverage | Current state and remaining target |
| --- | --- | --- | --- |
| `src/arch/riscv` | `rem6-isa-riscv`, `rem6-cpu`, `rem6-system` | partial | RISC-V architectural state, decode/execute slices, traps, CSR, translation, PMP/PMA, SE ecalls, and GDB target descriptions exist. Remaining work is full RV64GC/vector breadth, Linux privileged path completion, and broader ABI/syscall/device interactions. |
| `src/arch/x86` | `rem6-isa-x86`, future CPU/system work | partial | Narrow prefix-scan and interrupt-flag semantics are tested. Full x86 decode, execution, paging, interrupts, KVM-style switching, and device integration remain open. |
| `src/arch/arm` | `rem6-platform`, `rem6-uart`, `rem6-timer`, future ISA crate | partial | Several ARM platform devices are modeled, but Arm ISA execution, board boot, and debug register coverage remain open. |
| `src/cpu/simple` | `rem6-cpu` | partial | RISC-V atomic execution is functional and test-backed. TimingSimple-style cycle stalls and memory-mode interactions remain open. |
| `src/cpu/minor` | `rem6-cpu` | partial | rem6 has reusable in-order pipeline state, cycle summaries, and RISC-V retire events now emit in-order commit-cycle evidence for non-deferred instructions, completed memory-data accesses, and completed MMIO loads. A Minor-equivalent multi-stage executable frontend with fetch/decode stalls, squashes, and memory backpressure remains open. |
| `src/cpu/o3` | `rem6-cpu` | partial | O3 policy pieces exist for dependencies, issue, writeback, branch-target safety, and unblock behavior. A running O3 engine with ROB, LSQ, rename map, commit, store-to-load forwarding, and FU scheduling remains open. |
| `src/cpu/pred` | `rem6-cpu` | partial | Several typed predictors exist, and the RISC-V core now trains basic, GShare, and Tournament predictors from real retired control flow. Fetch steering, speculation snapshots, rollback, and richer predictor consumers remain open. |
| `src/cpu/kvm`, `src/arch/*/kvm` | `rem6-system`, future host adapters | partial | Host-assisted switch admission is typed and tested for unsafe takeover shapes. Real KVM fast-forward execution remains open. |
| `src/cpu/testers` | `rem6-traffic`, `rem6-workload` | partial | Traffic generators, GUPS, trace replay, and workload replay cover some tester roles. Full tester breadth and gem5 test-program migration remain open. |
| `src/mem/cache` | `rem6-cache`, `rem6-system` | partial | Replacement, MSHR, maintenance, bank, workload data-cache consumers, and an MSI CacheBank/MSHR responder for RISC-V and accelerator-DMA topology data accesses exist. CPU-facing L1/L2/L3 integration, bank-backed scheduler/resource accounting parity, more protocol bank responders, and broader cache policies remain open. |
| `src/mem/ruby` | `rem6-coherence`, protocol crates, `rem6-directory` | partial | MSI/MESI/MOESI/CHI line and harness coverage exists without generated protocol code. Ruby-scale transactions, networks, and tests remain open. |
| `src/mem/dram_interface`, `src/mem/mem_ctrl` | `rem6-dram` | partial | DDR/HBM/LPDDR/NVM profile counters and routed execution exist. Refresh, validated preset breadth, detailed scheduling policies, and full QoS coupling remain open. |
| `src/mem/probes`, `src/mem/packet_queue` | `rem6-stats`, `rem6-transport`, `rem6-traffic` | partial | Real retired-instruction and RISC-V data-access probe producers feed stats trackers, and transport traces expose per-route latency counters. More probe consumers and queue-level stats remain open. |
| `src/dev/riscv` | `rem6-interrupt`, `rem6-timer`, `rem6-platform`, `rem6-system` | partial | CLINT, PLIC, DTB handoff, initrd handoff, and RISC-V interrupt routes exist. SBI runtime, richer platform devices, and Linux boot validation remain open. |
| `src/dev/serial`, `src/dev/uart` | `rem6-uart`, `rem6-mmio` | partial | UART and PL011 register models, interrupts, snapshots, and platform attachment exist. Additional UART variants and board-specific wiring remain open. |
| `src/dev/storage`, `src/dev/virtio` | `rem6-storage`, `rem6-virtio`, `rem6-pci`, `rem6-system` | partial | Storage images, SimpleDisk, IDE, VirtIO block/console/RNG, PCI/MMIO config, and checkpoint banks exist. More disk formats, queue features, and board coverage remain open. |
| `src/dev/net` | `rem6-net`, `rem6-pci`, `rem6-system` | partial | Ethernet links/switching and SINIC MMIO/PCI/DMA/checkpoint paths exist. Real host networking, more NICs, and TAP/TUN adapters remain open. |
| `src/sim/syscall_emul` | `rem6-system`, `rem6` CLI | partial | RISC-V SE has real ecall wiring, startup stack, newlib smoke coverage, typed unknown-syscall records, and many file/time/resource syscalls. Process/thread lifecycle, blocking waits, host filesystem policy, and broad Linux table parity remain open. |
| `src/sim/pseudo_inst` | `rem6-isa-riscv`, `rem6-system`, `rem6-workload` | partial | RISC-V gem5-style m5 exit/fail/stats/checkpoint/work markers reach typed host actions. Clock-domain details, repeat scheduling, payload breadth, and other ISA entries remain open. |
| `src/sim/serialize` | `rem6-checkpoint`, subsystem checkpoint banks | partial | Decode-first checkpoint capture/restore exists across scheduler, memory, devices, storage, VirtIO, and workload manifests. Remaining work is broader component coverage and pending-state restore breadth. |
| `src/sim/stat_control`, `src/base/statistics` | `rem6-stats`, `rem6-power` | partial | Hierarchical counters, reset policies, dumps, deltas, probes, power metric binding, and external exports exist. First-class histograms and more runtime-resource counters remain open. |
| `configs`, `src/python/m5` | `rem6`, `rem6-platform`, `rem6-workload` | partial | TOML, CLI, typed builders, workload manifests, resource payloads, and suite plans provide checked composition. More ergonomic full-system experiment definitions and acquisition tooling remain open. |
| `tests/gem5`, `tests/test-progs` | rem6 test crates and `docs/architecture/gem5-test-migration.md` | partial | Many gem5-shaped behaviors have Rust tests, and the migration ledger now records core test anchors. Per-test parity rows remain open for continued migration. |

## Evidence Index

Current evidence is intentionally summarized here; detailed proof belongs in
tests and runtime artifacts.

- Kernel and scheduling: tests cover deterministic event order, conservative
  parallel epochs, worker limits, remote-delivery preflight, wait-for graphs,
  checkpoint quiescence, warmup/live restore separation, progress diagnostics,
  and worker-lane summaries.
- RISC-V execution: tests cover integer, compressed, atomics, fences, WFI,
  SFENCE.VMA, traps, counters, PMP/PMA, Sv39 translation and walker behavior,
  FP scalar slices, NaN-boxing, accrued FP flags, GDB register access, HTM
  checkpoints, store-conditional progress, in-order retire-cycle evidence for
  non-deferred instructions, completed memory-data instructions, and completed
  MMIO loads, retired-PC probes, data-access probes, and basic/GShare/Tournament
  branch-predictor training from real retired control flow.
- RISC-V SE mode: CLI/system tests drive real user-mode ecalls for startup
  stack construction, `brk`, `mmap`, stdio, registered guest files, stat/link
  operations, vector I/O, clock/getrandom/cwd/resource syscalls, `wait4`, and
  typed unknown-syscall `ENOSYS` records. Detection-based static newlib
  regressions build and run the same ELF under qemu and rem6 when the RISC-V
  toolchain is available.
- Memory, cache, coherence, and DRAM: tests cover stores, request/response
  payloads, page maps, TLBs, translation queues, replacement policies,
  MSHR-backed banks, write queues, maintenance operations, CHI/MESI/MOESI/MSI
  harnesses, MSI CacheBank-backed RISC-V and accelerator-DMA topology data
  paths, data-cache trace sidebands, DRAM/NVM profile counters, and fabric
  activity summaries.
- Devices and platform: tests cover MMIO buses, UARTs, CLINT/PLIC, RTC/timer
  devices, ARM timer/watchdog slices, PCI, VirtIO, storage, IDE, network/SINIC,
  RISC-V DTB/initrd handoff, checkpoint banks, and topology validation.
- Workloads and configuration: tests cover CLI/TOML parsing, run artifacts,
  stats-output files, trace replay, workload manifests, resource payloads,
  suite planning, dispatch expectations, exact remote-send contracts, and
  replay summaries.
- Stats, probes, power, and thermal: tests cover hierarchical counters,
  reset/dump histories, stat deltas, real RISC-V retired-instruction and
  data-access probe producers, power-state residency, expression power models,
  thermal networks, and external power-analysis exports.

## Open Alignment Work

- CPU timing integration is the main microarchitectural gap. The typed
  in-order and O3 pieces must become executable CPU engines with per-cycle
  fetch/decode/execute/commit behavior, branch predictor steering and rollback,
  ROB/LSQ/rename tables, FU latency tables, and precise memory-order behavior.
- The RISC-V software path must keep moving from strong SE smoke coverage to
  broader static-libc and distro-like coverage, then to full Linux boot through
  an SBI-class runtime and complete privileged/device interactions.
- ISA parity must expand beyond the current RISC-V focus. RISC-V still needs
  remaining F/D, vector, compressed, and exception-flag breadth; other major
  gem5 ISAs need crate ownership and test migration.
- CPU data paths still need end-to-end multi-level cache and NoC integration
  for normal instruction/data traffic, not only trace replay, topology harness,
  or direct-memory paths.
- Cache/coherence/DRAM need broader CHI, Ruby-scale scenarios, DRAM refresh and
  preset breadth, flit-level NoC routing, richer QoS, and additional prefetcher
  consumers.
- Full-system platform coverage needs broader devices, real TUN/TAP or host
  adapters where appropriate, non-RISC-V device trees, and board-level Linux
  boot validation.
- GPU, accelerator, and heterogeneous runtime support must advance from typed
  command/DMA models to ISA-level or workload-visible execution with memory
  hierarchy interactions.
- The configuration and workload-resource surface must reach gem5-style
  experiment ergonomics without giving up typed validation, reproducible
  manifests, and deterministic artifacts.
- The gem5 test migration ledger must grow from core test anchors into
  per-family rows that map gem5 unittest families to rem6 equivalents, missing
  features, and executable evidence.
- The stats surface should continue adding first-class histograms and
  bank/cache/fabric/DRAM hierarchy counters wherever runtime resources already
  expose typed activity.

## Maintenance Rules

- Keep this file as an overview. Put detailed evidence in tests, focused design
  docs, or runtime artifacts, and link only stable semantic anchors here.
- Do not cite exact line ranges from gem5 or rem6. Use directory names,
  functions, type names, test names, or documentation headings.
- Do not treat a green local test as broad parity. A test proves only the
  capability it actually exercises.
- Do not add rem6 production dependencies on the local reference tree.
- Keep module mapping honest. `partial` is the correct status until current
  evidence proves gem5-equivalent or stronger behavior.
