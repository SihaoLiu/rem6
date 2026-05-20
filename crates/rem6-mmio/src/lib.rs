use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, SchedulerContext, SchedulerError, Tick,
};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask, MemoryError};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MmioRequestId(u64);

impl MmioRequestId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MmioOperation {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MmioAccess {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

impl MmioAccess {
    const fn allows(self, operation: MmioOperation) -> bool {
        matches!(
            (self, operation),
            (Self::ReadOnly, MmioOperation::Read)
                | (Self::WriteOnly, MmioOperation::Write)
                | (Self::ReadWrite, _)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MmioRequest {
    id: MmioRequestId,
    operation: MmioOperation,
    range: AddressRange,
    data: Option<Vec<u8>>,
    byte_mask: Option<ByteMask>,
}

impl MmioRequest {
    pub fn read(id: MmioRequestId, address: Address, size: AccessSize) -> Result<Self, MmioError> {
        Ok(Self {
            id,
            operation: MmioOperation::Read,
            range: AddressRange::new(address, size).map_err(MmioError::Memory)?,
            data: None,
            byte_mask: None,
        })
    }

    pub fn write(
        id: MmioRequestId,
        address: Address,
        data: Vec<u8>,
        byte_mask: ByteMask,
    ) -> Result<Self, MmioError> {
        let size = AccessSize::new(data.len() as u64).map_err(MmioError::Memory)?;
        if byte_mask.len() != size.bytes() {
            return Err(MmioError::ByteMaskSizeMismatch {
                request: id,
                expected: size.bytes(),
                actual: byte_mask.len(),
            });
        }

        Ok(Self {
            id,
            operation: MmioOperation::Write,
            range: AddressRange::new(address, size).map_err(MmioError::Memory)?,
            data: Some(data),
            byte_mask: Some(byte_mask),
        })
    }

    pub const fn id(&self) -> MmioRequestId {
        self.id
    }

    pub const fn operation(&self) -> MmioOperation {
        self.operation
    }

    pub const fn range(&self) -> AddressRange {
        self.range
    }

    pub const fn size(&self) -> AccessSize {
        self.range.size()
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    pub fn byte_mask(&self) -> Option<&ByteMask> {
        self.byte_mask.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MmioResponse {
    request: MmioRequestId,
    data: Option<Vec<u8>>,
}

impl MmioResponse {
    pub fn completed(request: MmioRequestId, data: Option<Vec<u8>>) -> Self {
        Self { request, data }
    }

    pub const fn request(&self) -> MmioRequestId {
        self.request
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MmioRegisterBank {
    base: Address,
    range: AddressRange,
    registers: BTreeMap<Address, MmioRegister>,
}

impl MmioRegisterBank {
    pub fn new(base: Address, size: AccessSize) -> Result<Self, MmioError> {
        Ok(Self {
            base,
            range: AddressRange::new(base, size).map_err(MmioError::Memory)?,
            registers: BTreeMap::new(),
        })
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub const fn range(&self) -> AddressRange {
        self.range
    }

    pub fn register_count(&self) -> usize {
        self.registers.len()
    }

    pub fn insert_register(
        &mut self,
        offset: u64,
        size: AccessSize,
        access: MmioAccess,
        reset_value: Vec<u8>,
    ) -> Result<(), MmioError> {
        if reset_value.len() as u64 != size.bytes() {
            return Err(MmioError::ResetValueSizeMismatch {
                expected: size.bytes(),
                actual: reset_value.len() as u64,
            });
        }
        let requested = self.absolute_range(offset, size)?;
        if !self.range.contains_range(requested) {
            return Err(MmioError::RegisterOutOfRange {
                register_start: requested.start(),
                register_end: requested.end(),
                bank_start: self.range.start(),
                bank_end: self.range.end(),
            });
        }
        if let Some(existing) = self
            .registers
            .values()
            .find(|register| register.range.overlaps(requested))
        {
            return Err(MmioError::OverlappingRegister {
                existing_start: existing.range.start(),
                existing_end: existing.range.end(),
                requested_start: requested.start(),
                requested_end: requested.end(),
            });
        }

        self.registers.insert(
            requested.start(),
            MmioRegister {
                range: requested,
                access,
                value: reset_value,
            },
        );
        Ok(())
    }

    pub fn respond(&mut self, request: &MmioRequest) -> Result<MmioResponse, MmioError> {
        let register = self.register_for(request)?;
        if !register.access.allows(request.operation()) {
            return Err(MmioError::AccessDenied {
                request: request.id(),
                operation: request.operation(),
                access: register.access,
            });
        }

        match request.operation() {
            MmioOperation::Read => {
                let data = register.read(request.range())?;
                Ok(MmioResponse::completed(request.id(), Some(data)))
            }
            MmioOperation::Write => {
                register.write(request)?;
                Ok(MmioResponse::completed(request.id(), None))
            }
        }
    }

    fn absolute_range(&self, offset: u64, size: AccessSize) -> Result<AddressRange, MmioError> {
        let start = self
            .base
            .get()
            .checked_add(offset)
            .map(Address::new)
            .ok_or(MmioError::AddressOverflow {
                address: self.base,
                offset,
            })?;
        AddressRange::new(start, size).map_err(MmioError::Memory)
    }

    fn register_for(&mut self, request: &MmioRequest) -> Result<&mut MmioRegister, MmioError> {
        let range = request.range();
        let Some(register) = self
            .registers
            .values_mut()
            .find(|register| register.range.contains(range.start()))
        else {
            return Err(MmioError::UnmappedAddress {
                address: range.start(),
            });
        };

        if !register.range.contains_range(range) {
            return Err(MmioError::RegisterBoundaryCrossed {
                request: request.id(),
                register_start: register.range.start(),
                register_end: register.range.end(),
                requested_start: range.start(),
                requested_end: range.end(),
            });
        }

        Ok(register)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MmioRegister {
    range: AddressRange,
    access: MmioAccess,
    value: Vec<u8>,
}

impl MmioRegister {
    fn read(&self, range: AddressRange) -> Result<Vec<u8>, MmioError> {
        let offset = self.offset(range.start())?;
        let size = range.size().bytes() as usize;
        Ok(self.value[offset..offset + size].to_vec())
    }

    fn write(&mut self, request: &MmioRequest) -> Result<(), MmioError> {
        let payload = request.data().ok_or(MmioError::MissingWriteData {
            request: request.id(),
        })?;
        if payload.len() as u64 != request.size().bytes() {
            return Err(MmioError::PayloadSizeMismatch {
                request: request.id(),
                expected: request.size().bytes(),
                actual: payload.len() as u64,
            });
        }
        let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
            request: request.id(),
        })?;
        if mask.len() != request.size().bytes() {
            return Err(MmioError::ByteMaskSizeMismatch {
                request: request.id(),
                expected: request.size().bytes(),
                actual: mask.len(),
            });
        }

        let offset = self.offset(request.range().start())?;
        for (index, byte) in payload.iter().enumerate() {
            if mask.bits()[index] {
                self.value[offset + index] = *byte;
            }
        }
        Ok(())
    }

    fn offset(&self, address: Address) -> Result<usize, MmioError> {
        (address.get() - self.range.start().get())
            .try_into()
            .map_err(|_| MmioError::HostOffsetTooLarge { address })
    }
}

pub trait MmioDevice: Send + Sync {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError>;
}

impl<T> MmioDevice for Arc<T>
where
    T: MmioDevice + ?Sized,
{
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        (**self).respond(context, request)
    }
}

impl MmioDevice for Mutex<MmioRegisterBank> {
    fn respond(
        &self,
        _context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.lock()
            .expect("mmio register bank device lock")
            .respond(request)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MmioRouteLatency {
    Request,
    Response,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MmioRoute {
    source_partition: PartitionId,
    target_partition: PartitionId,
    request_latency: Tick,
    response_latency: Tick,
}

impl MmioRoute {
    pub const fn new(
        source_partition: PartitionId,
        target_partition: PartitionId,
        request_latency: Tick,
        response_latency: Tick,
    ) -> Result<Self, MmioError> {
        if request_latency == 0 {
            return Err(MmioError::ZeroRouteLatency {
                latency: MmioRouteLatency::Request,
            });
        }
        if response_latency == 0 {
            return Err(MmioError::ZeroRouteLatency {
                latency: MmioRouteLatency::Response,
            });
        }

        Ok(Self {
            source_partition,
            target_partition,
            request_latency,
            response_latency,
        })
    }

    pub const fn source_partition(self) -> PartitionId {
        self.source_partition
    }

    pub const fn target_partition(self) -> PartitionId {
        self.target_partition
    }

    pub const fn request_latency(self) -> Tick {
        self.request_latency
    }

    pub const fn response_latency(self) -> Tick {
        self.response_latency
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MmioDelivery {
    tick: Tick,
    route: MmioRoute,
    request: MmioRequest,
}

impl MmioDelivery {
    pub const fn new(tick: Tick, route: MmioRoute, request: MmioRequest) -> Self {
        Self {
            tick,
            route,
            request,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn route(&self) -> MmioRoute {
        self.route
    }

    pub const fn request(&self) -> &MmioRequest {
        &self.request
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MmioCompletion {
    tick: Tick,
    route: MmioRoute,
    response: Result<MmioResponse, MmioError>,
}

impl MmioCompletion {
    pub const fn new(
        tick: Tick,
        route: MmioRoute,
        response: Result<MmioResponse, MmioError>,
    ) -> Self {
        Self {
            tick,
            route,
            response,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn route(&self) -> MmioRoute {
        self.route
    }

    pub const fn response(&self) -> &Result<MmioResponse, MmioError> {
        &self.response
    }
}

#[derive(Clone, Debug)]
pub struct MmioChannel {
    route: MmioRoute,
    response_errors: Arc<Mutex<Vec<MmioError>>>,
}

impl MmioChannel {
    pub fn new(route: MmioRoute) -> Self {
        Self {
            route,
            response_errors: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub const fn route(&self) -> MmioRoute {
        self.route
    }

    pub fn response_errors(&self) -> Arc<Mutex<Vec<MmioError>>> {
        Arc::clone(&self.response_errors)
    }

    pub fn submit<F, G>(
        &self,
        context: &mut SchedulerContext<'_>,
        request: MmioRequest,
        responder: F,
        completion_sink: G,
    ) -> Result<PartitionEventId, MmioError>
    where
        F: FnOnce(MmioDelivery, &mut SchedulerContext<'_>) -> Result<MmioResponse, MmioError>
            + Send
            + 'static,
        G: FnOnce(MmioCompletion) + Send + 'static,
    {
        let route = self.route;
        let response_errors = Arc::clone(&self.response_errors);
        context
            .schedule_remote_after(
                route.target_partition(),
                route.request_latency(),
                move |context| {
                    let response =
                        responder(MmioDelivery::new(context.now(), route, request), context);
                    if let Err(error) = context.schedule_remote_after(
                        route.source_partition(),
                        route.response_latency(),
                        move |context| {
                            completion_sink(MmioCompletion::new(context.now(), route, response));
                        },
                    ) {
                        response_errors
                            .lock()
                            .expect("mmio response error lock")
                            .push(MmioError::Scheduler(error));
                    }
                },
            )
            .map_err(MmioError::Scheduler)
    }

    pub fn submit_parallel<F, G>(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: MmioRequest,
        responder: F,
        completion_sink: G,
    ) -> Result<PartitionEventId, MmioError>
    where
        F: FnOnce(
                MmioDelivery,
                &mut ParallelSchedulerContext<'_>,
            ) -> Result<MmioResponse, MmioError>
            + Send
            + 'static,
        G: FnOnce(MmioCompletion) + Send + 'static,
    {
        let route = self.route;
        let response_errors = Arc::clone(&self.response_errors);
        context
            .schedule_remote_after(
                route.target_partition(),
                route.request_latency(),
                move |context| {
                    let response =
                        responder(MmioDelivery::new(context.now(), route, request), context);
                    if let Err(error) = context.schedule_remote_after(
                        route.source_partition(),
                        route.response_latency(),
                        move |context| {
                            completion_sink(MmioCompletion::new(context.now(), route, response));
                        },
                    ) {
                        response_errors
                            .lock()
                            .expect("mmio response error lock")
                            .push(MmioError::Scheduler(error));
                    }
                },
            )
            .map_err(MmioError::Scheduler)
    }
}

#[derive(Clone)]
pub struct MmioBus {
    devices: Vec<MmioDeviceEntry>,
}

impl MmioBus {
    pub const fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    pub fn insert_device<F>(
        &mut self,
        range: AddressRange,
        route: MmioRoute,
        device: F,
    ) -> Result<(), MmioError>
    where
        F: MmioDevice + 'static,
    {
        if let Some(existing) = self
            .devices
            .iter()
            .find(|device| device.range.overlaps(range))
        {
            return Err(MmioError::OverlappingDeviceRegion {
                existing_start: existing.range.start(),
                existing_end: existing.range.end(),
                requested_start: range.start(),
                requested_end: range.end(),
            });
        }

        self.devices.push(MmioDeviceEntry {
            range,
            channel: MmioChannel::new(route),
            device: Arc::new(device),
        });
        self.devices
            .sort_by_key(|device| device.range.start().get());
        Ok(())
    }

    pub fn route_for(&self, request: &MmioRequest) -> Result<MmioRoute, MmioError> {
        Ok(self.device_for(request)?.channel.route())
    }

    pub fn response_errors(&self) -> Vec<MmioError> {
        let mut errors = Vec::new();
        for device in &self.devices {
            errors.extend(
                device
                    .channel
                    .response_errors()
                    .lock()
                    .expect("mmio bus response error lock")
                    .iter()
                    .cloned(),
            );
        }
        errors
    }

    pub fn submit<G>(
        &self,
        context: &mut SchedulerContext<'_>,
        request: MmioRequest,
        completion_sink: G,
    ) -> Result<PartitionEventId, MmioError>
    where
        G: FnOnce(MmioCompletion) + Send + 'static,
    {
        let device = self.device_for(&request)?.clone();
        let responder = Arc::clone(&device.device);
        device.channel.submit(
            context,
            request,
            move |delivery, context| responder.respond(context, delivery.request()),
            completion_sink,
        )
    }

    fn device_for(&self, request: &MmioRequest) -> Result<&MmioDeviceEntry, MmioError> {
        let requested = request.range();
        for device in &self.devices {
            if device.range.contains(requested.start()) {
                if !device.range.contains_range(requested) {
                    return Err(MmioError::DeviceBoundaryCrossed {
                        request: request.id(),
                        device_start: device.range.start(),
                        device_end: device.range.end(),
                        requested_start: requested.start(),
                        requested_end: requested.end(),
                    });
                }
                return Ok(device);
            }
        }

        Err(MmioError::UnmappedAddress {
            address: requested.start(),
        })
    }
}

impl Default for MmioBus {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
struct MmioDeviceEntry {
    range: AddressRange,
    channel: MmioChannel,
    device: Arc<dyn MmioDevice>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MmioError {
    Memory(MemoryError),
    AddressOverflow {
        address: Address,
        offset: u64,
    },
    ResetValueSizeMismatch {
        expected: u64,
        actual: u64,
    },
    RegisterOutOfRange {
        register_start: Address,
        register_end: Address,
        bank_start: Address,
        bank_end: Address,
    },
    OverlappingRegister {
        existing_start: Address,
        existing_end: Address,
        requested_start: Address,
        requested_end: Address,
    },
    OverlappingDeviceRegion {
        existing_start: Address,
        existing_end: Address,
        requested_start: Address,
        requested_end: Address,
    },
    UnmappedAddress {
        address: Address,
    },
    RegisterBoundaryCrossed {
        request: MmioRequestId,
        register_start: Address,
        register_end: Address,
        requested_start: Address,
        requested_end: Address,
    },
    DeviceBoundaryCrossed {
        request: MmioRequestId,
        device_start: Address,
        device_end: Address,
        requested_start: Address,
        requested_end: Address,
    },
    AccessDenied {
        request: MmioRequestId,
        operation: MmioOperation,
        access: MmioAccess,
    },
    MissingWriteData {
        request: MmioRequestId,
    },
    MissingByteMask {
        request: MmioRequestId,
    },
    PayloadSizeMismatch {
        request: MmioRequestId,
        expected: u64,
        actual: u64,
    },
    ByteMaskSizeMismatch {
        request: MmioRequestId,
        expected: u64,
        actual: u64,
    },
    AccessSizeMismatch {
        request: MmioRequestId,
        expected: u64,
        actual: u64,
    },
    HostOffsetTooLarge {
        address: Address,
    },
    DeviceError {
        request: MmioRequestId,
        message: String,
    },
    ZeroRouteLatency {
        latency: MmioRouteLatency,
    },
    Scheduler(SchedulerError),
}

impl fmt::Display for MmioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::AddressOverflow { address, offset } => {
                write!(
                    formatter,
                    "address {:#x} overflows when adding offset {offset}",
                    address.get()
                )
            }
            Self::ResetValueSizeMismatch { expected, actual } => write!(
                formatter,
                "reset value has {actual} bytes but register expects {expected}"
            ),
            Self::RegisterOutOfRange {
                register_start,
                register_end,
                bank_start,
                bank_end,
            } => write!(
                formatter,
                "register {:#x}..{:#x} is outside bank {:#x}..{:#x}",
                register_start.get(),
                register_end.get(),
                bank_start.get(),
                bank_end.get()
            ),
            Self::OverlappingRegister {
                existing_start,
                existing_end,
                requested_start,
                requested_end,
            } => write!(
                formatter,
                "register {:#x}..{:#x} overlaps existing register {:#x}..{:#x}",
                requested_start.get(),
                requested_end.get(),
                existing_start.get(),
                existing_end.get()
            ),
            Self::OverlappingDeviceRegion {
                existing_start,
                existing_end,
                requested_start,
                requested_end,
            } => write!(
                formatter,
                "MMIO device region {:#x}..{:#x} overlaps existing region {:#x}..{:#x}",
                requested_start.get(),
                requested_end.get(),
                existing_start.get(),
                existing_end.get()
            ),
            Self::UnmappedAddress { address } => {
                write!(formatter, "MMIO address {:#x} is not mapped", address.get())
            }
            Self::RegisterBoundaryCrossed {
                request,
                register_start,
                register_end,
                requested_start,
                requested_end,
            } => write!(
                formatter,
                "MMIO request {} crosses register {:#x}..{:#x} with access {:#x}..{:#x}",
                request.get(),
                register_start.get(),
                register_end.get(),
                requested_start.get(),
                requested_end.get()
            ),
            Self::DeviceBoundaryCrossed {
                request,
                device_start,
                device_end,
                requested_start,
                requested_end,
            } => write!(
                formatter,
                "MMIO request {} crosses device region {:#x}..{:#x} with access {:#x}..{:#x}",
                request.get(),
                device_start.get(),
                device_end.get(),
                requested_start.get(),
                requested_end.get()
            ),
            Self::AccessDenied {
                request,
                operation,
                access,
            } => write!(
                formatter,
                "MMIO request {} {operation:?} is not allowed for {access:?}",
                request.get()
            ),
            Self::MissingWriteData { request } => {
                write!(
                    formatter,
                    "MMIO write request {} has no payload",
                    request.get()
                )
            }
            Self::MissingByteMask { request } => {
                write!(
                    formatter,
                    "MMIO write request {} has no byte mask",
                    request.get()
                )
            }
            Self::PayloadSizeMismatch {
                request,
                expected,
                actual,
            } => write!(
                formatter,
                "MMIO request {} payload has {actual} bytes but expects {expected}",
                request.get()
            ),
            Self::ByteMaskSizeMismatch {
                request,
                expected,
                actual,
            } => write!(
                formatter,
                "MMIO request {} byte mask has {actual} bits but expects {expected}",
                request.get()
            ),
            Self::AccessSizeMismatch {
                request,
                expected,
                actual,
            } => write!(
                formatter,
                "MMIO request {} has {actual} bytes but expects {expected}",
                request.get()
            ),
            Self::HostOffsetTooLarge { address } => write!(
                formatter,
                "MMIO address {:#x} register offset does not fit host usize",
                address.get()
            ),
            Self::DeviceError { request, message } => {
                write!(
                    formatter,
                    "MMIO request {} device error: {message}",
                    request.get()
                )
            }
            Self::ZeroRouteLatency { latency } => {
                write!(formatter, "{latency:?} MMIO route latency must be positive")
            }
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for MmioError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            _ => None,
        }
    }
}
