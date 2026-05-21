use rem6_dram::{DramMemoryTechnology, ExternalMemoryProfile, ExternalMemoryTopology};

use crate::{
    CheckpointLineage, HostEventIntent, WorkloadBootImage, WorkloadHostEvent, WorkloadId,
    WorkloadManifestIdentity, WorkloadResource, WorkloadResourceId, WorkloadTopology,
};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

pub(crate) fn manifest_identity(
    id: &WorkloadId,
    boot: &WorkloadBootImage,
    topology: Option<&WorkloadTopology>,
    resources: &[WorkloadResource],
    required_resources: &[WorkloadResourceId],
    host_events: &[WorkloadHostEvent],
    checkpoint_lineage: Option<&CheckpointLineage>,
) -> WorkloadManifestIdentity {
    let mut hash = FNV_OFFSET;
    hash_str(&mut hash, "rem6.workload.manifest.v1");
    hash_str(&mut hash, id.as_str());
    hash_u64(&mut hash, boot.entry().get());
    hash_u64(&mut hash, boot.segments().len() as u64);
    for segment in boot.segments() {
        hash_u64(&mut hash, segment.range().start().get());
        hash_u64(&mut hash, segment.range().size().bytes());
        hash_bytes(&mut hash, segment.data());
    }
    hash_topology(&mut hash, topology);
    hash_u64(&mut hash, resources.len() as u64);
    for resource in resources {
        hash_str(&mut hash, resource.id().as_str());
        hash_u64(&mut hash, resource.kind() as u64);
        hash_str(&mut hash, resource.digest());
        hash_str(&mut hash, resource.locator());
    }
    hash_u64(&mut hash, required_resources.len() as u64);
    for resource in required_resources {
        hash_str(&mut hash, resource.as_str());
    }
    hash_u64(&mut hash, host_events.len() as u64);
    for event in host_events {
        hash_u64(&mut hash, event.tick());
        hash_host_event(&mut hash, event.intent());
    }
    hash_checkpoint_lineage(&mut hash, checkpoint_lineage);
    WorkloadManifestIdentity::new(hash)
}

fn hash_topology(hash: &mut u64, topology: Option<&WorkloadTopology>) {
    let Some(topology) = topology else {
        hash_str(hash, "topology.none");
        return;
    };

    hash_str(hash, "topology.riscv.v1");
    hash_u64(hash, u64::from(topology.partition_count()));
    hash_u64(hash, topology.min_remote_delay());
    hash_u64(hash, topology.parallel_worker_limit() as u64);
    hash_u64(hash, u64::from(topology.host().partition()));
    hash_u64(hash, topology.host().latency());
    hash_u64(hash, u64::from(topology.host().source()));
    hash_u64(hash, topology.memory_targets().len() as u64);
    for target in topology.memory_targets() {
        hash_u64(hash, u64::from(target.target()));
        hash_u64(hash, target.line_bytes());
        hash_u64(hash, target.range().start().get());
        hash_u64(hash, target.range().size().bytes());
        hash_external_memory_profile(hash, target.external_memory_profile());
    }
    hash_u64(hash, topology.memory_routes().len() as u64);
    for route in topology.memory_routes() {
        hash_str(hash, route.id().as_str());
        hash_str(hash, route.source_endpoint());
        hash_u64(hash, u64::from(route.source_partition()));
        hash_str(hash, route.target_endpoint());
        hash_u64(hash, u64::from(route.target_partition()));
        hash_u64(hash, route.request_latency());
        hash_u64(hash, route.response_latency());
    }
    hash_u64(hash, topology.riscv_cores().len() as u64);
    for core in topology.riscv_cores() {
        hash_u64(hash, u64::from(core.cpu()));
        hash_u64(hash, u64::from(core.partition()));
        hash_u64(hash, u64::from(core.agent()));
        hash_u64(hash, core.entry().get());
        hash_str(hash, core.fetch_endpoint());
        hash_str(hash, core.fetch_route().as_str());
        match (core.data_endpoint(), core.data_route()) {
            (Some(endpoint), Some(route)) => {
                hash_str(hash, "data");
                hash_str(hash, endpoint);
                hash_str(hash, route.as_str());
            }
            (None, None) => hash_str(hash, "data.none"),
            _ => hash_str(hash, "data.invalid"),
        }
    }
    match topology.riscv_data_cache() {
        Some(cache) => {
            hash_str(hash, "riscv.data_cache");
            hash_str(hash, cache.protocol().as_str());
            hash_u64(hash, u64::from(cache.memory_target()));
            for line_address in cache.line_addresses() {
                hash_u64(hash, line_address.get());
            }
            hash_u64(hash, u64::from(cache.directory_partition()));
            hash_str(hash, cache.directory_endpoint());
        }
        None => hash_str(hash, "riscv.data_cache.none"),
    }
    hash_u64(hash, topology.gpu_devices().len() as u64);
    for device in topology.gpu_devices() {
        hash_str(hash, "gpu.device");
        hash_u64(hash, u64::from(device.device()));
        hash_u64(hash, u64::from(device.partition()));
        hash_u64(hash, u64::from(device.compute_units()));
        hash_u64(hash, u64::from(device.wave_slots_per_compute_unit()));
        hash_str(hash, device.command_route().as_str());
    }
    hash_u64(hash, topology.gpu_kernel_launches().len() as u64);
    for launch in topology.gpu_kernel_launches() {
        hash_str(hash, "gpu.kernel_launch");
        hash_u64(hash, u64::from(launch.device()));
        hash_u64(hash, launch.kernel());
        hash_u64(hash, u64::from(launch.workgroups()));
        hash_u64(hash, launch.workgroup_latency());
    }
    hash_u64(hash, topology.gpu_dma_copies().len() as u64);
    for copy in topology.gpu_dma_copies() {
        hash_str(hash, "gpu.dma_copy");
        hash_u64(hash, u64::from(copy.device()));
        hash_u64(hash, copy.transfer());
        hash_str(hash, copy.route().as_str());
        hash_u64(hash, u64::from(copy.agent()));
        hash_u64(hash, copy.source().get());
        hash_u64(hash, copy.destination().get());
        hash_u64(hash, copy.bytes());
    }
    hash_u64(hash, topology.accelerator_devices().len() as u64);
    for device in topology.accelerator_devices() {
        hash_str(hash, "accelerator.device");
        hash_u64(hash, u64::from(device.engine()));
        hash_u64(hash, u64::from(device.partition()));
        hash_u64(hash, u64::from(device.lanes()));
        hash_str(hash, device.command_route().as_str());
    }
    hash_u64(hash, topology.accelerator_commands().len() as u64);
    for command in topology.accelerator_commands() {
        hash_str(hash, "accelerator.command");
        hash_u64(hash, u64::from(command.engine()));
        hash_u64(hash, command.command());
        hash_accelerator_command_kind(hash, command.kind());
        hash_u64(hash, command.execution_latency());
    }
    hash_u64(hash, topology.accelerator_dma_copies().len() as u64);
    for copy in topology.accelerator_dma_copies() {
        hash_str(hash, "accelerator.dma_copy");
        hash_u64(hash, u64::from(copy.engine()));
        hash_u64(hash, copy.transfer());
        hash_str(hash, copy.route().as_str());
        hash_u64(hash, u64::from(copy.agent()));
        hash_u64(hash, copy.source().get());
        hash_u64(hash, copy.destination().get());
        hash_u64(hash, copy.bytes());
    }
}

fn hash_accelerator_command_kind(hash: &mut u64, kind: &crate::WorkloadAcceleratorCommandKind) {
    match kind {
        crate::WorkloadAcceleratorCommandKind::GpuKernel { workgroups } => {
            hash_str(hash, "gpu_kernel");
            hash_u64(hash, u64::from(*workgroups));
        }
        crate::WorkloadAcceleratorCommandKind::NpuInference { tiles } => {
            hash_str(hash, "npu_inference");
            hash_u64(hash, u64::from(*tiles));
        }
        crate::WorkloadAcceleratorCommandKind::DmaCopy { bytes } => {
            hash_str(hash, "dma_copy");
            hash_u64(hash, *bytes);
        }
    }
}

fn hash_external_memory_profile(hash: &mut u64, profile: Option<&ExternalMemoryProfile>) {
    let Some(profile) = profile else {
        hash_str(hash, "memory.profile.none");
        return;
    };

    hash_str(hash, "memory.profile.v1");
    hash_u64(hash, u64::from(profile.target().get()));
    hash_u64(hash, profile.line_layout().bytes());
    hash_u64(hash, u64::from(profile.geometry().bank_count()));
    hash_u64(hash, profile.geometry().row_size());
    hash_u64(hash, profile.geometry().line_size());
    hash_u64(hash, profile.timing().activate_latency());
    hash_u64(hash, profile.timing().read_latency());
    hash_u64(hash, profile.timing().write_latency());
    hash_u64(hash, profile.timing().precharge_latency());
    hash_u64(hash, profile.timing().bus_turnaround());
    match profile.technology() {
        DramMemoryTechnology::Ddr => hash_str(hash, "ddr"),
        DramMemoryTechnology::Hbm => hash_str(hash, "hbm"),
        DramMemoryTechnology::Lpddr => hash_str(hash, "lpddr"),
    }
    match profile.topology() {
        ExternalMemoryTopology::Ddr {
            channels,
            ranks_per_channel,
        } => {
            hash_str(hash, "ddr.topology");
            hash_u64(hash, u64::from(channels));
            hash_u64(hash, u64::from(ranks_per_channel));
        }
        ExternalMemoryTopology::Hbm {
            stacks,
            pseudo_channels_per_stack,
        } => {
            hash_str(hash, "hbm.topology");
            hash_u64(hash, u64::from(stacks));
            hash_u64(hash, u64::from(pseudo_channels_per_stack));
        }
        ExternalMemoryTopology::Lpddr {
            channels,
            dies_per_channel,
        } => {
            hash_str(hash, "lpddr.topology");
            hash_u64(hash, u64::from(channels));
            hash_u64(hash, u64::from(dies_per_channel));
        }
    }
}

fn hash_host_event(hash: &mut u64, intent: &HostEventIntent) {
    match intent {
        HostEventIntent::RoiBegin { label } => {
            hash_str(hash, "roi_begin");
            hash_str(hash, label);
        }
        HostEventIntent::RoiEnd { label } => {
            hash_str(hash, "roi_end");
            hash_str(hash, label);
        }
        HostEventIntent::StatsReset { label } => {
            hash_str(hash, "stats_reset");
            hash_str(hash, label);
        }
        HostEventIntent::StatsDump { label } => {
            hash_str(hash, "stats_dump");
            hash_str(hash, label);
        }
        HostEventIntent::Checkpoint { label } => {
            hash_str(hash, "checkpoint");
            hash_str(hash, label);
        }
        HostEventIntent::Stop { reason } => {
            hash_str(hash, "stop");
            hash_str(hash, reason);
        }
    }
}

fn hash_checkpoint_lineage(hash: &mut u64, lineage: Option<&CheckpointLineage>) {
    match lineage {
        None => hash_str(hash, "lineage.none"),
        Some(CheckpointLineage::CreatedByWorkload { label }) => {
            hash_str(hash, "lineage.created");
            hash_str(hash, label);
        }
        Some(CheckpointLineage::RestoredFrom {
            label,
            manifest_identity,
        }) => {
            hash_str(hash, "lineage.restored");
            hash_str(hash, label);
            hash_str(hash, manifest_identity);
        }
    }
}

fn hash_str(hash: &mut u64, value: &str) {
    hash_u64(hash, value.len() as u64);
    hash_bytes(hash, value.as_bytes());
}

fn hash_u64(hash: &mut u64, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}
