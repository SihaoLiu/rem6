use rem6_system::RiscvDataCacheProtocol;

use super::optional_count_json;
use super::parallel::empty_parallel_json;
use super::transport::empty_transport_json;
use crate::formatting::{
    elf_architecture_name, elf_class_name, elf_endian_name, elf_os_name, json_escape,
};
use crate::{
    CliCachePrefetcher, Rem6DramSummary, Rem6ExecutionSummary, Rem6HostActionSummary,
    Rem6LoadBlobSummary, Rem6MemoryResourceSummary, Rem6ReadfileSummary,
    Rem6RiscvSbiConsoleSummary, Rem6RunArtifact, Rem6RunFabricSummary, RequestedIsa,
    RunFabricConfig,
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
                    self.config.memory_system(),
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
        let memory_resources = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_memory_resources_json)
            .unwrap_or_else(|| Rem6MemoryResourceSummary::default().to_json());
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
        let riscv_sbi_console = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_console_json)
            .unwrap_or_else(|| Rem6RiscvSbiConsoleSummary::default().to_json());
        let riscv_sbi_timers = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_timers_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_sbi_hsm_events = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_hsm_events_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_sbi_hsm_wakes = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_hsm_wakes_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_sbi_ipis = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_ipis_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_sbi_rfences = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_rfences_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_sbi_resets = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_resets_json)
            .unwrap_or_else(|| "[]".to_string());
        let host_actions = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_host_actions_json)
            .unwrap_or_else(|| Rem6HostActionSummary::default().to_json());
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
        let empty_fabric = Rem6RunFabricSummary::default();
        let fabric = self
            .execution
            .as_ref()
            .map(|execution| execution.to_fabric_json(self.config.fabric()))
            .unwrap_or_else(|| run_fabric_json(self.config.fabric(), &empty_fabric));
        let debug = self
            .execution
            .as_ref()
            .and_then(Rem6ExecutionSummary::debug_json_field)
            .unwrap_or_default();
        let load_blobs = self
            .load_blobs
            .iter()
            .map(Rem6LoadBlobSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let readfiles = self
            .readfiles
            .iter()
            .map(Rem6ReadfileSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let riscv_boot = if self.config.isa() == RequestedIsa::Riscv {
            format!(
                ",\"riscv_boot\":{{\"a0\":\"0x{:x}\",\"a1\":\"0x{:x}\",\"sbi\":{},\"se\":{}}}",
                self.config.riscv_boot_a0(),
                self.config.riscv_boot_a1(),
                self.config.riscv_sbi(),
                self.config.riscv_se()
            )
        } else {
            String::new()
        };
        let instruction_cache_protocol =
            optional_riscv_cache_protocol_json(self.config.instruction_cache_protocol());
        let instruction_cache_l2_protocol =
            optional_riscv_cache_protocol_json(self.config.instruction_cache_l2_protocol());
        let instruction_cache_l3_protocol =
            optional_riscv_cache_protocol_json(self.config.instruction_cache_l3_protocol());
        let instruction_cache_prefetcher =
            optional_cache_prefetcher_json(self.config.instruction_cache_prefetcher());
        let data_cache_protocol =
            optional_riscv_cache_protocol_json(self.config.data_cache_protocol());
        let data_cache_l2_protocol =
            optional_riscv_cache_protocol_json(self.config.data_cache_l2_protocol());
        let data_cache_l3_protocol =
            optional_riscv_cache_protocol_json(self.config.data_cache_l3_protocol());
        let data_cache_prefetcher =
            optional_cache_prefetcher_json(self.config.data_cache_prefetcher());
        let power_analysis = self
            .power_analysis
            .as_ref()
            .map(|artifact| format!(",\"power_analysis\":{}", artifact.to_json()))
            .unwrap_or_default();
        format!(
            "{{\"schema\":\"{}\",\"isa\":\"{}\",\"binary\":\"{}\",\"entry\":\"0x{:x}\",\"start_address\":\"0x{:x}\"{},\"instruction_cache_protocol\":{},\"instruction_cache_l2_protocol\":{},\"instruction_cache_l3_protocol\":{},\"instruction_cache_prefetcher\":{},\"data_cache_protocol\":{},\"data_cache_l2_protocol\":{},\"data_cache_l3_protocol\":{},\"data_cache_prefetcher\":{},\"load_blobs\":[{}],\"readfiles\":[{}],\"elf\":{{\"class\":\"{}\",\"endian\":\"{}\",\"architecture\":\"{}\",\"os\":\"{}\",\"machine\":{},\"flags\":{}}},\"simulation\":{},\"parallel\":{},\"cores\":{},\"memory\":{},\"memory_resources\":{},\"riscv_guest_writes\":{},\"riscv_unknown_syscalls\":{},\"riscv_sbi_console\":{},\"riscv_sbi_timers\":{},\"riscv_sbi_hsm_events\":{},\"riscv_sbi_hsm_wakes\":{},\"riscv_sbi_ipis\":{},\"riscv_sbi_rfences\":{},\"riscv_sbi_resets\":{},\"host_actions\":{},\"dram\":{},\"transport\":{},\"fabric\":{}{},\"stats\":{}{}}}\n",
            self.schema,
            self.config.isa().as_str(),
            json_escape(&self.config.binary().display().to_string()),
            self.entry,
            self.start_address,
            riscv_boot,
            instruction_cache_protocol,
            instruction_cache_l2_protocol,
            instruction_cache_l3_protocol,
            instruction_cache_prefetcher,
            data_cache_protocol,
            data_cache_l2_protocol,
            data_cache_l3_protocol,
            data_cache_prefetcher,
            load_blobs,
            readfiles,
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
            memory_resources,
            riscv_guest_writes,
            riscv_unknown_syscalls,
            riscv_sbi_console,
            riscv_sbi_timers,
            riscv_sbi_hsm_events,
            riscv_sbi_hsm_wakes,
            riscv_sbi_ipis,
            riscv_sbi_rfences,
            riscv_sbi_resets,
            host_actions,
            dram,
            transport,
            fabric,
            debug,
            self.stats_json,
            power_analysis,
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
    fn to_fabric_json(&self, config: Option<&RunFabricConfig>) -> String {
        run_fabric_json(config, &self.fabric)
    }
}

fn run_fabric_json(config: Option<&RunFabricConfig>, summary: &Rem6RunFabricSummary) -> String {
    let Some(config) = config else {
        return "null".to_string();
    };
    let credit_depth = config
        .credit_depth()
        .map(|depth| depth.to_string())
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"link\":\"{}\",\"bandwidth_bytes_per_tick\":{},\"request_virtual_network\":{},\"response_virtual_network\":{},\"credit_depth\":{},\"active_lanes\":{},\"active_virtual_networks\":{},\"transfers\":{},\"bytes\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"contended_lanes\":{},\"lane_activities\":[{}],\"hop_activities\":[{}]}}",
        json_escape(config.link()),
        config.bandwidth_bytes_per_tick(),
        config.request_virtual_network(),
        config.response_virtual_network(),
        credit_depth,
        summary.active_lanes(),
        summary.active_virtual_networks(),
        summary.transfers(),
        summary.bytes(),
        summary.occupied_ticks(),
        summary.queue_delay_ticks(),
        summary.max_queue_delay_ticks(),
        summary.contended_lanes(),
        run_fabric_lane_activities_json(summary),
        run_fabric_hop_activities_json(summary),
    )
}

fn run_fabric_lane_activities_json(summary: &Rem6RunFabricSummary) -> String {
    summary
        .lane_activities()
        .iter()
        .map(|activity| {
            format!(
                "{{\"link\":\"{}\",\"virtual_network\":{},\"transfer_count\":{},\"byte_count\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"first_tick\":{},\"last_tick\":{}}}",
                json_escape(activity.link().as_str()),
                activity.virtual_network().get(),
                activity.transfer_count(),
                activity.byte_count(),
                activity.occupied_ticks(),
                activity.queue_delay_ticks(),
                activity.max_queue_delay_ticks(),
                activity.first_tick(),
                activity.last_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn run_fabric_hop_activities_json(summary: &Rem6RunFabricSummary) -> String {
    summary
        .hop_activities()
        .iter()
        .map(|activity| {
            format!(
                "{{\"packet\":{},\"hop_index\":{},\"link\":\"{}\",\"virtual_network\":{},\"bytes\":{},\"ready_tick\":{},\"start_tick\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"depart_tick\":{},\"arrival_tick\":{}}}",
                activity.packet().get(),
                activity.hop_index(),
                json_escape(activity.link().as_str()),
                activity.virtual_network().get(),
                activity.bytes(),
                activity.ready_tick(),
                activity.start_tick(),
                activity.occupied_ticks(),
                activity.queue_delay_ticks(),
                activity.depart_tick(),
                activity.arrival_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

impl Rem6LoadBlobSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"address\":\"0x{:x}\",\"bytes\":{},\"path\":\"{}\"}}",
            self.address(),
            self.bytes(),
            json_escape(self.source())
        )
    }
}

impl Rem6ReadfileSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"base\":\"0x{:x}\",\"size\":{},\"bytes\":{},\"path\":\"{}\"}}",
            self.base(),
            self.size(),
            self.bytes(),
            json_escape(self.path())
        )
    }
}

fn optional_riscv_cache_protocol_json(value: Option<RiscvDataCacheProtocol>) -> String {
    value
        .map(|protocol| format!("\"{}\"", riscv_cache_protocol_name(protocol)))
        .unwrap_or_else(|| "null".to_string())
}

const fn riscv_cache_protocol_name(protocol: RiscvDataCacheProtocol) -> &'static str {
    match protocol {
        RiscvDataCacheProtocol::Msi => "msi",
        RiscvDataCacheProtocol::Mesi => "mesi",
        RiscvDataCacheProtocol::Moesi => "moesi",
        RiscvDataCacheProtocol::Chi => "chi",
    }
}

fn optional_cache_prefetcher_json(value: Option<CliCachePrefetcher>) -> String {
    value
        .map(|prefetcher| format!("\"{}\"", prefetcher.as_str()))
        .unwrap_or_else(|| "null".to_string())
}
