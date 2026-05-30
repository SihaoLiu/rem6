use super::{
    bytes_to_hex, elf_architecture_name, elf_class_name, elf_endian_name, elf_os_name, json_escape,
    Rem6CoreSummary, Rem6ExecutionSummary, Rem6MemoryDump, Rem6ParallelPartitionSummary,
    Rem6RunArtifact,
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
            None => format!(
                "{{\"scheduler\":{{\"worker_limit\":{},\"worker_slots\":[],\"worker_lanes\":[],\"partitions\":[]}}}}",
                self.config.parallel_workers(),
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
        format!(
            "{{\"schema\":\"{}\",\"isa\":\"{}\",\"binary\":\"{}\",\"entry\":\"0x{:x}\",\"elf\":{{\"class\":\"{}\",\"endian\":\"{}\",\"architecture\":\"{}\",\"os\":\"{}\",\"machine\":{},\"flags\":{}}},\"simulation\":{},\"parallel\":{},\"cores\":{},\"memory\":{},\"stats\":{}}}\n",
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
            .map(Rem6ParallelWorkerSlotSummaryJson::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let lanes = self
            .parallel_scheduler_worker_lanes
            .iter()
            .map(Rem6ParallelWorkerLaneSummaryJson::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let partitions = self
            .parallel_scheduler_partitions
            .iter()
            .map(Rem6ParallelPartitionSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"scheduler\":{{\"worker_limit\":{},\"epochs\":{},\"dispatches\":{},\"batches\":{},\"max_workers\":{},\"total_workers\":{},\"active_partitions\":{},\"remote_sends\":{},\"batch_worker_ticks\":{},\"batch_worker_capacity_ticks\":{},\"batch_idle_worker_ticks\":{},\"worker_slots\":[{}],\"worker_lanes\":[{}],\"partitions\":[{}]}}}}",
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
}

trait Rem6ParallelWorkerSlotSummaryJson {
    fn to_json(&self) -> String;
}

impl Rem6ParallelWorkerSlotSummaryJson for super::Rem6ParallelWorkerSlotSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"slot\":{},\"active_ticks\":{},\"idle_ticks\":{}}}",
            self.slot, self.active_ticks, self.idle_ticks
        )
    }
}

trait Rem6ParallelWorkerLaneSummaryJson {
    fn to_json(&self) -> String;
}

impl Rem6ParallelWorkerLaneSummaryJson for super::Rem6ParallelWorkerLaneSummary {
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
