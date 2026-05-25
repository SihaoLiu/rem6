use rem6_dram::{DramMemoryTechnology, ExternalMemoryProfile, ExternalMemoryTopology};

use crate::{
    CheckpointLineage, HostEventIntent, WorkloadBootImage, WorkloadExpectedParallelPartitionUse,
    WorkloadExpectedParallelRemoteFlow, WorkloadExpectedParallelWorkerUse, WorkloadHostEvent,
    WorkloadId, WorkloadLinuxBootHandoff, WorkloadManifestIdentity,
    WorkloadParallelRemoteFlowScope, WorkloadResource, WorkloadResourceId, WorkloadTopology,
};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

pub(crate) struct ManifestIdentityInput<'a> {
    pub(crate) id: &'a WorkloadId,
    pub(crate) boot: &'a WorkloadBootImage,
    pub(crate) linux_boot_handoff: Option<&'a WorkloadLinuxBootHandoff>,
    pub(crate) topology: Option<&'a WorkloadTopology>,
    pub(crate) resources: &'a [WorkloadResource],
    pub(crate) required_resources: &'a [WorkloadResourceId],
    pub(crate) host_events: &'a [WorkloadHostEvent],
    pub(crate) expected_parallel_remote_flows: &'a [WorkloadExpectedParallelRemoteFlow],
    pub(crate) expected_parallel_worker_use: &'a [WorkloadExpectedParallelWorkerUse],
    pub(crate) expected_parallel_partition_use: &'a [WorkloadExpectedParallelPartitionUse],
    pub(crate) checkpoint_lineage: Option<&'a CheckpointLineage>,
}

pub(crate) fn manifest_identity(input: ManifestIdentityInput<'_>) -> WorkloadManifestIdentity {
    let mut hash = FNV_OFFSET;
    hash_str(&mut hash, "rem6.workload.manifest.v1");
    hash_str(&mut hash, input.id.as_str());
    hash_u64(&mut hash, input.boot.entry().get());
    hash_u64(&mut hash, input.boot.segments().len() as u64);
    for segment in input.boot.segments() {
        hash_u64(&mut hash, segment.range().start().get());
        hash_u64(&mut hash, segment.range().size().bytes());
        hash_bytes(&mut hash, segment.data());
    }
    hash_linux_boot_handoff(&mut hash, input.linux_boot_handoff);
    hash_topology(&mut hash, input.topology);
    hash_u64(&mut hash, input.resources.len() as u64);
    for resource in input.resources {
        hash_str(&mut hash, resource.id().as_str());
        hash_u64(&mut hash, resource.kind() as u64);
        hash_str(&mut hash, resource.digest());
        hash_str(&mut hash, resource.locator());
    }
    hash_u64(&mut hash, input.required_resources.len() as u64);
    for resource in input.required_resources {
        hash_str(&mut hash, resource.as_str());
    }
    hash_u64(&mut hash, input.host_events.len() as u64);
    for event in input.host_events {
        hash_u64(&mut hash, event.tick());
        hash_host_event(&mut hash, event.intent());
    }
    hash_u64(&mut hash, input.expected_parallel_remote_flows.len() as u64);
    for expected in input.expected_parallel_remote_flows {
        hash_expected_parallel_remote_flow(&mut hash, *expected);
    }
    hash_u64(&mut hash, input.expected_parallel_worker_use.len() as u64);
    for expected in input.expected_parallel_worker_use {
        hash_expected_parallel_worker_use(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_partition_use.len() as u64,
    );
    for expected in input.expected_parallel_partition_use {
        hash_expected_parallel_partition_use(&mut hash, *expected);
    }
    hash_checkpoint_lineage(&mut hash, input.checkpoint_lineage);
    WorkloadManifestIdentity::new(hash)
}

fn hash_expected_parallel_remote_flow(
    hash: &mut u64,
    expected: WorkloadExpectedParallelRemoteFlow,
) {
    hash_parallel_remote_flow_scope(hash, expected.scope());
    hash_u64(hash, u64::from(expected.source().index()));
    hash_u64(hash, u64::from(expected.target().index()));
    hash_u64(hash, expected.send_count() as u64);
}

fn hash_parallel_remote_flow_scope(hash: &mut u64, scope: WorkloadParallelRemoteFlowScope) {
    hash_str(hash, scope.as_str());
}

fn hash_expected_parallel_worker_use(hash: &mut u64, expected: WorkloadExpectedParallelWorkerUse) {
    hash_parallel_remote_flow_scope(hash, expected.scope());
    hash_u64(hash, expected.minimum_max_workers() as u64);
}

fn hash_expected_parallel_partition_use(
    hash: &mut u64,
    expected: WorkloadExpectedParallelPartitionUse,
) {
    hash_parallel_remote_flow_scope(hash, expected.scope());
    hash_u64(hash, expected.minimum_active_partitions() as u64);
}

fn hash_linux_boot_handoff(hash: &mut u64, handoff: Option<&WorkloadLinuxBootHandoff>) {
    let Some(handoff) = handoff else {
        hash_str(hash, "linux.boot_handoff.none");
        return;
    };

    hash_str(hash, "linux.boot_handoff.v2");
    hash_u64(hash, handoff.dtb_addr().get());
    match handoff.device_tree_resource() {
        Some(resource) => {
            hash_str(hash, "device_tree.some");
            hash_str(hash, resource.as_str());
        }
        None => hash_str(hash, "device_tree.none"),
    }
    match handoff.bootargs() {
        Some(bootargs) => {
            hash_str(hash, "bootargs.some");
            hash_str(hash, bootargs);
        }
        None => hash_str(hash, "bootargs.none"),
    }
    match handoff.initrd() {
        Some(initrd) => {
            hash_str(hash, "initrd.some");
            hash_str(hash, initrd.resource().as_str());
            hash_u64(hash, initrd.start().get());
            hash_u64(hash, initrd.size().bytes());
        }
        None => hash_str(hash, "initrd.none"),
    }
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
    hash_qos_policy(hash, topology.qos_policy());
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
        hash_u64(hash, route.hops().len() as u64);
        for hop in route.hops() {
            hash_str(hash, "route.hop");
            hash_str(hash, hop.endpoint());
            hash_u64(hash, u64::from(hop.partition()));
            hash_u64(hash, hop.request_latency());
            hash_u64(hash, hop.response_latency());
            hash_route_fabric(hash, hop.fabric());
        }
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
            hash_str(hash, cache.backing_route().as_str());
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
        hash_str(hash, device.command_endpoint());
        hash_str(hash, device.dma_endpoint());
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
        hash_str(hash, device.command_endpoint());
        hash_str(hash, device.dma_endpoint());
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

fn hash_qos_policy(hash: &mut u64, policy: Option<&crate::WorkloadQosPolicy>) {
    let Some(policy) = policy else {
        hash_str(hash, "qos.policy.none");
        return;
    };

    hash_str(hash, "qos.policy.fixed_priority.v1");
    hash_u64(hash, u64::from(policy.priority_levels()));
    hash_u64(hash, u64::from(policy.default_priority().get()));
    hash_str(hash, policy.queue_policy().as_str());
    hash_str(hash, policy.turnaround_policy().as_str());
    hash_u64(
        hash,
        if policy.priority_escalation_enabled() {
            1
        } else {
            0
        },
    );
    hash_u64(hash, policy.requestor_priorities().len() as u64);
    for requestor in policy.requestor_priorities() {
        hash_u64(hash, u64::from(requestor.requestor().get()));
        hash_u64(hash, u64::from(requestor.priority().get()));
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
    match profile.geometry().bank_group_count() {
        Some(bank_group_count) => {
            hash_str(hash, "geometry.bank_groups.some");
            hash_u64(hash, u64::from(bank_group_count));
        }
        None => hash_str(hash, "geometry.bank_groups.none"),
    }
    hash_u64(hash, profile.timing().activate_latency());
    hash_u64(hash, profile.timing().read_latency());
    hash_u64(hash, profile.timing().write_latency());
    hash_u64(hash, profile.timing().precharge_latency());
    hash_u64(hash, profile.timing().bus_turnaround());
    hash_u64(hash, profile.timing().burst_spacing());
    match profile.timing().same_bank_group_burst_spacing() {
        Some(burst_spacing) => {
            hash_str(hash, "timing.same_bank_group_burst_spacing.some");
            hash_u64(hash, burst_spacing);
        }
        None => hash_str(hash, "timing.same_bank_group_burst_spacing.none"),
    }
    match profile.timing().command_window() {
        Some(command_window) => {
            hash_str(hash, "timing.command_window.some");
            hash_u64(hash, command_window.window_cycles());
            hash_u64(hash, u64::from(command_window.max_commands()));
        }
        None => hash_str(hash, "timing.command_window.none"),
    }
    match profile.technology() {
        DramMemoryTechnology::Ddr => hash_str(hash, "ddr"),
        DramMemoryTechnology::Hbm => hash_str(hash, "hbm"),
        DramMemoryTechnology::Lpddr => hash_str(hash, "lpddr"),
        DramMemoryTechnology::Nvm => hash_str(hash, "nvm"),
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
        ExternalMemoryTopology::Nvm {
            controllers,
            media_banks_per_controller,
        } => {
            hash_str(hash, "nvm.topology");
            hash_u64(hash, u64::from(controllers));
            hash_u64(hash, u64::from(media_banks_per_controller));
        }
    }
    match profile.nvm_media_timing() {
        Some(nvm_media_timing) => {
            hash_str(hash, "nvm.media");
            hash_u64(hash, nvm_media_timing.read_media_latency());
            hash_u64(hash, nvm_media_timing.write_media_latency());
            hash_u64(hash, nvm_media_timing.send_latency());
            hash_u64(hash, u64::from(nvm_media_timing.max_pending_reads()));
            hash_u64(hash, u64::from(nvm_media_timing.max_pending_writes()));
        }
        None => hash_str(hash, "nvm.media.none"),
    }
}

fn hash_route_fabric(hash: &mut u64, fabric: Option<&crate::WorkloadRouteFabric>) {
    let Some(fabric) = fabric else {
        hash_str(hash, "route.fabric.none");
        return;
    };

    hash_str(hash, "route.fabric.v1");
    hash_str(hash, fabric.link());
    hash_u64(hash, fabric.bandwidth_bytes_per_tick());
    hash_u64(hash, u64::from(fabric.request_virtual_network()));
    hash_u64(hash, u64::from(fabric.response_virtual_network()));
    match fabric.credit_depth() {
        Some(credit_depth) => {
            hash_str(hash, "route.fabric.credit");
            hash_u64(hash, u64::from(credit_depth));
        }
        None => hash_str(hash, "route.fabric.no_credit"),
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
        HostEventIntent::SwitchExecutionMode { target, mode } => {
            hash_str(hash, "execution_mode");
            hash_str(hash, target);
            hash_str(hash, mode.as_str());
        }
        HostEventIntent::Checkpoint { label } => {
            hash_str(hash, "checkpoint");
            hash_str(hash, label);
        }
        HostEventIntent::RestoreCheckpoint { label } => {
            hash_str(hash, "restore_checkpoint");
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
