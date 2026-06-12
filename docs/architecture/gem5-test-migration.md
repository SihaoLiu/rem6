# gem5 Test Migration Ledger

This ledger tracks migration of the read-only gem5 test tree into rem6's Rust
tests, typed traces, runtime artifacts, and replay contracts. It is not parity
proof by itself. A row is complete only when the named gem5 capability has an
equivalent rem6 test or stronger typed evidence.

The reference tree under `temp/reference_designs/gem5` remains read-only audit
input. Do not copy test binaries, generated outputs, or build products into
rem6.

## Coverage Rules

- Use stable test anchors such as directory names, test names, workload names,
  and public helper APIs. Do not cite fragile line ranges.
- Keep rem6 evidence executable: Rust tests, CLI artifacts, stats dumps,
  checkpoint records, workload manifests, or typed trace replay summaries.
- Mark a row `covered` only when current rem6 tests exercise the same behavior
  boundary. Use `partial` when rem6 covers a narrower slice, and `open` when
  the equivalent is not yet implemented.
- Prefer stronger rem6 contracts when gem5 tests rely on logs, Python object
  mutation, generated protocol scaffolding, or host side effects.
- Update this file when adding or retiring a migration target, and keep
  detailed evidence in the owning tests rather than expanding this ledger into
  a second evidence log.

## Migration Ledger

| gem5 test anchor | rem6 owner | Status | Current evidence | Next evidence |
| --- | --- | --- | --- | --- |
| `tests/gem5/arm_boot_tests` | future ARM ISA crate, `rem6-platform` | open | ARM platform device slices exist, but no Arm ISA execution or boot flow exists. | Add Arm ISA, board handoff, device-tree, and kernel boot tests. |
| `tests/gem5/asmtest` | ISA crates, `rem6` CLI | partial | RISC-V no-libc and ISA unit tests cover selected instruction and ecall paths. | Add generated assembly-program smoke tests for every supported ISA and compare architectural state plus exit evidence. |
| `tests/gem5/checkpoint_tests` | `rem6-checkpoint`, subsystem checkpoint banks | partial | Scheduler, memory, storage, VirtIO, timer, interrupt, platform, workload, and manifest checkpoint tests exist with decode-first validation. | Add remaining device and CPU pipeline checkpoints plus non-quiescent restore cases for every runtime owner. |
| `tests/gem5/chi_protocol` | `rem6-coherence`, protocol crates, `rem6-cache` | partial | CHI-like line, controller, bank, dirty peer sourcing, reservation, and Evict-hazard tests exist. | Add Ruby-scale CHI transactions, topology networks, directory races, and workload-level protocol checks. |
| `tests/gem5/chi_tlm_tests` | future adapter crates, `rem6-coherence` | open | Core CHI models are native Rust, but no TLM bridge is modeled. | Add optional typed TLM adapter tests only after an adapter boundary exists. |
| `tests/gem5/config_output_files` | `rem6` CLI, `rem6-workload` | partial | CLI output paths, stats-output paths, JSON run artifacts, and text stats output tests exist. | Add config-driven file layout coverage for full-system manifests and multi-artifact workloads. |
| `tests/gem5/cpu_tests` | `rem6-cpu`, `rem6-system` | partial | RISC-V atomic execution, frontend, retired branch predictor training, non-deferred, completed memory-data, and completed MMIO-load retire-to-in-order commit-cycle records, O3 policy, and store-conditional progress tests exist. | Extend in-order timing across fetch/decode/execute stalls and memory backpressure, then add ROB/LSQ-backed O3 execution tests. |
| `tests/gem5/dram_lowp` | `rem6-dram`, `rem6-power` | partial | DRAM/NVM profile counters and low-power timing constants are surfaced in run artifacts. | Add executable low-power state transition tests and validate wake/latency effects through routed memory requests. |
| `tests/gem5/example_configs`, `tests/gem5/learning_gem5` | `rem6` CLI, `rem6-platform`, `rem6-workload` | partial | CLI and TOML tests cover several execution and trace-replay paths. | Add equivalent rem6 examples that run from data files rather than Rust recompilation. |
| `tests/gem5/fdp_tests` | `rem6-cache` | partial | Fetch-directed prefetcher state and error paths exist in cache tests. | Add FDP execution tests through cache-bank and CPU/frontend consumers. |
| `tests/gem5/fs` | `rem6-platform`, `rem6-system`, device crates | partial | RISC-V DTB/initrd handoff, CLINT/PLIC, selected devices, checkpoints, and SE paths exist. | Add full-system Linux boot tests with SBI, storage, network, console, timer, interrupt, and shutdown evidence. |
| `tests/gem5/gem5_resources` | `rem6-workload` | partial | Resource declarations, payload identity, acquisition provenance, and disk-image construction records exist. | Add acquisition executor tests and broader artifact-kind coverage. |
| `tests/gem5/gpu` | `rem6-gpu`, `rem6-accelerator`, `rem6-transport` | partial | GPU and accelerator topology, command, and DMA route tests exist. | Add ISA-visible GPU execution, compute-unit scheduling, memory coalescing, and cache/DRAM interactions. |
| `tests/gem5/insttest_se` | `rem6-isa-riscv`, future ISA crates | partial | RISC-V ISA unit tests cover many integer, atomic, CSR, trap, and FP slices. | Add systematic instruction-test import coverage for RV64GC, vector, x86, Arm, and other target ISAs. |
| `tests/gem5/kvm_fork_tests`, `tests/gem5/kvm_switch_tests` | `rem6-system`, future host adapters | partial | Host-assisted takeover admission rejects unsafe switch shapes, but no KVM execution exists. | Add explicit fast-forward adapter boundaries before implementing KVM-like fork or switch tests. |
| `tests/gem5/m5_util`, `tests/test-progs/m5-exit` | `rem6-isa-riscv`, `rem6-system`, `rem6-workload` | partial | RISC-V m5 exit, fail, stats, checkpoint, and work markers reach typed host actions. | Add payload breadth, repeat scheduling, other ISA entry paths, and clock-domain behavior. |
| `tests/gem5/m5threads_test_atomic` | `rem6-isa-riscv`, `rem6-cpu`, `rem6-coherence` | partial | RISC-V LR/SC and AMO operations plus coherence reservation invalidation tests exist. | Add multi-threaded SE or full-system atomic tests that run real thread code through shared memory. |
| `tests/gem5/se_mode` | `rem6-system`, `rem6` CLI | partial | RISC-V SE startup, real ecall handling, newlib `printf` and file/stdin smoke tests, `ENOSYS` records, and guest write artifacts exist. | Add more static-libc syscalls, process/thread lifecycle behavior, host-backed file policy, and distro-like user binaries. |
| `tests/gem5/memory` | `rem6-memory`, `rem6-cache`, `rem6-dram`, `rem6-fabric` | partial | Stores, requests, responses, page maps, TLBs, translation queues, cache banks, DRAM/NVM counters, and fabric activity tests exist. | Add full CPU-facing multi-level cache and NoC path tests plus DRAM refresh and preset coverage. |
| `tests/gem5/multisim`, `tests/gem5/suite_tests` | `rem6-workload`, `rem6-kernel` | partial | Workload suite planning, dispatch, exact execution summaries, and parallel occupancy contracts exist. | Add multi-run simulator orchestration and suite-level artifact compatibility tests. |
| `tests/gem5/parsec_benchmarks` | `rem6-workload`, `rem6-system`, ISA crates | open | Workload suites exist, but real PARSEC-class programs do not run yet. | Add static/dynamic user workload support and ROI/stat hooks before PARSEC migration. |
| `tests/gem5/processor_switch_tests` | `rem6-system`, `rem6-cpu` | partial | Host-assisted switch admission and execution-mode metadata exist. | Add executable CPU model switching with quiescence, state transfer, and post-switch run evidence. |
| `tests/gem5/py_port` | `rem6` CLI, `rem6-workload` | open | rem6 does not expose a Python embedding port. | Decide whether to provide a typed external control adapter or document a Rust/CLI replacement. |
| `tests/gem5/pyunit` | `rem6` test crates, `rem6-workload`, `rem6-stats` | partial | Rust tests cover typed stats, workload, config, and helper behavior without Python runtime dependence. | Map each pyunit helper family to a Rust owner and add missing utility semantics. |
| `tests/gem5/readfile_tests` | `rem6-platform`, `rem6-system`, `rem6` CLI | partial | DTB/initrd handoff and CLI input-file plumbing exist. | Add guest-visible readfile device or replacement semantics for full-system tests. |
| `tests/pyunit` | `rem6-stats`, `rem6-workload`, future utility owners | partial | Some pystats and stdlib semantics are covered through typed Rust tests. | Add row-level parity for pystats, stdlib helper APIs, and utility parsing. |
| `tests/gem5/regression_tests` | all rem6 crates | partial | Workspace tests act as the current regression suite. | Add migration tags or per-family regression rows as gem5 tests are ported. |
| `tests/gem5/replacement_policies` | `rem6-cache` | partial | LRU, weighted LRU, skewed indexing, dueling, and compressed or sector tag tests exist. | Add the remaining gem5 replacement policies and cross-check them through cache-bank consumers. |
| `tests/gem5/riscv_boot_tests` | `rem6-platform`, `rem6-system`, `rem6-isa-riscv` | partial | RISC-V DTB/initrd handoff, CLINT/PLIC, traps, privileged CSRs, page-fault cause values, and translated data faults are tested. | Add SBI firmware behavior and a real Linux boot smoke with typed console, timer, interrupt, and stop evidence. |
| `tests/gem5/stats` | `rem6-stats`, `rem6` CLI, `rem6-power` | partial | Hierarchical counters, reset/dump histories, deltas, descriptions, real RISC-V probe producers, power bindings, and CLI stats output exist. | Add first-class histograms, more bank/cache/fabric/DRAM counters, and stricter gem5 text-stat compatibility checks. |
| `tests/gem5/stdlib` | `rem6-workload`, `rem6-platform`, `rem6` CLI | partial | Typed workload manifests, resource payloads, suite dispatch plans, Linux handoff intent, and TOML/CLI configuration tests exist. | Add broader gem5 stdlib object coverage and ergonomic topology/workload definitions without dynamic Python objects. |
| `tests/test-progs` | `rem6-system`, `rem6` CLI, ISA crates | partial | Static RISC-V no-libc and newlib smoke binaries are generated in tests when tools exist; artifacts stay out of version control. | Add durable generated-program fixtures for hello, threads, and m5 utility shapes across supported ISAs. |
| `tests/gem5/traffic_gen` | `rem6-traffic`, `rem6-system`, `rem6-workload` | partial | Text config parsing, GUPS, packet trace replay, request flags, maintenance, HTM, responses, and workload trace summaries exist. | Add remaining generator modes, response/error paths, and workload consumers for broader CPU/cache/memory routes. |
| `tests/gem5/x86_boot_tests` | `rem6-isa-x86`, future platform work | open | Narrow x86 prefix and interrupt-flag semantics exist, but no x86 boot path exists. | Add x86 ISA execution, paging, interrupt, platform, and boot-image tests before claiming boot-test migration. |

## Current Priority

The next highest-leverage migrations are:

- `tests/gem5/cpu_tests`: connect existing in-order and O3 state to executable
  CPU engines.
- `tests/gem5/se_mode` and `tests/test-progs`: broaden static-libc and
  user-program coverage beyond current newlib stdio and registered-file cases.
- `tests/gem5/memory` plus `tests/gem5/chi_protocol`: wire CPU data paths
  through cache banks, coherence, NoC, and DRAM instead of only trace or
  harness consumers.
- `tests/gem5/stats`: promote histograms and lower-level resource counters to
  first-class stats.
