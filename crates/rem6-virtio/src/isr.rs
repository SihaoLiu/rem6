use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, SchedulerContext, Tick};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{
    MmioAccess, MmioDevice, MmioError, MmioOperation, MmioRequest, MmioRequestId, MmioResponse,
};

use crate::VirtioError;

pub const VIRTIO_PCI_ISR_STATUS_SIZE: u64 = 1;

const VIRTIO_PCI_ISR_QUEUE_INTERRUPT: u8 = 0x01;
const VIRTIO_PCI_ISR_CONFIGURATION_CHANGE: u8 = 0x02;
const VIRTIO_PCI_ISR_KNOWN_BITS: u8 =
    VIRTIO_PCI_ISR_QUEUE_INTERRUPT | VIRTIO_PCI_ISR_CONFIGURATION_CHANGE;
const VIRTIO_PCI_ISR_SNAPSHOT_MAGIC: &[u8; 8] = b"VIOISR01";
const VIRTIO_PCI_ISR_SNAPSHOT_VERSION: u16 = 1;
const VIRTIO_PCI_ISR_EVENT_BYTES: usize = 11;
const U64_BYTES: usize = 8;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VirtioPciIsrStatus(u8);

impl VirtioPciIsrStatus {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn queue_interrupt() -> Self {
        Self(VIRTIO_PCI_ISR_QUEUE_INTERRUPT)
    }

    pub const fn configuration_change_interrupt() -> Self {
        Self(VIRTIO_PCI_ISR_CONFIGURATION_CHANGE)
    }

    pub const fn queue_and_config() -> Self {
        Self(VIRTIO_PCI_ISR_KNOWN_BITS)
    }

    pub const fn from_bits_truncate(bits: u8) -> Self {
        Self(bits & VIRTIO_PCI_ISR_KNOWN_BITS)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    const fn with(self, other: Self) -> Self {
        Self::from_bits_truncate(self.0 | other.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtioPciIsrEventKind {
    QueueInterrupt,
    ConfigurationChangeInterrupt,
    DriverReadClear,
}

impl VirtioPciIsrEventKind {
    const fn status_bit(self) -> Option<VirtioPciIsrStatus> {
        match self {
            Self::QueueInterrupt => Some(VirtioPciIsrStatus::queue_interrupt()),
            Self::ConfigurationChangeInterrupt => {
                Some(VirtioPciIsrStatus::configuration_change_interrupt())
            }
            Self::DriverReadClear => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioPciIsrEvent {
    tick: Tick,
    kind: VirtioPciIsrEventKind,
    status_before: VirtioPciIsrStatus,
    status_after: VirtioPciIsrStatus,
}

impl VirtioPciIsrEvent {
    pub const fn new(
        tick: Tick,
        kind: VirtioPciIsrEventKind,
        status_before: VirtioPciIsrStatus,
        status_after: VirtioPciIsrStatus,
    ) -> Self {
        Self {
            tick,
            kind,
            status_before,
            status_after,
        }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn kind(self) -> VirtioPciIsrEventKind {
        self.kind
    }

    pub const fn status_before(self) -> VirtioPciIsrStatus {
        self.status_before
    }

    pub const fn status_after(self) -> VirtioPciIsrStatus {
        self.status_after
    }
}

#[derive(Clone, Debug)]
pub struct VirtioPciIsrDevice {
    state: Arc<Mutex<VirtioPciIsrState>>,
}

impl VirtioPciIsrDevice {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(VirtioPciIsrState::default())),
        }
    }

    pub fn range(&self) -> AddressRange {
        AddressRange::new(
            Address::new(0),
            AccessSize::new(VIRTIO_PCI_ISR_STATUS_SIZE).unwrap(),
        )
        .unwrap()
    }

    pub fn status(&self) -> VirtioPciIsrStatus {
        self.state.lock().expect("virtio pci isr lock").status
    }

    pub fn events(&self) -> Vec<VirtioPciIsrEvent> {
        self.state
            .lock()
            .expect("virtio pci isr lock")
            .events
            .clone()
    }

    pub fn raise_queue_interrupt(&self, tick: Tick) -> VirtioPciIsrStatus {
        self.raise(VirtioPciIsrEventKind::QueueInterrupt, tick)
    }

    pub fn raise_configuration_change_interrupt(&self, tick: Tick) -> VirtioPciIsrStatus {
        self.raise(VirtioPciIsrEventKind::ConfigurationChangeInterrupt, tick)
    }

    pub fn snapshot(&self) -> VirtioPciIsrSnapshot {
        VirtioPciIsrSnapshot {
            state: self.state.lock().expect("virtio pci isr lock").clone(),
        }
    }

    pub fn restore(&self, snapshot: &VirtioPciIsrSnapshot) {
        *self.state.lock().expect("virtio pci isr lock") = snapshot.state.clone();
    }

    pub fn read_local(&self, address: Address, size: AccessSize) -> Result<Vec<u8>, MmioError> {
        self.read_at(MmioRequestId::new(0), address, size, 0)
    }

    pub fn write_local(
        &self,
        address: Address,
        data: Vec<u8>,
        byte_mask: ByteMask,
    ) -> Result<(), MmioError> {
        let size = AccessSize::new(data.len() as u64).map_err(MmioError::Memory)?;
        self.write_at(MmioRequestId::new(0), address, size, &data, &byte_mask)
    }

    fn raise(&self, kind: VirtioPciIsrEventKind, tick: Tick) -> VirtioPciIsrStatus {
        let bit = kind
            .status_bit()
            .expect("virtio pci isr device raise event");
        let mut state = self.state.lock().expect("virtio pci isr lock");
        let before = state.status;
        state.status = state.status.with(bit);
        let after = state.status;
        state
            .events
            .push(VirtioPciIsrEvent::new(tick, kind, before, after));
        after
    }

    fn read_at(
        &self,
        request: MmioRequestId,
        address: Address,
        size: AccessSize,
        tick: Tick,
    ) -> Result<Vec<u8>, MmioError> {
        self.validate_access(request, address, size)?;
        let mut state = self.state.lock().expect("virtio pci isr lock");
        let status = state.status;
        if !status.is_empty() {
            state.status = VirtioPciIsrStatus::empty();
            state.events.push(VirtioPciIsrEvent::new(
                tick,
                VirtioPciIsrEventKind::DriverReadClear,
                status,
                VirtioPciIsrStatus::empty(),
            ));
        }
        Ok(vec![status.bits()])
    }

    fn write_at(
        &self,
        request: MmioRequestId,
        address: Address,
        size: AccessSize,
        data: &[u8],
        byte_mask: &ByteMask,
    ) -> Result<(), MmioError> {
        self.validate_access(request, address, size)?;
        if data.len() as u64 != size.bytes() {
            return Err(MmioError::PayloadSizeMismatch {
                request,
                expected: size.bytes(),
                actual: data.len() as u64,
            });
        }
        if byte_mask.len() != size.bytes() {
            return Err(MmioError::ByteMaskSizeMismatch {
                request,
                expected: size.bytes(),
                actual: byte_mask.len(),
            });
        }
        Err(MmioError::AccessDenied {
            request,
            operation: MmioOperation::Write,
            access: MmioAccess::ReadOnly,
        })
    }

    fn validate_access(
        &self,
        request: MmioRequestId,
        address: Address,
        size: AccessSize,
    ) -> Result<(), MmioError> {
        if size.bytes() != VIRTIO_PCI_ISR_STATUS_SIZE {
            return Err(MmioError::AccessSizeMismatch {
                request,
                expected: VIRTIO_PCI_ISR_STATUS_SIZE,
                actual: size.bytes(),
            });
        }

        let requested = AddressRange::new(address, size).map_err(MmioError::Memory)?;
        let range = self.range();
        if !range.contains_range(requested) {
            return Err(MmioError::DeviceBoundaryCrossed {
                request,
                device_start: range.start(),
                device_end: range.end(),
                requested_start: requested.start(),
                requested_end: requested.end(),
            });
        }
        Ok(())
    }
}

impl Default for VirtioPciIsrDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl MmioDevice for VirtioPciIsrDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        match request.operation() {
            MmioOperation::Read => self
                .read_at(
                    request.id(),
                    request.range().start(),
                    request.size(),
                    context.now(),
                )
                .map(|data| MmioResponse::completed(request.id(), Some(data))),
            MmioOperation::Write => {
                let data = request.data().ok_or(MmioError::MissingWriteData {
                    request: request.id(),
                })?;
                let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
                    request: request.id(),
                })?;
                self.write_at(
                    request.id(),
                    request.range().start(),
                    request.size(),
                    data,
                    mask,
                )?;
                Ok(MmioResponse::completed(request.id(), None))
            }
        }
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        match request.operation() {
            MmioOperation::Read => self
                .read_at(
                    request.id(),
                    request.range().start(),
                    request.size(),
                    context.now(),
                )
                .map(|data| MmioResponse::completed(request.id(), Some(data))),
            MmioOperation::Write => {
                let data = request.data().ok_or(MmioError::MissingWriteData {
                    request: request.id(),
                })?;
                let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
                    request: request.id(),
                })?;
                self.write_at(
                    request.id(),
                    request.range().start(),
                    request.size(),
                    data,
                    mask,
                )?;
                Ok(MmioResponse::completed(request.id(), None))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioPciIsrSnapshot {
    state: VirtioPciIsrState,
}

impl VirtioPciIsrSnapshot {
    pub fn new(status: VirtioPciIsrStatus, events: Vec<VirtioPciIsrEvent>) -> Self {
        Self {
            state: VirtioPciIsrState { status, events },
        }
    }

    pub const fn status(&self) -> VirtioPciIsrStatus {
        self.state.status
    }

    pub fn events(&self) -> &[VirtioPciIsrEvent] {
        &self.state.events
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(VIRTIO_PCI_ISR_SNAPSHOT_MAGIC);
        payload.extend_from_slice(&VIRTIO_PCI_ISR_SNAPSHOT_VERSION.to_le_bytes());
        payload.push(self.status().bits());
        payload.extend_from_slice(&(self.events().len() as u64).to_le_bytes());
        for event in self.events() {
            payload.push(isr_event_kind_code(event.kind()));
            payload.push(event.status_before().bits());
            payload.push(event.status_after().bits());
            payload.extend_from_slice(&event.tick().to_le_bytes());
        }
        payload
    }

    pub fn from_bytes(payload: &[u8]) -> Result<Self, VirtioError> {
        let mut cursor = VirtioPciIsrSnapshotCursor::new(payload);
        cursor.read_magic()?;
        let version = cursor.read_u16()?;
        if version != VIRTIO_PCI_ISR_SNAPSHOT_VERSION {
            return Err(VirtioError::InvalidPciIsrSnapshot);
        }
        let status = decode_isr_status(cursor.read_u8()?)?;
        let event_count =
            usize::try_from(cursor.read_u64()?).map_err(|_| VirtioError::InvalidPciIsrSnapshot)?;
        let event_bytes = event_count
            .checked_mul(VIRTIO_PCI_ISR_EVENT_BYTES)
            .ok_or(VirtioError::InvalidPciIsrSnapshot)?;
        if cursor.remaining() != event_bytes {
            return Err(VirtioError::InvalidPciIsrSnapshot);
        }
        let mut events = Vec::with_capacity(event_count);
        for _ in 0..event_count {
            let kind = isr_event_kind_from_code(cursor.read_u8()?)?;
            let status_before = decode_isr_status(cursor.read_u8()?)?;
            let status_after = decode_isr_status(cursor.read_u8()?)?;
            let tick = cursor.read_u64()?;
            events.push(VirtioPciIsrEvent::new(
                tick,
                kind,
                status_before,
                status_after,
            ));
        }
        cursor.finish()?;
        Ok(Self::new(status, events))
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct VirtioPciIsrState {
    status: VirtioPciIsrStatus,
    events: Vec<VirtioPciIsrEvent>,
}

fn isr_event_kind_code(kind: VirtioPciIsrEventKind) -> u8 {
    match kind {
        VirtioPciIsrEventKind::QueueInterrupt => 0,
        VirtioPciIsrEventKind::ConfigurationChangeInterrupt => 1,
        VirtioPciIsrEventKind::DriverReadClear => 2,
    }
}

fn isr_event_kind_from_code(code: u8) -> Result<VirtioPciIsrEventKind, VirtioError> {
    match code {
        0 => Ok(VirtioPciIsrEventKind::QueueInterrupt),
        1 => Ok(VirtioPciIsrEventKind::ConfigurationChangeInterrupt),
        2 => Ok(VirtioPciIsrEventKind::DriverReadClear),
        _ => Err(VirtioError::InvalidPciIsrSnapshot),
    }
}

fn decode_isr_status(value: u8) -> Result<VirtioPciIsrStatus, VirtioError> {
    let status = VirtioPciIsrStatus::from_bits_truncate(value);
    if status.bits() == value {
        Ok(status)
    } else {
        Err(VirtioError::InvalidPciIsrSnapshot)
    }
}

struct VirtioPciIsrSnapshotCursor<'a> {
    payload: &'a [u8],
    offset: usize,
}

impl<'a> VirtioPciIsrSnapshotCursor<'a> {
    fn new(payload: &'a [u8]) -> Self {
        Self { payload, offset: 0 }
    }

    fn read_magic(&mut self) -> Result<(), VirtioError> {
        let magic = self.read_exact(VIRTIO_PCI_ISR_SNAPSHOT_MAGIC.len())?;
        if magic == VIRTIO_PCI_ISR_SNAPSHOT_MAGIC {
            Ok(())
        } else {
            Err(VirtioError::InvalidPciIsrSnapshot)
        }
    }

    fn read_u8(&mut self) -> Result<u8, VirtioError> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, VirtioError> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_le_bytes(
            bytes.try_into().expect("snapshot u16 width is fixed"),
        ))
    }

    fn read_u64(&mut self) -> Result<u64, VirtioError> {
        let bytes = self.read_exact(U64_BYTES)?;
        Ok(u64::from_le_bytes(
            bytes.try_into().expect("snapshot u64 width is fixed"),
        ))
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], VirtioError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(VirtioError::InvalidPciIsrSnapshot)?;
        let bytes = self
            .payload
            .get(self.offset..end)
            .ok_or(VirtioError::InvalidPciIsrSnapshot)?;
        self.offset = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), VirtioError> {
        if self.offset == self.payload.len() {
            Ok(())
        } else {
            Err(VirtioError::InvalidPciIsrSnapshot)
        }
    }

    fn remaining(&self) -> usize {
        self.payload.len() - self.offset
    }
}
