use std::sync::{Arc, Mutex};

use rem6_dram::{DramLowPowerState, DramMemoryActivityProfile, DramMemoryController};
use rem6_memory::{
    AccessSize, Address, CacheLineLayout, MemoryRequest, MemoryRequestId, PartitionedMemoryStore,
};
use rem6_transport::{RequestDelivery, TargetOutcome};

use crate::config::CliDramMemoryProfile;
use crate::guest_memory::{build_cli_dram_memory, build_cli_memory_store, CLI_MEMORY_TARGET};
use crate::{
    execute_error, LoadedBlob, MemoryDumpRequest, Rem6CliError, Rem6DramSummary, Rem6MemoryDump,
    CLI_MEMORY_DUMP_AGENT,
};

#[derive(Clone)]
pub(super) enum CliMemoryRuntime {
    Store(Arc<Mutex<PartitionedMemoryStore>>),
    Dram(Arc<Mutex<DramMemoryController>>),
}

impl CliMemoryRuntime {
    pub(super) fn new(
        image: &rem6_boot::BootImage,
        load_blobs: &[LoadedBlob],
        line_layout: CacheLineLayout,
        use_dram: bool,
        dram_profile: CliDramMemoryProfile,
    ) -> Result<Self, Rem6CliError> {
        if use_dram {
            return Ok(Self::Dram(Arc::new(Mutex::new(build_cli_dram_memory(
                image,
                load_blobs,
                line_layout,
                dram_profile,
            )?))));
        }

        Ok(Self::Store(Arc::new(Mutex::new(build_cli_memory_store(
            image,
            load_blobs,
            line_layout,
        )?))))
    }

    pub(super) fn dram_summary_until(&self, final_tick: u64) -> Rem6DramSummary {
        match self {
            Self::Store(_) => Rem6DramSummary::default(),
            Self::Dram(memory) => Rem6DramSummary::from_profile(
                memory
                    .lock()
                    .expect("CLI DRAM memory lock")
                    .activity_profile_until(final_tick),
            ),
        }
    }
}

pub(super) fn cli_memory_response(
    memory: &CliMemoryRuntime,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    match memory {
        CliMemoryRuntime::Store(store) => {
            let outcome = store
                .lock()
                .expect("CLI memory store lock")
                .respond(delivery.request())
                .expect("CLI memory response");
            match outcome.response().cloned() {
                Some(response) => TargetOutcome::Respond(response),
                None => TargetOutcome::NoResponse,
            }
        }
        CliMemoryRuntime::Dram(memory) => {
            let outcome = memory
                .lock()
                .expect("CLI DRAM memory lock")
                .accept(delivery.tick(), delivery.request())
                .expect("CLI DRAM memory response");
            let Some(response) = outcome.response().cloned() else {
                return TargetOutcome::NoResponse;
            };
            let delay = outcome
                .ready_cycle()
                .checked_sub(delivery.tick())
                .expect("CLI DRAM response is not ready before request arrival");
            if delay == 0 {
                TargetOutcome::Respond(response)
            } else {
                TargetOutcome::RespondAfter { delay, response }
            }
        }
    }
}

impl Rem6DramSummary {
    fn from_profile(profile: DramMemoryActivityProfile) -> Self {
        let nvm_profile = profile.profile_technology_label() == Some("nvm");
        let timing = profile.profile_timing();
        let nvm_media_timing = profile.profile_nvm_media_timing();
        Self {
            active_targets: profile.active_target_count() as u64,
            active_ports: profile.active_port_count() as u64,
            active_banks: profile.active_bank_count() as u64,
            accesses: profile.access_count() as u64,
            reads: profile.read_count() as u64,
            writes: profile.write_count() as u64,
            row_hits: profile.row_hit_count() as u64,
            row_misses: profile.row_miss_count() as u64,
            commands: profile.command_count() as u64,
            turnarounds: profile.turnaround_count() as u64,
            total_ready_latency_ticks: profile.total_ready_latency_cycles(),
            max_ready_latency_ticks: profile.max_ready_latency_cycles(),
            profiled_targets: profile.profiled_target_count() as u64,
            profile_technology: profile.profile_technology_label(),
            profile_parallel_port_label: profile.profile_parallel_port_label(),
            profile_topology_unit_label: profile.profile_topology_unit_label(),
            profile_timing_activate_latency: timing
                .map(|timing| timing.activate_latency())
                .unwrap_or(0),
            profile_timing_read_latency: timing.map(|timing| timing.read_latency()).unwrap_or(0),
            profile_timing_write_latency: timing.map(|timing| timing.write_latency()).unwrap_or(0),
            profile_timing_precharge_latency: timing
                .map(|timing| timing.precharge_latency())
                .unwrap_or(0),
            profile_timing_bus_turnaround: timing
                .map(|timing| timing.bus_turnaround())
                .unwrap_or(0),
            profile_timing_burst_spacing: timing.map(|timing| timing.burst_spacing()).unwrap_or(0),
            profile_timing_same_bank_group_burst_spacing: timing
                .and_then(|timing| timing.same_bank_group_burst_spacing())
                .unwrap_or(0),
            profile_nvm_media_read_latency: nvm_media_timing
                .map(|timing| timing.read_media_latency())
                .unwrap_or(0),
            profile_nvm_media_write_latency: nvm_media_timing
                .map(|timing| timing.write_media_latency())
                .unwrap_or(0),
            profile_nvm_media_send_latency: nvm_media_timing
                .map(|timing| timing.send_latency())
                .unwrap_or(0),
            profile_nvm_media_max_pending_reads: nvm_media_timing
                .map(|timing| u64::from(timing.max_pending_reads()))
                .unwrap_or(0),
            profile_nvm_media_max_pending_writes: nvm_media_timing
                .map(|timing| u64::from(timing.max_pending_writes()))
                .unwrap_or(0),
            profile_parallel_ports: profile.profile_parallel_port_capacity(),
            profile_topology_units: profile.profile_topology_unit_capacity(),
            profile_scheduler_banks: profile.profile_scheduler_bank_capacity(),
            profile_topology_banks: profile.profile_topology_bank_capacity(),
            profile_scheduler_bank_groups: profile.profile_scheduler_bank_group_capacity(),
            nvm_persistent_writes: if nvm_profile {
                profile.write_count() as u64
            } else {
                0
            },
            nvm_persistent_write_bytes: if nvm_profile {
                profile.write_byte_count()
            } else {
                0
            },
            nvm_max_pending_reads: if nvm_profile {
                profile.max_pending_nvm_reads() as u64
            } else {
                0
            },
            nvm_max_pending_persistent_writes: if nvm_profile {
                profile.max_pending_persistent_writes() as u64
            } else {
                0
            },
            low_power_active_powerdown_entries: profile
                .low_power_entry_count(DramLowPowerState::ActivePowerdown)
                as u64,
            low_power_active_powerdown_ticks: profile
                .low_power_cycle_count(DramLowPowerState::ActivePowerdown),
            low_power_precharge_powerdown_entries: profile
                .low_power_entry_count(DramLowPowerState::PrechargePowerdown)
                as u64,
            low_power_precharge_powerdown_ticks: profile
                .low_power_cycle_count(DramLowPowerState::PrechargePowerdown),
            low_power_self_refresh_entries: profile
                .low_power_entry_count(DramLowPowerState::SelfRefresh)
                as u64,
            low_power_self_refresh_ticks: profile
                .low_power_cycle_count(DramLowPowerState::SelfRefresh),
            low_power_exits: profile.low_power_exit_count() as u64,
            low_power_exit_latency_ticks: profile.low_power_exit_latency_cycles(),
        }
    }
}

pub(super) fn read_memory_dumps(
    memory: &CliMemoryRuntime,
    line_layout: CacheLineLayout,
    requests: &[MemoryDumpRequest],
) -> Result<Vec<Rem6MemoryDump>, Rem6CliError> {
    requests
        .iter()
        .enumerate()
        .map(|(index, request)| read_memory_dump(memory, line_layout, index as u64, *request))
        .collect()
}

fn read_memory_dump(
    memory: &CliMemoryRuntime,
    line_layout: CacheLineLayout,
    sequence: u64,
    dump: MemoryDumpRequest,
) -> Result<Rem6MemoryDump, Rem6CliError> {
    match memory {
        CliMemoryRuntime::Store(store) => {
            read_memory_dump_from_store(store, line_layout, sequence, dump)
        }
        CliMemoryRuntime::Dram(memory) => read_memory_dump_from_dram(memory, line_layout, dump),
    }
}

fn read_memory_dump_from_store(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    line_layout: CacheLineLayout,
    sequence: u64,
    dump: MemoryDumpRequest,
) -> Result<Rem6MemoryDump, Rem6CliError> {
    let request = MemoryRequest::read_shared(
        MemoryRequestId::new(CLI_MEMORY_DUMP_AGENT, sequence),
        Address::new(dump.address()),
        AccessSize::new(dump.bytes()).map_err(execute_error)?,
        line_layout,
    )
    .map_err(execute_error)?;
    let outcome = store
        .lock()
        .expect("CLI memory store lock")
        .respond(&request)
        .map_err(execute_error)?;
    let data = outcome
        .response()
        .and_then(|response| response.data())
        .ok_or_else(|| Rem6CliError::Execute {
            error: format!("memory dump at 0x{:x} returned no data", dump.address()),
        })?
        .to_vec();
    Ok(Rem6MemoryDump {
        address: dump.address(),
        data,
    })
}

fn read_memory_dump_from_dram(
    memory: &Arc<Mutex<DramMemoryController>>,
    line_layout: CacheLineLayout,
    dump: MemoryDumpRequest,
) -> Result<Rem6MemoryDump, Rem6CliError> {
    let capacity = usize::try_from(dump.bytes()).map_err(|_| {
        execute_error(format!(
            "memory dump size {} does not fit usize",
            dump.bytes()
        ))
    })?;
    let mut data = Vec::with_capacity(capacity);
    let mut cursor = dump.address();
    let end = dump
        .address()
        .checked_add(dump.bytes())
        .ok_or_else(|| execute_error("memory dump range overflow"))?;
    let memory = memory.lock().expect("CLI DRAM memory lock");
    while cursor < end {
        let address = Address::new(cursor);
        let line = line_layout.line_address(address);
        let line_offset = line_layout.line_offset(address);
        let available = line_layout.bytes() - line_offset;
        let bytes = available.min(end - cursor);
        let line_data = memory
            .line_data(CLI_MEMORY_TARGET, line)
            .map_err(execute_error)?;
        let start = line_offset as usize;
        data.extend_from_slice(&line_data[start..start + bytes as usize]);
        cursor += bytes;
    }
    Ok(Rem6MemoryDump {
        address: dump.address(),
        data,
    })
}
