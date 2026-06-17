use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_dram::{
    DramBankActivity, DramLowPowerState, DramMemoryActivityProfile, DramMemoryController,
    DramMemoryError, DramPortActivity, DramTargetActivity,
};
use rem6_memory::{
    AccessSize, Address, AddressRange, AgentId, ByteMask, CacheLineLayout, MemoryError,
    MemoryRequest, MemoryRequestId, PartitionedMemoryStore,
};
use rem6_system::{RiscvGuestMemoryMapResult, RiscvSeStartupImage, RISCV_LINUX_STACK_LIMIT_BYTES};
use rem6_transport::{RequestDelivery, TargetOutcome};

use crate::config::CliDramMemoryProfile;
use crate::guest_memory::{
    build_cli_dram_memory, build_cli_memory_store, cli_fully_covered_cache_line_ranges,
    cli_source_backed_cache_line_ranges, merge_line_ranges, CLI_MEMORY_TARGET,
};
use crate::{
    execute_error, LoadedBlob, MemoryDumpRequest, Rem6CliError, Rem6DramBankSummary,
    Rem6DramPortSummary, Rem6DramSummary, Rem6DramTargetSummary, Rem6MemoryDump,
    CLI_MEMORY_DUMP_AGENT,
};

const CLI_GUEST_MEMORY_AGENT: AgentId = AgentId::new(u32::MAX - 1);

#[derive(Clone)]
pub(super) enum CliMemoryRuntime {
    Store {
        store: Arc<Mutex<PartitionedMemoryStore>>,
        full_line_backing: Arc<Mutex<Vec<AddressRange>>>,
    },
    Dram {
        memory: Arc<Mutex<DramMemoryController>>,
        full_line_backing: Arc<Mutex<Vec<AddressRange>>>,
    },
}

impl CliMemoryRuntime {
    pub(super) fn new(
        image: &rem6_boot::BootImage,
        load_blobs: &[LoadedBlob],
        line_layout: CacheLineLayout,
        use_dram: bool,
        dram_profile: CliDramMemoryProfile,
    ) -> Result<Self, Rem6CliError> {
        let full_line_backing = Arc::new(Mutex::new(cli_source_backed_cache_line_ranges(
            image,
            load_blobs,
            line_layout,
        )?));
        if use_dram {
            return Ok(Self::Dram {
                memory: Arc::new(Mutex::new(build_cli_dram_memory(
                    image,
                    load_blobs,
                    line_layout,
                    dram_profile,
                )?)),
                full_line_backing,
            });
        }

        Ok(Self::Store {
            store: Arc::new(Mutex::new(build_cli_memory_store(
                image,
                load_blobs,
                line_layout,
            )?)),
            full_line_backing,
        })
    }

    pub(super) fn dram_summary_until(&self, final_tick: u64) -> Rem6DramSummary {
        match self {
            Self::Store { .. } => Rem6DramSummary::default(),
            Self::Dram { memory, .. } => {
                let memory = memory.lock().expect("CLI DRAM memory lock");
                Rem6DramSummary::from_target_activities(memory.target_activities_until(final_tick))
            }
        }
    }

    pub(super) const fn uses_dram(&self) -> bool {
        matches!(self, Self::Dram { .. })
    }

    pub(super) fn with_store_mut<R>(
        &self,
        operation: impl FnOnce(&mut PartitionedMemoryStore) -> R,
    ) -> Option<R> {
        match self {
            Self::Store { store, .. } => {
                let mut store = store.lock().expect("CLI memory store lock");
                Some(operation(&mut store))
            }
            Self::Dram { .. } => None,
        }
    }

    pub(super) fn install_riscv_se_startup(
        &self,
        startup: &RiscvSeStartupImage,
        line_layout: CacheLineLayout,
    ) -> Result<(), Rem6CliError> {
        let (stack_start, stack_size) = riscv_se_stack_region(startup)?;
        match self {
            Self::Store {
                store,
                full_line_backing,
            } => {
                let mut store = store.lock().expect("CLI memory store lock");
                store
                    .map_region(CLI_MEMORY_TARGET, stack_start, stack_size)
                    .map_err(execute_error)?;
                if !insert_zero_guest_lines_in_store(
                    &mut store,
                    stack_start.get(),
                    stack_size.bytes(),
                    line_layout,
                ) {
                    return Err(execute_error("failed to install RISC-V SE stack backing"));
                }
                insert_full_line_backing(
                    full_line_backing,
                    stack_start.get(),
                    stack_size.bytes(),
                    line_layout,
                )?;
                write_startup_stack_to_store(&mut store, startup, line_layout)
            }
            Self::Dram {
                memory,
                full_line_backing,
            } => {
                let mut memory = memory.lock().expect("CLI DRAM memory lock");
                memory
                    .map_region(CLI_MEMORY_TARGET, stack_start, stack_size)
                    .map_err(execute_error)?;
                if !insert_zero_guest_lines_in_dram(
                    &mut memory,
                    stack_start.get(),
                    stack_size.bytes(),
                    line_layout,
                ) {
                    return Err(execute_error("failed to install RISC-V SE stack backing"));
                }
                insert_full_line_backing(
                    full_line_backing,
                    stack_start.get(),
                    stack_size.bytes(),
                    line_layout,
                )?;
                write_startup_stack_to_dram(&mut memory, startup, line_layout)
            }
        }
    }

    pub(super) fn read_guest_memory(
        &self,
        address: u64,
        bytes: usize,
        line_layout: CacheLineLayout,
    ) -> Option<Vec<u8>> {
        if bytes == 0 {
            return Some(Vec::new());
        }
        let chunks = guest_memory_chunks(address, bytes, line_layout)?;
        match self {
            Self::Store { store, .. } => {
                let mut store = store.lock().expect("CLI memory store lock");
                read_guest_memory_from_store(&mut store, &chunks, line_layout)
            }
            Self::Dram { memory, .. } => {
                let memory = memory.lock().expect("CLI DRAM memory lock");
                read_guest_memory_from_dram(&memory, &chunks, line_layout)
            }
        }
    }

    pub(super) fn read_guest_cache_line(
        &self,
        line: Address,
        line_layout: CacheLineLayout,
    ) -> Option<Vec<u8>> {
        let full_line_backing = match self {
            Self::Store {
                full_line_backing, ..
            }
            | Self::Dram {
                full_line_backing, ..
            } => full_line_backing,
        };
        if !contains_full_line_backing(full_line_backing, line, line_layout) {
            return None;
        }
        self.read_guest_memory(line.get(), line_layout.bytes() as usize, line_layout)
    }

    pub(super) fn read_guest_cache_line_for_fill(
        &self,
        line: Address,
        line_layout: CacheLineLayout,
        tick: u64,
    ) -> Option<Vec<u8>> {
        let full_line_backing = match self {
            Self::Store {
                full_line_backing, ..
            }
            | Self::Dram {
                full_line_backing, ..
            } => full_line_backing,
        };
        if !contains_full_line_backing(full_line_backing, line, line_layout) {
            return None;
        }
        match self {
            Self::Store { .. } => {
                self.read_guest_memory(line.get(), line_layout.bytes() as usize, line_layout)
            }
            Self::Dram { memory, .. } => {
                let mut memory = memory.lock().expect("CLI DRAM memory lock");
                read_guest_cache_line_from_dram_for_fill(&mut memory, line, line_layout, tick)
            }
        }
    }

    pub(super) fn write_guest_memory(
        &self,
        address: u64,
        bytes: &[u8],
        line_layout: CacheLineLayout,
    ) -> bool {
        if bytes.is_empty() {
            return self.can_write_guest_memory(address, 1, line_layout);
        }
        let Some(chunks) = guest_memory_chunks(address, bytes.len(), line_layout) else {
            return false;
        };
        let Some(requests) = guest_memory_write_requests(bytes, &chunks, line_layout) else {
            return false;
        };
        match self {
            Self::Store { store, .. } => {
                let mut store = store.lock().expect("CLI memory store lock");
                write_guest_memory_to_store(&mut store, &requests, &chunks, line_layout)
            }
            Self::Dram { memory, .. } => {
                let mut memory = memory.lock().expect("CLI DRAM memory lock");
                write_guest_memory_to_dram(&mut memory, &requests, &chunks, line_layout)
            }
        }
    }

    pub(super) fn can_write_guest_memory(
        &self,
        address: u64,
        bytes: usize,
        line_layout: CacheLineLayout,
    ) -> bool {
        if bytes == 0 {
            return true;
        }
        match self {
            Self::Store { store, .. } => {
                let mut store = store.lock().expect("CLI memory store lock");
                prevalidate_store_guest_memory_range(&mut store, address, bytes, line_layout)
            }
            Self::Dram { memory, .. } => {
                let memory = memory.lock().expect("CLI DRAM memory lock");
                prevalidate_dram_guest_memory_range(&memory, address, bytes, line_layout)
            }
        }
    }

    pub(super) fn map_guest_memory(
        &self,
        address: u64,
        bytes: u64,
        line_layout: CacheLineLayout,
        replace_existing: bool,
    ) -> RiscvGuestMemoryMapResult {
        if bytes == 0 {
            return RiscvGuestMemoryMapResult::Mapped;
        }
        let Ok(size) = AccessSize::new(bytes) else {
            return RiscvGuestMemoryMapResult::Failed;
        };
        match self {
            Self::Store {
                store,
                full_line_backing,
            } => {
                let mut store = store.lock().expect("CLI memory store lock");
                let result = store_map_region_result(
                    store.map_region(CLI_MEMORY_TARGET, Address::new(address), size),
                    replace_existing,
                );
                if result != RiscvGuestMemoryMapResult::Mapped {
                    return result;
                }
                if insert_zero_guest_lines_in_store(&mut store, address, bytes, line_layout) {
                    if insert_full_line_backing(full_line_backing, address, bytes, line_layout)
                        .is_err()
                    {
                        return RiscvGuestMemoryMapResult::Failed;
                    }
                    RiscvGuestMemoryMapResult::Mapped
                } else {
                    RiscvGuestMemoryMapResult::Failed
                }
            }
            Self::Dram {
                memory,
                full_line_backing,
            } => {
                let mut memory = memory.lock().expect("CLI DRAM memory lock");
                let result = dram_map_region_result(
                    memory.map_region(CLI_MEMORY_TARGET, Address::new(address), size),
                    replace_existing,
                );
                if result != RiscvGuestMemoryMapResult::Mapped {
                    return result;
                }
                if insert_zero_guest_lines_in_dram(&mut memory, address, bytes, line_layout) {
                    if insert_full_line_backing(full_line_backing, address, bytes, line_layout)
                        .is_err()
                    {
                        return RiscvGuestMemoryMapResult::Failed;
                    }
                    RiscvGuestMemoryMapResult::Mapped
                } else {
                    RiscvGuestMemoryMapResult::Failed
                }
            }
        }
    }
}

fn insert_full_line_backing(
    full_line_backing: &Arc<Mutex<Vec<AddressRange>>>,
    address: u64,
    bytes: u64,
    line_layout: CacheLineLayout,
) -> Result<(), Rem6CliError> {
    let mut ranges = cli_fully_covered_cache_line_ranges(address, bytes, line_layout)?;
    let mut full_line_backing = full_line_backing
        .lock()
        .expect("CLI memory full-line backing lock");
    if full_line_backing.is_empty() {
        *full_line_backing = ranges;
        return Ok(());
    }

    let mut merged = full_line_backing.clone();
    merged.append(&mut ranges);
    merged.sort_by_key(|range| (range.start(), range.end()));
    *full_line_backing = merge_line_ranges(merged)?;
    Ok(())
}

fn contains_full_line_backing(
    full_line_backing: &Arc<Mutex<Vec<AddressRange>>>,
    line: Address,
    line_layout: CacheLineLayout,
) -> bool {
    let Some(end) = line.get().checked_add(line_layout.bytes()) else {
        return false;
    };
    full_line_backing
        .lock()
        .expect("CLI memory full-line backing lock")
        .iter()
        .any(|range| range.start().get() <= line.get() && end <= range.end().get())
}

fn riscv_se_stack_region(
    startup: &RiscvSeStartupImage,
) -> Result<(Address, AccessSize), Rem6CliError> {
    let stack_top = startup.stack_range().end().get();
    let stack_start = stack_top
        .checked_sub(RISCV_LINUX_STACK_LIMIT_BYTES)
        .ok_or_else(|| execute_error("RISC-V SE stack top cannot contain stack limit"))?;
    if startup.stack_range().start().get() < stack_start {
        return Err(execute_error(format!(
            "RISC-V SE startup stack image starts below stack backing at 0x{stack_start:x}"
        )));
    }
    let stack_size = AccessSize::new(RISCV_LINUX_STACK_LIMIT_BYTES).map_err(execute_error)?;
    Ok((Address::new(stack_start), stack_size))
}

fn store_map_region_result(
    result: Result<(), MemoryError>,
    replace_existing: bool,
) -> RiscvGuestMemoryMapResult {
    match result {
        Ok(()) => RiscvGuestMemoryMapResult::Mapped,
        Err(MemoryError::OverlappingAddressRegion { .. }) if replace_existing => {
            RiscvGuestMemoryMapResult::Mapped
        }
        Err(MemoryError::OverlappingAddressRegion { .. }) => RiscvGuestMemoryMapResult::Overlap,
        Err(_) => RiscvGuestMemoryMapResult::Failed,
    }
}

fn dram_map_region_result(
    result: Result<(), DramMemoryError>,
    replace_existing: bool,
) -> RiscvGuestMemoryMapResult {
    match result {
        Ok(()) => RiscvGuestMemoryMapResult::Mapped,
        Err(DramMemoryError::Memory(MemoryError::OverlappingAddressRegion { .. }))
            if replace_existing =>
        {
            RiscvGuestMemoryMapResult::Mapped
        }
        Err(DramMemoryError::Memory(MemoryError::OverlappingAddressRegion { .. })) => {
            RiscvGuestMemoryMapResult::Overlap
        }
        Err(_) => RiscvGuestMemoryMapResult::Failed,
    }
}

fn insert_zero_guest_lines_in_store(
    store: &mut PartitionedMemoryStore,
    address: u64,
    bytes: u64,
    line_layout: CacheLineLayout,
) -> bool {
    let Some(end) = address.checked_add(bytes) else {
        return false;
    };
    let zero_line = vec![0; line_layout.bytes() as usize];
    let mut line = line_layout.line_address(Address::new(address));
    while line.get() < end {
        if store
            .insert_line(CLI_MEMORY_TARGET, line, zero_line.clone())
            .is_err()
        {
            return false;
        }
        let Some(next) = line.get().checked_add(line_layout.bytes()) else {
            return false;
        };
        line = Address::new(next);
    }
    true
}

fn insert_zero_guest_lines_in_dram(
    memory: &mut DramMemoryController,
    address: u64,
    bytes: u64,
    line_layout: CacheLineLayout,
) -> bool {
    let Some(end) = address.checked_add(bytes) else {
        return false;
    };
    let zero_line = vec![0; line_layout.bytes() as usize];
    let mut line = line_layout.line_address(Address::new(address));
    while line.get() < end {
        if memory
            .insert_line(CLI_MEMORY_TARGET, line, zero_line.clone())
            .is_err()
        {
            return false;
        }
        let Some(next) = line.get().checked_add(line_layout.bytes()) else {
            return false;
        };
        line = Address::new(next);
    }
    true
}
#[derive(Clone, Copy)]
struct GuestMemoryChunk {
    address: u64,
    data_offset: usize,
    bytes: usize,
}

fn read_guest_memory_from_store(
    store: &mut PartitionedMemoryStore,
    chunks: &[GuestMemoryChunk],
    line_layout: CacheLineLayout,
) -> Option<Vec<u8>> {
    let mut data = Vec::with_capacity(chunks.iter().map(|chunk| chunk.bytes).sum());
    for chunk in chunks {
        let request = guest_memory_read_request(chunk.address, chunk.bytes, line_layout)?;
        let outcome = store.respond(&request).ok()?;
        let response_data = outcome.response()?.data()?;
        if response_data.len() != chunk.bytes {
            return None;
        }
        data.extend_from_slice(response_data);
    }
    Some(data)
}

fn read_guest_memory_from_dram(
    memory: &DramMemoryController,
    chunks: &[GuestMemoryChunk],
    line_layout: CacheLineLayout,
) -> Option<Vec<u8>> {
    let mut data = Vec::with_capacity(chunks.iter().map(|chunk| chunk.bytes).sum());
    for chunk in chunks {
        let request = guest_memory_read_request(chunk.address, chunk.bytes, line_layout)?;
        let response_data = memory.response_data(&request).ok()?;
        if response_data.len() != chunk.bytes {
            return None;
        }
        data.extend_from_slice(&response_data);
    }
    Some(data)
}

fn read_guest_cache_line_from_dram_for_fill(
    memory: &mut DramMemoryController,
    line: Address,
    line_layout: CacheLineLayout,
    tick: u64,
) -> Option<Vec<u8>> {
    let request = guest_memory_read_request(line.get(), line_layout.bytes() as usize, line_layout)?;
    let outcome = memory.accept(tick, &request).ok()?;
    let response_data = outcome.response()?.data()?;
    if response_data.len() != line_layout.bytes() as usize {
        return None;
    }
    Some(response_data.to_vec())
}

fn write_guest_memory_to_store(
    store: &mut PartitionedMemoryStore,
    requests: &[MemoryRequest],
    chunks: &[GuestMemoryChunk],
    line_layout: CacheLineLayout,
) -> bool {
    if !prevalidate_store_guest_memory(store, chunks, line_layout) {
        return false;
    }
    requests
        .iter()
        .all(|request| store.respond(request).is_ok())
}

fn write_guest_memory_to_dram(
    memory: &mut DramMemoryController,
    requests: &[MemoryRequest],
    chunks: &[GuestMemoryChunk],
    line_layout: CacheLineLayout,
) -> bool {
    if !prevalidate_dram_guest_memory(memory, chunks, line_layout) {
        return false;
    }
    requests
        .iter()
        .all(|request| memory.accept(0, request).is_ok())
}

fn prevalidate_store_guest_memory(
    store: &mut PartitionedMemoryStore,
    chunks: &[GuestMemoryChunk],
    line_layout: CacheLineLayout,
) -> bool {
    chunks
        .iter()
        .copied()
        .all(|chunk| prevalidate_store_guest_memory_chunk(store, chunk, line_layout))
}

fn prevalidate_store_guest_memory_range(
    store: &mut PartitionedMemoryStore,
    address: u64,
    bytes: usize,
    line_layout: CacheLineLayout,
) -> bool {
    guest_memory_chunks_for_each(address, bytes, line_layout, |chunk| {
        prevalidate_store_guest_memory_chunk(store, chunk, line_layout)
    })
}

fn prevalidate_store_guest_memory_chunk(
    store: &mut PartitionedMemoryStore,
    chunk: GuestMemoryChunk,
    line_layout: CacheLineLayout,
) -> bool {
    let Some(request) = guest_memory_read_request(chunk.address, chunk.bytes, line_layout) else {
        return false;
    };
    let Ok(outcome) = store.respond(&request) else {
        return false;
    };
    outcome
        .response()
        .and_then(|response| response.data())
        .is_some_and(|data| data.len() == chunk.bytes)
}

fn prevalidate_dram_guest_memory(
    memory: &DramMemoryController,
    chunks: &[GuestMemoryChunk],
    line_layout: CacheLineLayout,
) -> bool {
    chunks
        .iter()
        .copied()
        .all(|chunk| prevalidate_dram_guest_memory_chunk(memory, chunk, line_layout))
}

fn prevalidate_dram_guest_memory_range(
    memory: &DramMemoryController,
    address: u64,
    bytes: usize,
    line_layout: CacheLineLayout,
) -> bool {
    guest_memory_chunks_for_each(address, bytes, line_layout, |chunk| {
        prevalidate_dram_guest_memory_chunk(memory, chunk, line_layout)
    })
}

fn prevalidate_dram_guest_memory_chunk(
    memory: &DramMemoryController,
    chunk: GuestMemoryChunk,
    line_layout: CacheLineLayout,
) -> bool {
    let Some(request) = guest_memory_read_request(chunk.address, chunk.bytes, line_layout) else {
        return false;
    };
    memory
        .response_data(&request)
        .is_ok_and(|data| data.len() == chunk.bytes)
}

fn guest_memory_read_request(
    address: u64,
    bytes: usize,
    line_layout: CacheLineLayout,
) -> Option<MemoryRequest> {
    let size = AccessSize::new(u64::try_from(bytes).ok()?).ok()?;
    MemoryRequest::read_shared(
        MemoryRequestId::new(CLI_GUEST_MEMORY_AGENT, 0),
        Address::new(address),
        size,
        line_layout,
    )
    .ok()
}

fn guest_memory_write_request(
    address: u64,
    bytes: &[u8],
    line_layout: CacheLineLayout,
) -> Option<MemoryRequest> {
    let size = AccessSize::new(u64::try_from(bytes.len()).ok()?).ok()?;
    MemoryRequest::write(
        MemoryRequestId::new(CLI_GUEST_MEMORY_AGENT, 0),
        Address::new(address),
        size,
        bytes.to_vec(),
        ByteMask::full(size).ok()?,
        line_layout,
    )
    .ok()
}

fn guest_memory_write_requests(
    bytes: &[u8],
    chunks: &[GuestMemoryChunk],
    line_layout: CacheLineLayout,
) -> Option<Vec<MemoryRequest>> {
    chunks
        .iter()
        .map(|chunk| {
            let start = chunk.data_offset;
            let end = start.checked_add(chunk.bytes)?;
            guest_memory_write_request(chunk.address, &bytes[start..end], line_layout)
        })
        .collect()
}

fn guest_memory_chunks(
    address: u64,
    bytes: usize,
    line_layout: CacheLineLayout,
) -> Option<Vec<GuestMemoryChunk>> {
    let mut chunks = Vec::new();
    if !guest_memory_chunks_for_each(address, bytes, line_layout, |chunk| {
        chunks.push(chunk);
        true
    }) {
        return None;
    }
    Some(chunks)
}

fn guest_memory_chunks_for_each(
    address: u64,
    bytes: usize,
    line_layout: CacheLineLayout,
    mut handle: impl FnMut(GuestMemoryChunk) -> bool,
) -> bool {
    let mut cursor = address;
    let mut data_offset = 0usize;
    while data_offset < bytes {
        let line_offset = line_layout.line_offset(Address::new(cursor));
        let Some(available) = line_layout
            .bytes()
            .checked_sub(line_offset)
            .and_then(|bytes| usize::try_from(bytes).ok())
        else {
            return false;
        };
        if available == 0 {
            return false;
        }
        let chunk_bytes = available.min(bytes - data_offset);
        if !handle(GuestMemoryChunk {
            address: cursor,
            data_offset,
            bytes: chunk_bytes,
        }) {
            return false;
        }
        let Some(next_cursor) = u64::try_from(chunk_bytes)
            .ok()
            .and_then(|chunk_bytes| cursor.checked_add(chunk_bytes))
        else {
            return false;
        };
        cursor = next_cursor;
        data_offset += chunk_bytes;
    }
    true
}

fn write_startup_stack_to_store(
    store: &mut PartitionedMemoryStore,
    startup: &RiscvSeStartupImage,
    line_layout: CacheLineLayout,
) -> Result<(), Rem6CliError> {
    let mut cursor = startup.stack_range().start().get();
    let mut data_offset = 0usize;
    while data_offset < startup.stack_data().len() {
        let (line, line_offset, bytes, next_offset) =
            startup_stack_chunk(startup.stack_data(), line_layout, cursor, data_offset);
        let mut line_data = store
            .line_data(CLI_MEMORY_TARGET, line)
            .unwrap_or_else(|_| vec![0; line_layout.bytes() as usize]);
        let start = line_offset as usize;
        line_data[start..start + bytes as usize]
            .copy_from_slice(&startup.stack_data()[data_offset..next_offset]);
        store
            .insert_line(CLI_MEMORY_TARGET, line, line_data)
            .map_err(execute_error)?;
        cursor += bytes;
        data_offset = next_offset;
    }
    Ok(())
}

fn write_startup_stack_to_dram(
    memory: &mut DramMemoryController,
    startup: &RiscvSeStartupImage,
    line_layout: CacheLineLayout,
) -> Result<(), Rem6CliError> {
    let mut cursor = startup.stack_range().start().get();
    let mut data_offset = 0usize;
    while data_offset < startup.stack_data().len() {
        let (line, line_offset, bytes, next_offset) =
            startup_stack_chunk(startup.stack_data(), line_layout, cursor, data_offset);
        let mut line_data = memory
            .line_data(CLI_MEMORY_TARGET, line)
            .unwrap_or_else(|_| vec![0; line_layout.bytes() as usize]);
        let start = line_offset as usize;
        line_data[start..start + bytes as usize]
            .copy_from_slice(&startup.stack_data()[data_offset..next_offset]);
        memory
            .insert_line(CLI_MEMORY_TARGET, line, line_data)
            .map_err(execute_error)?;
        cursor += bytes;
        data_offset = next_offset;
    }
    Ok(())
}

fn startup_stack_chunk(
    data: &[u8],
    line_layout: CacheLineLayout,
    cursor: u64,
    data_offset: usize,
) -> (Address, u64, u64, usize) {
    let address = Address::new(cursor);
    let line = line_layout.line_address(address);
    let line_offset = line_layout.line_offset(address);
    let available_in_line = line_layout.bytes() - line_offset;
    let remaining = (data.len() - data_offset) as u64;
    let bytes = available_in_line.min(remaining);
    let next_data_offset = data_offset + bytes as usize;
    (line, line_offset, bytes, next_data_offset)
}

pub(super) fn cli_memory_response(
    memory: &CliMemoryRuntime,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    match memory {
        CliMemoryRuntime::Store { store, .. } => {
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
        CliMemoryRuntime::Dram { memory, .. } => {
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
    fn from_target_activities<I>(activities: I) -> Self
    where
        I: IntoIterator<Item = DramTargetActivity>,
    {
        let activities = activities.into_iter().collect::<Vec<_>>();
        let profile = DramMemoryActivityProfile::from_target_activities(activities.iter());
        let mut summary = Self::from_profile(profile);
        summary.targets = activities
            .iter()
            .map(Rem6DramTargetSummary::from_activity)
            .collect();
        summary
    }

    fn from_profile(profile: DramMemoryActivityProfile) -> Self {
        let nvm_profile = profile.profile_technology_label() == Some("nvm");
        let geometry = profile.profile_geometry();
        let timing = profile.profile_timing();
        let low_power_timing = profile.profile_low_power_timing();
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
            refreshes: profile.refresh_count() as u64,
            refresh_ticks: profile.refresh_cycle_count(),
            commands: profile.command_count() as u64,
            turnarounds: profile.turnaround_count() as u64,
            total_ready_latency_ticks: profile.total_ready_latency_cycles(),
            max_ready_latency_ticks: profile.max_ready_latency_cycles(),
            profiled_targets: profile.profiled_target_count() as u64,
            profile_technology: profile.profile_technology_label(),
            profile_parallel_port_label: profile.profile_parallel_port_label(),
            profile_topology_unit_label: profile.profile_topology_unit_label(),
            profile_geometry_bank_count: geometry
                .map(|geometry| u64::from(geometry.bank_count()))
                .unwrap_or(0),
            profile_geometry_row_size: geometry.map(|geometry| geometry.row_size()).unwrap_or(0),
            profile_geometry_line_size: geometry.map(|geometry| geometry.line_size()).unwrap_or(0),
            profile_geometry_lines_per_row: geometry
                .map(|geometry| geometry.lines_per_row())
                .unwrap_or(0),
            profile_geometry_bank_group_count: geometry
                .and_then(|geometry| geometry.bank_group_count())
                .map(u64::from)
                .unwrap_or(0),
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
            profile_timing_refresh_interval: timing
                .and_then(|timing| timing.refresh_timing())
                .map(|refresh| refresh.interval())
                .unwrap_or(0),
            profile_timing_refresh_recovery: timing
                .and_then(|timing| timing.refresh_timing())
                .map(|refresh| refresh.recovery())
                .unwrap_or(0),
            profile_timing_command_window_cycles: timing
                .and_then(|timing| timing.command_window())
                .map(|window| window.window_cycles())
                .unwrap_or(0),
            profile_timing_command_window_max_commands: timing
                .and_then(|timing| timing.command_window())
                .map(|window| u64::from(window.max_commands()))
                .unwrap_or(0),
            profile_low_power_precharge_powerdown_entry_delay: low_power_timing
                .map(|timing| timing.precharge_powerdown_entry_delay())
                .unwrap_or(0),
            profile_low_power_self_refresh_entry_delay: low_power_timing
                .map(|timing| timing.self_refresh_entry_delay())
                .unwrap_or(0),
            profile_low_power_exit_latency: low_power_timing
                .map(|timing| timing.exit_latency())
                .unwrap_or(0),
            profile_low_power_self_refresh_exit_latency: low_power_timing
                .map(|timing| timing.self_refresh_exit_latency())
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
            targets: Vec::new(),
        }
    }
}

impl Rem6DramTargetSummary {
    fn from_activity(activity: &DramTargetActivity) -> Self {
        let profile = activity.profile();
        let mut ports = activity
            .port_activities()
            .iter()
            .map(|(port, activity)| (*port, Rem6DramPortSummary::from_activity(*port, *activity)))
            .collect::<BTreeMap<_, _>>();
        for ((port, bank), activity) in activity.bank_activities() {
            ports
                .entry(*port)
                .or_insert_with(|| {
                    Rem6DramPortSummary::from_activity(*port, DramPortActivity::default())
                })
                .banks
                .push(Rem6DramBankSummary::from_activity(*bank, activity));
        }
        Self::from_profile(
            activity.target().get(),
            &profile,
            ports.into_values().collect(),
        )
    }

    fn from_profile(
        target: u32,
        profile: &rem6_dram::DramActivityProfile,
        ports: Vec<Rem6DramPortSummary>,
    ) -> Self {
        Self {
            target,
            active_ports: profile.active_port_count() as u64,
            active_banks: profile.active_bank_count() as u64,
            accesses: profile.access_count() as u64,
            reads: profile.read_count() as u64,
            writes: profile.write_count() as u64,
            row_hits: profile.row_hit_count() as u64,
            row_misses: profile.row_miss_count() as u64,
            refreshes: profile.refresh_count() as u64,
            refresh_ticks: profile.refresh_cycle_count(),
            commands: profile.command_count() as u64,
            turnarounds: profile.turnaround_count() as u64,
            total_ready_latency_ticks: profile.total_ready_latency_cycles(),
            max_ready_latency_ticks: profile.max_ready_latency_cycles(),
            ports,
        }
    }
}

impl Rem6DramPortSummary {
    fn from_activity(port: u32, activity: DramPortActivity) -> Self {
        Self {
            port,
            accesses: activity.access_count() as u64,
            reads: activity.read_count() as u64,
            writes: activity.write_count() as u64,
            turnarounds: activity.turnaround_count() as u64,
            commands: activity.command_count() as u64,
            banks: Vec::new(),
        }
    }
}

impl Rem6DramBankSummary {
    fn from_activity(bank: u32, activity: &DramBankActivity) -> Self {
        Self {
            bank,
            accesses: activity.access_count() as u64,
            read_bytes: activity.read_byte_count(),
            write_bytes: activity.write_byte_count(),
            row_hits: activity.row_hit_count() as u64,
            row_misses: activity.row_miss_count() as u64,
            refreshes: activity.refresh_count() as u64,
            refresh_ticks: activity.refresh_cycle_count(),
            commands: activity.command_count() as u64,
            total_ready_latency_ticks: activity.total_ready_latency_cycles(),
            max_ready_latency_ticks: activity.max_ready_latency_cycles(),
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
        CliMemoryRuntime::Store { store, .. } => {
            read_memory_dump_from_store(store, line_layout, sequence, dump)
        }
        CliMemoryRuntime::Dram { memory, .. } => {
            read_memory_dump_from_dram(memory, line_layout, dump)
        }
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
