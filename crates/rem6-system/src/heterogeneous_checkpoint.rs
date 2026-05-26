use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_accelerator::{
    AcceleratorCommand, AcceleratorCommandId, AcceleratorCommandKind, AcceleratorCompletion,
    AcceleratorDmaCompletion, AcceleratorDmaCopy, AcceleratorEngine, AcceleratorEngineSnapshot,
    AcceleratorPendingDmaWrite, AcceleratorQueuedCommandSnapshot, AcceleratorTraceEvent,
    AcceleratorTraceKind,
};
use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_gpu::{
    GpuDevice, GpuDeviceSnapshot, GpuDmaCompletion, GpuDmaCopy, GpuDmaId, GpuKernelId,
    GpuPendingDmaWrite, GpuQueuedWorkgroupSnapshot, GpuSlotSnapshot, GpuTraceEvent, GpuTraceKind,
    GpuWorkgroupCompletion, GpuWorkgroupId,
};
use rem6_kernel::{PartitionId, Tick};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryAccessOrdering,
    MemoryBarrierSet, MemoryOperation, MemoryRequest, MemoryRequestId,
};
use rem6_transport::MemoryRouteId;

const ACCELERATOR_CHUNK: &str = "accelerator";
const GPU_CHUNK: &str = "gpu";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: AcceleratorEngineSnapshot,
}

impl AcceleratorCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: AcceleratorEngineSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &AcceleratorEngineSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct AcceleratorCheckpointPort {
    component: CheckpointComponentId,
    engine: AcceleratorEngine,
}

impl AcceleratorCheckpointPort {
    pub const fn new(component: CheckpointComponentId, engine: AcceleratorEngine) -> Self {
        Self { component, engine }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn engine(&self) -> AcceleratorEngine {
        self.engine.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<AcceleratorCheckpointRecord, CheckpointError> {
        let snapshot = self.engine.snapshot();
        registry.write_chunk(
            &self.component,
            ACCELERATOR_CHUNK,
            encode_accelerator_snapshot(&snapshot),
        )?;
        Ok(AcceleratorCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<AcceleratorCheckpointRecord, AcceleratorCheckpointError> {
        let record = self.decode_from(registry)?;
        self.engine.restore(record.snapshot());
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<AcceleratorCheckpointRecord, AcceleratorCheckpointError> {
        let payload = registry
            .chunk(&self.component, ACCELERATOR_CHUNK)
            .ok_or_else(|| AcceleratorCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: ACCELERATOR_CHUNK.to_string(),
            })?;
        let snapshot = decode_accelerator_snapshot(&self.component, payload)?;
        Ok(AcceleratorCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }
}

#[derive(Clone, Debug, Default)]
pub struct AcceleratorCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, AcceleratorCheckpointPort>,
}

impl AcceleratorCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = AcceleratorCheckpointPort>,
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

    pub fn register_all(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        for port in self.ports.values() {
            port.register(registry)?;
        }
        Ok(())
    }

    pub fn capture_all_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Vec<AcceleratorCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<AcceleratorCheckpointRecord>, AcceleratorCheckpointError> {
        let records = self
            .ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in self.ports.values().zip(&records) {
            port.engine.restore(record.snapshot());
        }
        Ok(records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceleratorCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
}

impl fmt::Display for AcceleratorCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "accelerator checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "accelerator checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
        }
    }
}

impl Error for AcceleratorCheckpointError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: GpuDeviceSnapshot,
}

impl GpuCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: GpuDeviceSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &GpuDeviceSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct GpuCheckpointPort {
    component: CheckpointComponentId,
    gpu: GpuDevice,
}

impl GpuCheckpointPort {
    pub const fn new(component: CheckpointComponentId, gpu: GpuDevice) -> Self {
        Self { component, gpu }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn gpu(&self) -> GpuDevice {
        self.gpu.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<GpuCheckpointRecord, CheckpointError> {
        let snapshot = self.gpu.snapshot();
        registry.write_chunk(&self.component, GPU_CHUNK, encode_gpu_snapshot(&snapshot))?;
        Ok(GpuCheckpointRecord::new(self.component.clone(), snapshot))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<GpuCheckpointRecord, GpuCheckpointError> {
        let record = self.decode_from(registry)?;
        self.gpu.restore(record.snapshot());
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<GpuCheckpointRecord, GpuCheckpointError> {
        let payload = registry.chunk(&self.component, GPU_CHUNK).ok_or_else(|| {
            GpuCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: GPU_CHUNK.to_string(),
            }
        })?;
        let snapshot = decode_gpu_snapshot(&self.component, payload)?;
        Ok(GpuCheckpointRecord::new(self.component.clone(), snapshot))
    }
}

#[derive(Clone, Debug, Default)]
pub struct GpuCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, GpuCheckpointPort>,
}

impl GpuCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = GpuCheckpointPort>,
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

    pub fn register_all(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        for port in self.ports.values() {
            port.register(registry)?;
        }
        Ok(())
    }

    pub fn capture_all_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Vec<GpuCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<GpuCheckpointRecord>, GpuCheckpointError> {
        let records = self
            .ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in self.ports.values().zip(&records) {
            port.gpu.restore(record.snapshot());
        }
        Ok(records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
}

impl fmt::Display for GpuCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "GPU checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "GPU checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
        }
    }
}

impl Error for GpuCheckpointError {}

fn encode_accelerator_snapshot(snapshot: &AcceleratorEngineSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64_vec(&mut payload, snapshot.lane_busy_until());
    write_accelerator_queued_vec(&mut payload, snapshot.queued_commands());
    write_accelerator_trace_vec(&mut payload, snapshot.trace());
    write_accelerator_completion_vec(&mut payload, snapshot.completed());
    write_accelerator_pending_vec(&mut payload, snapshot.pending_dma_writes());
    write_accelerator_dma_completion_vec(&mut payload, snapshot.dma_completions());
    payload
}

fn decode_accelerator_snapshot(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<AcceleratorEngineSnapshot, AcceleratorCheckpointError> {
    let mut reader = ChunkReader::new(payload);
    let lanes =
        reader
            .read_u64_vec()
            .map_err(|reason| AcceleratorCheckpointError::InvalidChunk {
                component: component.clone(),
                reason,
            })?;
    let queued_commands = read_accelerator_queued_vec(&mut reader).map_err(|reason| {
        AcceleratorCheckpointError::InvalidChunk {
            component: component.clone(),
            reason,
        }
    })?;
    let trace = read_accelerator_trace_vec(&mut reader).map_err(|reason| {
        AcceleratorCheckpointError::InvalidChunk {
            component: component.clone(),
            reason,
        }
    })?;
    let completed = read_accelerator_completion_vec(&mut reader).map_err(|reason| {
        AcceleratorCheckpointError::InvalidChunk {
            component: component.clone(),
            reason,
        }
    })?;
    let pending_dma_writes = read_accelerator_pending_vec(&mut reader).map_err(|reason| {
        AcceleratorCheckpointError::InvalidChunk {
            component: component.clone(),
            reason,
        }
    })?;
    let dma_completions = read_accelerator_dma_completion_vec(&mut reader).map_err(|reason| {
        AcceleratorCheckpointError::InvalidChunk {
            component: component.clone(),
            reason,
        }
    })?;
    reader
        .finish()
        .map_err(|reason| AcceleratorCheckpointError::InvalidChunk {
            component: component.clone(),
            reason,
        })?;
    Ok(
        AcceleratorEngineSnapshot::new(
            lanes,
            trace,
            completed,
            pending_dma_writes,
            dma_completions,
        )
        .with_queued_commands(queued_commands),
    )
}

fn encode_gpu_snapshot(snapshot: &GpuDeviceSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_gpu_slot_vec(&mut payload, snapshot.slots());
    write_gpu_trace_vec(&mut payload, snapshot.trace());
    write_gpu_completion_vec(&mut payload, snapshot.completions());
    write_gpu_pending_vec(&mut payload, snapshot.pending_dma_writes());
    write_gpu_dma_completion_vec(&mut payload, snapshot.dma_completions());
    payload
}

fn decode_gpu_snapshot(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<GpuDeviceSnapshot, GpuCheckpointError> {
    let mut reader = ChunkReader::new(payload);
    let slots =
        read_gpu_slot_vec(&mut reader).map_err(|reason| GpuCheckpointError::InvalidChunk {
            component: component.clone(),
            reason,
        })?;
    let trace =
        read_gpu_trace_vec(&mut reader).map_err(|reason| GpuCheckpointError::InvalidChunk {
            component: component.clone(),
            reason,
        })?;
    let completions = read_gpu_completion_vec(&mut reader).map_err(|reason| {
        GpuCheckpointError::InvalidChunk {
            component: component.clone(),
            reason,
        }
    })?;
    let pending_dma_writes =
        read_gpu_pending_vec(&mut reader).map_err(|reason| GpuCheckpointError::InvalidChunk {
            component: component.clone(),
            reason,
        })?;
    let dma_completions = read_gpu_dma_completion_vec(&mut reader).map_err(|reason| {
        GpuCheckpointError::InvalidChunk {
            component: component.clone(),
            reason,
        }
    })?;
    reader
        .finish()
        .map_err(|reason| GpuCheckpointError::InvalidChunk {
            component: component.clone(),
            reason,
        })?;
    Ok(GpuDeviceSnapshot::new(
        slots,
        trace,
        completions,
        pending_dma_writes,
        dma_completions,
    ))
}

fn write_accelerator_queued_vec(
    payload: &mut Vec<u8>,
    queued: &[AcceleratorQueuedCommandSnapshot],
) {
    write_count(payload, queued.len());
    for command in queued {
        write_u64(payload, command.command().id().get());
        write_accelerator_command_kind(payload, command.command().kind());
        write_u64(payload, command.command().execution_latency());
        write_u32(payload, command.lane());
        write_u64(payload, command.queued_at());
        write_u64(payload, command.started_at());
        write_u64(payload, command.completed_at());
    }
}

fn read_accelerator_queued_vec(
    reader: &mut ChunkReader<'_>,
) -> Result<Vec<AcceleratorQueuedCommandSnapshot>, String> {
    let count = reader.read_count("accelerator queued command count")?;
    let mut queued = Vec::with_capacity(count);
    for _ in 0..count {
        let command = AcceleratorCommand::new(
            AcceleratorCommandId::new(reader.read_u64("accelerator queued command")?),
            read_accelerator_command_kind(reader)?,
            reader.read_u64("accelerator queued command latency")?,
        )
        .map_err(|error| error.to_string())?;
        queued.push(AcceleratorQueuedCommandSnapshot::new(
            command,
            reader.read_u32("accelerator queued lane")?,
            reader.read_u64("accelerator queued enqueue")?,
            reader.read_u64("accelerator queued start")?,
            reader.read_u64("accelerator queued completion")?,
        ));
    }
    Ok(queued)
}

fn write_accelerator_trace_vec(payload: &mut Vec<u8>, events: &[AcceleratorTraceEvent]) {
    write_count(payload, events.len());
    for event in events {
        write_u64(payload, event.tick());
        match event.kind() {
            AcceleratorTraceKind::Submitted {
                command,
                source,
                target,
            } => {
                write_u8(payload, 0);
                write_u64(payload, command.get());
                write_u32(payload, source.index());
                write_u32(payload, target.index());
            }
            AcceleratorTraceKind::Started {
                command,
                lane,
                complete_at,
            } => {
                write_u8(payload, 1);
                write_u64(payload, command.get());
                write_u32(payload, *lane);
                write_u64(payload, *complete_at);
            }
            AcceleratorTraceKind::Completed { command, lane } => {
                write_u8(payload, 2);
                write_u64(payload, command.get());
                write_u32(payload, *lane);
            }
            AcceleratorTraceKind::DmaReadIssued { command, request } => {
                write_u8(payload, 3);
                write_u64(payload, command.get());
                write_request_id(payload, *request);
            }
            AcceleratorTraceKind::DmaReadCompleted {
                command,
                request,
                bytes,
            } => {
                write_u8(payload, 4);
                write_u64(payload, command.get());
                write_request_id(payload, *request);
                write_u64(payload, *bytes);
            }
            AcceleratorTraceKind::DmaWriteIssued { command, request } => {
                write_u8(payload, 5);
                write_u64(payload, command.get());
                write_request_id(payload, *request);
            }
            AcceleratorTraceKind::DmaWriteCompleted { command, request } => {
                write_u8(payload, 6);
                write_u64(payload, command.get());
                write_request_id(payload, *request);
            }
        }
    }
}

fn read_accelerator_trace_vec(
    reader: &mut ChunkReader<'_>,
) -> Result<Vec<AcceleratorTraceEvent>, String> {
    let count = reader.read_count("accelerator trace count")?;
    let mut events = Vec::with_capacity(count);
    for _ in 0..count {
        let tick = reader.read_u64("accelerator trace tick")?;
        let kind = match reader.read_u8("accelerator trace kind")? {
            0 => AcceleratorTraceKind::Submitted {
                command: AcceleratorCommandId::new(reader.read_u64("accelerator command")?),
                source: PartitionId::new(reader.read_u32("accelerator source partition")?),
                target: PartitionId::new(reader.read_u32("accelerator target partition")?),
            },
            1 => AcceleratorTraceKind::Started {
                command: AcceleratorCommandId::new(reader.read_u64("accelerator command")?),
                lane: reader.read_u32("accelerator lane")?,
                complete_at: reader.read_u64("accelerator complete tick")?,
            },
            2 => AcceleratorTraceKind::Completed {
                command: AcceleratorCommandId::new(reader.read_u64("accelerator command")?),
                lane: reader.read_u32("accelerator lane")?,
            },
            3 => AcceleratorTraceKind::DmaReadIssued {
                command: AcceleratorCommandId::new(reader.read_u64("accelerator command")?),
                request: reader.read_request_id("accelerator request")?,
            },
            4 => AcceleratorTraceKind::DmaReadCompleted {
                command: AcceleratorCommandId::new(reader.read_u64("accelerator command")?),
                request: reader.read_request_id("accelerator request")?,
                bytes: reader.read_u64("accelerator bytes")?,
            },
            5 => AcceleratorTraceKind::DmaWriteIssued {
                command: AcceleratorCommandId::new(reader.read_u64("accelerator command")?),
                request: reader.read_request_id("accelerator request")?,
            },
            6 => AcceleratorTraceKind::DmaWriteCompleted {
                command: AcceleratorCommandId::new(reader.read_u64("accelerator command")?),
                request: reader.read_request_id("accelerator request")?,
            },
            tag => return Err(format!("unknown accelerator trace kind {tag}")),
        };
        events.push(AcceleratorTraceEvent::new(tick, kind));
    }
    Ok(events)
}

fn write_accelerator_completion_vec(payload: &mut Vec<u8>, completions: &[AcceleratorCompletion]) {
    write_count(payload, completions.len());
    for completion in completions {
        write_u64(payload, completion.command().get());
        write_accelerator_command_kind(payload, completion.kind());
        write_u32(payload, completion.lane());
        write_u64(payload, completion.started_at());
        write_u64(payload, completion.completed_at());
    }
}

fn read_accelerator_completion_vec(
    reader: &mut ChunkReader<'_>,
) -> Result<Vec<AcceleratorCompletion>, String> {
    let count = reader.read_count("accelerator completion count")?;
    let mut completions = Vec::with_capacity(count);
    for _ in 0..count {
        completions.push(AcceleratorCompletion::new(
            AcceleratorCommandId::new(reader.read_u64("accelerator completed command")?),
            read_accelerator_command_kind(reader)?,
            reader.read_u32("accelerator completion lane")?,
            reader.read_u64("accelerator completion start")?,
            reader.read_u64("accelerator completion end")?,
        ));
    }
    Ok(completions)
}

fn write_accelerator_command_kind(payload: &mut Vec<u8>, kind: &AcceleratorCommandKind) {
    match kind {
        AcceleratorCommandKind::GpuKernel { workgroups } => {
            write_u8(payload, 0);
            write_u32(payload, *workgroups);
        }
        AcceleratorCommandKind::NpuInference { tiles } => {
            write_u8(payload, 1);
            write_u32(payload, *tiles);
        }
        AcceleratorCommandKind::DmaCopy { bytes } => {
            write_u8(payload, 2);
            write_u64(payload, *bytes);
        }
    }
}

fn read_accelerator_command_kind(
    reader: &mut ChunkReader<'_>,
) -> Result<AcceleratorCommandKind, String> {
    match reader.read_u8("accelerator command kind")? {
        0 => Ok(AcceleratorCommandKind::GpuKernel {
            workgroups: reader.read_u32("GPU workgroups")?,
        }),
        1 => Ok(AcceleratorCommandKind::NpuInference {
            tiles: reader.read_u32("NPU tiles")?,
        }),
        2 => Ok(AcceleratorCommandKind::DmaCopy {
            bytes: reader.read_u64("DMA bytes")?,
        }),
        tag => Err(format!("unknown accelerator command kind {tag}")),
    }
}

fn write_accelerator_pending_vec(payload: &mut Vec<u8>, pending: &[AcceleratorPendingDmaWrite]) {
    write_count(payload, pending.len());
    for item in pending {
        write_accelerator_dma_copy(payload, item.copy());
        write_bytes(payload, item.data());
        write_u64(payload, item.read_completed_at());
    }
}

fn read_accelerator_pending_vec(
    reader: &mut ChunkReader<'_>,
) -> Result<Vec<AcceleratorPendingDmaWrite>, String> {
    let count = reader.read_count("accelerator pending count")?;
    let mut pending = Vec::with_capacity(count);
    for _ in 0..count {
        pending.push(AcceleratorPendingDmaWrite::new(
            read_accelerator_dma_copy(reader)?,
            reader.read_bytes("accelerator pending data")?,
            reader.read_u64("accelerator pending read tick")?,
        ));
    }
    Ok(pending)
}

fn write_accelerator_dma_completion_vec(
    payload: &mut Vec<u8>,
    completions: &[AcceleratorDmaCompletion],
) {
    write_count(payload, completions.len());
    for completion in completions {
        write_u64(payload, completion.command().get());
        write_request_id(payload, completion.read_request());
        write_request_id(payload, completion.write_request());
        write_u64(payload, completion.read_completed_at());
        write_u64(payload, completion.write_completed_at());
    }
}

fn read_accelerator_dma_completion_vec(
    reader: &mut ChunkReader<'_>,
) -> Result<Vec<AcceleratorDmaCompletion>, String> {
    let count = reader.read_count("accelerator DMA completion count")?;
    let mut completions = Vec::with_capacity(count);
    for _ in 0..count {
        completions.push(AcceleratorDmaCompletion::new(
            AcceleratorCommandId::new(reader.read_u64("accelerator DMA command")?),
            reader.read_request_id("accelerator DMA read request")?,
            reader.read_request_id("accelerator DMA write request")?,
            reader.read_u64("accelerator DMA read tick")?,
            reader.read_u64("accelerator DMA write tick")?,
        ));
    }
    Ok(completions)
}

fn write_accelerator_dma_copy(payload: &mut Vec<u8>, copy: &AcceleratorDmaCopy) {
    write_u64(payload, copy.command().get());
    write_u64(payload, copy.read_route().get());
    write_memory_request(payload, copy.read_request());
    write_u64(payload, copy.write_route().get());
    write_request_id(payload, copy.write_request());
    write_u64(payload, copy.destination().get());
}

fn read_accelerator_dma_copy(reader: &mut ChunkReader<'_>) -> Result<AcceleratorDmaCopy, String> {
    AcceleratorDmaCopy::new(
        AcceleratorCommandId::new(reader.read_u64("accelerator DMA command")?),
        MemoryRouteId::new(reader.read_u64("accelerator read route")?),
        read_memory_request(reader)?,
        MemoryRouteId::new(reader.read_u64("accelerator write route")?),
        reader.read_request_id("accelerator write request")?,
        Address::new(reader.read_u64("accelerator destination")?),
    )
    .map_err(|error| error.to_string())
}

fn write_gpu_trace_vec(payload: &mut Vec<u8>, events: &[GpuTraceEvent]) {
    write_count(payload, events.len());
    for event in events {
        write_u64(payload, event.tick());
        match event.kind() {
            GpuTraceKind::LaunchSubmitted {
                kernel,
                source,
                target,
            } => {
                write_u8(payload, 0);
                write_u64(payload, kernel.get());
                write_u32(payload, source.index());
                write_u32(payload, target.index());
            }
            GpuTraceKind::LaunchAccepted { kernel, workgroups } => {
                write_u8(payload, 1);
                write_u64(payload, kernel.get());
                write_u32(payload, *workgroups);
            }
            GpuTraceKind::WorkgroupStarted {
                kernel,
                workgroup,
                compute_unit,
                slot,
                complete_at,
            } => {
                write_u8(payload, 2);
                write_u64(payload, kernel.get());
                write_u32(payload, workgroup.get());
                write_u32(payload, *compute_unit);
                write_u32(payload, *slot);
                write_u64(payload, *complete_at);
            }
            GpuTraceKind::WorkgroupCompleted {
                kernel,
                workgroup,
                compute_unit,
                slot,
            } => {
                write_u8(payload, 3);
                write_u64(payload, kernel.get());
                write_u32(payload, workgroup.get());
                write_u32(payload, *compute_unit);
                write_u32(payload, *slot);
            }
            GpuTraceKind::DmaReadIssued { transfer, request } => {
                write_u8(payload, 4);
                write_u64(payload, transfer.get());
                write_request_id(payload, *request);
            }
            GpuTraceKind::DmaReadCompleted {
                transfer,
                request,
                bytes,
            } => {
                write_u8(payload, 5);
                write_u64(payload, transfer.get());
                write_request_id(payload, *request);
                write_u64(payload, *bytes);
            }
            GpuTraceKind::DmaWriteIssued { transfer, request } => {
                write_u8(payload, 6);
                write_u64(payload, transfer.get());
                write_request_id(payload, *request);
            }
            GpuTraceKind::DmaWriteCompleted { transfer, request } => {
                write_u8(payload, 7);
                write_u64(payload, transfer.get());
                write_request_id(payload, *request);
            }
        }
    }
}

fn read_gpu_trace_vec(reader: &mut ChunkReader<'_>) -> Result<Vec<GpuTraceEvent>, String> {
    let count = reader.read_count("GPU trace count")?;
    let mut events = Vec::with_capacity(count);
    for _ in 0..count {
        let tick = reader.read_u64("GPU trace tick")?;
        let kind = match reader.read_u8("GPU trace kind")? {
            0 => GpuTraceKind::LaunchSubmitted {
                kernel: GpuKernelId::new(reader.read_u64("GPU kernel")?),
                source: PartitionId::new(reader.read_u32("GPU source partition")?),
                target: PartitionId::new(reader.read_u32("GPU target partition")?),
            },
            1 => GpuTraceKind::LaunchAccepted {
                kernel: GpuKernelId::new(reader.read_u64("GPU kernel")?),
                workgroups: reader.read_u32("GPU workgroups")?,
            },
            2 => GpuTraceKind::WorkgroupStarted {
                kernel: GpuKernelId::new(reader.read_u64("GPU kernel")?),
                workgroup: GpuWorkgroupId::new(reader.read_u32("GPU workgroup")?),
                compute_unit: reader.read_u32("GPU compute unit")?,
                slot: reader.read_u32("GPU slot")?,
                complete_at: reader.read_u64("GPU complete tick")?,
            },
            3 => GpuTraceKind::WorkgroupCompleted {
                kernel: GpuKernelId::new(reader.read_u64("GPU kernel")?),
                workgroup: GpuWorkgroupId::new(reader.read_u32("GPU workgroup")?),
                compute_unit: reader.read_u32("GPU compute unit")?,
                slot: reader.read_u32("GPU slot")?,
            },
            4 => GpuTraceKind::DmaReadIssued {
                transfer: GpuDmaId::new(reader.read_u64("GPU DMA transfer")?),
                request: reader.read_request_id("GPU DMA request")?,
            },
            5 => GpuTraceKind::DmaReadCompleted {
                transfer: GpuDmaId::new(reader.read_u64("GPU DMA transfer")?),
                request: reader.read_request_id("GPU DMA request")?,
                bytes: reader.read_u64("GPU DMA bytes")?,
            },
            6 => GpuTraceKind::DmaWriteIssued {
                transfer: GpuDmaId::new(reader.read_u64("GPU DMA transfer")?),
                request: reader.read_request_id("GPU DMA request")?,
            },
            7 => GpuTraceKind::DmaWriteCompleted {
                transfer: GpuDmaId::new(reader.read_u64("GPU DMA transfer")?),
                request: reader.read_request_id("GPU DMA request")?,
            },
            tag => return Err(format!("unknown GPU trace kind {tag}")),
        };
        events.push(GpuTraceEvent::new(tick, kind));
    }
    Ok(events)
}

fn write_gpu_completion_vec(payload: &mut Vec<u8>, completions: &[GpuWorkgroupCompletion]) {
    write_count(payload, completions.len());
    for completion in completions {
        write_u64(payload, completion.kernel().get());
        write_u32(payload, completion.workgroup().get());
        write_u32(payload, completion.compute_unit());
        write_u32(payload, completion.slot());
        write_u64(payload, completion.started_at());
        write_u64(payload, completion.completed_at());
    }
}

fn read_gpu_completion_vec(
    reader: &mut ChunkReader<'_>,
) -> Result<Vec<GpuWorkgroupCompletion>, String> {
    let count = reader.read_count("GPU completion count")?;
    let mut completions = Vec::with_capacity(count);
    for _ in 0..count {
        completions.push(GpuWorkgroupCompletion::new(
            GpuKernelId::new(reader.read_u64("GPU completion kernel")?),
            GpuWorkgroupId::new(reader.read_u32("GPU completion workgroup")?),
            reader.read_u32("GPU completion compute unit")?,
            reader.read_u32("GPU completion slot")?,
            reader.read_u64("GPU completion start")?,
            reader.read_u64("GPU completion end")?,
        ));
    }
    Ok(completions)
}

fn write_gpu_slot_vec(payload: &mut Vec<u8>, slots: &[GpuSlotSnapshot]) {
    write_count(payload, slots.len());
    for slot in slots {
        write_u64(payload, slot.available_at());
        write_bool(payload, slot.pump_scheduled());
        write_count(payload, slot.queued().len());
        for workgroup in slot.queued() {
            write_u64(payload, workgroup.kernel().get());
            write_u32(payload, workgroup.workgroup().get());
            write_u32(payload, workgroup.compute_unit());
            write_u32(payload, workgroup.slot());
            write_u64(payload, workgroup.queued_at());
            write_u64(payload, workgroup.started_at());
            write_u64(payload, workgroup.completed_at());
        }
    }
}

fn read_gpu_slot_vec(reader: &mut ChunkReader<'_>) -> Result<Vec<GpuSlotSnapshot>, String> {
    let count = reader.read_count("GPU slot count")?;
    let mut slots = Vec::with_capacity(count);
    for _ in 0..count {
        let available_at = reader.read_u64("GPU slot available tick")?;
        let pump_scheduled = reader.read_bool("GPU slot pump state")?;
        let queued_count = reader.read_count("GPU queued workgroup count")?;
        let mut queued = Vec::with_capacity(queued_count);
        for _ in 0..queued_count {
            queued.push(GpuQueuedWorkgroupSnapshot::new(
                GpuKernelId::new(reader.read_u64("GPU queued kernel")?),
                GpuWorkgroupId::new(reader.read_u32("GPU queued workgroup")?),
                reader.read_u32("GPU queued compute unit")?,
                reader.read_u32("GPU queued slot")?,
                reader.read_u64("GPU queued enqueue")?,
                reader.read_u64("GPU queued start")?,
                reader.read_u64("GPU queued completion")?,
            ));
        }
        slots.push(GpuSlotSnapshot::new(available_at, pump_scheduled, queued));
    }
    Ok(slots)
}

fn write_gpu_pending_vec(payload: &mut Vec<u8>, pending: &[GpuPendingDmaWrite]) {
    write_count(payload, pending.len());
    for item in pending {
        write_gpu_dma_copy(payload, item.copy());
        write_bytes(payload, item.data());
        write_u64(payload, item.read_completed_at());
    }
}

fn read_gpu_pending_vec(reader: &mut ChunkReader<'_>) -> Result<Vec<GpuPendingDmaWrite>, String> {
    let count = reader.read_count("GPU pending count")?;
    let mut pending = Vec::with_capacity(count);
    for _ in 0..count {
        pending.push(GpuPendingDmaWrite::new(
            read_gpu_dma_copy(reader)?,
            reader.read_bytes("GPU pending data")?,
            reader.read_u64("GPU pending read tick")?,
        ));
    }
    Ok(pending)
}

fn write_gpu_dma_completion_vec(payload: &mut Vec<u8>, completions: &[GpuDmaCompletion]) {
    write_count(payload, completions.len());
    for completion in completions {
        write_u64(payload, completion.transfer().get());
        write_request_id(payload, completion.read_request());
        write_request_id(payload, completion.write_request());
        write_u64(payload, completion.read_completed_at());
        write_u64(payload, completion.write_completed_at());
    }
}

fn read_gpu_dma_completion_vec(
    reader: &mut ChunkReader<'_>,
) -> Result<Vec<GpuDmaCompletion>, String> {
    let count = reader.read_count("GPU DMA completion count")?;
    let mut completions = Vec::with_capacity(count);
    for _ in 0..count {
        completions.push(GpuDmaCompletion::new(
            GpuDmaId::new(reader.read_u64("GPU DMA transfer")?),
            reader.read_request_id("GPU DMA read request")?,
            reader.read_request_id("GPU DMA write request")?,
            reader.read_u64("GPU DMA read tick")?,
            reader.read_u64("GPU DMA write tick")?,
        ));
    }
    Ok(completions)
}

fn write_gpu_dma_copy(payload: &mut Vec<u8>, copy: &GpuDmaCopy) {
    write_u64(payload, copy.transfer().get());
    write_u64(payload, copy.read_route().get());
    write_memory_request(payload, copy.read_request());
    write_u64(payload, copy.write_route().get());
    write_request_id(payload, copy.write_request());
    write_u64(payload, copy.destination().get());
}

fn read_gpu_dma_copy(reader: &mut ChunkReader<'_>) -> Result<GpuDmaCopy, String> {
    GpuDmaCopy::new(
        GpuDmaId::new(reader.read_u64("GPU DMA transfer")?),
        MemoryRouteId::new(reader.read_u64("GPU read route")?),
        read_memory_request(reader)?,
        MemoryRouteId::new(reader.read_u64("GPU write route")?),
        reader.read_request_id("GPU write request")?,
        Address::new(reader.read_u64("GPU destination")?),
    )
    .map_err(|error| error.to_string())
}

fn write_memory_request(payload: &mut Vec<u8>, request: &MemoryRequest) {
    write_request_id(payload, request.id());
    write_memory_operation(payload, request.operation());
    write_u64(payload, request.range().start().get());
    write_u64(payload, request.size().bytes());
    write_u64(payload, request.line_layout().bytes());
    write_optional_bytes(payload, request.data());
    write_optional_mask(payload, request.byte_mask());
    write_memory_access_ordering(payload, request.ordering());
}

fn read_memory_request(reader: &mut ChunkReader<'_>) -> Result<MemoryRequest, String> {
    let id = reader.read_request_id("memory request id")?;
    let operation = read_memory_operation(reader)?;
    let address = Address::new(reader.read_u64("memory request address")?);
    let size = AccessSize::new(reader.read_u64("memory request size")?)
        .map_err(|error| error.to_string())?;
    let layout = CacheLineLayout::new(reader.read_u64("memory request line size")?)
        .map_err(|error| error.to_string())?;
    let data = reader.read_optional_bytes("memory request data")?;
    let byte_mask = reader.read_optional_mask("memory request byte mask")?;
    let ordering = read_memory_access_ordering(reader)?;
    let request = match operation {
        MemoryOperation::InstructionFetch => {
            MemoryRequest::instruction_fetch(id, address, size, layout)
        }
        MemoryOperation::ReadShared => MemoryRequest::read_shared(id, address, size, layout),
        MemoryOperation::ReadUnique => MemoryRequest::read_unique(id, address, size, layout),
        MemoryOperation::Write => MemoryRequest::write(
            id,
            address,
            size,
            data.ok_or_else(|| "write request missing data".to_string())?,
            byte_mask.ok_or_else(|| "write request missing byte mask".to_string())?,
            layout,
        ),
        MemoryOperation::Upgrade => MemoryRequest::upgrade(id, address, size, layout),
        MemoryOperation::WritebackClean => MemoryRequest::writeback_clean(
            id,
            address,
            data.ok_or_else(|| "writeback-clean request missing data".to_string())?,
            layout,
        ),
        MemoryOperation::WritebackDirty => MemoryRequest::writeback_dirty(
            id,
            address,
            data.ok_or_else(|| "writeback-dirty request missing data".to_string())?,
            layout,
        ),
        other => return Err(format!("unsupported checkpoint memory operation {other:?}")),
    }
    .map_err(|error| error.to_string())?;
    Ok(request.with_ordering(ordering))
}

fn write_memory_access_ordering(payload: &mut Vec<u8>, ordering: MemoryAccessOrdering) {
    write_optional_memory_barrier_set(payload, ordering.before());
    write_optional_memory_barrier_set(payload, ordering.after());
}

fn read_memory_access_ordering(
    reader: &mut ChunkReader<'_>,
) -> Result<MemoryAccessOrdering, String> {
    let before = read_optional_memory_barrier_set(
        reader,
        "memory request before-ordering flag",
        "memory request before-ordering read flag",
        "memory request before-ordering write flag",
    )?;
    let after = read_optional_memory_barrier_set(
        reader,
        "memory request after-ordering flag",
        "memory request after-ordering read flag",
        "memory request after-ordering write flag",
    )?;
    Ok(MemoryAccessOrdering::new(before, after))
}

fn write_optional_memory_barrier_set(payload: &mut Vec<u8>, barrier: Option<MemoryBarrierSet>) {
    match barrier {
        Some(barrier) => {
            write_bool(payload, true);
            write_bool(payload, barrier.read());
            write_bool(payload, barrier.write());
        }
        None => write_bool(payload, false),
    }
}

fn read_optional_memory_barrier_set(
    reader: &mut ChunkReader<'_>,
    flag_name: &str,
    read_name: &str,
    write_name: &str,
) -> Result<Option<MemoryBarrierSet>, String> {
    if !reader.read_bool(flag_name)? {
        return Ok(None);
    }
    let read = reader.read_bool(read_name)?;
    let write = reader.read_bool(write_name)?;
    Ok(Some(MemoryBarrierSet::new(read, write)))
}

fn write_memory_operation(payload: &mut Vec<u8>, operation: MemoryOperation) {
    write_u8(
        payload,
        match operation {
            MemoryOperation::InstructionFetch => 0,
            MemoryOperation::ReadShared => 1,
            MemoryOperation::ReadUnique => 2,
            MemoryOperation::Write => 3,
            MemoryOperation::Upgrade => 4,
            MemoryOperation::WritebackClean => 5,
            MemoryOperation::WritebackDirty => 6,
            MemoryOperation::Atomic => 7,
            MemoryOperation::PrefetchRead => 8,
            MemoryOperation::PrefetchWrite => 9,
            MemoryOperation::CleanEvict => 10,
            MemoryOperation::Invalidate => 11,
        },
    );
}

fn read_memory_operation(reader: &mut ChunkReader<'_>) -> Result<MemoryOperation, String> {
    match reader.read_u8("memory operation")? {
        0 => Ok(MemoryOperation::InstructionFetch),
        1 => Ok(MemoryOperation::ReadShared),
        2 => Ok(MemoryOperation::ReadUnique),
        3 => Ok(MemoryOperation::Write),
        4 => Ok(MemoryOperation::Upgrade),
        5 => Ok(MemoryOperation::WritebackClean),
        6 => Ok(MemoryOperation::WritebackDirty),
        7 => Ok(MemoryOperation::Atomic),
        8 => Ok(MemoryOperation::PrefetchRead),
        9 => Ok(MemoryOperation::PrefetchWrite),
        10 => Ok(MemoryOperation::CleanEvict),
        11 => Ok(MemoryOperation::Invalidate),
        tag => Err(format!("unknown memory operation {tag}")),
    }
}

fn write_request_id(payload: &mut Vec<u8>, request: MemoryRequestId) {
    write_u32(payload, request.agent().get());
    write_u64(payload, request.sequence());
}

fn write_u64_vec(payload: &mut Vec<u8>, values: &[Tick]) {
    write_count(payload, values.len());
    for value in values {
        write_u64(payload, *value);
    }
}

fn write_optional_bytes(payload: &mut Vec<u8>, value: Option<&[u8]>) {
    match value {
        Some(bytes) => {
            write_bool(payload, true);
            write_bytes(payload, bytes);
        }
        None => write_bool(payload, false),
    }
}

fn write_optional_mask(payload: &mut Vec<u8>, mask: Option<&ByteMask>) {
    match mask {
        Some(mask) => {
            write_bool(payload, true);
            write_count(payload, mask.bits().len());
            for bit in mask.bits() {
                write_bool(payload, *bit);
            }
        }
        None => write_bool(payload, false),
    }
}

fn write_bytes(payload: &mut Vec<u8>, bytes: &[u8]) {
    write_count(payload, bytes.len());
    payload.extend_from_slice(bytes);
}

fn write_count(payload: &mut Vec<u8>, count: usize) {
    write_u64(payload, count as u64);
}

fn write_bool(payload: &mut Vec<u8>, value: bool) {
    write_u8(payload, u8::from(value));
}

fn write_u8(payload: &mut Vec<u8>, value: u8) {
    payload.push(value);
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

struct ChunkReader<'a> {
    payload: &'a [u8],
    offset: usize,
}

impl<'a> ChunkReader<'a> {
    fn new(payload: &'a [u8]) -> Self {
        Self { payload, offset: 0 }
    }

    fn read_u64_vec(&mut self) -> Result<Vec<Tick>, String> {
        let count = self.read_count("u64 vector count")?;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(self.read_u64("u64 vector item")?);
        }
        Ok(values)
    }

    fn read_request_id(&mut self, name: &str) -> Result<MemoryRequestId, String> {
        Ok(MemoryRequestId::new(
            AgentId::new(self.read_u32(name)?),
            self.read_u64(name)?,
        ))
    }

    fn read_optional_bytes(&mut self, name: &str) -> Result<Option<Vec<u8>>, String> {
        if self.read_bool(name)? {
            Ok(Some(self.read_bytes(name)?))
        } else {
            Ok(None)
        }
    }

    fn read_optional_mask(&mut self, name: &str) -> Result<Option<ByteMask>, String> {
        if !self.read_bool(name)? {
            return Ok(None);
        }
        let count = self.read_count(name)?;
        let mut bits = Vec::with_capacity(count);
        for _ in 0..count {
            bits.push(self.read_bool(name)?);
        }
        ByteMask::from_bits(bits)
            .map(Some)
            .map_err(|error| error.to_string())
    }

    fn read_bytes(&mut self, name: &str) -> Result<Vec<u8>, String> {
        let len = self.read_count(name)?;
        Ok(self.read_exact(name, len)?.to_vec())
    }

    fn read_count(&mut self, name: &str) -> Result<usize, String> {
        self.read_u64(name)?
            .try_into()
            .map_err(|_| format!("{name} does not fit usize"))
    }

    fn read_bool(&mut self, name: &str) -> Result<bool, String> {
        match self.read_u8(name)? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(format!("{name} has invalid bool value {value}")),
        }
    }

    fn read_u8(&mut self, name: &str) -> Result<u8, String> {
        Ok(self.read_exact(name, 1)?[0])
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, String> {
        let bytes = self.read_exact(name, 4)?;
        Ok(u32::from_le_bytes(bytes.try_into().expect("u32 slice len")))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, String> {
        let bytes = self.read_exact(name, 8)?;
        Ok(u64::from_le_bytes(bytes.try_into().expect("u64 slice len")))
    }

    fn read_exact(&mut self, name: &str, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| format!("{name} length overflows"))?;
        if end > self.payload.len() {
            return Err(format!(
                "{name} needs {len} bytes at offset {}; payload has {} bytes",
                self.offset,
                self.payload.len()
            ));
        }
        let bytes = &self.payload[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), String> {
        if self.offset == self.payload.len() {
            Ok(())
        } else {
            Err(format!(
                "{} trailing bytes after checkpoint chunk",
                self.payload.len() - self.offset
            ))
        }
    }
}
