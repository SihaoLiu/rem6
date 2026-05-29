use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_interrupt::{
    InterruptError, InterruptEventKind, InterruptLineId, InterruptRoute, InterruptSourceId,
    InterruptTargetId,
};
use rem6_kernel::{PartitionEventId, PartitionId, SchedulerError};
use rem6_uart::{
    Pl011UartMmioDevice, Pl011UartSnapshot, Pl011UartSnapshotFields, UartInterruptError,
    UartMmioDevice, UartRxByte, UartSnapshot, UartTxByte,
};

const UART_CHUNK: &str = "uart";
const PL011_UART_CHUNK: &str = "pl011";
const U8_BYTES: usize = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UartCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: UartSnapshot,
}

impl UartCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: UartSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &UartSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct UartCheckpointPort {
    component: CheckpointComponentId,
    device: UartMmioDevice,
}

impl UartCheckpointPort {
    pub const fn new(component: CheckpointComponentId, device: UartMmioDevice) -> Self {
        Self { component, device }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn device(&self) -> UartMmioDevice {
        self.device.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<UartCheckpointRecord, CheckpointError> {
        let snapshot = self.device.snapshot();
        registry.write_chunk(&self.component, UART_CHUNK, encode_uart(&snapshot))?;
        Ok(UartCheckpointRecord::new(self.component.clone(), snapshot))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<UartCheckpointRecord, UartCheckpointError> {
        let record = self.decode_from(registry)?;
        self.device.restore(record.snapshot());
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<UartCheckpointRecord, UartCheckpointError> {
        let payload = registry.chunk(&self.component, UART_CHUNK).ok_or_else(|| {
            UartCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: UART_CHUNK.to_string(),
            }
        })?;
        let snapshot = decode_uart(&self.component, payload)?;
        Ok(UartCheckpointRecord::new(self.component.clone(), snapshot))
    }
}

#[derive(Clone, Debug, Default)]
pub struct UartCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, UartCheckpointPort>,
}

impl UartCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = UartCheckpointPort>,
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
    ) -> Result<Vec<UartCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<UartCheckpointRecord>, UartCheckpointError> {
        self.validate_restore_from(registry)?;
        let records = self
            .ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in self.ports.values().zip(&records) {
            port.device.restore(record.snapshot());
        }
        Ok(records)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), UartCheckpointError> {
        for port in self.ports.values() {
            port.decode_from(registry)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UartCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
}

impl UartCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. } | Self::InvalidChunk { component, .. } => {
                component
            }
        }
    }
}

impl fmt::Display for UartCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "UART checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "UART checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
        }
    }
}

impl Error for UartCheckpointError {}

pub type Pl011UartCheckpointError = UartCheckpointError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pl011UartCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: Pl011UartSnapshot,
}

impl Pl011UartCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: Pl011UartSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &Pl011UartSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct Pl011UartCheckpointPort {
    component: CheckpointComponentId,
    device: Pl011UartMmioDevice,
}

impl Pl011UartCheckpointPort {
    pub const fn new(component: CheckpointComponentId, device: Pl011UartMmioDevice) -> Self {
        Self { component, device }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn device(&self) -> Pl011UartMmioDevice {
        self.device.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Pl011UartCheckpointRecord, CheckpointError> {
        let snapshot = self.device.snapshot();
        registry.write_chunk(
            &self.component,
            PL011_UART_CHUNK,
            encode_pl011_uart(&snapshot),
        )?;
        Ok(Pl011UartCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Pl011UartCheckpointRecord, Pl011UartCheckpointError> {
        let record = self.decode_from(registry)?;
        self.device.restore(record.snapshot());
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Pl011UartCheckpointRecord, Pl011UartCheckpointError> {
        let payload = registry
            .chunk(&self.component, PL011_UART_CHUNK)
            .ok_or_else(|| UartCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: PL011_UART_CHUNK.to_string(),
            })?;
        let snapshot = decode_pl011_uart(&self.component, payload)?;
        Ok(Pl011UartCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Pl011UartCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, Pl011UartCheckpointPort>,
}

impl Pl011UartCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = Pl011UartCheckpointPort>,
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
    ) -> Result<Vec<Pl011UartCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<Pl011UartCheckpointRecord>, Pl011UartCheckpointError> {
        self.validate_restore_from(registry)?;
        let records = self
            .ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in self.ports.values().zip(&records) {
            port.device.restore(record.snapshot());
        }
        Ok(records)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), Pl011UartCheckpointError> {
        for port in self.ports.values() {
            port.decode_from(registry)?;
        }
        Ok(())
    }
}

fn encode_uart(snapshot: &UartSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_uart_bytes(&mut payload, snapshot.tx_bytes());
    write_uart_bytes(&mut payload, snapshot.rx_injected());
    write_u64(&mut payload, snapshot.rx_pending().len() as u64);
    payload.extend_from_slice(snapshot.rx_pending());
    write_uart_bytes(&mut payload, snapshot.rx_consumed());
    write_u64(&mut payload, snapshot.interrupt_errors().len() as u64);
    for error in snapshot.interrupt_errors() {
        encode_uart_interrupt_error(&mut payload, error);
    }
    payload
}

fn encode_pl011_uart(snapshot: &Pl011UartSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_uart_bytes(&mut payload, snapshot.tx_bytes());
    write_uart_bytes(&mut payload, snapshot.rx_injected());
    write_u64(&mut payload, snapshot.rx_pending().len() as u64);
    payload.extend_from_slice(snapshot.rx_pending());
    write_uart_bytes(&mut payload, snapshot.rx_consumed());
    write_u64(&mut payload, snapshot.interrupt_errors().len() as u64);
    for error in snapshot.interrupt_errors() {
        encode_uart_interrupt_error(&mut payload, error);
    }
    write_u16(&mut payload, snapshot.control());
    write_u16(&mut payload, snapshot.integer_baud_divisor());
    write_u16(&mut payload, snapshot.fractional_baud_divisor());
    write_u16(&mut payload, snapshot.line_control());
    write_u16(&mut payload, snapshot.interrupt_fifo_level());
    write_u16(&mut payload, snapshot.interrupt_mask());
    write_u16(&mut payload, snapshot.raw_interrupt());
    payload
}

fn write_uart_bytes<T>(payload: &mut Vec<u8>, bytes: &[T])
where
    T: UartByteRecord,
{
    write_u64(payload, bytes.len() as u64);
    for byte in bytes {
        write_u64(payload, byte.tick());
        write_u8(payload, byte.byte());
    }
}

trait UartByteRecord {
    fn tick(&self) -> u64;
    fn byte(&self) -> u8;
}

impl UartByteRecord for UartTxByte {
    fn tick(&self) -> u64 {
        (*self).tick()
    }

    fn byte(&self) -> u8 {
        (*self).byte()
    }
}

impl UartByteRecord for UartRxByte {
    fn tick(&self) -> u64 {
        (*self).tick()
    }

    fn byte(&self) -> u8 {
        (*self).byte()
    }
}

fn decode_uart(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<UartSnapshot, UartCheckpointError> {
    let mut cursor = PayloadCursor::new(component.clone(), payload);
    let tx_bytes = read_tx_bytes(&mut cursor)?;
    let rx_injected = read_rx_bytes(&mut cursor)?;
    let pending_len = cursor.read_count("UART pending RX byte count")?;
    let rx_pending = cursor
        .read_bytes("UART pending RX bytes", pending_len)?
        .to_vec();
    let rx_consumed = read_rx_bytes(&mut cursor)?;
    let error_count = cursor.read_count("UART interrupt error count")?;
    let mut interrupt_errors = Vec::with_capacity(error_count);
    for _ in 0..error_count {
        interrupt_errors.push(decode_uart_interrupt_error(&mut cursor)?);
    }
    cursor.finish()?;
    Ok(UartSnapshot::new(
        tx_bytes,
        rx_injected,
        rx_pending,
        rx_consumed,
        interrupt_errors,
    ))
}

fn decode_pl011_uart(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<Pl011UartSnapshot, Pl011UartCheckpointError> {
    let mut cursor = PayloadCursor::new(component.clone(), payload);
    let tx_bytes = read_tx_bytes(&mut cursor)?;
    let rx_injected = read_rx_bytes(&mut cursor)?;
    let pending_len = cursor.read_count("PL011 pending RX byte count")?;
    let rx_pending = cursor
        .read_bytes("PL011 pending RX bytes", pending_len)?
        .to_vec();
    let rx_consumed = read_rx_bytes(&mut cursor)?;
    let error_count = cursor.read_count("PL011 interrupt error count")?;
    let mut interrupt_errors = Vec::with_capacity(error_count);
    for _ in 0..error_count {
        interrupt_errors.push(decode_uart_interrupt_error(&mut cursor)?);
    }
    let control = read_u16(&mut cursor, "PL011 control")?;
    let integer_baud_divisor = read_u16(&mut cursor, "PL011 integer baud divisor")?;
    let fractional_baud_divisor = read_u16(&mut cursor, "PL011 fractional baud divisor")?;
    let line_control = read_u16(&mut cursor, "PL011 line control")?;
    let interrupt_fifo_level = read_u16(&mut cursor, "PL011 interrupt FIFO level")?;
    let interrupt_mask = read_u16(&mut cursor, "PL011 interrupt mask")?;
    let raw_interrupt = read_u16(&mut cursor, "PL011 raw interrupt")?;
    cursor.finish()?;
    Ok(Pl011UartSnapshot::from_fields(Pl011UartSnapshotFields {
        tx_bytes,
        rx_injected,
        rx_pending,
        rx_consumed,
        interrupt_errors,
        control,
        integer_baud_divisor,
        fractional_baud_divisor,
        line_control,
        interrupt_fifo_level,
        interrupt_mask,
        raw_interrupt,
    }))
}

fn read_tx_bytes(cursor: &mut PayloadCursor<'_>) -> Result<Vec<UartTxByte>, UartCheckpointError> {
    let count = cursor.read_count("UART TX byte count")?;
    let mut bytes = Vec::with_capacity(count);
    for _ in 0..count {
        bytes.push(UartTxByte::new(
            cursor.read_u64("UART TX byte tick")?,
            cursor.read_u8("UART TX byte")?,
        ));
    }
    Ok(bytes)
}

fn read_rx_bytes(cursor: &mut PayloadCursor<'_>) -> Result<Vec<UartRxByte>, UartCheckpointError> {
    let count = cursor.read_count("UART RX byte count")?;
    let mut bytes = Vec::with_capacity(count);
    for _ in 0..count {
        bytes.push(UartRxByte::new(
            cursor.read_u64("UART RX byte tick")?,
            cursor.read_u8("UART RX byte")?,
        ));
    }
    Ok(bytes)
}

fn encode_uart_interrupt_error(payload: &mut Vec<u8>, error: &UartInterruptError) {
    write_u64(payload, error.tick());
    write_u32(payload, error.source().get());
    encode_interrupt_kind(payload, error.kind());
    encode_interrupt_error(payload, error.error());
}

fn decode_uart_interrupt_error(
    cursor: &mut PayloadCursor<'_>,
) -> Result<UartInterruptError, UartCheckpointError> {
    let tick = cursor.read_u64("UART interrupt error tick")?;
    let source = InterruptSourceId::new(cursor.read_u32("UART interrupt error source")?);
    let kind = decode_interrupt_kind(cursor, "UART interrupt error kind")?;
    let error = decode_interrupt_error(cursor)?;
    Ok(UartInterruptError::new(tick, source, kind, error))
}

fn encode_interrupt_kind(payload: &mut Vec<u8>, kind: InterruptEventKind) {
    write_u64(
        payload,
        match kind {
            InterruptEventKind::Assert => 0,
            InterruptEventKind::Deassert => 1,
            InterruptEventKind::Claim => 2,
            InterruptEventKind::Complete => 3,
        },
    );
}

fn decode_interrupt_kind(
    cursor: &mut PayloadCursor<'_>,
    name: &str,
) -> Result<InterruptEventKind, UartCheckpointError> {
    match cursor.read_u64(name)? {
        0 => Ok(InterruptEventKind::Assert),
        1 => Ok(InterruptEventKind::Deassert),
        2 => Ok(InterruptEventKind::Claim),
        3 => Ok(InterruptEventKind::Complete),
        value => Err(cursor.invalid(format!("{name} has invalid kind {value}"))),
    }
}

fn encode_interrupt_error(payload: &mut Vec<u8>, error: &InterruptError) {
    match error {
        InterruptError::ZeroSignalLatency => write_u64(payload, 0),
        InterruptError::DuplicateLine { line } => {
            write_u64(payload, 1);
            write_u64(payload, line.get());
        }
        InterruptError::UnknownLine { line } => {
            write_u64(payload, 2);
            write_u64(payload, line.get());
        }
        InterruptError::AlreadyPending { line, source } => {
            write_u64(payload, 3);
            write_u64(payload, line.get());
            write_u32(payload, source.get());
        }
        InterruptError::NotPending { line } => {
            write_u64(payload, 4);
            write_u64(payload, line.get());
        }
        InterruptError::SourceMismatch {
            line,
            expected,
            actual,
        } => {
            write_u64(payload, 5);
            write_u64(payload, line.get());
            write_u32(payload, expected.get());
            write_u32(payload, actual.get());
        }
        InterruptError::RouteMismatch {
            line,
            expected,
            actual,
        } => {
            write_u64(payload, 6);
            write_u64(payload, line.get());
            encode_interrupt_route(payload, *expected);
            encode_interrupt_route(payload, *actual);
        }
        InterruptError::NoClaimedInterrupt {
            target,
            target_partition,
        } => {
            write_u64(payload, 7);
            write_u32(payload, target.get());
            write_u32(payload, target_partition.index());
        }
        InterruptError::ClaimMismatch {
            target,
            target_partition,
            expected,
            actual,
        } => {
            write_u64(payload, 8);
            write_u32(payload, target.get());
            write_u32(payload, target_partition.index());
            write_u64(payload, expected.get());
            write_u64(payload, actual.get());
        }
        InterruptError::NonSignalDelivery { kind } => {
            write_u64(payload, 9);
            encode_interrupt_kind(payload, *kind);
        }
        InterruptError::Scheduler(error) => {
            write_u64(payload, 10);
            encode_scheduler_error(payload, error);
        }
    }
}

fn decode_interrupt_error(
    cursor: &mut PayloadCursor<'_>,
) -> Result<InterruptError, UartCheckpointError> {
    match cursor.read_u64("interrupt error kind")? {
        0 => Ok(InterruptError::ZeroSignalLatency),
        1 => Ok(InterruptError::DuplicateLine {
            line: InterruptLineId::new(cursor.read_u64("interrupt duplicate line")?),
        }),
        2 => Ok(InterruptError::UnknownLine {
            line: InterruptLineId::new(cursor.read_u64("interrupt unknown line")?),
        }),
        3 => Ok(InterruptError::AlreadyPending {
            line: InterruptLineId::new(cursor.read_u64("interrupt pending line")?),
            source: InterruptSourceId::new(cursor.read_u32("interrupt pending source")?),
        }),
        4 => Ok(InterruptError::NotPending {
            line: InterruptLineId::new(cursor.read_u64("interrupt not-pending line")?),
        }),
        5 => Ok(InterruptError::SourceMismatch {
            line: InterruptLineId::new(cursor.read_u64("interrupt source mismatch line")?),
            expected: InterruptSourceId::new(
                cursor.read_u32("interrupt source mismatch expected")?,
            ),
            actual: InterruptSourceId::new(cursor.read_u32("interrupt source mismatch actual")?),
        }),
        6 => Ok(InterruptError::RouteMismatch {
            line: InterruptLineId::new(cursor.read_u64("interrupt route mismatch line")?),
            expected: decode_interrupt_route(cursor)?,
            actual: decode_interrupt_route(cursor)?,
        }),
        7 => Ok(InterruptError::NoClaimedInterrupt {
            target: InterruptTargetId::new(cursor.read_u32("interrupt unclaimed target")?),
            target_partition: PartitionId::new(
                cursor.read_u32("interrupt unclaimed target partition")?,
            ),
        }),
        8 => Ok(InterruptError::ClaimMismatch {
            target: InterruptTargetId::new(cursor.read_u32("interrupt claim target")?),
            target_partition: PartitionId::new(
                cursor.read_u32("interrupt claim target partition")?,
            ),
            expected: InterruptLineId::new(cursor.read_u64("interrupt claim expected line")?),
            actual: InterruptLineId::new(cursor.read_u64("interrupt claim actual line")?),
        }),
        9 => Ok(InterruptError::NonSignalDelivery {
            kind: decode_interrupt_kind(cursor, "interrupt non-signal kind")?,
        }),
        10 => Ok(InterruptError::Scheduler(decode_scheduler_error(cursor)?)),
        value => Err(cursor.invalid(format!("interrupt error has invalid kind {value}"))),
    }
}

fn encode_interrupt_route(payload: &mut Vec<u8>, route: InterruptRoute) {
    write_u64(payload, route.line().get());
    write_u32(payload, route.target().get());
    write_u32(payload, route.target_partition().index());
}

fn decode_interrupt_route(
    cursor: &mut PayloadCursor<'_>,
) -> Result<InterruptRoute, UartCheckpointError> {
    Ok(InterruptRoute::new(
        InterruptLineId::new(cursor.read_u64("interrupt route line")?),
        InterruptTargetId::new(cursor.read_u32("interrupt route target")?),
        PartitionId::new(cursor.read_u32("interrupt route target partition")?),
    ))
}

fn encode_scheduler_error(payload: &mut Vec<u8>, error: &SchedulerError) {
    match error {
        SchedulerError::NoPartitions => write_u64(payload, 0),
        SchedulerError::ZeroLookahead => write_u64(payload, 1),
        SchedulerError::ZeroParallelWorkers => write_u64(payload, 13),
        SchedulerError::ParallelWorkerPanicked { partition } => {
            write_u64(payload, 15);
            write_u32(payload, partition.index());
        }
        SchedulerError::UnknownPartition {
            partition,
            partitions,
        } => {
            write_u64(payload, 2);
            write_u32(payload, partition.index());
            write_u32(payload, *partitions);
        }
        SchedulerError::InThePast {
            partition,
            now,
            requested,
        } => {
            write_u64(payload, 3);
            write_u32(payload, partition.index());
            write_u64(payload, *now);
            write_u64(payload, *requested);
        }
        SchedulerError::TickOverflow { now, delay } => {
            write_u64(payload, 4);
            write_u64(payload, *now);
            write_u64(payload, *delay);
        }
        SchedulerError::ZeroDelayRemoteMessage { source, target } => {
            write_u64(payload, 5);
            write_u32(payload, source.index());
            write_u32(payload, target.index());
        }
        SchedulerError::RemoteDelayBelowLookahead {
            source,
            target,
            delay,
            minimum,
        } => {
            write_u64(payload, 6);
            write_u32(payload, source.index());
            write_u32(payload, target.index());
            write_u64(payload, *delay);
            write_u64(payload, *minimum);
        }
        SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
            source,
            target,
            source_tick,
            delivery_tick,
            minimum_delivery_tick,
        } => {
            write_u64(payload, 16);
            write_u32(payload, source.index());
            write_u32(payload, target.index());
            write_u64(payload, *source_tick);
            write_u64(payload, *delivery_tick);
            write_u64(payload, *minimum_delivery_tick);
        }
        SchedulerError::SerialEventInParallelEpoch { partition, tick } => {
            write_u64(payload, 7);
            write_u32(payload, partition.index());
            write_u64(payload, *tick);
        }
        SchedulerError::EpochHorizonOverflow {
            partition,
            now,
            delay,
        } => {
            write_u64(payload, 12);
            write_u32(payload, partition.index());
            write_u64(payload, *now);
            write_u64(payload, *delay);
        }
        SchedulerError::EventNotPending { id } => {
            write_u64(payload, 18);
            write_u32(payload, id.partition().index());
            write_u64(payload, id.local());
        }
        SchedulerError::SnapshotContainsPendingEvents { pending_events } => {
            write_u64(payload, 8);
            write_u64(payload, *pending_events as u64);
        }
        SchedulerError::RestoreWouldDiscardPendingEvents { pending_events } => {
            write_u64(payload, 9);
            write_u64(payload, *pending_events as u64);
        }
        SchedulerError::SnapshotPartitionCountMismatch {
            snapshot_partitions,
            scheduler_partitions,
        } => {
            write_u64(payload, 10);
            write_u32(payload, *snapshot_partitions);
            write_u32(payload, *scheduler_partitions);
        }
        SchedulerError::SnapshotPartitionIdMismatch {
            expected_partition,
            snapshot_partition,
        } => {
            write_u64(payload, 17);
            write_u32(payload, expected_partition.index());
            write_u32(payload, snapshot_partition.index());
        }
        SchedulerError::SnapshotLookaheadMismatch {
            snapshot_min_remote_delay,
            scheduler_min_remote_delay,
        } => {
            write_u64(payload, 11);
            write_u64(payload, *snapshot_min_remote_delay);
            write_u64(payload, *scheduler_min_remote_delay);
        }
        SchedulerError::SnapshotParallelWorkerLimitMismatch {
            snapshot_max_parallel_workers,
            scheduler_max_parallel_workers,
        } => {
            write_u64(payload, 14);
            write_u64(payload, *snapshot_max_parallel_workers as u64);
            write_u64(payload, *scheduler_max_parallel_workers as u64);
        }
    }
}

fn decode_scheduler_error(
    cursor: &mut PayloadCursor<'_>,
) -> Result<SchedulerError, UartCheckpointError> {
    match cursor.read_u64("scheduler error kind")? {
        0 => Ok(SchedulerError::NoPartitions),
        1 => Ok(SchedulerError::ZeroLookahead),
        13 => Ok(SchedulerError::ZeroParallelWorkers),
        15 => Ok(SchedulerError::ParallelWorkerPanicked {
            partition: PartitionId::new(cursor.read_u32("scheduler panicked worker partition")?),
        }),
        2 => Ok(SchedulerError::UnknownPartition {
            partition: PartitionId::new(cursor.read_u32("scheduler unknown partition")?),
            partitions: cursor.read_u32("scheduler partition count")?,
        }),
        3 => Ok(SchedulerError::InThePast {
            partition: PartitionId::new(cursor.read_u32("scheduler past partition")?),
            now: cursor.read_u64("scheduler past now")?,
            requested: cursor.read_u64("scheduler past requested")?,
        }),
        4 => Ok(SchedulerError::TickOverflow {
            now: cursor.read_u64("scheduler overflow now")?,
            delay: cursor.read_u64("scheduler overflow delay")?,
        }),
        5 => Ok(SchedulerError::ZeroDelayRemoteMessage {
            source: PartitionId::new(cursor.read_u32("scheduler zero-delay source")?),
            target: PartitionId::new(cursor.read_u32("scheduler zero-delay target")?),
        }),
        6 => Ok(SchedulerError::RemoteDelayBelowLookahead {
            source: PartitionId::new(cursor.read_u32("scheduler lookahead source")?),
            target: PartitionId::new(cursor.read_u32("scheduler lookahead target")?),
            delay: cursor.read_u64("scheduler lookahead delay")?,
            minimum: cursor.read_u64("scheduler lookahead minimum")?,
        }),
        16 => Ok(SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
            source: PartitionId::new(cursor.read_u32("scheduler boundary source")?),
            target: PartitionId::new(cursor.read_u32("scheduler boundary target")?),
            source_tick: cursor.read_u64("scheduler boundary source tick")?,
            delivery_tick: cursor.read_u64("scheduler boundary delivery tick")?,
            minimum_delivery_tick: cursor.read_u64("scheduler boundary minimum delivery tick")?,
        }),
        7 => Ok(SchedulerError::SerialEventInParallelEpoch {
            partition: PartitionId::new(cursor.read_u32("scheduler serial partition")?),
            tick: cursor.read_u64("scheduler serial tick")?,
        }),
        8 => Ok(SchedulerError::SnapshotContainsPendingEvents {
            pending_events: cursor.read_count("scheduler snapshot pending events")?,
        }),
        9 => Ok(SchedulerError::RestoreWouldDiscardPendingEvents {
            pending_events: cursor.read_count("scheduler restore pending events")?,
        }),
        10 => Ok(SchedulerError::SnapshotPartitionCountMismatch {
            snapshot_partitions: cursor.read_u32("scheduler snapshot partition count")?,
            scheduler_partitions: cursor.read_u32("scheduler live partition count")?,
        }),
        17 => Ok(SchedulerError::SnapshotPartitionIdMismatch {
            expected_partition: PartitionId::new(cursor.read_u32("scheduler expected partition")?),
            snapshot_partition: PartitionId::new(cursor.read_u32("scheduler snapshot partition")?),
        }),
        11 => Ok(SchedulerError::SnapshotLookaheadMismatch {
            snapshot_min_remote_delay: cursor.read_u64("scheduler snapshot lookahead")?,
            scheduler_min_remote_delay: cursor.read_u64("scheduler live lookahead")?,
        }),
        12 => Ok(SchedulerError::EpochHorizonOverflow {
            partition: PartitionId::new(cursor.read_u32("scheduler horizon partition")?),
            now: cursor.read_u64("scheduler horizon now")?,
            delay: cursor.read_u64("scheduler horizon delay")?,
        }),
        18 => Ok(SchedulerError::EventNotPending {
            id: PartitionEventId::new(
                PartitionId::new(cursor.read_u32("scheduler pending event partition")?),
                cursor.read_u64("scheduler pending event local")?,
            ),
        }),
        14 => Ok(SchedulerError::SnapshotParallelWorkerLimitMismatch {
            snapshot_max_parallel_workers: cursor
                .read_count("scheduler snapshot parallel worker limit")?,
            scheduler_max_parallel_workers: cursor
                .read_count("scheduler live parallel worker limit")?,
        }),
        value => Err(cursor.invalid(format!("scheduler error has invalid kind {value}"))),
    }
}

fn write_u8(payload: &mut Vec<u8>, value: u8) {
    payload.push(value);
}

fn write_u16(payload: &mut Vec<u8>, value: u16) {
    write_u32(payload, value as u32);
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn read_u16(cursor: &mut PayloadCursor<'_>, name: &str) -> Result<u16, UartCheckpointError> {
    cursor
        .read_u32(name)?
        .try_into()
        .map_err(|_| cursor.invalid(format!("{name} does not fit u16")))
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

    fn read_count(&mut self, name: &str) -> Result<usize, UartCheckpointError> {
        self.read_u64(name)?
            .try_into()
            .map_err(|_| self.invalid(format!("{name} does not fit host usize")))
    }

    fn read_u8(&mut self, name: &str) -> Result<u8, UartCheckpointError> {
        Ok(self.read_bytes(name, U8_BYTES)?[0])
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, UartCheckpointError> {
        let bytes = self.read_bytes(name, U32_BYTES)?;
        Ok(u32::from_le_bytes(
            bytes.try_into().expect("u32 byte count checked"),
        ))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, UartCheckpointError> {
        let bytes = self.read_bytes(name, U64_BYTES)?;
        Ok(u64::from_le_bytes(
            bytes.try_into().expect("u64 byte count checked"),
        ))
    }

    fn read_bytes(&mut self, name: &str, count: usize) -> Result<&'a [u8], UartCheckpointError> {
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

    fn finish(&self) -> Result<(), UartCheckpointError> {
        if self.offset == self.payload.len() {
            return Ok(());
        }
        Err(self.invalid(format!(
            "{} trailing bytes",
            self.payload.len() - self.offset
        )))
    }

    fn invalid(&self, reason: String) -> UartCheckpointError {
        UartCheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}
