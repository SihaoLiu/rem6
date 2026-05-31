use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, SchedulerContext, Tick};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{
    MmioAccess, MmioDevice, MmioError, MmioOperation, MmioRequest, MmioRequestId, MmioResponse,
};

use crate::{
    virtio_device_error, VirtioError, VirtioPciDeviceConfigDevice, VirtioQueueIndex,
    VirtioQueueNotification, VirtioQueueSpec, VirtioSplitQueue,
};

pub const VIRTIO_MMIO_MAGIC_OFFSET: u64 = 0x00;
pub const VIRTIO_MMIO_VERSION_OFFSET: u64 = 0x04;
pub const VIRTIO_MMIO_DEVICE_ID_OFFSET: u64 = 0x08;
pub const VIRTIO_MMIO_VENDOR_ID_OFFSET: u64 = 0x0c;
pub const VIRTIO_MMIO_DEVICE_FEATURES_OFFSET: u64 = 0x10;
pub const VIRTIO_MMIO_DEVICE_FEATURES_SELECT_OFFSET: u64 = 0x14;
pub const VIRTIO_MMIO_DRIVER_FEATURES_OFFSET: u64 = 0x20;
pub const VIRTIO_MMIO_DRIVER_FEATURES_SELECT_OFFSET: u64 = 0x24;
pub const VIRTIO_MMIO_GUEST_PAGE_SIZE_OFFSET: u64 = 0x28;
pub const VIRTIO_MMIO_QUEUE_SELECT_OFFSET: u64 = 0x30;
pub const VIRTIO_MMIO_QUEUE_NUM_MAX_OFFSET: u64 = 0x34;
pub const VIRTIO_MMIO_QUEUE_NUM_OFFSET: u64 = 0x38;
pub const VIRTIO_MMIO_QUEUE_ALIGN_OFFSET: u64 = 0x3c;
pub const VIRTIO_MMIO_QUEUE_PFN_OFFSET: u64 = 0x40;
pub const VIRTIO_MMIO_QUEUE_NOTIFY_OFFSET: u64 = 0x50;
pub const VIRTIO_MMIO_INTERRUPT_STATUS_OFFSET: u64 = 0x60;
pub const VIRTIO_MMIO_INTERRUPT_ACK_OFFSET: u64 = 0x64;
pub const VIRTIO_MMIO_STATUS_OFFSET: u64 = 0x70;
pub const VIRTIO_MMIO_CONFIG_OFFSET: u64 = 0x100;

pub const VIRTIO_MMIO_MAGIC_VALUE: u32 = 0x7472_6976;
pub const VIRTIO_MMIO_VERSION: u32 = 1;
pub const VIRTIO_MMIO_VENDOR_ID: u32 = 0x1af4;
pub const VIRTIO_MMIO_INTERRUPT_USED_RING: u32 = 1 << 0;
pub const VIRTIO_MMIO_INTERRUPT_CONFIG: u32 = 1 << 1;

const VIRTIO_MMIO_LEGACY_PAGE_SIZE: u32 = 4096;
const VIRTIO_MMIO_LEGACY_QUEUE_ALIGN: u32 = 4096;
const VIRTIO_SPLIT_DESCRIPTOR_BYTES: u64 = 16;

#[derive(Clone, Debug)]
pub struct VirtioMmioTransportDevice {
    device_config: Option<VirtioPciDeviceConfigDevice>,
    notifications: Arc<Mutex<Vec<VirtioQueueNotification>>>,
    state: Arc<Mutex<VirtioMmioTransportState>>,
}

impl VirtioMmioTransportDevice {
    pub fn new(
        device_id: u16,
        device_features: impl IntoIterator<Item = (u32, u32)>,
        queues: impl IntoIterator<Item = VirtioQueueSpec>,
        device_config: Option<VirtioPciDeviceConfigDevice>,
    ) -> Result<Self, VirtioError> {
        let mut by_page = BTreeMap::new();
        for (page, features) in device_features {
            if features != 0 {
                by_page.insert(page, features);
            }
        }

        let mut by_index = BTreeMap::new();
        for (index, spec) in queues.into_iter().enumerate() {
            let index = u16::try_from(index).map_err(|_| VirtioError::TooManyQueues {
                count: usize::from(u16::MAX) + 1,
            })?;
            validate_queue_size(index, spec.size(), spec.size())?;
            by_index.insert(
                VirtioQueueIndex::new(index).unwrap(),
                VirtioMmioQueueState::new(spec),
            );
        }

        Ok(Self {
            device_config,
            notifications: Arc::new(Mutex::new(Vec::new())),
            state: Arc::new(Mutex::new(VirtioMmioTransportState {
                device_id: u32::from(device_id),
                device_features: by_page,
                driver_features: BTreeMap::new(),
                device_features_select: 0,
                driver_features_select: 0,
                guest_page_size: 0,
                queue_select: VirtioQueueIndex::new(0).unwrap(),
                queues: by_index,
                interrupt_status: 0,
                device_status: 0,
            })),
        })
    }

    pub fn range(&self) -> AddressRange {
        let bytes = self
            .device_config
            .as_ref()
            .map(|device| VIRTIO_MMIO_CONFIG_OFFSET + device.range().size().bytes())
            .unwrap_or(VIRTIO_MMIO_CONFIG_OFFSET);
        AddressRange::new(Address::new(0), AccessSize::new(bytes).unwrap()).unwrap()
    }

    pub fn read_local(&self, address: Address, size: AccessSize) -> Result<Vec<u8>, MmioError> {
        let request = MmioRequest::read(MmioRequestId::new(0), address, size)?;
        let response = self.respond_local(0, &request)?;
        Ok(response.data().map_or_else(Vec::new, <[u8]>::to_vec))
    }

    pub fn write_local(
        &self,
        address: Address,
        data: Vec<u8>,
        byte_mask: ByteMask,
        tick: Tick,
    ) -> Result<(), MmioError> {
        let request = MmioRequest::write(MmioRequestId::new(0), address, data, byte_mask)?;
        self.respond_local(tick, &request)?;
        Ok(())
    }

    pub fn snapshot(&self) -> VirtioMmioTransportSnapshot {
        self.state
            .lock()
            .expect("virtio mmio transport lock")
            .snapshot()
    }

    pub fn notifications(&self) -> Vec<VirtioQueueNotification> {
        self.notifications
            .lock()
            .expect("virtio mmio notification lock")
            .clone()
    }

    pub fn raise_used_ring_interrupt(&self) {
        self.raise_interrupt(VIRTIO_MMIO_INTERRUPT_USED_RING);
    }

    pub fn raise_config_interrupt(&self) {
        self.raise_interrupt(VIRTIO_MMIO_INTERRUPT_CONFIG);
    }

    pub fn split_queue(
        &self,
        index: VirtioQueueIndex,
    ) -> Result<Option<VirtioSplitQueue>, VirtioError> {
        let state = self.state.lock().expect("virtio mmio transport lock");
        let Some(queue) = state.queues.get(&index) else {
            return Ok(None);
        };
        if queue.pfn == 0 {
            return Ok(None);
        }
        let page_size = u64::from(state.guest_page_size);
        if page_size == 0 {
            return Err(VirtioError::PciTransportRuntimeConfig {
                message: format!(
                    "VirtIO MMIO queue {} has a PFN before guest page size is configured",
                    index.get()
                ),
            });
        }
        let descriptor = u64::from(queue.pfn).checked_mul(page_size).ok_or_else(|| {
            VirtioError::PciTransportRuntimeConfig {
                message: format!("VirtIO MMIO queue {} PFN address overflows", index.get()),
            }
        })?;
        let available = descriptor
            .checked_add(VIRTIO_SPLIT_DESCRIPTOR_BYTES * u64::from(queue.size))
            .ok_or_else(|| VirtioError::PciTransportRuntimeConfig {
                message: format!(
                    "VirtIO MMIO queue {} available-ring address overflows",
                    index.get()
                ),
            })?;
        let used_base = available
            .checked_add(4 + 2 * u64::from(queue.size))
            .ok_or_else(|| VirtioError::PciTransportRuntimeConfig {
                message: format!(
                    "VirtIO MMIO queue {} used-ring address overflows",
                    index.get()
                ),
            })?;
        let used = align_up(used_base, u64::from(queue.align)).ok_or_else(|| {
            VirtioError::PciTransportRuntimeConfig {
                message: format!(
                    "VirtIO MMIO queue {} used-ring address overflows",
                    index.get()
                ),
            }
        })?;
        VirtioSplitQueue::new(
            queue.size,
            Address::new(descriptor),
            Address::new(available),
            Address::new(used),
            0,
        )
        .map(Some)
    }

    fn raise_interrupt(&self, interrupt: u32) {
        self.state
            .lock()
            .expect("virtio mmio transport lock")
            .interrupt_status |= interrupt;
    }

    fn respond_local(&self, tick: Tick, request: &MmioRequest) -> Result<MmioResponse, MmioError> {
        self.validate_request_boundary(request)?;
        if request.range().start().get() >= VIRTIO_MMIO_CONFIG_OFFSET {
            return self.respond_config_local(request);
        }
        match request.operation() {
            MmioOperation::Read => {
                let value = self.read_register(request)?;
                Ok(MmioResponse::completed(
                    request.id(),
                    Some(value.to_le_bytes().to_vec()),
                ))
            }
            MmioOperation::Write => {
                let value = self.write_value(request)?;
                self.write_register(tick, request, value)?;
                Ok(MmioResponse::completed(request.id(), None))
            }
        }
    }

    fn respond_config_local(&self, request: &MmioRequest) -> Result<MmioResponse, MmioError> {
        let local = local_config_request(request)?;
        let Some(config) = &self.device_config else {
            return Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            });
        };
        match local.operation() {
            MmioOperation::Read => config
                .read_local(local.range().start(), local.size())
                .map(|data| MmioResponse::completed(request.id(), Some(data))),
            MmioOperation::Write => {
                config.write_local(
                    local.range().start(),
                    local.data().unwrap().to_vec(),
                    local.byte_mask().unwrap().clone(),
                )?;
                Ok(MmioResponse::completed(request.id(), None))
            }
        }
    }

    fn respond_config_with_context(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let local = local_config_request(request)?;
        let Some(config) = &self.device_config else {
            return Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            });
        };
        config.respond(context, &local)
    }

    fn respond_config_parallel_with_context(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let local = local_config_request(request)?;
        let Some(config) = &self.device_config else {
            return Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            });
        };
        config.respond_parallel(context, &local)
    }

    fn read_register(&self, request: &MmioRequest) -> Result<u32, MmioError> {
        validate_register_size(request)?;
        let state = self.state.lock().expect("virtio mmio transport lock");
        let value = match request.range().start().get() {
            VIRTIO_MMIO_MAGIC_OFFSET => VIRTIO_MMIO_MAGIC_VALUE,
            VIRTIO_MMIO_VERSION_OFFSET => VIRTIO_MMIO_VERSION,
            VIRTIO_MMIO_DEVICE_ID_OFFSET => state.device_id,
            VIRTIO_MMIO_VENDOR_ID_OFFSET => VIRTIO_MMIO_VENDOR_ID,
            VIRTIO_MMIO_DEVICE_FEATURES_OFFSET => state
                .device_features
                .get(&state.device_features_select)
                .copied()
                .unwrap_or(0),
            VIRTIO_MMIO_DEVICE_FEATURES_SELECT_OFFSET => state.device_features_select,
            VIRTIO_MMIO_DRIVER_FEATURES_OFFSET => state
                .driver_features
                .get(&state.driver_features_select)
                .copied()
                .unwrap_or(0),
            VIRTIO_MMIO_DRIVER_FEATURES_SELECT_OFFSET => state.driver_features_select,
            VIRTIO_MMIO_GUEST_PAGE_SIZE_OFFSET => state.guest_page_size,
            VIRTIO_MMIO_QUEUE_SELECT_OFFSET => u32::from(state.queue_select.get()),
            VIRTIO_MMIO_QUEUE_NUM_MAX_OFFSET => state
                .selected_queue()
                .map(|queue| u32::from(queue.max_size))
                .unwrap_or(0),
            VIRTIO_MMIO_QUEUE_NUM_OFFSET => state
                .selected_queue()
                .map(|queue| u32::from(queue.size))
                .unwrap_or(0),
            VIRTIO_MMIO_QUEUE_ALIGN_OFFSET => state
                .selected_queue()
                .map(|queue| queue.align)
                .unwrap_or(VIRTIO_MMIO_LEGACY_QUEUE_ALIGN),
            VIRTIO_MMIO_QUEUE_PFN_OFFSET => {
                state.selected_queue().map(|queue| queue.pfn).unwrap_or(0)
            }
            VIRTIO_MMIO_INTERRUPT_STATUS_OFFSET => state.interrupt_status,
            VIRTIO_MMIO_STATUS_OFFSET => u32::from(state.device_status),
            VIRTIO_MMIO_QUEUE_NOTIFY_OFFSET | VIRTIO_MMIO_INTERRUPT_ACK_OFFSET => {
                return Err(MmioError::AccessDenied {
                    request: request.id(),
                    operation: MmioOperation::Read,
                    access: MmioAccess::WriteOnly,
                });
            }
            _ => {
                return Err(MmioError::UnmappedAddress {
                    address: request.range().start(),
                });
            }
        };
        Ok(value)
    }

    fn write_register(
        &self,
        tick: Tick,
        request: &MmioRequest,
        value: u32,
    ) -> Result<(), MmioError> {
        match request.range().start().get() {
            VIRTIO_MMIO_DEVICE_FEATURES_SELECT_OFFSET => {
                self.state
                    .lock()
                    .expect("virtio mmio transport lock")
                    .device_features_select = value;
                Ok(())
            }
            VIRTIO_MMIO_DRIVER_FEATURES_OFFSET => self.write_driver_features(request.id(), value),
            VIRTIO_MMIO_DRIVER_FEATURES_SELECT_OFFSET => {
                self.state
                    .lock()
                    .expect("virtio mmio transport lock")
                    .driver_features_select = value;
                Ok(())
            }
            VIRTIO_MMIO_GUEST_PAGE_SIZE_OFFSET => self.write_guest_page_size(request.id(), value),
            VIRTIO_MMIO_QUEUE_SELECT_OFFSET => self.write_queue_select(request.id(), value),
            VIRTIO_MMIO_QUEUE_NUM_OFFSET => self.write_queue_num(request.id(), value),
            VIRTIO_MMIO_QUEUE_ALIGN_OFFSET => self.write_queue_align(request.id(), value),
            VIRTIO_MMIO_QUEUE_PFN_OFFSET => self.write_queue_pfn(request.id(), value),
            VIRTIO_MMIO_QUEUE_NOTIFY_OFFSET => self.write_queue_notify(request.id(), value, tick),
            VIRTIO_MMIO_INTERRUPT_ACK_OFFSET => {
                self.state
                    .lock()
                    .expect("virtio mmio transport lock")
                    .interrupt_status &= !value;
                Ok(())
            }
            VIRTIO_MMIO_STATUS_OFFSET => self.write_status(request.id(), value),
            VIRTIO_MMIO_MAGIC_OFFSET
            | VIRTIO_MMIO_VERSION_OFFSET
            | VIRTIO_MMIO_DEVICE_ID_OFFSET
            | VIRTIO_MMIO_VENDOR_ID_OFFSET
            | VIRTIO_MMIO_DEVICE_FEATURES_OFFSET
            | VIRTIO_MMIO_QUEUE_NUM_MAX_OFFSET
            | VIRTIO_MMIO_INTERRUPT_STATUS_OFFSET => Err(MmioError::AccessDenied {
                request: request.id(),
                operation: MmioOperation::Write,
                access: MmioAccess::ReadOnly,
            }),
            _ => Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            }),
        }
    }

    fn write_driver_features(
        &self,
        request: MmioRequestId,
        features: u32,
    ) -> Result<(), MmioError> {
        let mut state = self.state.lock().expect("virtio mmio transport lock");
        let page = state.driver_features_select;
        let supported = state.device_features.get(&page).copied().unwrap_or(0);
        if features & !supported != 0 {
            return Err(virtio_device_error(
                request,
                VirtioError::UnsupportedFeatureBits {
                    page,
                    requested: features,
                    supported,
                },
            ));
        }
        if features == 0 {
            state.driver_features.remove(&page);
        } else {
            state.driver_features.insert(page, features);
        }
        Ok(())
    }

    fn write_guest_page_size(&self, request: MmioRequestId, value: u32) -> Result<(), MmioError> {
        if value != VIRTIO_MMIO_LEGACY_PAGE_SIZE {
            return Err(virtio_device_error(
                request,
                VirtioError::UnsupportedMmioPageSize { size: value },
            ));
        }
        self.state
            .lock()
            .expect("virtio mmio transport lock")
            .guest_page_size = value;
        Ok(())
    }

    fn write_queue_select(&self, request: MmioRequestId, value: u32) -> Result<(), MmioError> {
        let index = u16::try_from(value).map_err(|_| {
            virtio_device_error(request, VirtioError::UnavailableQueue { index: u16::MAX })
        })?;
        self.state
            .lock()
            .expect("virtio mmio transport lock")
            .queue_select = VirtioQueueIndex::new(index).unwrap();
        Ok(())
    }

    fn write_queue_num(&self, request: MmioRequestId, value: u32) -> Result<(), MmioError> {
        let size = u16::try_from(value).map_err(|_| {
            virtio_device_error(
                request,
                VirtioError::InvalidQueueRuntimeSize {
                    index: u16::MAX,
                    size: u16::MAX,
                    max_size: u16::MAX,
                },
            )
        })?;
        let mut state = self.state.lock().expect("virtio mmio transport lock");
        let queue_index = state.queue_select;
        let queue = selected_queue_mut(&mut state).ok_or_else(|| {
            virtio_device_error(
                request,
                VirtioError::UnavailableQueue {
                    index: queue_index.get(),
                },
            )
        })?;
        validate_queue_size(queue_index.get(), size, queue.max_size)
            .map_err(|error| virtio_device_error(request, error))?;
        queue.size = size;
        Ok(())
    }

    fn write_queue_align(&self, request: MmioRequestId, value: u32) -> Result<(), MmioError> {
        if value != VIRTIO_MMIO_LEGACY_QUEUE_ALIGN {
            return Err(virtio_device_error(
                request,
                VirtioError::UnsupportedMmioQueueAlign { align: value },
            ));
        }
        let mut state = self.state.lock().expect("virtio mmio transport lock");
        let queue_index = state.queue_select;
        let queue = selected_queue_mut(&mut state).ok_or_else(|| {
            virtio_device_error(
                request,
                VirtioError::UnavailableQueue {
                    index: queue_index.get(),
                },
            )
        })?;
        queue.align = value;
        Ok(())
    }

    fn write_queue_pfn(&self, request: MmioRequestId, value: u32) -> Result<(), MmioError> {
        let mut state = self.state.lock().expect("virtio mmio transport lock");
        if state.guest_page_size == 0 && value != 0 {
            return Err(virtio_device_error(
                request,
                VirtioError::PciTransportRuntimeConfig {
                    message: "VirtIO MMIO guest page size must be configured before queue PFN"
                        .to_string(),
                },
            ));
        }
        let queue_index = state.queue_select;
        let queue = selected_queue_mut(&mut state).ok_or_else(|| {
            virtio_device_error(
                request,
                VirtioError::UnavailableQueue {
                    index: queue_index.get(),
                },
            )
        })?;
        queue.pfn = value;
        Ok(())
    }

    fn write_queue_notify(
        &self,
        request: MmioRequestId,
        value: u32,
        tick: Tick,
    ) -> Result<(), MmioError> {
        let index = u16::try_from(value).map_err(|_| {
            virtio_device_error(request, VirtioError::UnavailableQueue { index: u16::MAX })
        })?;
        let queue = VirtioQueueIndex::new(index).unwrap();
        if !self
            .state
            .lock()
            .expect("virtio mmio transport lock")
            .queues
            .contains_key(&queue)
        {
            return Err(virtio_device_error(
                request,
                VirtioError::UnavailableQueue { index },
            ));
        }
        self.notifications
            .lock()
            .expect("virtio mmio notification lock")
            .push(VirtioQueueNotification::new(
                tick,
                queue,
                index,
                Address::new(VIRTIO_MMIO_QUEUE_NOTIFY_OFFSET),
            ));
        Ok(())
    }

    fn write_status(&self, request: MmioRequestId, value: u32) -> Result<(), MmioError> {
        let status = u8::try_from(value).map_err(|_| {
            virtio_device_error(
                request,
                VirtioError::PciTransportRuntimeConfig {
                    message: format!("VirtIO MMIO status {value:#x} does not fit in 8 bits"),
                },
            )
        })?;
        let mut state = self.state.lock().expect("virtio mmio transport lock");
        if status == 0 {
            state.reset();
        } else {
            state.device_status = status;
        }
        Ok(())
    }

    fn write_value(&self, request: &MmioRequest) -> Result<u32, MmioError> {
        validate_register_size(request)?;
        let data = request.data().ok_or(MmioError::MissingWriteData {
            request: request.id(),
        })?;
        if data.len() != 4 {
            return Err(MmioError::PayloadSizeMismatch {
                request: request.id(),
                expected: 4,
                actual: data.len() as u64,
            });
        }
        let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
            request: request.id(),
        })?;
        if mask.len() != 4 {
            return Err(MmioError::ByteMaskSizeMismatch {
                request: request.id(),
                expected: 4,
                actual: mask.len(),
            });
        }
        if !mask.bits().iter().all(|bit| *bit) {
            return Err(virtio_device_error(
                request.id(),
                VirtioError::PciTransportRuntimeConfig {
                    message: "VirtIO MMIO register writes require a full 32-bit byte mask"
                        .to_string(),
                },
            ));
        }
        Ok(u32::from_le_bytes(data.try_into().unwrap()))
    }

    fn validate_request_boundary(&self, request: &MmioRequest) -> Result<(), MmioError> {
        let range = self.range();
        let requested = request.range();
        if !range.contains_range(requested) {
            return Err(MmioError::DeviceBoundaryCrossed {
                request: request.id(),
                device_start: range.start(),
                device_end: range.end(),
                requested_start: requested.start(),
                requested_end: requested.end(),
            });
        }
        Ok(())
    }
}

impl MmioDevice for VirtioMmioTransportDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.validate_request_boundary(request)?;
        if request.range().start().get() >= VIRTIO_MMIO_CONFIG_OFFSET {
            self.respond_config_with_context(context, request)
        } else {
            self.respond_local(context.now(), request)
        }
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.validate_request_boundary(request)?;
        if request.range().start().get() >= VIRTIO_MMIO_CONFIG_OFFSET {
            self.respond_config_parallel_with_context(context, request)
        } else {
            self.respond_local(context.now(), request)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioMmioTransportSnapshot {
    device_id: u32,
    device_features: BTreeMap<u32, u32>,
    driver_features: BTreeMap<u32, u32>,
    device_features_select: u32,
    driver_features_select: u32,
    guest_page_size: u32,
    queue_select: VirtioQueueIndex,
    queues: BTreeMap<VirtioQueueIndex, VirtioMmioQueueSnapshot>,
    interrupt_status: u32,
    device_status: u8,
}

impl VirtioMmioTransportSnapshot {
    pub const fn device_id(&self) -> u32 {
        self.device_id
    }

    pub fn device_feature_page(&self, page: u32) -> u32 {
        self.device_features.get(&page).copied().unwrap_or(0)
    }

    pub fn driver_feature_page(&self, page: u32) -> u32 {
        self.driver_features.get(&page).copied().unwrap_or(0)
    }

    pub const fn device_features_select(&self) -> u32 {
        self.device_features_select
    }

    pub const fn driver_features_select(&self) -> u32 {
        self.driver_features_select
    }

    pub const fn guest_page_size(&self) -> u32 {
        self.guest_page_size
    }

    pub const fn queue_select(&self) -> VirtioQueueIndex {
        self.queue_select
    }

    pub fn queue(&self, index: VirtioQueueIndex) -> Option<&VirtioMmioQueueSnapshot> {
        self.queues.get(&index)
    }

    pub const fn interrupt_status(&self) -> u32 {
        self.interrupt_status
    }

    pub const fn device_status(&self) -> u8 {
        self.device_status
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioMmioQueueSnapshot {
    max_size: u16,
    size: u16,
    align: u32,
    pfn: u32,
}

impl VirtioMmioQueueSnapshot {
    pub const fn max_size(self) -> u16 {
        self.max_size
    }

    pub const fn size(self) -> u16 {
        self.size
    }

    pub const fn align(self) -> u32 {
        self.align
    }

    pub const fn pfn(self) -> u32 {
        self.pfn
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VirtioMmioTransportState {
    device_id: u32,
    device_features: BTreeMap<u32, u32>,
    driver_features: BTreeMap<u32, u32>,
    device_features_select: u32,
    driver_features_select: u32,
    guest_page_size: u32,
    queue_select: VirtioQueueIndex,
    queues: BTreeMap<VirtioQueueIndex, VirtioMmioQueueState>,
    interrupt_status: u32,
    device_status: u8,
}

impl VirtioMmioTransportState {
    fn selected_queue(&self) -> Option<&VirtioMmioQueueState> {
        self.queues.get(&self.queue_select)
    }

    fn snapshot(&self) -> VirtioMmioTransportSnapshot {
        VirtioMmioTransportSnapshot {
            device_id: self.device_id,
            device_features: self.device_features.clone(),
            driver_features: self.driver_features.clone(),
            device_features_select: self.device_features_select,
            driver_features_select: self.driver_features_select,
            guest_page_size: self.guest_page_size,
            queue_select: self.queue_select,
            queues: self
                .queues
                .iter()
                .map(|(index, queue)| (*index, queue.snapshot()))
                .collect(),
            interrupt_status: self.interrupt_status,
            device_status: self.device_status,
        }
    }

    fn reset(&mut self) {
        self.driver_features.clear();
        self.device_features_select = 0;
        self.driver_features_select = 0;
        self.guest_page_size = 0;
        self.queue_select = VirtioQueueIndex::new(0).unwrap();
        for queue in self.queues.values_mut() {
            queue.reset();
        }
        self.interrupt_status = 0;
        self.device_status = 0;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VirtioMmioQueueState {
    max_size: u16,
    size: u16,
    align: u32,
    pfn: u32,
}

impl VirtioMmioQueueState {
    fn new(spec: VirtioQueueSpec) -> Self {
        Self {
            max_size: spec.size(),
            size: spec.size(),
            align: VIRTIO_MMIO_LEGACY_QUEUE_ALIGN,
            pfn: 0,
        }
    }

    const fn snapshot(&self) -> VirtioMmioQueueSnapshot {
        VirtioMmioQueueSnapshot {
            max_size: self.max_size,
            size: self.size,
            align: self.align,
            pfn: self.pfn,
        }
    }

    fn reset(&mut self) {
        self.size = self.max_size;
        self.align = VIRTIO_MMIO_LEGACY_QUEUE_ALIGN;
        self.pfn = 0;
    }
}

fn selected_queue_mut(state: &mut VirtioMmioTransportState) -> Option<&mut VirtioMmioQueueState> {
    state.queues.get_mut(&state.queue_select)
}

fn validate_register_size(request: &MmioRequest) -> Result<(), MmioError> {
    if request.size().bytes() != 4 {
        return Err(MmioError::AccessSizeMismatch {
            request: request.id(),
            expected: 4,
            actual: request.size().bytes(),
        });
    }
    Ok(())
}

fn validate_queue_size(index: u16, size: u16, max_size: u16) -> Result<(), VirtioError> {
    if size == 0 || !size.is_power_of_two() || size > max_size {
        return Err(VirtioError::InvalidQueueRuntimeSize {
            index,
            size,
            max_size,
        });
    }
    Ok(())
}

fn local_config_request(request: &MmioRequest) -> Result<MmioRequest, MmioError> {
    let offset = request.range().start().get() - VIRTIO_MMIO_CONFIG_OFFSET;
    match request.operation() {
        MmioOperation::Read => {
            MmioRequest::read(request.id(), Address::new(offset), request.size())
        }
        MmioOperation::Write => MmioRequest::write(
            request.id(),
            Address::new(offset),
            request
                .data()
                .ok_or(MmioError::MissingWriteData {
                    request: request.id(),
                })?
                .to_vec(),
            request
                .byte_mask()
                .ok_or(MmioError::MissingByteMask {
                    request: request.id(),
                })?
                .clone(),
        ),
    }
}

fn align_up(value: u64, align: u64) -> Option<u64> {
    let mask = align.checked_sub(1)?;
    value.checked_add(mask).map(|value| value & !mask)
}
