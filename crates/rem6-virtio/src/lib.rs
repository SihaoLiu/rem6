use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, SchedulerContext, Tick};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{
    MmioAccess, MmioDevice, MmioError, MmioOperation, MmioRequest, MmioRequestId, MmioResponse,
};

mod device_config;
mod isr;
mod shared_memory;

pub use device_config::{
    VirtioPciDeviceConfigAccess, VirtioPciDeviceConfigDevice, VirtioPciDeviceConfigSnapshot,
    VirtioPciDeviceConfigSpec,
};
pub use isr::{
    VirtioPciIsrDevice, VirtioPciIsrEvent, VirtioPciIsrEventKind, VirtioPciIsrSnapshot,
    VirtioPciIsrStatus, VIRTIO_PCI_ISR_STATUS_SIZE,
};
pub use shared_memory::{
    VirtioPciBarIndex, VirtioPciSharedMemoryCap64Fields, VirtioPciSharedMemoryId,
    VirtioPciSharedMemoryRegion, VirtioPciSharedMemoryRegionSpec, VirtioPciSharedMemoryRegistry,
};

pub const VIRTIO_PCI_COMMON_CONFIG_SIZE: u64 = 0x40;
pub const VIRTIO_PCI_DEVICE_FEATURE_SELECT_OFFSET: u64 = 0x00;
pub const VIRTIO_PCI_DEVICE_FEATURE_OFFSET: u64 = 0x04;
pub const VIRTIO_PCI_DRIVER_FEATURE_SELECT_OFFSET: u64 = 0x08;
pub const VIRTIO_PCI_DRIVER_FEATURE_OFFSET: u64 = 0x0c;
pub const VIRTIO_PCI_CONFIG_MSIX_VECTOR_OFFSET: u64 = 0x10;
pub const VIRTIO_PCI_NUM_QUEUES_OFFSET: u64 = 0x12;
pub const VIRTIO_PCI_DEVICE_STATUS_OFFSET: u64 = 0x14;
pub const VIRTIO_PCI_CONFIG_GENERATION_OFFSET: u64 = 0x15;
pub const VIRTIO_PCI_QUEUE_SELECT_OFFSET: u64 = 0x16;
pub const VIRTIO_PCI_QUEUE_SIZE_OFFSET: u64 = 0x18;
pub const VIRTIO_PCI_QUEUE_MSIX_VECTOR_OFFSET: u64 = 0x1a;
pub const VIRTIO_PCI_QUEUE_ENABLE_OFFSET: u64 = 0x1c;
pub const VIRTIO_PCI_QUEUE_NOTIFY_OFF_OFFSET: u64 = 0x1e;
pub const VIRTIO_PCI_QUEUE_DESC_OFFSET: u64 = 0x20;
pub const VIRTIO_PCI_QUEUE_DRIVER_OFFSET: u64 = 0x28;
pub const VIRTIO_PCI_QUEUE_DEVICE_OFFSET: u64 = 0x30;
pub const VIRTIO_PCI_QUEUE_NOTIF_CONFIG_DATA_OFFSET: u64 = 0x38;
pub const VIRTIO_PCI_QUEUE_RESET_OFFSET: u64 = 0x3a;
pub const VIRTIO_PCI_ADMIN_QUEUE_INDEX_OFFSET: u64 = 0x3c;
pub const VIRTIO_PCI_ADMIN_QUEUE_NUM_OFFSET: u64 = 0x3e;

pub const VIRTIO_STATUS_ACKNOWLEDGE: u8 = 0x01;
pub const VIRTIO_STATUS_DRIVER: u8 = 0x02;
pub const VIRTIO_STATUS_FEATURES_OK: u8 = 0x08;
pub const VIRTIO_STATUS_DRIVER_OK: u8 = 0x04;
pub const VIRTIO_STATUS_DEVICE_NEEDS_RESET: u8 = 0x40;
pub const VIRTIO_STATUS_FAILED: u8 = 0x80;

const VIRTIO_MSI_NO_VECTOR: u16 = 0xffff;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VirtioQueueIndex(u16);

impl VirtioQueueIndex {
    pub const fn new(value: u16) -> Option<Self> {
        Some(Self(value))
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioQueueSpec {
    size: u16,
    notify_offset: u16,
    notify_config_data: u16,
}

impl VirtioQueueSpec {
    pub const fn available(size: u16, notify_offset: u16) -> Self {
        Self {
            size,
            notify_offset,
            notify_config_data: notify_offset,
        }
    }

    pub const fn with_notify_config_data(mut self, notify_config_data: u16) -> Self {
        self.notify_config_data = notify_config_data;
        self
    }

    pub const fn size(self) -> u16 {
        self.size
    }

    pub const fn notify_offset(self) -> u16 {
        self.notify_offset
    }

    pub const fn notify_config_data(self) -> u16 {
        self.notify_config_data
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioQueueNotifySpec {
    queue: VirtioQueueIndex,
    notify_offset: u16,
}

impl VirtioQueueNotifySpec {
    pub const fn new(queue: VirtioQueueIndex, notify_offset: u16) -> Self {
        Self {
            queue,
            notify_offset,
        }
    }

    pub const fn queue(self) -> VirtioQueueIndex {
        self.queue
    }

    pub const fn notify_offset(self) -> u16 {
        self.notify_offset
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioQueueNotification {
    tick: Tick,
    queue: VirtioQueueIndex,
    value: u16,
    address: Address,
}

impl VirtioQueueNotification {
    pub const fn new(tick: Tick, queue: VirtioQueueIndex, value: u16, address: Address) -> Self {
        Self {
            tick,
            queue,
            value,
            address,
        }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn queue(self) -> VirtioQueueIndex {
        self.queue
    }

    pub const fn value(self) -> u16 {
        self.value
    }

    pub const fn address(self) -> Address {
        self.address
    }
}

#[derive(Clone, Debug)]
pub struct VirtioPciNotifyDevice {
    range: AddressRange,
    slots: Arc<Vec<VirtioNotifySlot>>,
    notifications: Arc<Mutex<Vec<VirtioQueueNotification>>>,
}

impl VirtioPciNotifyDevice {
    pub fn new(
        notify_off_multiplier: u32,
        queues: impl IntoIterator<Item = VirtioQueueNotifySpec>,
    ) -> Result<Self, VirtioError> {
        validate_notify_multiplier(notify_off_multiplier)?;
        let slots = queues
            .into_iter()
            .map(|spec| VirtioNotifySlot::new(notify_off_multiplier, spec))
            .collect::<Result<Vec<_>, _>>()?;
        if slots.is_empty() {
            return Err(VirtioError::NoNotifyQueues);
        }
        for (index, slot) in slots.iter().enumerate() {
            if slots
                .iter()
                .skip(index + 1)
                .any(|other| other.queue == slot.queue)
            {
                return Err(VirtioError::DuplicateNotifyQueue {
                    index: slot.queue.get(),
                });
            }
        }

        let length = slots
            .iter()
            .map(|slot| slot.address.get() + 2)
            .max()
            .unwrap_or(2);
        Ok(Self {
            range: AddressRange::new(Address::new(0), AccessSize::new(length).unwrap()).unwrap(),
            slots: Arc::new(slots),
            notifications: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub const fn range(&self) -> AddressRange {
        self.range
    }

    pub fn notifications(&self) -> Vec<VirtioQueueNotification> {
        self.notifications
            .lock()
            .expect("virtio notify notification lock")
            .clone()
    }

    pub fn snapshot(&self) -> VirtioPciNotifySnapshot {
        VirtioPciNotifySnapshot {
            notifications: self.notifications(),
        }
    }

    pub fn restore(&self, snapshot: &VirtioPciNotifySnapshot) {
        *self
            .notifications
            .lock()
            .expect("virtio notify notification lock") = snapshot.notifications.clone();
    }

    pub fn read_local(&self, address: Address, size: AccessSize) -> Result<Vec<u8>, MmioError> {
        self.read_at(MmioRequestId::new(0), address, size)
    }

    pub fn write_local(
        &self,
        address: Address,
        data: Vec<u8>,
        byte_mask: ByteMask,
        tick: Tick,
    ) -> Result<(), MmioError> {
        let size = AccessSize::new(data.len() as u64).map_err(MmioError::Memory)?;
        self.write_at(
            MmioRequestId::new(0),
            address,
            size,
            &data,
            &byte_mask,
            tick,
        )
    }

    fn read_at(
        &self,
        request: MmioRequestId,
        address: Address,
        size: AccessSize,
    ) -> Result<Vec<u8>, MmioError> {
        self.validate_access_range(request, address, size)?;
        Err(MmioError::AccessDenied {
            request,
            operation: MmioOperation::Read,
            access: MmioAccess::WriteOnly,
        })
    }

    fn write_at(
        &self,
        request: MmioRequestId,
        address: Address,
        size: AccessSize,
        data: &[u8],
        byte_mask: &ByteMask,
        tick: Tick,
    ) -> Result<(), MmioError> {
        self.validate_access_range(request, address, size)?;
        if size.bytes() != 2 {
            return Err(MmioError::AccessSizeMismatch {
                request,
                expected: 2,
                actual: size.bytes(),
            });
        }
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
        if !byte_mask.bits().iter().any(|bit| *bit) {
            return Ok(());
        }
        if !byte_mask.bits().iter().all(|bit| *bit) {
            return Err(virtio_device_error(
                request,
                VirtioError::PartialNotifyWrite,
            ));
        }

        let value = le_u16(data);
        let matching_address = self
            .slots
            .iter()
            .filter(|slot| slot.address == address)
            .collect::<Vec<_>>();
        if matching_address.is_empty() {
            return Err(virtio_device_error(
                request,
                VirtioError::NoQueueForNotifyAddress { address },
            ));
        }
        let Some(slot) = matching_address
            .iter()
            .find(|slot| slot.queue.get() == value)
            .copied()
        else {
            return Err(virtio_device_error(
                request,
                VirtioError::NotifyValueMismatch { address, value },
            ));
        };

        self.notifications
            .lock()
            .expect("virtio notify notification lock")
            .push(VirtioQueueNotification::new(
                tick, slot.queue, value, address,
            ));
        Ok(())
    }

    fn validate_access_range(
        &self,
        request: MmioRequestId,
        address: Address,
        size: AccessSize,
    ) -> Result<(), MmioError> {
        let requested = AddressRange::new(address, size).map_err(MmioError::Memory)?;
        if !self.range.contains_range(requested) {
            return Err(MmioError::DeviceBoundaryCrossed {
                request,
                device_start: self.range.start(),
                device_end: self.range.end(),
                requested_start: requested.start(),
                requested_end: requested.end(),
            });
        }
        Ok(())
    }
}

impl MmioDevice for VirtioPciNotifyDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        match request.operation() {
            MmioOperation::Read => self
                .read_at(request.id(), request.range().start(), request.size())
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
                    context.now(),
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
                .read_at(request.id(), request.range().start(), request.size())
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
                    context.now(),
                )?;
                Ok(MmioResponse::completed(request.id(), None))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioPciNotifySnapshot {
    notifications: Vec<VirtioQueueNotification>,
}

impl VirtioPciNotifySnapshot {
    pub fn notifications(&self) -> &[VirtioQueueNotification] {
        &self.notifications
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VirtioNotifySlot {
    queue: VirtioQueueIndex,
    address: Address,
}

impl VirtioNotifySlot {
    fn new(notify_off_multiplier: u32, spec: VirtioQueueNotifySpec) -> Result<Self, VirtioError> {
        let address = if notify_off_multiplier == 0 {
            Address::new(0)
        } else {
            let offset = u64::from(spec.notify_offset)
                .checked_mul(u64::from(notify_off_multiplier))
                .ok_or(VirtioError::NotifyAddressOverflow {
                    queue: spec.queue.get(),
                    notify_offset: spec.notify_offset,
                    notify_off_multiplier,
                })?;
            Address::new(offset)
        };
        Ok(Self {
            queue: spec.queue,
            address,
        })
    }
}

fn validate_notify_multiplier(multiplier: u32) -> Result<(), VirtioError> {
    if multiplier == 0 || (multiplier.is_power_of_two() && multiplier.is_multiple_of(2)) {
        return Ok(());
    }
    Err(VirtioError::InvalidNotifyMultiplier { multiplier })
}

#[derive(Clone, Debug)]
pub struct VirtioPciCommonConfigDevice {
    state: Arc<Mutex<VirtioPciCommonState>>,
}

impl VirtioPciCommonConfigDevice {
    pub fn new(
        device_features: impl IntoIterator<Item = (u32, u32)>,
        queues: impl IntoIterator<Item = VirtioQueueSpec>,
    ) -> Result<Self, VirtioError> {
        Ok(Self {
            state: Arc::new(Mutex::new(VirtioPciCommonState::new(
                device_features,
                queues,
            )?)),
        })
    }

    pub fn range(&self) -> AddressRange {
        AddressRange::new(
            Address::new(0),
            AccessSize::new(VIRTIO_PCI_COMMON_CONFIG_SIZE).unwrap(),
        )
        .unwrap()
    }

    pub fn read_local(&self, address: Address, size: AccessSize) -> Result<Vec<u8>, MmioError> {
        self.read_at(MmioRequestId::new(0), address, size)
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

    pub fn snapshot(&self) -> VirtioPciCommonSnapshot {
        VirtioPciCommonSnapshot {
            state: self
                .state
                .lock()
                .expect("virtio common config lock")
                .clone(),
        }
    }

    pub fn restore(&self, snapshot: &VirtioPciCommonSnapshot) {
        *self.state.lock().expect("virtio common config lock") = snapshot.state.clone();
    }

    fn read_at(
        &self,
        request: MmioRequestId,
        address: Address,
        size: AccessSize,
    ) -> Result<Vec<u8>, MmioError> {
        let register = register_for(request, address, size)?;
        let state = self.state.lock().expect("virtio common config lock");
        Ok(register.read(&state))
    }

    fn write_at(
        &self,
        request: MmioRequestId,
        address: Address,
        size: AccessSize,
        data: &[u8],
        byte_mask: &ByteMask,
    ) -> Result<(), MmioError> {
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
        if !byte_mask.bits().iter().any(|bit| *bit) {
            return Ok(());
        }

        let register = register_for(request, address, size)?;
        if register.access() == MmioAccess::ReadOnly {
            return Err(MmioError::AccessDenied {
                request,
                operation: MmioOperation::Write,
                access: MmioAccess::ReadOnly,
            });
        }

        let mut state = self.state.lock().expect("virtio common config lock");
        let mut merged = register.read(&state);
        for (index, byte) in data.iter().enumerate() {
            if byte_mask.bits()[index] {
                merged[index] = *byte;
            }
        }
        register.write(request, &mut state, &merged)
    }
}

impl MmioDevice for VirtioPciCommonConfigDevice {
    fn respond(
        &self,
        _context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        match request.operation() {
            MmioOperation::Read => self
                .read_at(request.id(), request.range().start(), request.size())
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
        _context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        match request.operation() {
            MmioOperation::Read => self
                .read_at(request.id(), request.range().start(), request.size())
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
pub struct VirtioPciCommonSnapshot {
    state: VirtioPciCommonState,
}

impl VirtioPciCommonSnapshot {
    pub const fn device_status(&self) -> u8 {
        self.state.device_status
    }

    pub fn driver_feature_page(&self, page: u32) -> u32 {
        self.state.driver_features.get(&page).copied().unwrap_or(0)
    }

    pub fn queue(&self, index: VirtioQueueIndex) -> Option<VirtioQueueSnapshot> {
        self.state
            .queues
            .get(index.get() as usize)
            .copied()
            .map(VirtioQueueSnapshot)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioQueueSnapshot(VirtioQueueState);

impl VirtioQueueSnapshot {
    pub const fn max_size(self) -> u16 {
        self.0.max_size
    }

    pub const fn size(self) -> u16 {
        self.0.size
    }

    pub const fn enabled(self) -> bool {
        self.0.enabled
    }

    pub const fn notify_offset(self) -> u16 {
        self.0.notify_offset
    }

    pub const fn desc_address(self) -> u64 {
        self.0.desc_address
    }

    pub const fn driver_address(self) -> u64 {
        self.0.driver_address
    }

    pub const fn device_address(self) -> u64 {
        self.0.device_address
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VirtioPciCommonState {
    device_features: BTreeMap<u32, u32>,
    driver_features: BTreeMap<u32, u32>,
    device_feature_select: u32,
    driver_feature_select: u32,
    config_msix_vector: u16,
    device_status: u8,
    config_generation: u8,
    queue_select: u16,
    queues: Vec<VirtioQueueState>,
    admin_queue_index: u16,
    admin_queue_num: u16,
}

impl VirtioPciCommonState {
    fn new(
        device_features: impl IntoIterator<Item = (u32, u32)>,
        queues: impl IntoIterator<Item = VirtioQueueSpec>,
    ) -> Result<Self, VirtioError> {
        let queues = queues
            .into_iter()
            .enumerate()
            .map(|(index, queue)| VirtioQueueState::new(index, queue))
            .collect::<Result<Vec<_>, _>>()?;
        if queues.len() > u16::MAX as usize {
            return Err(VirtioError::TooManyQueues {
                count: queues.len(),
            });
        }

        Ok(Self {
            device_features: device_features.into_iter().collect(),
            driver_features: BTreeMap::new(),
            device_feature_select: 0,
            driver_feature_select: 0,
            config_msix_vector: VIRTIO_MSI_NO_VECTOR,
            device_status: 0,
            config_generation: 0,
            queue_select: 0,
            queues,
            admin_queue_index: 0,
            admin_queue_num: 0,
        })
    }

    fn reset_device(&mut self) {
        self.driver_features.clear();
        self.device_feature_select = 0;
        self.driver_feature_select = 0;
        self.config_msix_vector = VIRTIO_MSI_NO_VECTOR;
        self.device_status = 0;
        self.config_generation = self.config_generation.wrapping_add(1);
        self.queue_select = 0;
        for queue in &mut self.queues {
            queue.reset();
        }
    }

    fn selected_queue(&self) -> Option<&VirtioQueueState> {
        self.queues.get(self.queue_select as usize)
    }

    fn selected_queue_mut(
        &mut self,
        request: MmioRequestId,
    ) -> Result<&mut VirtioQueueState, MmioError> {
        let index = self.queue_select;
        self.queues
            .get_mut(index as usize)
            .ok_or_else(|| virtio_device_error(request, VirtioError::UnavailableQueue { index }))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VirtioQueueState {
    max_size: u16,
    size: u16,
    notify_offset: u16,
    notify_config_data: u16,
    msix_vector: u16,
    enabled: bool,
    desc_address: u64,
    driver_address: u64,
    device_address: u64,
}

impl VirtioQueueState {
    fn new(index: usize, spec: VirtioQueueSpec) -> Result<Self, VirtioError> {
        validate_queue_size(index as u16, spec.size)?;
        Ok(Self {
            max_size: spec.size,
            size: spec.size,
            notify_offset: spec.notify_offset,
            notify_config_data: spec.notify_config_data,
            msix_vector: VIRTIO_MSI_NO_VECTOR,
            enabled: false,
            desc_address: 0,
            driver_address: 0,
            device_address: 0,
        })
    }

    fn reset(&mut self) {
        self.size = self.max_size;
        self.msix_vector = VIRTIO_MSI_NO_VECTOR;
        self.enabled = false;
        self.desc_address = 0;
        self.driver_address = 0;
        self.device_address = 0;
    }
}

fn validate_queue_size(index: u16, size: u16) -> Result<(), VirtioError> {
    if size == 0 || !size.is_power_of_two() {
        return Err(VirtioError::InvalidQueueSize { index, size });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommonRegister {
    DeviceFeatureSelect,
    DeviceFeature,
    DriverFeatureSelect,
    DriverFeature,
    ConfigMsixVector,
    NumQueues,
    DeviceStatus,
    ConfigGeneration,
    QueueSelect,
    QueueSize,
    QueueMsixVector,
    QueueEnable,
    QueueNotifyOff,
    QueueDesc,
    QueueDriver,
    QueueDevice,
    QueueNotifConfigData,
    QueueReset,
    AdminQueueIndex,
    AdminQueueNum,
}

impl CommonRegister {
    const fn offset(self) -> u64 {
        match self {
            Self::DeviceFeatureSelect => VIRTIO_PCI_DEVICE_FEATURE_SELECT_OFFSET,
            Self::DeviceFeature => VIRTIO_PCI_DEVICE_FEATURE_OFFSET,
            Self::DriverFeatureSelect => VIRTIO_PCI_DRIVER_FEATURE_SELECT_OFFSET,
            Self::DriverFeature => VIRTIO_PCI_DRIVER_FEATURE_OFFSET,
            Self::ConfigMsixVector => VIRTIO_PCI_CONFIG_MSIX_VECTOR_OFFSET,
            Self::NumQueues => VIRTIO_PCI_NUM_QUEUES_OFFSET,
            Self::DeviceStatus => VIRTIO_PCI_DEVICE_STATUS_OFFSET,
            Self::ConfigGeneration => VIRTIO_PCI_CONFIG_GENERATION_OFFSET,
            Self::QueueSelect => VIRTIO_PCI_QUEUE_SELECT_OFFSET,
            Self::QueueSize => VIRTIO_PCI_QUEUE_SIZE_OFFSET,
            Self::QueueMsixVector => VIRTIO_PCI_QUEUE_MSIX_VECTOR_OFFSET,
            Self::QueueEnable => VIRTIO_PCI_QUEUE_ENABLE_OFFSET,
            Self::QueueNotifyOff => VIRTIO_PCI_QUEUE_NOTIFY_OFF_OFFSET,
            Self::QueueDesc => VIRTIO_PCI_QUEUE_DESC_OFFSET,
            Self::QueueDriver => VIRTIO_PCI_QUEUE_DRIVER_OFFSET,
            Self::QueueDevice => VIRTIO_PCI_QUEUE_DEVICE_OFFSET,
            Self::QueueNotifConfigData => VIRTIO_PCI_QUEUE_NOTIF_CONFIG_DATA_OFFSET,
            Self::QueueReset => VIRTIO_PCI_QUEUE_RESET_OFFSET,
            Self::AdminQueueIndex => VIRTIO_PCI_ADMIN_QUEUE_INDEX_OFFSET,
            Self::AdminQueueNum => VIRTIO_PCI_ADMIN_QUEUE_NUM_OFFSET,
        }
    }

    const fn size(self) -> u64 {
        match self {
            Self::DeviceStatus | Self::ConfigGeneration => 1,
            Self::ConfigMsixVector
            | Self::NumQueues
            | Self::QueueSelect
            | Self::QueueSize
            | Self::QueueMsixVector
            | Self::QueueEnable
            | Self::QueueNotifyOff
            | Self::QueueNotifConfigData
            | Self::QueueReset
            | Self::AdminQueueIndex
            | Self::AdminQueueNum => 2,
            Self::DeviceFeatureSelect
            | Self::DeviceFeature
            | Self::DriverFeatureSelect
            | Self::DriverFeature => 4,
            Self::QueueDesc | Self::QueueDriver | Self::QueueDevice => 8,
        }
    }

    const fn access(self) -> MmioAccess {
        match self {
            Self::DeviceFeature
            | Self::NumQueues
            | Self::ConfigGeneration
            | Self::QueueNotifyOff
            | Self::QueueNotifConfigData
            | Self::AdminQueueIndex
            | Self::AdminQueueNum => MmioAccess::ReadOnly,
            _ => MmioAccess::ReadWrite,
        }
    }

    fn read(self, state: &VirtioPciCommonState) -> Vec<u8> {
        match self {
            Self::DeviceFeatureSelect => state.device_feature_select.to_le_bytes().to_vec(),
            Self::DeviceFeature => state
                .device_features
                .get(&state.device_feature_select)
                .copied()
                .unwrap_or(0)
                .to_le_bytes()
                .to_vec(),
            Self::DriverFeatureSelect => state.driver_feature_select.to_le_bytes().to_vec(),
            Self::DriverFeature => state
                .driver_features
                .get(&state.driver_feature_select)
                .copied()
                .unwrap_or(0)
                .to_le_bytes()
                .to_vec(),
            Self::ConfigMsixVector => state.config_msix_vector.to_le_bytes().to_vec(),
            Self::NumQueues => (state.queues.len() as u16).to_le_bytes().to_vec(),
            Self::DeviceStatus => vec![state.device_status],
            Self::ConfigGeneration => vec![state.config_generation],
            Self::QueueSelect => state.queue_select.to_le_bytes().to_vec(),
            Self::QueueSize => selected_queue_or_zero(state, |queue| queue.size),
            Self::QueueMsixVector => selected_queue_or_zero(state, |queue| queue.msix_vector),
            Self::QueueEnable => selected_queue_or_zero(state, |queue| u16::from(queue.enabled)),
            Self::QueueNotifyOff => selected_queue_or_zero(state, |queue| queue.notify_offset),
            Self::QueueDesc => selected_queue_or_zero_u64(state, |queue| queue.desc_address),
            Self::QueueDriver => selected_queue_or_zero_u64(state, |queue| queue.driver_address),
            Self::QueueDevice => selected_queue_or_zero_u64(state, |queue| queue.device_address),
            Self::QueueNotifConfigData => {
                selected_queue_or_zero(state, |queue| queue.notify_config_data)
            }
            Self::QueueReset => 0_u16.to_le_bytes().to_vec(),
            Self::AdminQueueIndex => state.admin_queue_index.to_le_bytes().to_vec(),
            Self::AdminQueueNum => state.admin_queue_num.to_le_bytes().to_vec(),
        }
    }

    fn write(
        self,
        request: MmioRequestId,
        state: &mut VirtioPciCommonState,
        bytes: &[u8],
    ) -> Result<(), MmioError> {
        match self {
            Self::DeviceFeatureSelect => {
                state.device_feature_select = le_u32(bytes);
                Ok(())
            }
            Self::DriverFeatureSelect => {
                state.driver_feature_select = le_u32(bytes);
                Ok(())
            }
            Self::DriverFeature => {
                state
                    .driver_features
                    .insert(state.driver_feature_select, le_u32(bytes));
                Ok(())
            }
            Self::ConfigMsixVector => {
                state.config_msix_vector = le_u16(bytes);
                Ok(())
            }
            Self::DeviceStatus => {
                let value = bytes[0];
                if value == 0 {
                    state.reset_device();
                } else {
                    state.device_status = value;
                }
                Ok(())
            }
            Self::QueueSelect => {
                state.queue_select = le_u16(bytes);
                Ok(())
            }
            Self::QueueSize => {
                let value = le_u16(bytes);
                let index = state.queue_select;
                let queue = state.selected_queue_mut(request)?;
                if queue.enabled {
                    return Err(virtio_device_error(
                        request,
                        VirtioError::EnabledQueueConfigWrite { index },
                    ));
                }
                if value == 0 || !value.is_power_of_two() || value > queue.max_size {
                    return Err(virtio_device_error(
                        request,
                        VirtioError::InvalidQueueRuntimeSize {
                            index,
                            size: value,
                            max_size: queue.max_size,
                        },
                    ));
                }
                queue.size = value;
                Ok(())
            }
            Self::QueueMsixVector => {
                state.selected_queue_mut(request)?.msix_vector = le_u16(bytes);
                Ok(())
            }
            Self::QueueEnable => {
                let value = le_u16(bytes);
                let queue = state.selected_queue_mut(request)?;
                match value {
                    1 => {
                        queue.enabled = true;
                        Ok(())
                    }
                    _ => Err(virtio_device_error(
                        request,
                        VirtioError::InvalidQueueEnable { value },
                    )),
                }
            }
            Self::QueueDesc => {
                state.selected_queue_mut(request)?.desc_address = le_u64(bytes);
                Ok(())
            }
            Self::QueueDriver => {
                state.selected_queue_mut(request)?.driver_address = le_u64(bytes);
                Ok(())
            }
            Self::QueueDevice => {
                state.selected_queue_mut(request)?.device_address = le_u64(bytes);
                Ok(())
            }
            Self::QueueReset => {
                let value = le_u16(bytes);
                match value {
                    0 => Ok(()),
                    1 => {
                        state.selected_queue_mut(request)?.reset();
                        Ok(())
                    }
                    _ => Err(virtio_device_error(
                        request,
                        VirtioError::InvalidQueueReset { value },
                    )),
                }
            }
            Self::DeviceFeature
            | Self::NumQueues
            | Self::ConfigGeneration
            | Self::QueueNotifyOff
            | Self::QueueNotifConfigData
            | Self::AdminQueueIndex
            | Self::AdminQueueNum => unreachable!("read-only register writes are rejected first"),
        }
    }
}

fn selected_queue_or_zero(
    state: &VirtioPciCommonState,
    accessor: impl FnOnce(&VirtioQueueState) -> u16,
) -> Vec<u8> {
    state
        .selected_queue()
        .map(accessor)
        .unwrap_or(0)
        .to_le_bytes()
        .to_vec()
}

fn selected_queue_or_zero_u64(
    state: &VirtioPciCommonState,
    accessor: impl FnOnce(&VirtioQueueState) -> u64,
) -> Vec<u8> {
    state
        .selected_queue()
        .map(accessor)
        .unwrap_or(0)
        .to_le_bytes()
        .to_vec()
}

fn le_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes(bytes.try_into().unwrap())
}

fn le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().unwrap())
}

fn le_u64(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().unwrap())
}

fn register_for(
    request: MmioRequestId,
    address: Address,
    size: AccessSize,
) -> Result<CommonRegister, MmioError> {
    let common_range = AddressRange::new(
        Address::new(0),
        AccessSize::new(VIRTIO_PCI_COMMON_CONFIG_SIZE).unwrap(),
    )
    .unwrap();
    let requested = AddressRange::new(address, size).map_err(MmioError::Memory)?;
    if !common_range.contains_range(requested) {
        return Err(MmioError::DeviceBoundaryCrossed {
            request,
            device_start: common_range.start(),
            device_end: common_range.end(),
            requested_start: requested.start(),
            requested_end: requested.end(),
        });
    }

    let register = COMMON_REGISTERS
        .iter()
        .copied()
        .find(|register| register.offset() == address.get())
        .ok_or(MmioError::UnmappedAddress { address })?;
    if register.size() != size.bytes() {
        return Err(MmioError::AccessSizeMismatch {
            request,
            expected: register.size(),
            actual: size.bytes(),
        });
    }
    Ok(register)
}

const COMMON_REGISTERS: &[CommonRegister] = &[
    CommonRegister::DeviceFeatureSelect,
    CommonRegister::DeviceFeature,
    CommonRegister::DriverFeatureSelect,
    CommonRegister::DriverFeature,
    CommonRegister::ConfigMsixVector,
    CommonRegister::NumQueues,
    CommonRegister::DeviceStatus,
    CommonRegister::ConfigGeneration,
    CommonRegister::QueueSelect,
    CommonRegister::QueueSize,
    CommonRegister::QueueMsixVector,
    CommonRegister::QueueEnable,
    CommonRegister::QueueNotifyOff,
    CommonRegister::QueueDesc,
    CommonRegister::QueueDriver,
    CommonRegister::QueueDevice,
    CommonRegister::QueueNotifConfigData,
    CommonRegister::QueueReset,
    CommonRegister::AdminQueueIndex,
    CommonRegister::AdminQueueNum,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VirtioError {
    TooManyQueues {
        count: usize,
    },
    InvalidQueueSize {
        index: u16,
        size: u16,
    },
    InvalidQueueRuntimeSize {
        index: u16,
        size: u16,
        max_size: u16,
    },
    EnabledQueueConfigWrite {
        index: u16,
    },
    InvalidQueueEnable {
        value: u16,
    },
    InvalidQueueReset {
        value: u16,
    },
    UnavailableQueue {
        index: u16,
    },
    InvalidNotifyMultiplier {
        multiplier: u32,
    },
    NoNotifyQueues,
    DuplicateNotifyQueue {
        index: u16,
    },
    NotifyAddressOverflow {
        queue: u16,
        notify_offset: u16,
        notify_off_multiplier: u32,
    },
    PartialNotifyWrite,
    NoQueueForNotifyAddress {
        address: Address,
    },
    NotifyValueMismatch {
        address: Address,
        value: u16,
    },
    EmptyDeviceConfig,
    DeviceConfigWritableMaskSizeMismatch {
        bytes: u64,
        mask: u64,
    },
    ReadOnlyDeviceConfigWrite {
        offset: u64,
    },
    ZeroSharedMemoryRegion {
        id: u8,
    },
    DuplicateSharedMemoryBar {
        bar: u8,
    },
    MissingSharedMemoryBar {
        id: u8,
        bar: u8,
    },
    SharedMemoryRegionAddressOverflow {
        id: u8,
        bar: u8,
        offset: u64,
        length: u64,
    },
    SharedMemoryRegionOutOfBar {
        id: u8,
        bar: u8,
        offset: u64,
        length: u64,
        bar_length: u64,
    },
    DuplicateSharedMemoryId {
        id: u8,
    },
    OverlappingSharedMemoryRegion {
        first: u8,
        second: u8,
        bar: u8,
    },
}

impl fmt::Display for VirtioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyQueues { count } => {
                write!(formatter, "VirtIO common config has too many queues: {count}")
            }
            Self::InvalidQueueSize { index, size } => write!(
                formatter,
                "VirtIO queue {index} size {size} must be a nonzero power of two"
            ),
            Self::InvalidQueueRuntimeSize {
                index,
                size,
                max_size,
            } => write!(
                formatter,
                "VirtIO queue {index} size {size} must be a nonzero power of two no larger than {max_size}"
            ),
            Self::EnabledQueueConfigWrite { index } => {
                write!(formatter, "VirtIO queue {index} cannot be reconfigured while enabled")
            }
            Self::InvalidQueueEnable { value } => {
                write!(formatter, "VirtIO queue_enable write value {value} must be 1")
            }
            Self::InvalidQueueReset { value } => {
                write!(formatter, "VirtIO queue_reset write value {value} must be 0 or 1")
            }
            Self::UnavailableQueue { index } => {
                write!(formatter, "VirtIO selected unavailable queue {index}")
            }
            Self::InvalidNotifyMultiplier { multiplier } => write!(
                formatter,
                "VirtIO notify_off_multiplier {multiplier} must be 0 or an even power of two"
            ),
            Self::NoNotifyQueues => {
                write!(formatter, "VirtIO notify device must expose at least one queue")
            }
            Self::DuplicateNotifyQueue { index } => {
                write!(formatter, "VirtIO notify queue {index} is declared more than once")
            }
            Self::NotifyAddressOverflow {
                queue,
                notify_offset,
                notify_off_multiplier,
            } => write!(
                formatter,
                "VirtIO notify queue {queue} offset {notify_offset} overflows with notify_off_multiplier {notify_off_multiplier}"
            ),
            Self::PartialNotifyWrite => {
                write!(formatter, "VirtIO notify writes require a full 16-bit byte mask")
            }
            Self::NoQueueForNotifyAddress { address } => write!(
                formatter,
                "VirtIO notify address {:#x} has no queue",
                address.get()
            ),
            Self::NotifyValueMismatch { address, value } => write!(
                formatter,
                "VirtIO notify value {value} does not match any queue at address {:#x}",
                address.get()
            ),
            Self::EmptyDeviceConfig => {
                write!(formatter, "VirtIO device config must contain at least one byte")
            }
            Self::DeviceConfigWritableMaskSizeMismatch { bytes, mask } => write!(
                formatter,
                "VirtIO device config writable mask has {mask} bytes for {bytes} config bytes"
            ),
            Self::ReadOnlyDeviceConfigWrite { offset } => {
                write!(formatter, "VirtIO device config byte {offset} is read-only")
            }
            Self::ZeroSharedMemoryRegion { id } => {
                write!(formatter, "VirtIO shared memory region id {id} has zero length")
            }
            Self::DuplicateSharedMemoryBar { bar } => {
                write!(formatter, "VirtIO shared memory BAR {bar} is declared more than once")
            }
            Self::MissingSharedMemoryBar { id, bar } => write!(
                formatter,
                "VirtIO shared memory region id {id} references undeclared BAR {bar}"
            ),
            Self::SharedMemoryRegionAddressOverflow {
                id,
                bar,
                offset,
                length,
            } => write!(
                formatter,
                "VirtIO shared memory region id {id} in BAR {bar} offset {offset:#x} overflows with length {length:#x}"
            ),
            Self::SharedMemoryRegionOutOfBar {
                id,
                bar,
                offset,
                length,
                bar_length,
            } => write!(
                formatter,
                "VirtIO shared memory region id {id} offset {offset:#x} length {length:#x} must be contained within BAR {bar} length {bar_length:#x}"
            ),
            Self::DuplicateSharedMemoryId { id } => {
                write!(formatter, "VirtIO shared memory region id {id} is declared more than once")
            }
            Self::OverlappingSharedMemoryRegion { first, second, bar } => write!(
                formatter,
                "VirtIO shared memory region id {second} overlaps id {first} in BAR {bar}"
            ),
        }
    }
}

impl Error for VirtioError {}

fn virtio_device_error(request: MmioRequestId, error: VirtioError) -> MmioError {
    MmioError::DeviceError {
        request,
        message: error.to_string(),
    }
}
