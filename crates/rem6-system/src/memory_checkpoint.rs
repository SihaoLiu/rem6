use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_dram::{
    DramAccessKind, DramBankState, DramControllerSnapshot, DramGeometry, DramMemoryController,
    DramMemoryError, DramMemorySnapshot, DramMemoryTargetSnapshot, DramTiming,
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
        let payload = registry
            .chunk(&self.component, STORE_CHUNK)
            .ok_or_else(|| MemoryStoreCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: STORE_CHUNK.to_string(),
            })?;
        let snapshot = decode_store(&self.component, payload)?;
        self.store
            .lock()
            .expect("partitioned memory lock")
            .restore(&snapshot)
            .map_err(|error| MemoryStoreCheckpointError::Memory {
                component: self.component.clone(),
                error,
            })?;
        Ok(MemoryStoreCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
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
        self.ports
            .values()
            .map(|port| port.restore_from(registry))
            .collect()
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
        let payload = registry.chunk(&self.component, DRAM_CHUNK).ok_or_else(|| {
            DramMemoryCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: DRAM_CHUNK.to_string(),
            }
        })?;
        let snapshot = decode_dram_memory(&self.component, payload)?;
        self.controller
            .lock()
            .expect("DRAM memory lock")
            .restore(&snapshot)
            .map_err(|error| DramMemoryCheckpointError::DramMemory {
                component: self.component.clone(),
                error,
            })?;
        Ok(DramMemoryCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
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
        self.ports
            .values()
            .map(|port| port.restore_from(registry))
            .collect()
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
        write_u64(&mut payload, timing.activate_latency());
        write_u64(&mut payload, timing.read_latency());
        write_u64(&mut payload, timing.write_latency());
        write_u64(&mut payload, timing.precharge_latency());
        write_u64(&mut payload, timing.bus_turnaround());
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
        write_u64(&mut payload, controller.bus_available_cycle());
        write_u64(
            &mut payload,
            match controller.last_access_kind() {
                None => 0,
                Some(DramAccessKind::Read) => 1,
                Some(DramAccessKind::Write) => 2,
            },
        );
    }

    payload
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
        let timing = DramTiming::new(
            cursor.read_u64("DRAM activate latency")?,
            cursor.read_u64("DRAM read latency")?,
            cursor.read_u64("DRAM write latency")?,
            cursor.read_u64("DRAM precharge latency")?,
            cursor.read_u64("DRAM bus turnaround")?,
        )
        .map_err(|error| {
            cursor.invalid(format!(
                "DRAM target {} has invalid timing: {error}",
                target.get()
            ))
        })?;
        let encoded_bank_count = cursor.read_count("DRAM bank state count")?;
        if encoded_bank_count != geometry.bank_count() as usize {
            return Err(cursor.invalid(format!(
                "DRAM target {} has {} bank states for {} banks",
                target.get(),
                encoded_bank_count,
                geometry.bank_count()
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

        let bus_available_cycle = cursor.read_u64("DRAM bus available cycle")?;
        let last_access_kind = match cursor.read_u64("DRAM last access kind")? {
            0 => None,
            1 => Some(DramAccessKind::Read),
            2 => Some(DramAccessKind::Write),
            value => {
                return Err(cursor.invalid(format!(
                    "DRAM target {} has invalid last access kind {value}",
                    target.get()
                )));
            }
        };
        targets.push(DramMemoryTargetSnapshot::new(
            target,
            DramControllerSnapshot::new(
                geometry,
                timing,
                banks,
                bus_available_cycle,
                last_access_kind,
            ),
        ));
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
