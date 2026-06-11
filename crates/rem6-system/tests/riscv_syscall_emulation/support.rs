pub(crate) use std::sync::{Arc, Mutex};

pub(crate) use rem6_boot::BootImage;
pub(crate) use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
};
pub(crate) use rem6_isa_riscv::{Register, RiscvPrivilegeMode};
pub(crate) use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerContext};
pub(crate) use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemoryStore,
};
pub(crate) use rem6_stats::StatsRegistry;
pub(crate) use rem6_system::{
    GuestEventId, GuestSourceId, HostEventPolicy, RiscvMmapRegion, RiscvSystemRunDriver,
    RiscvTrapEventPort, StopRequest, SystemActionOutcome, SystemHostController,
    SystemHostEventPort,
};
pub(crate) use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome,
    TransportEndpointId,
};

pub(crate) fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

pub(crate) fn addi(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0x0, rd, 0x13)
}

pub(crate) fn lb(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0x0, rd, 0x03)
}

pub(crate) fn lui(rd: u8, imm: u32) -> u32 {
    (imm << 12) | (u32::from(rd) << 7) | 0x37
}

pub(crate) fn loaded_program_store(
    instructions: &[(u64, u32)],
) -> Arc<Mutex<PartitionedMemoryStore>> {
    loaded_program_store_with_data(instructions, &[])
}

pub(crate) fn loaded_program_store_with_data(
    instructions: &[(u64, u32)],
    data_segments: &[(u64, &[u8])],
) -> Arc<Mutex<PartitionedMemoryStore>> {
    let image = boot_image_with_data(instructions, data_segments);
    loaded_boot_image_store(&image)
}

pub(crate) fn boot_image_with_data(
    instructions: &[(u64, u32)],
    data_segments: &[(u64, &[u8])],
) -> BootImage {
    let mut image = BootImage::new(Address::new(instructions[0].0));
    for (address, instruction) in instructions {
        image = image
            .add_segment(Address::new(*address), word(*instruction))
            .unwrap();
    }
    for (address, data) in data_segments {
        image = image
            .add_segment(Address::new(*address), data.to_vec())
            .unwrap();
    }
    image
}

pub(crate) fn loaded_boot_image_store(image: &BootImage) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap();

    image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

pub(crate) fn riscv_core(
    cpu: u32,
    partition: u32,
    agent: u32,
    fetch_endpoint: &str,
    fetch_route: MemoryRouteId,
    entry: u64,
) -> RiscvCore {
    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(cpu),
                PartitionId::new(partition),
                AgentId::new(agent),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint(fetch_endpoint),
                fetch_route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn riscv_data_core(
    cpu: u32,
    partition: u32,
    agent: u32,
    fetch_endpoint: &str,
    fetch_route: MemoryRouteId,
    data_endpoint: &str,
    data_route: MemoryRouteId,
    entry: u64,
) -> RiscvCore {
    RiscvCore::with_data(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(cpu),
                PartitionId::new(partition),
                AgentId::new(agent),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint(fetch_endpoint),
                fetch_route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
        CpuDataConfig::new(endpoint(data_endpoint), data_route, layout()),
    )
}

fn memory_response(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    let response = store
        .lock()
        .unwrap()
        .respond(delivery.request())
        .unwrap()
        .response()
        .cloned()
        .unwrap();
    TargetOutcome::Respond(response)
}

pub(crate) fn responder(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static {
    move |delivery, _context| memory_response(&store, &delivery)
}

pub(crate) fn guest_memory_reader(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static {
    move |address, bytes| {
        let request = MemoryRequest::read_shared(
            MemoryRequestId::new(AgentId::new(99), 0),
            Address::new(address),
            AccessSize::new(bytes as u64).ok()?,
            layout(),
        )
        .ok()?;
        let outcome = store.lock().unwrap().respond(&request).ok()?;
        outcome
            .response()
            .and_then(|response| response.data())
            .map(Vec::from)
    }
}

pub(crate) fn guest_memory_writer(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl Fn(u64, &[u8]) -> bool + Send + Sync + 'static {
    move |address, bytes| {
        let Ok(size) = AccessSize::new(bytes.len() as u64) else {
            return false;
        };
        let Ok(byte_mask) = ByteMask::full(size) else {
            return false;
        };
        let Ok(request) = MemoryRequest::write(
            MemoryRequestId::new(AgentId::new(100), 0),
            Address::new(address),
            size,
            bytes.to_vec(),
            byte_mask,
            layout(),
        ) else {
            return false;
        };
        store.lock().unwrap().respond(&request).is_ok()
    }
}

pub(crate) fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}
