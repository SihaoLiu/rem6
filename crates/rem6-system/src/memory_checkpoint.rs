use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_dram::{
    DramAccessKind, DramBankState, DramCommandWindow, DramControllerSnapshot, DramGeometry,
    DramMemoryController, DramMemoryError, DramMemorySnapshot, DramMemoryTargetSnapshot,
    DramPortState, DramTiming, ExternalMemoryProfile, ExternalMemoryTopology, NvmMediaTiming,
};
use rem6_memory::{
    AccessSize, Address, CacheLineLayout, MemoryError, MemoryLineSnapshot, MemoryPartitionSnapshot,
    MemoryTargetId, PartitionedMemorySnapshot, PartitionedMemoryStore,
};

const DRAM_CHUNK: &str = "dram";
const STORE_CHUNK: &str = "store";
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryStoreCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: PartitionedMemorySnapshot,
}

impl MemoryStoreCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: PartitionedMemorySnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &PartitionedMemorySnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct MemoryStoreCheckpointPort {
    component: CheckpointComponentId,
    store: Arc<Mutex<PartitionedMemoryStore>>,
}

impl MemoryStoreCheckpointPort {
    pub fn new(
        component: CheckpointComponentId,
        store: Arc<Mutex<PartitionedMemoryStore>>,
    ) -> Self {
        Self { component, store }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn store(&self) -> Arc<Mutex<PartitionedMemoryStore>> {
        Arc::clone(&self.store)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<MemoryStoreCheckpointRecord, CheckpointError> {
        let snapshot = self
            .store
            .lock()
            .expect("partitioned memory lock")
            .snapshot();
        registry.write_chunk(&self.component, STORE_CHUNK, encode_store(&snapshot))?;
        Ok(MemoryStoreCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<MemoryStoreCheckpointRecord, MemoryStoreCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<MemoryStoreCheckpointRecord, MemoryStoreCheckpointError> {
        let payload = registry
            .chunk(&self.component, STORE_CHUNK)
            .ok_or_else(|| MemoryStoreCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: STORE_CHUNK.to_string(),
            })?;
        let snapshot = decode_store(&self.component, payload)?;
        Ok(MemoryStoreCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn validate_snapshot(
        &self,
        snapshot: &PartitionedMemorySnapshot,
    ) -> Result<(), MemoryStoreCheckpointError> {
        PartitionedMemoryStore::from_snapshot(snapshot)
            .map(|_| ())
            .map_err(|error| MemoryStoreCheckpointError::Memory {
                component: self.component.clone(),
                error,
            })
    }

    fn restore_snapshot(
        &self,
        snapshot: &PartitionedMemorySnapshot,
    ) -> Result<(), MemoryStoreCheckpointError> {
        self.store
            .lock()
            .expect("partitioned memory lock")
            .restore(snapshot)
            .map_err(|error| MemoryStoreCheckpointError::Memory {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct MemoryStoreCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, MemoryStoreCheckpointPort>,
}

impl MemoryStoreCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = MemoryStoreCheckpointPort>,
    {
        let mut by_component = BTreeMap::new();
        for port in ports {
            let component = port.component().clone();
            if by_component.contains_key(&component) {
                return Err(CheckpointError::DuplicateComponent { component });
            }
            by_component.insert(component, port);
        }

        Ok(Self {
            ports: by_component,
        })
    }

    pub fn component_count(&self) -> usize {
        self.ports.len()
    }

    pub fn components(&self) -> Vec<CheckpointComponentId> {
        self.ports.keys().cloned().collect()
    }

    pub fn register_all(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        for port in self.ports.values() {
            port.register(registry)?;
        }
        Ok(())
    }

    pub fn capture_all_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Vec<MemoryStoreCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<MemoryStoreCheckpointRecord>, MemoryStoreCheckpointError> {
        let mut decoded = Vec::new();
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
            decoded.push((port, record));
        }

        let mut restored = Vec::new();
        for (port, record) in decoded {
            port.restore_snapshot(record.snapshot())?;
            restored.push(record);
        }
        Ok(restored)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryStoreCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Memory {
        component: CheckpointComponentId,
        error: MemoryError,
    },
}

impl MemoryStoreCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Memory { component, .. } => component,
        }
    }
}

impl fmt::Display for MemoryStoreCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "memory checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "memory checkpoint component {} has invalid store chunk: {reason}",
                component.as_str()
            ),
            Self::Memory { component, error } => write!(
                formatter,
                "memory checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for MemoryStoreCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory { error, .. } => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramMemoryCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: DramMemorySnapshot,
}

impl DramMemoryCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: DramMemorySnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &DramMemorySnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct DramMemoryCheckpointPort {
    component: CheckpointComponentId,
    controller: Arc<Mutex<DramMemoryController>>,
}

impl DramMemoryCheckpointPort {
    pub fn new(
        component: CheckpointComponentId,
        controller: Arc<Mutex<DramMemoryController>>,
    ) -> Self {
        Self {
            component,
            controller,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn controller(&self) -> Arc<Mutex<DramMemoryController>> {
        Arc::clone(&self.controller)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<DramMemoryCheckpointRecord, CheckpointError> {
        let snapshot = self.controller.lock().expect("DRAM memory lock").snapshot();
        registry.write_chunk(&self.component, DRAM_CHUNK, encode_dram_memory(&snapshot))?;
        Ok(DramMemoryCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<DramMemoryCheckpointRecord, DramMemoryCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<DramMemoryCheckpointRecord, DramMemoryCheckpointError> {
        let payload = registry.chunk(&self.component, DRAM_CHUNK).ok_or_else(|| {
            DramMemoryCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: DRAM_CHUNK.to_string(),
            }
        })?;
        let snapshot = decode_dram_memory(&self.component, payload)?;
        Ok(DramMemoryCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn validate_snapshot(
        &self,
        snapshot: &DramMemorySnapshot,
    ) -> Result<(), DramMemoryCheckpointError> {
        DramMemoryController::from_snapshot(snapshot)
            .map(|_| ())
            .map_err(|error| DramMemoryCheckpointError::DramMemory {
                component: self.component.clone(),
                error,
            })
    }

    fn restore_snapshot(
        &self,
        snapshot: &DramMemorySnapshot,
    ) -> Result<(), DramMemoryCheckpointError> {
        self.controller
            .lock()
            .expect("DRAM memory lock")
            .restore(snapshot)
            .map_err(|error| DramMemoryCheckpointError::DramMemory {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct DramMemoryCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, DramMemoryCheckpointPort>,
}

impl DramMemoryCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = DramMemoryCheckpointPort>,
    {
        let mut by_component = BTreeMap::new();
        for port in ports {
            let component = port.component().clone();
            if by_component.contains_key(&component) {
                return Err(CheckpointError::DuplicateComponent { component });
            }
            by_component.insert(component, port);
        }

        Ok(Self {
            ports: by_component,
        })
    }

    pub fn component_count(&self) -> usize {
        self.ports.len()
    }

    pub fn components(&self) -> Vec<CheckpointComponentId> {
        self.ports.keys().cloned().collect()
    }

    pub fn register_all(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        for port in self.ports.values() {
            port.register(registry)?;
        }
        Ok(())
    }

    pub fn capture_all_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Vec<DramMemoryCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<DramMemoryCheckpointRecord>, DramMemoryCheckpointError> {
        let mut decoded = Vec::new();
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
            decoded.push((port, record));
        }

        let mut restored = Vec::new();
        for (port, record) in decoded {
            port.restore_snapshot(record.snapshot())?;
            restored.push(record);
        }
        Ok(restored)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DramMemoryCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    DramMemory {
        component: CheckpointComponentId,
        error: DramMemoryError,
    },
}

impl DramMemoryCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::DramMemory { component, .. } => component,
        }
    }
}

impl fmt::Display for DramMemoryCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "DRAM checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "DRAM checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::DramMemory { component, error } => write!(
                formatter,
                "DRAM checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for DramMemoryCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DramMemory { error, .. } => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}

fn encode_store(snapshot: &PartitionedMemorySnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64(&mut payload, snapshot.partitions().len() as u64);
    for partition in snapshot.partitions() {
        write_u32(&mut payload, partition.target().get());
        write_u64(&mut payload, partition.layout().bytes());
        write_u64(&mut payload, partition.lines().len() as u64);
        for line in partition.lines() {
            write_u64(&mut payload, line.line().get());
            write_u64(&mut payload, line.data().len() as u64);
            payload.extend_from_slice(line.data());
        }
    }

    write_u64(&mut payload, snapshot.regions().len() as u64);
    for (target, range) in snapshot.regions() {
        write_u32(&mut payload, target.get());
        write_u64(&mut payload, range.start().get());
        write_u64(&mut payload, range.size().bytes());
    }
    payload
}

fn encode_dram_memory(snapshot: &DramMemorySnapshot) -> Vec<u8> {
    let store_payload = encode_store(snapshot.store());
    let mut payload = Vec::new();
    write_u64(&mut payload, store_payload.len() as u64);
    payload.extend_from_slice(&store_payload);
    write_u64(&mut payload, snapshot.targets().len() as u64);

    for target in snapshot.targets() {
        let controller = target.controller();
        let geometry = controller.geometry();
        let timing = controller.timing();
        write_u32(&mut payload, target.target().get());
        write_u32(&mut payload, geometry.bank_count());
        write_u64(&mut payload, geometry.row_size());
        write_u64(&mut payload, geometry.line_size());
        write_optional_u32(&mut payload, geometry.bank_group_count());
        write_u64(&mut payload, timing.activate_latency());
        write_u64(&mut payload, timing.read_latency());
        write_u64(&mut payload, timing.write_latency());
        write_u64(&mut payload, timing.precharge_latency());
        write_u64(&mut payload, timing.bus_turnaround());
        write_u64(&mut payload, timing.burst_spacing());
        write_optional_u64(&mut payload, timing.same_bank_group_burst_spacing());
        write_command_window(&mut payload, timing.command_window());
        write_profile(&mut payload, target.profile());
        write_nvm_media_state(
            &mut payload,
            controller.nvm_media_timing(),
            controller.nvm_pending_read_completions(),
            controller.nvm_pending_write_completions(),
        );
        write_u64(&mut payload, controller.parallel_port_count() as u64);
        write_u64(&mut payload, controller.banks().len() as u64);
        for bank in controller.banks() {
            match bank.open_row() {
                Some(row) => {
                    write_u64(&mut payload, 1);
                    write_u64(&mut payload, row);
                }
                None => write_u64(&mut payload, 0),
            }
            write_u64(&mut payload, bank.available_cycle());
        }
        for port in controller.ports() {
            write_u64(&mut payload, port.bus_available_cycle());
            write_access_kind(&mut payload, port.last_access_kind());
            write_u64(&mut payload, port.command_window_starts().len() as u64);
            for window_start in port.command_window_starts() {
                write_u64(&mut payload, *window_start);
            }
            write_optional_u64(&mut payload, port.last_data_command_cycle());
            write_optional_u32(&mut payload, port.last_bank_group());
        }
    }

    payload
}

fn write_optional_u64(payload: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            write_u64(payload, 1);
            write_u64(payload, value);
        }
        None => write_u64(payload, 0),
    }
}

fn write_optional_u32(payload: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            write_u64(payload, 1);
            write_u32(payload, value);
        }
        None => write_u64(payload, 0),
    }
}

fn write_command_window(payload: &mut Vec<u8>, command_window: Option<DramCommandWindow>) {
    let Some(command_window) = command_window else {
        write_u64(payload, 0);
        return;
    };

    write_u64(payload, 1);
    write_u64(payload, command_window.window_cycles());
    write_u32(payload, command_window.max_commands());
}

fn write_profile(payload: &mut Vec<u8>, profile: Option<&ExternalMemoryProfile>) {
    let Some(profile) = profile else {
        write_u64(payload, 0);
        return;
    };

    write_u64(payload, 1);
    match profile.topology() {
        ExternalMemoryTopology::Ddr {
            channels,
            ranks_per_channel,
        } => {
            write_u64(payload, 1);
            write_u32(payload, channels);
            write_u32(payload, ranks_per_channel);
        }
        ExternalMemoryTopology::Hbm {
            stacks,
            pseudo_channels_per_stack,
        } => {
            write_u64(payload, 2);
            write_u32(payload, stacks);
            write_u32(payload, pseudo_channels_per_stack);
        }
        ExternalMemoryTopology::Lpddr {
            channels,
            dies_per_channel,
        } => {
            write_u64(payload, 3);
            write_u32(payload, channels);
            write_u32(payload, dies_per_channel);
        }
        ExternalMemoryTopology::Nvm {
            controllers,
            media_banks_per_controller,
        } => {
            write_u64(payload, 4);
            write_u32(payload, controllers);
            write_u32(payload, media_banks_per_controller);
        }
    }
    write_nvm_media_timing(payload, profile.nvm_media_timing());
}

fn write_nvm_media_state(
    payload: &mut Vec<u8>,
    nvm_media_timing: Option<NvmMediaTiming>,
    pending_read_completions: &[u64],
    pending_write_completions: &[u64],
) {
    write_nvm_media_timing(payload, nvm_media_timing);
    write_u64(payload, pending_read_completions.len() as u64);
    for completion in pending_read_completions {
        write_u64(payload, *completion);
    }
    write_u64(payload, pending_write_completions.len() as u64);
    for completion in pending_write_completions {
        write_u64(payload, *completion);
    }
}

fn write_nvm_media_timing(payload: &mut Vec<u8>, nvm_media_timing: Option<NvmMediaTiming>) {
    let Some(nvm_media_timing) = nvm_media_timing else {
        write_u64(payload, 0);
        return;
    };

    write_u64(payload, 1);
    write_u64(payload, nvm_media_timing.read_media_latency());
    write_u64(payload, nvm_media_timing.write_media_latency());
    write_u64(payload, nvm_media_timing.send_latency());
    write_u32(payload, nvm_media_timing.max_pending_reads());
    write_u32(payload, nvm_media_timing.max_pending_writes());
}

fn write_access_kind(payload: &mut Vec<u8>, kind: Option<DramAccessKind>) {
    write_u64(
        payload,
        match kind {
            None => 0,
            Some(DramAccessKind::Read) => 1,
            Some(DramAccessKind::Write) => 2,
        },
    );
}

fn decode_store(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<PartitionedMemorySnapshot, MemoryStoreCheckpointError> {
    let mut cursor = PayloadCursor::new(component.clone(), payload);
    let partition_count = cursor.read_count("partition count")?;
    let mut partitions = Vec::with_capacity(partition_count);
    for _ in 0..partition_count {
        let target = MemoryTargetId::new(cursor.read_u32("partition target")?);
        let layout =
            CacheLineLayout::new(cursor.read_u64("partition line size")?).map_err(|error| {
                MemoryStoreCheckpointError::Memory {
                    component: component.clone(),
                    error,
                }
            })?;
        let line_count = cursor.read_count("line count")?;
        let mut lines = Vec::with_capacity(line_count);
        for _ in 0..line_count {
            let line = Address::new(cursor.read_u64("line address")?);
            let line_len = cursor.read_count("line byte count")?;
            let data = cursor.read_bytes("line payload", line_len)?.to_vec();
            lines.push(MemoryLineSnapshot::new(line, data));
        }
        partitions.push(MemoryPartitionSnapshot::new(
            target,
            rem6_memory::LineMemorySnapshot::new(layout, lines),
        ));
    }

    let region_count = cursor.read_count("region count")?;
    let mut regions = Vec::with_capacity(region_count);
    for _ in 0..region_count {
        let target = MemoryTargetId::new(cursor.read_u32("region target")?);
        let start = Address::new(cursor.read_u64("region start")?);
        let size = AccessSize::new(cursor.read_u64("region size")?).map_err(|error| {
            MemoryStoreCheckpointError::Memory {
                component: component.clone(),
                error,
            }
        })?;
        let range = rem6_memory::AddressRange::new(start, size).map_err(|error| {
            MemoryStoreCheckpointError::Memory {
                component: component.clone(),
                error,
            }
        })?;
        regions.push((target, range));
    }
    cursor.finish()?;
    Ok(PartitionedMemorySnapshot::new(partitions, regions))
}

fn decode_dram_memory(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<DramMemorySnapshot, DramMemoryCheckpointError> {
    let mut cursor = DramPayloadCursor::new(component.clone(), payload);
    let store_len = cursor.read_count("store byte count")?;
    let store_payload = cursor.read_bytes("store payload", store_len)?;
    let store = decode_store(component, store_payload)
        .map_err(|error| map_store_decode_error(component, error))?;
    let target_count = cursor.read_count("DRAM target count")?;
    let mut targets = Vec::with_capacity(target_count);

    for _ in 0..target_count {
        let target = MemoryTargetId::new(cursor.read_u32("DRAM target")?);
        let bank_count = cursor.read_u32("DRAM bank count")?;
        let row_size = cursor.read_u64("DRAM row size")?;
        let line_size = cursor.read_u64("DRAM line size")?;
        let geometry = DramGeometry::new(bank_count, row_size, line_size).map_err(|error| {
            cursor.invalid(format!(
                "DRAM target {} has invalid geometry: {error}",
                target.get()
            ))
        })?;
        let geometry = match read_optional_u32(&mut cursor, "DRAM bank group count")? {
            Some(bank_group_count) => {
                geometry
                    .with_bank_groups(bank_group_count)
                    .map_err(|error| {
                        cursor.invalid(format!(
                            "DRAM target {} has invalid bank group geometry: {error}",
                            target.get()
                        ))
                    })?
            }
            None => geometry,
        };
        let activate_latency = cursor.read_u64("DRAM activate latency")?;
        let read_latency = cursor.read_u64("DRAM read latency")?;
        let write_latency = cursor.read_u64("DRAM write latency")?;
        let precharge_latency = cursor.read_u64("DRAM precharge latency")?;
        let bus_turnaround = cursor.read_u64("DRAM bus turnaround")?;
        let burst_spacing = cursor.read_u64("DRAM burst spacing")?;
        let timing = DramTiming::new(
            activate_latency,
            read_latency,
            write_latency,
            precharge_latency,
            bus_turnaround,
        )
        .and_then(|timing| timing.with_burst_spacing(burst_spacing))
        .map_err(|error| {
            cursor.invalid(format!(
                "DRAM target {} has invalid timing: {error}",
                target.get()
            ))
        })?;
        let timing = match read_optional_u64(&mut cursor, "DRAM same-bank-group burst spacing")? {
            Some(burst_spacing) => timing
                .with_same_bank_group_burst_spacing(burst_spacing)
                .map_err(|error| {
                    cursor.invalid(format!(
                        "DRAM target {} has invalid same-bank-group burst spacing: {error}",
                        target.get()
                    ))
                })?,
            None => timing,
        };
        let timing = read_command_window(&mut cursor, target, timing)?;
        let line_layout = CacheLineLayout::new(line_size).map_err(|error| {
            cursor.invalid(format!(
                "DRAM target {} has invalid profile line layout: {error}",
                target.get()
            ))
        })?;
        let profile = read_profile(&mut cursor, target, line_layout, geometry, timing)?;
        let (nvm_media_timing, nvm_pending_read_completions, nvm_pending_write_completions) =
            read_nvm_media_state(&mut cursor, target)?;
        let port_count = cursor.read_count("DRAM parallel port count")?;
        if port_count == 0 {
            return Err(cursor.invalid(format!(
                "DRAM target {} has zero parallel ports",
                target.get()
            )));
        }
        let encoded_bank_count = cursor.read_count("DRAM bank state count")?;
        let expected_bank_count = geometry.bank_count() as usize * port_count;
        if encoded_bank_count != expected_bank_count {
            return Err(cursor.invalid(format!(
                "DRAM target {} has {} bank states, expected {}",
                target.get(),
                encoded_bank_count,
                expected_bank_count
            )));
        }

        let mut banks = Vec::with_capacity(encoded_bank_count);
        for bank in 0..encoded_bank_count {
            let open_row = match cursor.read_u64("DRAM bank open-row flag")? {
                0 => None,
                1 => Some(cursor.read_u64("DRAM bank open row")?),
                value => {
                    return Err(cursor.invalid(format!(
                        "DRAM target {} bank {bank} has invalid open-row flag {value}",
                        target.get()
                    )));
                }
            };
            let available_cycle = cursor.read_u64("DRAM bank available cycle")?;
            banks.push(DramBankState::from_snapshot(open_row, available_cycle));
        }

        let mut ports = Vec::with_capacity(port_count);
        for port in 0..port_count {
            let bus_available_cycle = cursor.read_u64("DRAM port bus available cycle")?;
            let last_access_kind = read_access_kind(&mut cursor, target, format!("port {port}"))?;
            let command_window_count = cursor.read_count("DRAM port command window start count")?;
            let mut command_window_starts = Vec::with_capacity(command_window_count);
            for _ in 0..command_window_count {
                command_window_starts.push(cursor.read_u64("DRAM port command window start")?);
            }
            let last_data_command_cycle =
                read_optional_u64(&mut cursor, "DRAM port last data command cycle")?;
            let last_bank_group = read_optional_u32(&mut cursor, "DRAM port last bank group")?;
            ports.push(DramPortState::from_snapshot_with_port_history(
                bus_available_cycle,
                last_access_kind,
                command_window_starts,
                last_data_command_cycle,
                last_bank_group,
            ));
        }

        let controller = DramControllerSnapshot::with_ports(geometry, timing, banks, ports)
            .with_nvm_media_state(
                nvm_media_timing,
                nvm_pending_read_completions,
                nvm_pending_write_completions,
            );
        if let Some(profile) = profile {
            targets.push(DramMemoryTargetSnapshot::with_profile(
                target, controller, profile,
            ));
        } else {
            targets.push(DramMemoryTargetSnapshot::new(target, controller));
        }
    }

    cursor.finish()?;
    let snapshot = DramMemorySnapshot::new(store, targets);
    DramMemoryController::from_snapshot(&snapshot).map_err(|error| {
        DramMemoryCheckpointError::DramMemory {
            component: component.clone(),
            error,
        }
    })?;
    Ok(snapshot)
}

fn read_profile(
    cursor: &mut DramPayloadCursor<'_>,
    target: MemoryTargetId,
    line_layout: CacheLineLayout,
    geometry: DramGeometry,
    timing: DramTiming,
) -> Result<Option<ExternalMemoryProfile>, DramMemoryCheckpointError> {
    let present = cursor.read_u64("DRAM profile presence")?;
    if present == 0 {
        return Ok(None);
    }
    if present != 1 {
        return Err(cursor.invalid(format!(
            "DRAM target {} has invalid profile presence {present}",
            target.get()
        )));
    }

    let technology = cursor.read_u64("DRAM profile technology")?;
    let first = cursor.read_u32("DRAM profile topology count")?;
    let second = cursor.read_u32("DRAM profile topology peer count")?;
    let profile = match technology {
        1 => ExternalMemoryProfile::ddr(target, line_layout, first, second, geometry, timing),
        2 => ExternalMemoryProfile::hbm(target, line_layout, first, second, geometry, timing),
        3 => ExternalMemoryProfile::lpddr(target, line_layout, first, second, geometry, timing),
        4 => ExternalMemoryProfile::nvm(target, line_layout, first, second, geometry, timing),
        value => {
            return Err(cursor.invalid(format!(
                "DRAM target {} has invalid profile technology {value}",
                target.get()
            )));
        }
    }
    .map_err(|error| {
        cursor.invalid(format!(
            "DRAM target {} has invalid profile: {error}",
            target.get()
        ))
    })?;

    let nvm_media_timing = read_nvm_media_timing(cursor, target)?;
    let profile = match nvm_media_timing {
        Some(nvm_media_timing) => profile.with_nvm_media_timing(nvm_media_timing),
        None => Ok(profile),
    }
    .map_err(|error| {
        cursor.invalid(format!(
            "DRAM target {} has invalid profile NVM media timing: {error}",
            target.get()
        ))
    })?;

    Ok(Some(profile))
}

type NvmMediaCheckpointState = (Option<NvmMediaTiming>, Vec<u64>, Vec<u64>);

fn read_nvm_media_state(
    cursor: &mut DramPayloadCursor<'_>,
    target: MemoryTargetId,
) -> Result<NvmMediaCheckpointState, DramMemoryCheckpointError> {
    let nvm_media_timing = read_nvm_media_timing(cursor, target)?;
    let pending_read_count = cursor.read_count("DRAM NVM pending read completion count")?;
    let mut pending_read_completions = Vec::with_capacity(pending_read_count);
    for _ in 0..pending_read_count {
        pending_read_completions.push(cursor.read_u64("DRAM NVM pending read completion")?);
    }
    pending_read_completions.sort_unstable();
    let pending_count = cursor.read_count("DRAM NVM pending write completion count")?;
    let mut pending_write_completions = Vec::with_capacity(pending_count);
    for _ in 0..pending_count {
        pending_write_completions.push(cursor.read_u64("DRAM NVM pending write completion")?);
    }
    pending_write_completions.sort_unstable();
    Ok((
        nvm_media_timing,
        pending_read_completions,
        pending_write_completions,
    ))
}

fn read_nvm_media_timing(
    cursor: &mut DramPayloadCursor<'_>,
    target: MemoryTargetId,
) -> Result<Option<NvmMediaTiming>, DramMemoryCheckpointError> {
    match cursor.read_u64("DRAM NVM media timing presence")? {
        0 => Ok(None),
        1 => NvmMediaTiming::new(
            cursor.read_u64("DRAM NVM read media latency")?,
            cursor.read_u64("DRAM NVM write media latency")?,
            cursor.read_u64("DRAM NVM send latency")?,
            cursor.read_u32("DRAM NVM max pending reads")?,
            cursor.read_u32("DRAM NVM max pending writes")?,
        )
        .map(Some)
        .map_err(|error| {
            cursor.invalid(format!(
                "DRAM target {} has invalid NVM media timing: {error}",
                target.get()
            ))
        }),
        value => Err(cursor.invalid(format!(
            "DRAM target {} has invalid NVM media timing presence {value}",
            target.get()
        ))),
    }
}

fn read_command_window(
    cursor: &mut DramPayloadCursor<'_>,
    target: MemoryTargetId,
    timing: DramTiming,
) -> Result<DramTiming, DramMemoryCheckpointError> {
    match cursor.read_u64("DRAM command window presence")? {
        0 => Ok(timing),
        1 => timing
            .with_command_window(
                cursor.read_u64("DRAM command window cycles")?,
                cursor.read_u32("DRAM command window max commands")?,
            )
            .map_err(|error| {
                cursor.invalid(format!(
                    "DRAM target {} has invalid command window timing: {error}",
                    target.get()
                ))
            }),
        value => Err(cursor.invalid(format!(
            "DRAM target {} has invalid command window presence {value}",
            target.get()
        ))),
    }
}

fn read_optional_u64(
    cursor: &mut DramPayloadCursor<'_>,
    context: &'static str,
) -> Result<Option<u64>, DramMemoryCheckpointError> {
    match cursor.read_u64(context)? {
        0 => Ok(None),
        1 => Ok(Some(cursor.read_u64(context)?)),
        value => Err(cursor.invalid(format!("{context} has invalid presence flag {value}"))),
    }
}

fn read_optional_u32(
    cursor: &mut DramPayloadCursor<'_>,
    context: &'static str,
) -> Result<Option<u32>, DramMemoryCheckpointError> {
    match cursor.read_u64(context)? {
        0 => Ok(None),
        1 => Ok(Some(cursor.read_u32(context)?)),
        value => Err(cursor.invalid(format!("{context} has invalid presence flag {value}"))),
    }
}

fn read_access_kind(
    cursor: &mut DramPayloadCursor<'_>,
    target: MemoryTargetId,
    context: String,
) -> Result<Option<DramAccessKind>, DramMemoryCheckpointError> {
    match cursor.read_u64("DRAM last access kind")? {
        0 => Ok(None),
        1 => Ok(Some(DramAccessKind::Read)),
        2 => Ok(Some(DramAccessKind::Write)),
        value => Err(cursor.invalid(format!(
            "DRAM target {} {context} has invalid last access kind {value}",
            target.get()
        ))),
    }
}

fn map_store_decode_error(
    component: &CheckpointComponentId,
    error: MemoryStoreCheckpointError,
) -> DramMemoryCheckpointError {
    match error {
        MemoryStoreCheckpointError::MissingChunk { name, .. } => {
            DramMemoryCheckpointError::InvalidChunk {
                component: component.clone(),
                reason: format!("store payload is missing chunk {name}"),
            }
        }
        MemoryStoreCheckpointError::InvalidChunk { reason, .. } => {
            DramMemoryCheckpointError::InvalidChunk {
                component: component.clone(),
                reason: format!("store payload {reason}"),
            }
        }
        MemoryStoreCheckpointError::Memory { error, .. } => DramMemoryCheckpointError::DramMemory {
            component: component.clone(),
            error: DramMemoryError::Memory(error),
        },
    }
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

struct PayloadCursor<'a> {
    component: CheckpointComponentId,
    payload: &'a [u8],
    offset: usize,
}

impl<'a> PayloadCursor<'a> {
    fn new(component: CheckpointComponentId, payload: &'a [u8]) -> Self {
        Self {
            component,
            payload,
            offset: 0,
        }
    }

    fn read_count(&mut self, name: &str) -> Result<usize, MemoryStoreCheckpointError> {
        self.read_u64(name)?
            .try_into()
            .map_err(|_| self.invalid(format!("{name} does not fit host usize")))
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, MemoryStoreCheckpointError> {
        let bytes = self.read_bytes(name, U32_BYTES)?;
        Ok(u32::from_le_bytes(
            bytes.try_into().expect("u32 byte count checked"),
        ))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, MemoryStoreCheckpointError> {
        let bytes = self.read_bytes(name, U64_BYTES)?;
        Ok(u64::from_le_bytes(
            bytes.try_into().expect("u64 byte count checked"),
        ))
    }

    fn read_bytes(
        &mut self,
        name: &str,
        count: usize,
    ) -> Result<&'a [u8], MemoryStoreCheckpointError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| self.invalid(format!("{name} byte count overflows")))?;
        if end > self.payload.len() {
            return Err(self.invalid(format!("{name} is truncated")));
        }
        let bytes = &self.payload[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), MemoryStoreCheckpointError> {
        if self.offset == self.payload.len() {
            return Ok(());
        }
        Err(self.invalid(format!(
            "{} trailing bytes",
            self.payload.len() - self.offset
        )))
    }

    fn invalid(&self, reason: String) -> MemoryStoreCheckpointError {
        MemoryStoreCheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}

struct DramPayloadCursor<'a> {
    component: CheckpointComponentId,
    payload: &'a [u8],
    offset: usize,
}

impl<'a> DramPayloadCursor<'a> {
    fn new(component: CheckpointComponentId, payload: &'a [u8]) -> Self {
        Self {
            component,
            payload,
            offset: 0,
        }
    }

    fn read_count(&mut self, name: &str) -> Result<usize, DramMemoryCheckpointError> {
        self.read_u64(name)?
            .try_into()
            .map_err(|_| self.invalid(format!("{name} does not fit host usize")))
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, DramMemoryCheckpointError> {
        let bytes = self.read_bytes(name, U32_BYTES)?;
        Ok(u32::from_le_bytes(
            bytes.try_into().expect("u32 byte count checked"),
        ))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, DramMemoryCheckpointError> {
        let bytes = self.read_bytes(name, U64_BYTES)?;
        Ok(u64::from_le_bytes(
            bytes.try_into().expect("u64 byte count checked"),
        ))
    }

    fn read_bytes(
        &mut self,
        name: &str,
        count: usize,
    ) -> Result<&'a [u8], DramMemoryCheckpointError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| self.invalid(format!("{name} byte count overflows")))?;
        if end > self.payload.len() {
            return Err(self.invalid(format!("{name} is truncated")));
        }
        let bytes = &self.payload[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), DramMemoryCheckpointError> {
        if self.offset == self.payload.len() {
            return Ok(());
        }
        Err(self.invalid(format!(
            "{} trailing bytes",
            self.payload.len() - self.offset
        )))
    }

    fn invalid(&self, reason: String) -> DramMemoryCheckpointError {
        DramMemoryCheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}
