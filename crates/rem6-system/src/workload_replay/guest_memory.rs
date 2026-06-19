use std::sync::{Arc, Mutex};

use rem6_memory::{AccessSize, Address, AddressRange, CacheLineLayout, MemoryTargetId};
use rem6_workload::{WorkloadMemoryTarget, WorkloadTopology};

use super::data_cache_backend::WorkloadDataCacheBackend;
use super::memory_backend::WorkloadMemoryBackend;
use super::RiscvWorkloadReplayError;
use crate::riscv_syscall::RiscvGuestMemoryWriter;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkloadGuestMemoryReadTarget {
    target: MemoryTargetId,
    range: AddressRange,
    layout: CacheLineLayout,
}

impl WorkloadGuestMemoryReadTarget {
    fn from_target(target: WorkloadMemoryTarget) -> Result<Self, RiscvWorkloadReplayError> {
        Ok(Self {
            target: MemoryTargetId::new(target.target()),
            range: target.range(),
            layout: CacheLineLayout::new(target.line_bytes())
                .map_err(RiscvWorkloadReplayError::Memory)?,
        })
    }
}

enum WorkloadGuestMemoryWritePatch {
    Memory {
        target: MemoryTargetId,
        line: Address,
        data: Vec<u8>,
    },
    DataCache {
        target: MemoryTargetId,
        line: Address,
        data: Vec<u8>,
    },
}

pub(super) fn workload_functional_guest_memory_reader(
    memory: WorkloadMemoryBackend,
    data_cache: Option<Arc<Mutex<WorkloadDataCacheBackend>>>,
    topology: &WorkloadTopology,
) -> Result<impl Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static, RiscvWorkloadReplayError>
{
    let targets = topology
        .memory_targets()
        .iter()
        .copied()
        .map(WorkloadGuestMemoryReadTarget::from_target)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(move |address, bytes| {
        read_workload_guest_memory(&memory, data_cache.as_ref(), &targets, address, bytes)
    })
}

pub(super) fn workload_functional_guest_memory_writer(
    memory: WorkloadMemoryBackend,
    data_cache: Option<Arc<Mutex<WorkloadDataCacheBackend>>>,
    topology: &WorkloadTopology,
) -> Result<RiscvGuestMemoryWriter, RiscvWorkloadReplayError> {
    let targets = topology
        .memory_targets()
        .iter()
        .copied()
        .map(WorkloadGuestMemoryReadTarget::from_target)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RiscvGuestMemoryWriter::new(move |address, bytes| {
        write_workload_guest_memory(&memory, data_cache.as_ref(), &targets, address, bytes)
            .is_some()
    }))
}

fn read_workload_guest_memory(
    memory: &WorkloadMemoryBackend,
    data_cache: Option<&Arc<Mutex<WorkloadDataCacheBackend>>>,
    targets: &[WorkloadGuestMemoryReadTarget],
    address: u64,
    bytes: usize,
) -> Option<Vec<u8>> {
    if bytes == 0 {
        return Some(Vec::new());
    }
    let size = AccessSize::new(u64::try_from(bytes).ok()?).ok()?;
    let range = AddressRange::new(Address::new(address), size).ok()?;
    let target = targets
        .iter()
        .find(|target| target.range.contains_range(range))?;
    let mut cursor = address;
    let mut remaining = bytes;
    let mut output = Vec::with_capacity(bytes);
    while remaining != 0 {
        let address = Address::new(cursor);
        let line = target.layout.line_address(address);
        let offset = usize::try_from(target.layout.line_offset(address)).ok()?;
        let line_bytes = usize::try_from(target.layout.bytes()).ok()?;
        let take = (line_bytes - offset).min(remaining);
        let line_data = read_workload_guest_memory_line(memory, data_cache, target.target, line)?;
        let end = offset.checked_add(take)?;
        if line_data.len() < end {
            return None;
        }
        output.extend_from_slice(&line_data[offset..end]);
        cursor = cursor.checked_add(u64::try_from(take).ok()?)?;
        remaining -= take;
    }
    Some(output)
}

fn read_workload_guest_memory_line(
    memory: &WorkloadMemoryBackend,
    data_cache: Option<&Arc<Mutex<WorkloadDataCacheBackend>>>,
    target: MemoryTargetId,
    line: Address,
) -> Option<Vec<u8>> {
    if let Some(data_cache) = data_cache {
        let data_cache = data_cache.lock().expect("workload data cache lock");
        if let Some(line_data) = data_cache.functional_line_data(target, line) {
            return Some(line_data);
        }
    }
    memory.line_data(target, line).ok()
}

fn write_workload_guest_memory(
    memory: &WorkloadMemoryBackend,
    data_cache: Option<&Arc<Mutex<WorkloadDataCacheBackend>>>,
    targets: &[WorkloadGuestMemoryReadTarget],
    address: u64,
    bytes: &[u8],
) -> Option<()> {
    if bytes.is_empty() {
        readable_workload_guest_memory_target(targets, address, 1)?;
        return Some(());
    }
    let target = readable_workload_guest_memory_target(targets, address, bytes.len())?;
    let mut cursor = address;
    let mut remaining = bytes.len();
    let mut offset = 0usize;
    let mut patches = Vec::new();
    while remaining != 0 {
        let address = Address::new(cursor);
        let line = target.layout.line_address(address);
        let line_offset = usize::try_from(target.layout.line_offset(address)).ok()?;
        let line_bytes = usize::try_from(target.layout.bytes()).ok()?;
        let take = (line_bytes - line_offset).min(remaining);
        let (mut line_data, cache_resident) =
            write_workload_guest_memory_line(memory, data_cache, target.target, line)?;
        let end = line_offset.checked_add(take)?;
        if line_data.len() < end {
            return None;
        }
        line_data[line_offset..end].copy_from_slice(&bytes[offset..offset + take]);
        patches.push(if cache_resident {
            WorkloadGuestMemoryWritePatch::DataCache {
                target: target.target,
                line,
                data: line_data,
            }
        } else {
            WorkloadGuestMemoryWritePatch::Memory {
                target: target.target,
                line,
                data: line_data,
            }
        });
        cursor = cursor.checked_add(u64::try_from(take).ok()?)?;
        offset += take;
        remaining -= take;
    }
    for patch in patches {
        match patch {
            WorkloadGuestMemoryWritePatch::Memory { target, line, data } => {
                memory.insert_line(target, line, data).ok()?;
            }
            WorkloadGuestMemoryWritePatch::DataCache { target, line, data } => {
                data_cache?
                    .lock()
                    .expect("workload data cache lock")
                    .functional_replace_line_data(target, line, data)
                    .ok()?
                    .then_some(())?;
            }
        }
    }
    Some(())
}

fn write_workload_guest_memory_line(
    memory: &WorkloadMemoryBackend,
    data_cache: Option<&Arc<Mutex<WorkloadDataCacheBackend>>>,
    target: MemoryTargetId,
    line: Address,
) -> Option<(Vec<u8>, bool)> {
    if let Some(data_cache) = data_cache {
        let data_cache = data_cache.lock().expect("workload data cache lock");
        if let Some(line_data) = data_cache.functional_line_data(target, line) {
            return Some((line_data, true));
        }
    }
    Some((memory.line_data(target, line).ok()?, false))
}

fn readable_workload_guest_memory_target(
    targets: &[WorkloadGuestMemoryReadTarget],
    address: u64,
    bytes: usize,
) -> Option<WorkloadGuestMemoryReadTarget> {
    let size = AccessSize::new(u64::try_from(bytes).ok()?).ok()?;
    let range = AddressRange::new(Address::new(address), size).ok()?;
    targets
        .iter()
        .copied()
        .find(|target| target.range.contains_range(range))
}
