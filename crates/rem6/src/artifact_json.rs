use super::formatting::{
    bytes_to_hex, elf_architecture_name, elf_class_name, elf_endian_name, elf_os_name, json_escape,
};
use super::{
    Rem6CoreSummary, Rem6DataAccessProbeSummary, Rem6DramSummary, Rem6ExecutionStop,
    Rem6ExecutionSummary, Rem6GupsArtifact, Rem6GupsExecutionSummary, Rem6LoadBlobSummary,
    Rem6MemoryDump, Rem6MemoryTransportCounters, Rem6MemoryTransportRouteSummary,
    Rem6MemoryTransportSummary, Rem6ParallelFrontierSummary, Rem6ParallelPartitionSummary,
    Rem6ParallelReadyPartitionSummary, Rem6RiscvGuestWriteSummary, Rem6RiscvUnknownSyscallSummary,
    Rem6RunArtifact, Rem6TraceReplayArtifact, Rem6TraceReplayExecutionSummary, RequestedIsa,
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
        let riscv_guest_writes = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_guest_writes_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_unknown_syscalls = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_unknown_syscalls_json)
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
                ",\"riscv_boot\":{{\"a0\":\"0x{:x}\",\"a1\":\"0x{:x}\",\"se\":{}}}",
                self.config.riscv_boot_a0(),
                self.config.riscv_boot_a1(),
                self.config.riscv_se()
            )
        } else {
            String::new()
        };
        format!(
            "{{\"schema\":\"{}\",\"isa\":\"{}\",\"binary\":\"{}\",\"entry\":\"0x{:x}\",\"start_address\":\"0x{:x}\"{},\"load_blobs\":[{}],\"elf\":{{\"class\":\"{}\",\"endian\":\"{}\",\"architecture\":\"{}\",\"os\":\"{}\",\"machine\":{},\"flags\":{}}},\"simulation\":{},\"parallel\":{},\"cores\":{},\"memory\":{},\"riscv_guest_writes\":{},\"riscv_unknown_syscalls\":{},\"dram\":{},\"transport\":{},\"stats\":{}}}\n",
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
            riscv_guest_writes,
            riscv_unknown_syscalls,
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

impl Rem6GupsArtifact {
    pub fn to_json(&self) -> String {
        let memory_dumps = self
            .memory_dumps
            .iter()
            .map(Rem6MemoryDump::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":\"{}\",\"generator\":\"gups\",\"memory_start\":\"0x{:x}\",\"memory_size\":{},\"updates\":{},\"rng_state\":\"0x{:x}\",\"simulation\":{},\"memory\":[{}],\"transport\":{{\"gups\":{}}},\"stats\":{}}}\n",
            self.schema,
            self.config.memory_start(),
            self.config.memory_size(),
            self.config.updates(),
            self.config.rng_state(),
            self.execution.to_json(self.config.max_tick()),
            memory_dumps,
            self.transport.to_json(),
            self.stats_json,
        )
    }
}

impl Rem6TraceReplayArtifact {
    pub fn to_json(&self) -> String {
        let data_cache_protocol = self
            .config
            .data_cache_protocol()
            .map(|protocol| format!("\"{}\"", protocol.as_str()))
            .unwrap_or_else(|| "null".to_string());
        format!(
            "{{\"schema\":\"{}\",\"generator\":\"trace-replay\",\"trace\":\"{}\",\"trace_digest\":\"{}\",\"route\":\"{}\",\"memory_start\":\"0x{:x}\",\"memory_size\":{},\"tick_frequency\":{},\"line_bytes\":{},\"agent\":{},\"control_partition\":{},\"data_cache_protocol\":{},\"simulation\":{},\"summary\":{},\"stats\":{}}}\n",
            self.schema,
            json_escape(&self.config.trace().display().to_string()),
            json_escape(&self.trace_digest),
            json_escape(self.config.route()),
            self.config.memory_start(),
            self.config.memory_size(),
            self.config.tick_frequency(),
            self.config.line_bytes(),
            self.config.agent(),
            self.config.control_partition(),
            data_cache_protocol,
            self.execution.to_json(self.config.max_tick()),
            traffic_trace_summary_json(self.execution.summary()),
            self.stats_json,
        )
    }
}

impl Rem6TraceReplayExecutionSummary {
    fn to_json(&self, max_tick: u64) -> String {
        format!(
            "{{\"status\":\"completed\",\"max_tick\":{},\"final_tick\":{}}}",
            max_tick,
            self.final_tick(),
        )
    }
}

impl Rem6GupsExecutionSummary {
    fn to_json(&self, max_tick: u64) -> String {
        format!(
            "{{\"status\":\"completed\",\"max_tick\":{},\"final_tick\":{},\"scheduled_requests\":{},\"response_stats\":{}}}",
            max_tick,
            self.final_tick,
            self.scheduled_requests,
            gups_response_stats_json(&self.response_stats),
        )
    }
}

fn traffic_trace_summary_json(
    summary: &rem6_workload::WorkloadTrafficTraceReplaySummary,
) -> String {
    let mut fields = vec![format!(
        "\"route\":\"{}\"",
        json_escape(summary.route().as_str())
    )];
    push_json_usize(&mut fields, "scheduled_count", summary.scheduled_count());
    push_json_usize(
        &mut fields,
        "response_delivery_count",
        summary.response_delivery_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_completed_response_count",
        summary.trace_completed_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_retry_response_count",
        summary.trace_retry_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_store_conditional_failed_response_count",
        summary.trace_store_conditional_failed_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_read_response_count",
        summary.trace_read_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_write_response_count",
        summary.trace_write_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_prefetch_response_count",
        summary.trace_prefetch_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_invalidate_response_count",
        summary.trace_invalidate_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_clean_response_count",
        summary.trace_clean_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_upgrade_response_count",
        summary.trace_upgrade_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_llsc_response_count",
        summary.trace_llsc_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_locked_rmw_response_count",
        summary.trace_locked_rmw_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_writable_intent_response_count",
        summary.trace_writable_intent_response_count(),
    );
    push_json_u64(
        &mut fields,
        "trace_response_data_byte_count",
        summary.trace_response_data_byte_count(),
    );
    push_json_u64(
        &mut fields,
        "trace_response_fill_data_byte_count",
        summary.trace_response_fill_data_byte_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_trace_event_count",
        summary.memory_trace_event_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_write_completion_count",
        summary.memory_write_completion_count(),
    );
    push_json_u64(
        &mut fields,
        "memory_write_completion_byte_count",
        summary.memory_write_completion_byte_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_response_count",
        summary.trace_data_cache_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_maintenance_response_count",
        summary.trace_data_cache_maintenance_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_clean_maintenance_response_count",
        summary.trace_data_cache_clean_maintenance_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_invalidate_maintenance_response_count",
        summary.trace_data_cache_invalidate_maintenance_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_error_count",
        summary.trace_data_cache_error_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_invalid_destination_error_count",
        summary.trace_data_cache_invalid_destination_error_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_bad_address_error_count",
        summary.trace_data_cache_bad_address_error_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_read_error_count",
        summary.trace_data_cache_read_error_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_write_error_count",
        summary.trace_data_cache_write_error_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_functional_read_error_count",
        summary.trace_data_cache_functional_read_error_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_functional_write_error_count",
        summary.trace_data_cache_functional_write_error_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_failure_count",
        summary.memory_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_failure_invalid_destination_count",
        summary.memory_failure_invalid_destination_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_failure_bad_address_count",
        summary.memory_failure_bad_address_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_failure_read_count",
        summary.memory_failure_read_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_failure_write_count",
        summary.memory_failure_write_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_failure_functional_read_count",
        summary.memory_failure_functional_read_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_failure_functional_write_count",
        summary.memory_failure_functional_write_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_error_count",
        summary.trace_error_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_error_invalid_destination_count",
        summary.trace_error_invalid_destination_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_error_bad_address_count",
        summary.trace_error_bad_address_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_error_read_count",
        summary.trace_error_read_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_error_write_count",
        summary.trace_error_write_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_error_functional_read_count",
        summary.trace_error_functional_read_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_error_functional_write_count",
        summary.trace_error_functional_write_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_htm_access_count",
        summary.trace_htm_access_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_htm_begin_count",
        summary.trace_htm_begin_count(),
    );
    push_json_usize(
        &mut fields,
        "control_ack_count",
        summary.control_ack_count(),
    );
    push_json_usize(
        &mut fields,
        "sync_control_ack_count",
        summary.sync_control_ack_count(),
    );
    push_json_usize(
        &mut fields,
        "htm_control_ack_count",
        summary.htm_control_ack_count(),
    );
    push_json_usize(
        &mut fields,
        "control_failure_count",
        summary.control_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "control_failure_invalid_destination_count",
        summary.control_failure_invalid_destination_count(),
    );
    push_json_usize(
        &mut fields,
        "control_failure_bad_address_count",
        summary.control_failure_bad_address_count(),
    );
    push_json_usize(
        &mut fields,
        "control_failure_read_count",
        summary.control_failure_read_count(),
    );
    push_json_usize(
        &mut fields,
        "control_failure_write_count",
        summary.control_failure_write_count(),
    );
    push_json_usize(
        &mut fields,
        "control_failure_functional_read_count",
        summary.control_failure_functional_read_count(),
    );
    push_json_usize(
        &mut fields,
        "control_failure_functional_write_count",
        summary.control_failure_functional_write_count(),
    );
    push_json_usize(
        &mut fields,
        "sync_control_failure_count",
        summary.sync_control_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "tlb_control_failure_count",
        summary.tlb_control_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "cache_control_failure_count",
        summary.cache_control_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "htm_control_failure_count",
        summary.htm_control_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "diagnostic_control_failure_count",
        summary.diagnostic_control_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "sideband_event_count",
        summary.sideband_event_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_sideband_failure_count",
        summary.trace_sideband_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "tlb_sync_event_count",
        summary.tlb_sync_event_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_tlb_sync_count",
        summary.trace_tlb_sync_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_tlb_sync_flushed_entry_count",
        summary.trace_tlb_sync_flushed_entry_count(),
    );
    push_json_usize(
        &mut fields,
        "cache_flush_event_count",
        summary.cache_flush_event_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_cache_flush_count",
        summary.trace_cache_flush_count(),
    );
    push_json_u64(
        &mut fields,
        "trace_cache_flush_data_byte_count",
        summary.trace_cache_flush_data_byte_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_l1_invalidation_count",
        summary.trace_l1_invalidation_count(),
    );
    push_json_usize(
        &mut fields,
        "diagnostic_print_event_count",
        summary.diagnostic_print_event_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_diagnostic_count",
        summary.trace_diagnostic_count(),
    );
    push_json_usize(
        &mut fields,
        "htm_abort_event_count",
        summary.htm_abort_event_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_htm_abort_count",
        summary.trace_htm_abort_count(),
    );
    format!("{{{}}}", fields.join(","))
}

fn push_json_usize(fields: &mut Vec<String>, name: &str, value: usize) {
    fields.push(format!("\"{name}\":{value}"));
}

fn push_json_u64(fields: &mut Vec<String>, name: &str, value: u64) {
    fields.push(format!("\"{name}\":{value}"));
}

fn gups_response_stats_json(stats: &rem6_system::TrafficGupsTransportResponseStats) -> String {
    format!(
        "{{\"responses\":{},\"completed\":{},\"retry\":{},\"store_conditional_failed\":{},\"reads\":{},\"writes\":{},\"data_bytes\":{}}}",
        stats.response_count(),
        stats.completed_response_count(),
        stats.retry_response_count(),
        stats.store_conditional_failed_response_count(),
        stats.read_response_count(),
        stats.write_response_count(),
        stats.response_data_byte_count(),
    )
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
            Rem6ExecutionStop::HostTrap { .. }
            | Rem6ExecutionStop::HostStop { .. }
            | Rem6ExecutionStop::TickLimit { .. } => max_instructions,
        };
        let common = format!(
            "\"max_tick\":{},\"instruction_limit\":{},\"memory_route_delay\":{},\"host_event_delay\":{},\"executed_ticks\":{},\"final_tick\":{},\"cores\":{},\"committed_instructions\":{},\"data_access_probes\":{}",
            max_tick,
            optional_count_json(instruction_limit),
            memory_route_delay,
            host_event_delay,
            self.final_tick,
            self.final_tick,
            self.cores.len(),
            self.committed_instructions,
            self.data_access_probes.to_json(),
        );
        match self.stop {
            Rem6ExecutionStop::HostTrap {
                stop_code,
                trap,
                trap_pc,
            } => format!(
                "{{\"status\":\"executed_until_trap\",\"stop_reason\":\"host_trap\",{},\"stop_code\":{},\"trap\":\"{}\",\"trap_pc\":\"0x{:x}\"}}",
                common, stop_code, trap, trap_pc
            ),
            Rem6ExecutionStop::HostStop { stop_code } => format!(
                "{{\"status\":\"stopped_by_host\",\"stop_reason\":\"host_stop\",{},\"stop_code\":{}}}",
                common, stop_code
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

    fn to_riscv_guest_writes_json(&self) -> String {
        let writes = self
            .riscv_guest_writes
            .iter()
            .map(Rem6RiscvGuestWriteSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!("[{writes}]")
    }

    fn to_riscv_unknown_syscalls_json(&self) -> String {
        let syscalls = self
            .riscv_unknown_syscalls
            .iter()
            .map(Rem6RiscvUnknownSyscallSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!("[{syscalls}]")
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

impl Rem6RiscvGuestWriteSummary {
    fn to_json(&self) -> String {
        let text = std::str::from_utf8(&self.bytes)
            .map(|value| format!("\"{}\"", json_escape(value)))
            .unwrap_or_else(|_| "null".to_string());
        format!(
            "{{\"fd\":{},\"address\":\"0x{:x}\",\"tick\":{},\"bytes\":{},\"text\":{},\"hex\":\"{}\"}}",
            self.fd,
            self.address,
            self.tick,
            self.bytes.len(),
            text,
            bytes_to_hex(&self.bytes),
        )
    }
}

impl Rem6RiscvUnknownSyscallSummary {
    fn to_json(&self) -> String {
        let arguments = self
            .arguments
            .iter()
            .map(|argument| format!("\"0x{argument:x}\""))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"pc\":\"0x{:x}\",\"number\":{},\"tick\":{},\"arguments\":[{}]}}",
            self.pc, self.number, self.tick, arguments
        )
    }
}

impl Rem6DataAccessProbeSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"sample_count\":{},\"stack_distance\":{{\"infinite_samples\":{},\"finite_samples\":{},\"stack_depth\":{}}},\"memory_footprint\":{{\"cache_line_bytes\":{},\"cache_line_total_bytes\":{},\"page_bytes\":{},\"page_total_bytes\":{}}}}}",
            self.sample_count,
            self.stack_distance_infinite_samples,
            self.stack_distance_finite_samples,
            self.stack_distance_stack_depth,
            self.memory_footprint_cache_line_bytes,
            self.memory_footprint_cache_line_total_bytes,
            self.memory_footprint_page_bytes,
            self.memory_footprint_page_total_bytes,
        )
    }
}

impl Rem6DramSummary {
    fn to_json(&self) -> String {
        let profile_technology = optional_string_json(self.profile_technology);
        let profile_parallel_port_label = optional_string_json(self.profile_parallel_port_label);
        let profile_topology_unit_label = optional_string_json(self.profile_topology_unit_label);
        format!(
            "{{\"active_targets\":{},\"active_ports\":{},\"active_banks\":{},\"accesses\":{},\"reads\":{},\"writes\":{},\"row_hits\":{},\"row_misses\":{},\"refreshes\":{},\"refresh_ticks\":{},\"commands\":{},\"turnarounds\":{},\"total_ready_latency_ticks\":{},\"max_ready_latency_ticks\":{},\"profile\":{{\"technology\":{},\"parallel_port_label\":{},\"topology_unit_label\":{},\"geometry\":{{\"bank_count\":{},\"row_size\":{},\"line_size\":{},\"lines_per_row\":{},\"bank_group_count\":{}}},\"timing\":{{\"activate_latency\":{},\"read_latency\":{},\"write_latency\":{},\"precharge_latency\":{},\"bus_turnaround\":{},\"burst_spacing\":{},\"same_bank_group_burst_spacing\":{},\"command_window\":{{\"window_cycles\":{},\"max_commands\":{}}}}},\"low_power_timing\":{{\"precharge_powerdown_entry_delay\":{},\"self_refresh_entry_delay\":{},\"exit_latency\":{},\"self_refresh_exit_latency\":{}}},\"nvm_media\":{{\"read_media_latency\":{},\"write_media_latency\":{},\"send_latency\":{},\"max_pending_reads\":{},\"max_pending_writes\":{}}},\"profiled_targets\":{},\"parallel_ports\":{},\"topology_units\":{},\"scheduler_banks\":{},\"topology_banks\":{},\"scheduler_bank_groups\":{}}},\"nvm\":{{\"persistent_writes\":{},\"persistent_write_bytes\":{},\"max_pending_reads\":{},\"max_pending_persistent_writes\":{}}},\"low_power\":{{\"active_powerdown\":{{\"entries\":{},\"ticks\":{}}},\"precharge_powerdown\":{{\"entries\":{},\"ticks\":{}}},\"self_refresh\":{{\"entries\":{},\"ticks\":{}}},\"exits\":{},\"exit_latency_ticks\":{}}}}}",
            self.active_targets,
            self.active_ports,
            self.active_banks,
            self.accesses,
            self.reads,
            self.writes,
            self.row_hits,
            self.row_misses,
            self.refreshes,
            self.refresh_ticks,
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
            "{{\"cpu\":{},\"pc\":\"0x{:x}\",\"committed_instructions\":{},\"in_order_pipeline\":{{\"cycles\":{},\"retired\":{}}},\"data_loads\":{},\"data_stores\":{},\"data_atomics\":{},\"data_load_bytes\":{},\"data_store_bytes\":{},\"data_atomic_bytes\":{},\"registers\":{{{}}}}}",
            self.cpu,
            self.pc,
            self.committed_instructions,
            self.in_order_pipeline_cycles,
            self.in_order_pipeline_retired,
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

#[cfg(test)]
mod tests {
    use rem6_workload::{WorkloadRouteId, WorkloadTrafficTraceReplaySummary};

    use super::traffic_trace_summary_json;

    #[test]
    fn traffic_trace_summary_json_emits_nonzero_cache_and_sideband_counters() {
        let summary = WorkloadTrafficTraceReplaySummary::new(route_id("cpu0.data"), 3)
            .with_trace_invalidate_response_count(1)
            .with_trace_clean_response_count(1)
            .with_trace_data_cache_response_count(3)
            .with_trace_data_cache_maintenance_response_count(2)
            .with_trace_data_cache_clean_maintenance_response_count(1)
            .with_trace_data_cache_invalidate_maintenance_response_count(1)
            .with_trace_error_count(2)
            .with_trace_error_write_count(1)
            .with_trace_error_functional_write_count(1)
            .with_trace_cache_flush_count(1)
            .with_trace_cache_flush_data_byte_count(64)
            .with_trace_l1_invalidation_count(1)
            .with_trace_diagnostic_count(1);

        let json = traffic_trace_summary_json(&summary);

        assert!(json.contains("\"trace_invalidate_response_count\":1"));
        assert!(json.contains("\"trace_clean_response_count\":1"));
        assert!(json.contains("\"trace_data_cache_response_count\":3"));
        assert!(json.contains("\"trace_data_cache_maintenance_response_count\":2"));
        assert!(json.contains("\"trace_data_cache_clean_maintenance_response_count\":1"));
        assert!(json.contains("\"trace_data_cache_invalidate_maintenance_response_count\":1"));
        assert!(json.contains("\"trace_error_count\":2"));
        assert!(json.contains("\"trace_error_write_count\":1"));
        assert!(json.contains("\"trace_error_functional_write_count\":1"));
        assert!(json.contains("\"trace_cache_flush_count\":1"));
        assert!(json.contains("\"trace_cache_flush_data_byte_count\":64"));
        assert!(json.contains("\"trace_l1_invalidation_count\":1"));
        assert!(json.contains("\"trace_diagnostic_count\":1"));
    }

    fn route_id(value: &str) -> WorkloadRouteId {
        WorkloadRouteId::new(value).unwrap()
    }
}
