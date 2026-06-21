use rem6_cpu::{CpuId, RiscvCluster};
use rem6_kernel::PartitionedScheduler;
use rem6_mmio::MmioBus;
use rem6_system::{GuestEventId, RiscvSystemRun, RiscvSystemRunDriver, RiscvSystemRunStopReason};
use rem6_transport::{MemoryTrace, MemoryTransport};

use crate::data_cache_runtime::{cli_data_memory_response, CliDataCacheRuntime};
use crate::runtime_memory::CliMemoryRuntime;

#[allow(clippy::too_many_arguments)]
pub(super) fn drive_cli_riscv_run(
    driver: &RiscvSystemRunDriver,
    cluster: &RiscvCluster,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    readfile_bus: Option<&MmioBus>,
    memory: &CliMemoryRuntime,
    instruction_cache: Option<CliDataCacheRuntime>,
    data_cache: Option<CliDataCacheRuntime>,
    data_cache_l2: Option<CliDataCacheRuntime>,
    fetch_trace: MemoryTrace,
    data_trace: MemoryTrace,
    tick_limit: u64,
    max_instructions: Option<u64>,
    retired_instruction_count: u64,
) -> Result<RiscvSystemRun, rem6_system::SystemError> {
    match max_instructions {
        Some(max_instructions) => {
            let remaining_instructions = max_instructions.saturating_sub(retired_instruction_count);
            if remaining_instructions == 0 {
                return Ok(RiscvSystemRun::new(
                    Vec::new(),
                    Vec::new(),
                    RiscvSystemRunStopReason::InstructionLimit {
                        tick: scheduler.now(),
                        limit: max_instructions,
                        committed: retired_instruction_count,
                    },
                ));
            }

            drive_cli_riscv_run_with_instruction_limit(
                driver,
                cluster,
                scheduler,
                transport,
                readfile_bus,
                memory,
                instruction_cache,
                data_cache,
                data_cache_l2,
                fetch_trace,
                data_trace,
                tick_limit,
                remaining_instructions,
            )
        }
        None => drive_cli_riscv_run_until_tick(
            driver,
            cluster,
            scheduler,
            transport,
            readfile_bus,
            memory,
            instruction_cache,
            data_cache,
            data_cache_l2,
            fetch_trace,
            data_trace,
            tick_limit,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn drive_cli_riscv_run_with_instruction_limit(
    driver: &RiscvSystemRunDriver,
    cluster: &RiscvCluster,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    readfile_bus: Option<&MmioBus>,
    memory: &CliMemoryRuntime,
    instruction_cache: Option<CliDataCacheRuntime>,
    data_cache: Option<CliDataCacheRuntime>,
    data_cache_l2: Option<CliDataCacheRuntime>,
    fetch_trace: MemoryTrace,
    data_trace: MemoryTrace,
    tick_limit: u64,
    remaining_instructions: u64,
) -> Result<RiscvSystemRun, rem6_system::SystemError> {
    if let Some(bus) = readfile_bus {
        let fetch_memory = memory.clone();
        let data_memory = memory.clone();
        return driver.drive_until_host_stop_or_instruction_limit_parallel_with_mmio(
            cluster,
            scheduler,
            transport,
            bus,
            fetch_trace,
            data_trace,
            move |_cpu| {
                let memory = fetch_memory.clone();
                let instruction_cache = instruction_cache.clone();
                move |delivery, _context| {
                    cli_data_memory_response(instruction_cache.as_ref(), None, &memory, &delivery)
                }
            },
            move |_cpu| {
                let memory = data_memory.clone();
                let data_cache = data_cache.clone();
                let data_cache_l2 = data_cache_l2.clone();
                move |delivery, _context| {
                    cli_data_memory_response(
                        data_cache.as_ref(),
                        data_cache_l2.as_ref(),
                        &memory,
                        &delivery,
                    )
                }
            },
            tick_limit,
            remaining_instructions,
            guest_event_for_cpu,
        );
    }

    let fetch_memory = memory.clone();
    let data_memory = memory.clone();
    driver.drive_until_host_stop_or_instruction_limit_parallel(
        cluster,
        scheduler,
        transport,
        fetch_trace,
        data_trace,
        move |_cpu| {
            let memory = fetch_memory.clone();
            let instruction_cache = instruction_cache.clone();
            move |delivery, _context| {
                cli_data_memory_response(instruction_cache.as_ref(), None, &memory, &delivery)
            }
        },
        move |_cpu| {
            let memory = data_memory.clone();
            let data_cache = data_cache.clone();
            let data_cache_l2 = data_cache_l2.clone();
            move |delivery, _context| {
                cli_data_memory_response(
                    data_cache.as_ref(),
                    data_cache_l2.as_ref(),
                    &memory,
                    &delivery,
                )
            }
        },
        tick_limit,
        remaining_instructions,
        guest_event_for_cpu,
    )
}

#[allow(clippy::too_many_arguments)]
fn drive_cli_riscv_run_until_tick(
    driver: &RiscvSystemRunDriver,
    cluster: &RiscvCluster,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    readfile_bus: Option<&MmioBus>,
    memory: &CliMemoryRuntime,
    instruction_cache: Option<CliDataCacheRuntime>,
    data_cache: Option<CliDataCacheRuntime>,
    data_cache_l2: Option<CliDataCacheRuntime>,
    fetch_trace: MemoryTrace,
    data_trace: MemoryTrace,
    tick_limit: u64,
) -> Result<RiscvSystemRun, rem6_system::SystemError> {
    if let Some(bus) = readfile_bus {
        let fetch_memory = memory.clone();
        let data_memory = memory.clone();
        return driver.drive_until_host_stop_or_tick_limit_parallel_with_mmio(
            cluster,
            scheduler,
            transport,
            bus,
            fetch_trace,
            data_trace,
            move |_cpu| {
                let memory = fetch_memory.clone();
                let instruction_cache = instruction_cache.clone();
                move |delivery, _context| {
                    cli_data_memory_response(instruction_cache.as_ref(), None, &memory, &delivery)
                }
            },
            move |_cpu| {
                let memory = data_memory.clone();
                let data_cache = data_cache.clone();
                let data_cache_l2 = data_cache_l2.clone();
                move |delivery, _context| {
                    cli_data_memory_response(
                        data_cache.as_ref(),
                        data_cache_l2.as_ref(),
                        &memory,
                        &delivery,
                    )
                }
            },
            tick_limit,
            guest_event_for_cpu,
        );
    }

    let fetch_memory = memory.clone();
    let data_memory = memory.clone();
    driver.drive_until_host_stop_or_tick_limit_parallel(
        cluster,
        scheduler,
        transport,
        fetch_trace,
        data_trace,
        move |_cpu| {
            let memory = fetch_memory.clone();
            let instruction_cache = instruction_cache.clone();
            move |delivery, _context| {
                cli_data_memory_response(instruction_cache.as_ref(), None, &memory, &delivery)
            }
        },
        move |_cpu| {
            let memory = data_memory.clone();
            let data_cache = data_cache.clone();
            let data_cache_l2 = data_cache_l2.clone();
            move |delivery, _context| {
                cli_data_memory_response(
                    data_cache.as_ref(),
                    data_cache_l2.as_ref(),
                    &memory,
                    &delivery,
                )
            }
        },
        tick_limit,
        guest_event_for_cpu,
    )
}

fn guest_event_for_cpu(cpu: CpuId) -> GuestEventId {
    GuestEventId::new(u64::from(cpu.get()))
}
