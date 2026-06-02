use super::formatting::{
    bytes_to_hex, elf_architecture_name, elf_class_name, elf_endian_name, elf_os_name, json_escape,
};
use super::{
    Rem6CoreSummary, Rem6DramSummary, Rem6ExecutionStop, Rem6ExecutionSummary, Rem6LoadBlobSummary,
    Rem6MemoryDump, Rem6MemoryTransportCounters, Rem6MemoryTransportRouteSummary,
    Rem6MemoryTransportSummary, Rem6ParallelFrontierSummary, Rem6ParallelPartitionSummary,
    Rem6ParallelReadyPartitionSummary, Rem6RunArtifact, RequestedIsa,
};

impl Rem6RunArtifact {
    pub fn to_json(&self) -> String {
        let simulation = match &self.execution {
            Some(execution) => {
                execution.to_simulation_json(
                    self.config.max_tick(),
                    self.config.max_instructions(),
                    self.config.memory_route_delay(),
                    self.config.host_event_delay(),
                )
            }
            None => format!(
                "{{\"status\":\"loaded\",\"max_tick\":{},\"instruction_limit\":{},\"memory_route_delay\":{},\"host_event_delay\":{},\"executed_ticks\":0,\"cores\":{}}}",
                self.config.max_tick(),
                optional_count_json(self.config.max_instructions()),
                self.config.memory_route_delay(),
                self.config.host_event_delay(),
                self.config.cores(),
            ),
        };
        let parallel = match &self.execution {
            Some(execution) => execution.to_parallel_json(
                self.config.parallel_workers(),
                self.config.min_remote_delay(),
            ),
            None => empty_parallel_json(
                self.config.parallel_workers(),
                self.config.min_remote_delay(),
            ),
        };
        let cores = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_cores_json)
            .unwrap_or_else(|| "[]".to_string());
        let memory = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_memory_json)
            .unwrap_or_else(|| "[]".to_string());
        let dram = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_dram_json)
            .unwrap_or_else(|| Rem6DramSummary::default().to_json());
        let transport = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_transport_json)
            .unwrap_or_else(empty_transport_json);
        let load_blobs = self
            .load_blobs
            .iter()
            .map(Rem6LoadBlobSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let riscv_boot = if self.config.isa() == RequestedIsa::Riscv {
            format!(
                ",\"riscv_boot\":{{\"a0\":\"0x{:x}\",\"a1\":\"0x{:x}\"}}",
                self.config.riscv_boot_a0(),
                self.config.riscv_boot_a1()
            )
        } else {
            String::new()
        };
        format!(
            "{{\"schema\":\"{}\",\"isa\":\"{}\",\"binary\":\"{}\",\"entry\":\"0x{:x}\",\"start_address\":\"0x{:x}\"{},\"load_blobs\":[{}],\"elf\":{{\"class\":\"{}\",\"endian\":\"{}\",\"architecture\":\"{}\",\"os\":\"{}\",\"machine\":{},\"flags\":{}}},\"simulation\":{},\"parallel\":{},\"cores\":{},\"memory\":{},\"dram\":{},\"transport\":{},\"stats\":{}}}\n",
            self.schema,
            self.config.isa().as_str(),
            json_escape(&self.config.binary().display().to_string()),
            self.entry,
            self.start_address,
            riscv_boot,
            load_blobs,
            elf_class_name(self.metadata.class()),
            elf_endian_name(self.metadata.endian()),
            elf_architecture_name(self.metadata.architecture()),
            elf_os_name(self.metadata.operating_system()),
            self.metadata.machine(),
            self.metadata.flags(),
            simulation,
            parallel,
            cores,
            memory,
            dram,
            transport,
            self.stats_json,
        )
    }

    pub const fn binary_bytes(&self) -> u64 {
        self.binary_bytes
    }

    pub const fn load_segments(&self) -> u64 {
        self.load_segments
    }
}

impl Rem6LoadBlobSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"address\":\"0x{:x}\",\"bytes\":{},\"path\":\"{}\"}}",
            self.address(),
            self.bytes(),
            json_escape(&self.path().display().to_string())
        )
    }
}

impl Rem6ExecutionSummary {
    fn to_simulation_json(
        &self,
        max_tick: u64,
        max_instructions: Option<u64>,
        memory_route_delay: u64,
        host_event_delay: u64,
    ) -> String {
        let instruction_limit = match self.stop {
            Rem6ExecutionStop::InstructionLimit { instruction_limit } => Some(instruction_limit),
            Rem6ExecutionStop::HostTrap { .. } | Rem6ExecutionStop::TickLimit { .. } => {
                max_instructions
            }
        };
        let common = format!(
            "\"max_tick\":{},\"instruction_limit\":{},\"memory_route_delay\":{},\"host_event_delay\":{},\"executed_ticks\":{},\"final_tick\":{},\"cores\":{},\"committed_instructions\":{}",
            max_tick,
            optional_count_json(instruction_limit),
            memory_route_delay,
            host_event_delay,
            self.final_tick,
            self.final_tick,
            self.cores.len(),
            self.committed_instructions,
        );
        match self.stop {
            Rem6ExecutionStop::HostTrap { stop_code, trap } => format!(
                "{{\"status\":\"executed_until_trap\",\"stop_reason\":\"host_trap\",{},\"stop_code\":{},\"trap\":\"{}\"}}",
                common, stop_code, trap
            ),
            Rem6ExecutionStop::TickLimit { tick_limit } => format!(
                "{{\"status\":\"stopped_at_tick_limit\",\"stop_reason\":\"tick_limit\",{},\"tick_limit\":{}}}",
                common, tick_limit
            ),
            Rem6ExecutionStop::InstructionLimit { .. } => format!(
                "{{\"status\":\"stopped_at_instruction_limit\",\"stop_reason\":\"instruction_limit\",{}}}",
                common
            ),
        }
    }

    fn to_parallel_json(&self, worker_limit: usize, min_remote_delay: u64) -> String {
        let slots = self
            .parallel_scheduler_worker_slots
            .iter()
            .map(super::Rem6ParallelWorkerSlotSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let lanes = self
            .parallel_scheduler_worker_lanes
            .iter()
            .map(super::Rem6ParallelWorkerLaneSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let partitions = self
            .parallel_scheduler_partitions
            .iter()
            .map(Rem6ParallelPartitionSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let frontiers = self
            .parallel_scheduler_frontiers
            .iter()
            .map(Rem6ParallelFrontierSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let final_frontiers = self
            .parallel_scheduler_final_frontiers
            .iter()
            .map(Rem6ParallelFrontierSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let ready_partitions = self
            .parallel_scheduler_ready_partitions
            .iter()
            .map(Rem6ParallelReadyPartitionSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"scheduler\":{{\"worker_limit\":{},\"min_remote_delay\":{},\"epochs\":{},\"dispatches\":{},\"batches\":{},\"max_workers\":{},\"total_workers\":{},\"active_partitions\":{},\"remote_sends\":{},\"batch_worker_ticks\":{},\"batch_worker_capacity_ticks\":{},\"batch_idle_worker_ticks\":{},\"worker_slots\":[{}],\"worker_lanes\":[{}],\"partitions\":[{}],\"frontiers\":[{}],\"final_frontiers\":[{}],\"ready_partitions\":[{}]}}}}",
            worker_limit,
            min_remote_delay,
            self.parallel_scheduler_epochs,
            self.parallel_scheduler_dispatches,
            self.parallel_scheduler_batches,
            self.parallel_scheduler_max_workers,
            self.parallel_scheduler_total_workers,
            self.parallel_scheduler_active_partitions,
            self.parallel_scheduler_remote_sends,
            self.parallel_scheduler_batch_worker_ticks,
            self.parallel_scheduler_batch_worker_capacity_ticks,
            self.parallel_scheduler_batch_idle_worker_ticks,
            slots,
            lanes,
            partitions,
            frontiers,
            final_frontiers,
            ready_partitions,
        )
    }

    fn to_cores_json(&self) -> String {
        let cores = self
            .cores
            .iter()
            .map(Rem6CoreSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!("[{cores}]")
    }

    fn to_memory_json(&self) -> String {
        let dumps = self
            .memory_dumps
            .iter()
            .map(Rem6MemoryDump::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!("[{dumps}]")
    }

    fn to_transport_json(&self) -> String {
        format!(
            "{{\"fetch\":{},\"data\":{}}}",
            self.fetch_transport.to_json(),
            self.data_transport.to_json()
        )
    }

    fn to_dram_json(&self) -> String {
        self.dram.to_json()
    }
}

impl Rem6DramSummary {
    fn to_json(self) -> String {
        let profile_technology = optional_string_json(self.profile_technology);
        let profile_parallel_port_label = optional_string_json(self.profile_parallel_port_label);
        let profile_topology_unit_label = optional_string_json(self.profile_topology_unit_label);
        format!(
            "{{\"active_targets\":{},\"active_ports\":{},\"active_banks\":{},\"accesses\":{},\"reads\":{},\"writes\":{},\"row_hits\":{},\"row_misses\":{},\"commands\":{},\"turnarounds\":{},\"total_ready_latency_ticks\":{},\"max_ready_latency_ticks\":{},\"profile\":{{\"technology\":{},\"parallel_port_label\":{},\"topology_unit_label\":{},\"geometry\":{{\"bank_count\":{},\"row_size\":{},\"line_size\":{},\"lines_per_row\":{},\"bank_group_count\":{}}},\"timing\":{{\"activate_latency\":{},\"read_latency\":{},\"write_latency\":{},\"precharge_latency\":{},\"bus_turnaround\":{},\"burst_spacing\":{},\"same_bank_group_burst_spacing\":{},\"command_window\":{{\"window_cycles\":{},\"max_commands\":{}}}}},\"low_power_timing\":{{\"precharge_powerdown_entry_delay\":{},\"self_refresh_entry_delay\":{},\"exit_latency\":{},\"self_refresh_exit_latency\":{}}},\"nvm_media\":{{\"read_media_latency\":{},\"write_media_latency\":{},\"send_latency\":{},\"max_pending_reads\":{},\"max_pending_writes\":{}}},\"profiled_targets\":{},\"parallel_ports\":{},\"topology_units\":{},\"scheduler_banks\":{},\"topology_banks\":{},\"scheduler_bank_groups\":{}}},\"nvm\":{{\"persistent_writes\":{},\"persistent_write_bytes\":{},\"max_pending_reads\":{},\"max_pending_persistent_writes\":{}}},\"low_power\":{{\"active_powerdown\":{{\"entries\":{},\"ticks\":{}}},\"precharge_powerdown\":{{\"entries\":{},\"ticks\":{}}},\"self_refresh\":{{\"entries\":{},\"ticks\":{}}},\"exits\":{},\"exit_latency_ticks\":{}}}}}",
            self.active_targets,
            self.active_ports,
            self.active_banks,
            self.accesses,
            self.reads,
            self.writes,
            self.row_hits,
            self.row_misses,
            self.commands,
            self.turnarounds,
            self.total_ready_latency_ticks,
            self.max_ready_latency_ticks,
            profile_technology,
            profile_parallel_port_label,
            profile_topology_unit_label,
            self.profile_geometry_bank_count,
            self.profile_geometry_row_size,
            self.profile_geometry_line_size,
            self.profile_geometry_lines_per_row,
            self.profile_geometry_bank_group_count,
            self.profile_timing_activate_latency,
            self.profile_timing_read_latency,
            self.profile_timing_write_latency,
            self.profile_timing_precharge_latency,
            self.profile_timing_bus_turnaround,
            self.profile_timing_burst_spacing,
            self.profile_timing_same_bank_group_burst_spacing,
            self.profile_timing_command_window_cycles,
            self.profile_timing_command_window_max_commands,
            self.profile_low_power_precharge_powerdown_entry_delay,
            self.profile_low_power_self_refresh_entry_delay,
            self.profile_low_power_exit_latency,
            self.profile_low_power_self_refresh_exit_latency,
            self.profile_nvm_media_read_latency,
            self.profile_nvm_media_write_latency,
            self.profile_nvm_media_send_latency,
            self.profile_nvm_media_max_pending_reads,
            self.profile_nvm_media_max_pending_writes,
            self.profiled_targets,
            self.profile_parallel_ports,
            self.profile_topology_units,
            self.profile_scheduler_banks,
            self.profile_topology_banks,
            self.profile_scheduler_bank_groups,
            self.nvm_persistent_writes,
            self.nvm_persistent_write_bytes,
            self.nvm_max_pending_reads,
            self.nvm_max_pending_persistent_writes,
            self.low_power_active_powerdown_entries,
            self.low_power_active_powerdown_ticks,
            self.low_power_precharge_powerdown_entries,
            self.low_power_precharge_powerdown_ticks,
            self.low_power_self_refresh_entries,
            self.low_power_self_refresh_ticks,
            self.low_power_exits,
            self.low_power_exit_latency_ticks,
        )
    }
}

fn optional_string_json(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", json_escape(value)))
        .unwrap_or_else(|| "null".to_string())
}

fn empty_parallel_json(worker_limit: usize, min_remote_delay: u64) -> String {
    format!(
        "{{\"scheduler\":{{\"worker_limit\":{},\"min_remote_delay\":{},\"epochs\":0,\"dispatches\":0,\"batches\":0,\"max_workers\":0,\"total_workers\":0,\"active_partitions\":0,\"remote_sends\":0,\"batch_worker_ticks\":0,\"batch_worker_capacity_ticks\":0,\"batch_idle_worker_ticks\":0,\"worker_slots\":[],\"worker_lanes\":[],\"partitions\":[],\"frontiers\":[],\"final_frontiers\":[],\"ready_partitions\":[]}}}}",
        worker_limit, min_remote_delay
    )
}

impl super::Rem6ParallelWorkerSlotSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"slot\":{},\"active_ticks\":{},\"idle_ticks\":{}}}",
            self.slot, self.active_ticks, self.idle_ticks
        )
    }
}

impl super::Rem6ParallelWorkerLaneSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"lane\":{},\"partition\":{},\"active_ticks\":{}}}",
            self.lane, self.partition, self.active_ticks
        )
    }
}

impl Rem6ParallelPartitionSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"partition\":{},\"workers\":{},\"dispatches\":{},\"remote_sends\":{},\"remote_receives\":{},\"max_pending_events\":{}}}",
            self.partition,
            self.workers,
            self.dispatches,
            self.remote_sends,
            self.remote_receives,
            self.max_pending_events,
        )
    }
}

impl Rem6ParallelFrontierSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"partition\":{},\"now\":{},\"safe_until\":{},\"next_tick\":{},\"pending_events\":{}}}",
            self.partition,
            self.now,
            self.safe_until,
            optional_tick_json(self.next_tick),
            self.pending_events,
        )
    }
}

impl Rem6ParallelReadyPartitionSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"partition\":{},\"next_tick\":{}}}",
            self.partition, self.next_tick
        )
    }
}

fn optional_tick_json(value: Option<u64>) -> String {
    value
        .map(|tick| tick.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn optional_count_json(value: Option<u64>) -> String {
    value
        .map(|count| count.to_string())
        .unwrap_or_else(|| "null".to_string())
}

impl Rem6CoreSummary {
    fn to_json(&self) -> String {
        let registers = self
            .registers
            .iter()
            .map(|(register, value)| format!("\"x{}\":\"0x{:x}\"", register, value))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"cpu\":{},\"pc\":\"0x{:x}\",\"committed_instructions\":{},\"data_loads\":{},\"data_stores\":{},\"data_atomics\":{},\"data_load_bytes\":{},\"data_store_bytes\":{},\"data_atomic_bytes\":{},\"registers\":{{{}}}}}",
            self.cpu,
            self.pc,
            self.committed_instructions,
            self.data_loads,
            self.data_stores,
            self.data_atomics,
            self.data_load_bytes,
            self.data_store_bytes,
            self.data_atomic_bytes,
            registers
        )
    }
}

impl Rem6MemoryDump {
    fn to_json(&self) -> String {
        format!(
            "{{\"address\":\"0x{:x}\",\"bytes\":{},\"hex\":\"{}\"}}",
            self.address,
            self.data.len(),
            bytes_to_hex(&self.data),
        )
    }
}

fn empty_transport_json() -> String {
    format!(
        "{{\"fetch\":{},\"data\":{}}}",
        empty_transport_scope_json(),
        empty_transport_scope_json()
    )
}

fn empty_transport_scope_json() -> String {
    "{\"requests\":0,\"request_arrivals\":0,\"responses\":0,\"response_arrivals\":0,\"round_trip_ticks\":0,\"max_round_trip_ticks\":0,\"routes\":[]}".to_string()
}

impl Rem6MemoryTransportSummary {
    fn to_json(&self) -> String {
        let routes = self
            .routes
            .iter()
            .map(Rem6MemoryTransportRouteSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{{},\"routes\":[{}]}}",
            self.counters.json_fields(),
            routes
        )
    }
}

impl Rem6MemoryTransportRouteSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"route\":{},\"source\":\"{}\",{}}}",
            self.route.get(),
            json_escape(&self.source),
            self.counters.json_fields()
        )
    }
}

impl Rem6MemoryTransportCounters {
    fn json_fields(&self) -> String {
        format!(
            "\"requests\":{},\"request_arrivals\":{},\"responses\":{},\"response_arrivals\":{},\"round_trip_ticks\":{},\"max_round_trip_ticks\":{}",
            self.requests,
            self.request_arrivals,
            self.responses,
            self.response_arrivals,
            self.round_trip_ticks,
            self.max_round_trip_ticks,
        )
    }
}
