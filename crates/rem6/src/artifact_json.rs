use super::formatting::{
    bytes_to_hex, elf_architecture_name, elf_class_name, elf_endian_name, elf_os_name, json_escape,
};
use super::{
    Rem6CoreSummary, Rem6ExecutionSummary, Rem6MemoryDump, Rem6MemoryTransportCounters,
    Rem6MemoryTransportRouteSummary, Rem6MemoryTransportSummary, Rem6ParallelFrontierSummary,
    Rem6ParallelPartitionSummary, Rem6ParallelReadyPartitionSummary, Rem6RunArtifact,
};

impl Rem6RunArtifact {
    pub fn to_json(&self) -> String {
        let simulation = match &self.execution {
            Some(execution) => execution.to_simulation_json(self.config.max_tick()),
            None => format!(
                "{{\"status\":\"loaded\",\"max_tick\":{},\"executed_ticks\":0,\"cores\":{}}}",
                self.config.max_tick(),
                self.config.cores(),
            ),
        };
        let parallel = match &self.execution {
            Some(execution) => execution.to_parallel_json(self.config.parallel_workers()),
            None => empty_parallel_json(self.config.parallel_workers()),
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
        let transport = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_transport_json)
            .unwrap_or_else(empty_transport_json);
        format!(
            "{{\"schema\":\"{}\",\"isa\":\"{}\",\"binary\":\"{}\",\"entry\":\"0x{:x}\",\"elf\":{{\"class\":\"{}\",\"endian\":\"{}\",\"architecture\":\"{}\",\"os\":\"{}\",\"machine\":{},\"flags\":{}}},\"simulation\":{},\"parallel\":{},\"cores\":{},\"memory\":{},\"transport\":{},\"stats\":{}}}\n",
            self.schema,
            self.config.isa().as_str(),
            json_escape(&self.config.binary().display().to_string()),
            self.entry,
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

impl Rem6ExecutionSummary {
    fn to_simulation_json(&self, max_tick: u64) -> String {
        format!(
            "{{\"status\":\"executed_until_trap\",\"max_tick\":{},\"executed_ticks\":{},\"final_tick\":{},\"cores\":{},\"stop_code\":{},\"trap\":\"{}\"}}",
            max_tick,
            self.final_tick,
            self.final_tick,
            self.cores.len(),
            self.stop_code,
            self.trap,
        )
    }

    fn to_parallel_json(&self, worker_limit: usize) -> String {
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
            "{{\"scheduler\":{{\"worker_limit\":{},\"epochs\":{},\"dispatches\":{},\"batches\":{},\"max_workers\":{},\"total_workers\":{},\"active_partitions\":{},\"remote_sends\":{},\"batch_worker_ticks\":{},\"batch_worker_capacity_ticks\":{},\"batch_idle_worker_ticks\":{},\"worker_slots\":[{}],\"worker_lanes\":[{}],\"partitions\":[{}],\"frontiers\":[{}],\"final_frontiers\":[{}],\"ready_partitions\":[{}]}}}}",
            worker_limit,
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
}

fn empty_parallel_json(worker_limit: usize) -> String {
    format!(
        "{{\"scheduler\":{{\"worker_limit\":{},\"epochs\":0,\"dispatches\":0,\"batches\":0,\"max_workers\":0,\"total_workers\":0,\"active_partitions\":0,\"remote_sends\":0,\"batch_worker_ticks\":0,\"batch_worker_capacity_ticks\":0,\"batch_idle_worker_ticks\":0,\"worker_slots\":[],\"worker_lanes\":[],\"partitions\":[],\"frontiers\":[],\"final_frontiers\":[],\"ready_partitions\":[]}}}}",
        worker_limit
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
