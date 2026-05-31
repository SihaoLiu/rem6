use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use rem6_kernel::Tick;
use rem6_memory::Address;

use crate::{
    block_queue::add_address, VirtioError, VirtioGuestMemory, VirtioPciIsrDevice, VirtioQueueIndex,
    VirtioSplitDescriptorChain, VirtioSplitQueue, VirtioSplitUsedElement, VirtioSplitUsedRing,
};

pub const VIRTIO_CONSOLE_DEVICE_ID: u16 = 3;
pub const VIRTIO_CONSOLE_F_SIZE: u32 = 1;
pub const VIRTIO_CONSOLE_CONFIG_SIZE: u64 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioConsoleConfig {
    cols: u16,
    rows: u16,
}

impl VirtioConsoleConfig {
    pub const fn default_size() -> Self {
        Self { cols: 80, rows: 24 }
    }

    pub fn new(cols: u16, rows: u16) -> Result<Self, VirtioError> {
        if cols == 0 || rows == 0 {
            return Err(VirtioError::InvalidConsoleSize { cols, rows });
        }
        Ok(Self { cols, rows })
    }

    pub const fn cols(self) -> u16 {
        self.cols
    }

    pub const fn rows(self) -> u16 {
        self.rows
    }

    pub fn to_le_bytes(self) -> [u8; VIRTIO_CONSOLE_CONFIG_SIZE as usize] {
        let cols = self.cols.to_le_bytes();
        let rows = self.rows.to_le_bytes();
        [cols[0], cols[1], rows[0], rows[1]]
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VirtioConsoleRequestId(u64);

impl VirtioConsoleRequestId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioConsoleReceiveRequest {
    id: VirtioConsoleRequestId,
    queue: VirtioQueueIndex,
    capacity: u64,
}

impl VirtioConsoleReceiveRequest {
    pub fn new(
        id: VirtioConsoleRequestId,
        queue: VirtioQueueIndex,
        capacity: u64,
    ) -> Result<Self, VirtioError> {
        if capacity == 0 {
            return Err(VirtioError::MissingVirtioConsoleReceiveDescriptor);
        }
        u32::try_from(capacity).map_err(|_| VirtioError::VirtioConsolePayloadLengthOverflow)?;
        Ok(Self {
            id,
            queue,
            capacity,
        })
    }

    pub const fn id(&self) -> VirtioConsoleRequestId {
        self.id
    }

    pub const fn queue(&self) -> VirtioQueueIndex {
        self.queue
    }

    pub const fn capacity(&self) -> u64 {
        self.capacity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioConsoleTransmitRequest {
    id: VirtioConsoleRequestId,
    queue: VirtioQueueIndex,
    data: Vec<u8>,
}

impl VirtioConsoleTransmitRequest {
    pub fn new(id: VirtioConsoleRequestId, queue: VirtioQueueIndex, data: Vec<u8>) -> Self {
        Self { id, queue, data }
    }

    pub const fn id(&self) -> VirtioConsoleRequestId {
        self.id
    }

    pub const fn queue(&self) -> VirtioQueueIndex {
        self.queue
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtioConsoleTransferKind {
    Receive,
    Transmit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioConsoleCompletion {
    request: VirtioConsoleRequestId,
    queue: VirtioQueueIndex,
    tick: Tick,
    kind: VirtioConsoleTransferKind,
    bytes: Vec<u8>,
}

impl VirtioConsoleCompletion {
    fn receive(request: &VirtioConsoleReceiveRequest, tick: Tick, bytes: Vec<u8>) -> Self {
        Self {
            request: request.id(),
            queue: request.queue(),
            tick,
            kind: VirtioConsoleTransferKind::Receive,
            bytes,
        }
    }

    fn transmit(request: &VirtioConsoleTransmitRequest, tick: Tick) -> Self {
        Self {
            request: request.id(),
            queue: request.queue(),
            tick,
            kind: VirtioConsoleTransferKind::Transmit,
            bytes: Vec::new(),
        }
    }

    pub const fn request(&self) -> VirtioConsoleRequestId {
        self.request
    }

    pub const fn queue(&self) -> VirtioQueueIndex {
        self.queue
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn kind(&self) -> VirtioConsoleTransferKind {
        self.kind
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn used_length(&self) -> u32 {
        match self.kind {
            VirtioConsoleTransferKind::Receive => self.bytes.len() as u32,
            VirtioConsoleTransferKind::Transmit => 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct VirtioConsoleDevice {
    config: VirtioConsoleConfig,
    host_input: Arc<Mutex<VecDeque<u8>>>,
    guest_output: Arc<Mutex<Vec<u8>>>,
    completions: Arc<Mutex<Vec<VirtioConsoleCompletion>>>,
}

impl VirtioConsoleDevice {
    pub fn new() -> Self {
        Self::with_config(VirtioConsoleConfig::default_size())
    }

    pub fn with_config(config: VirtioConsoleConfig) -> Self {
        Self {
            config,
            host_input: Arc::new(Mutex::new(VecDeque::new())),
            guest_output: Arc::new(Mutex::new(Vec::new())),
            completions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn feature_pages(&self) -> Vec<(u32, u32)> {
        vec![(0, VIRTIO_CONSOLE_F_SIZE)]
    }

    pub const fn config_size(&self) -> u64 {
        VIRTIO_CONSOLE_CONFIG_SIZE
    }

    pub const fn config(&self) -> VirtioConsoleConfig {
        self.config
    }

    pub fn config_bytes(&self) -> [u8; VIRTIO_CONSOLE_CONFIG_SIZE as usize] {
        self.config.to_le_bytes()
    }

    pub fn push_host_input(&self, bytes: Vec<u8>) {
        self.host_input
            .lock()
            .expect("virtio console input lock")
            .extend(bytes);
    }

    pub fn pending_host_input(&self) -> Vec<u8> {
        self.host_input
            .lock()
            .expect("virtio console input lock")
            .iter()
            .copied()
            .collect()
    }

    pub fn guest_output(&self) -> Vec<u8> {
        self.guest_output
            .lock()
            .expect("virtio console output lock")
            .clone()
    }

    pub fn receive_at(
        &self,
        tick: Tick,
        request: VirtioConsoleReceiveRequest,
    ) -> Result<VirtioConsoleCompletion, VirtioError> {
        let capacity = usize::try_from(request.capacity())
            .map_err(|_| VirtioError::VirtioConsolePayloadLengthOverflow)?;
        let mut host_input = self.host_input.lock().expect("virtio console input lock");
        let mut bytes = Vec::new();
        while bytes.len() < capacity {
            let Some(byte) = host_input.pop_front() else {
                break;
            };
            bytes.push(byte);
        }
        drop(host_input);

        let completion = VirtioConsoleCompletion::receive(&request, tick, bytes);
        self.completions
            .lock()
            .expect("virtio console completion lock")
            .push(completion.clone());
        Ok(completion)
    }

    pub fn transmit_at(
        &self,
        tick: Tick,
        request: VirtioConsoleTransmitRequest,
    ) -> Result<VirtioConsoleCompletion, VirtioError> {
        self.guest_output
            .lock()
            .expect("virtio console output lock")
            .extend_from_slice(request.data());
        let completion = VirtioConsoleCompletion::transmit(&request, tick);
        self.completions
            .lock()
            .expect("virtio console completion lock")
            .push(completion.clone());
        Ok(completion)
    }

    pub fn completions(&self) -> Vec<VirtioConsoleCompletion> {
        self.completions
            .lock()
            .expect("virtio console completion lock")
            .clone()
    }
}

impl Default for VirtioConsoleDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for VirtioConsoleConfig {
    fn default() -> Self {
        Self::default_size()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioConsoleDecodedReceive {
    request: VirtioConsoleReceiveRequest,
    writable_descriptors: Vec<VirtioConsoleWritableDescriptor>,
}

impl VirtioConsoleDecodedReceive {
    pub const fn request(&self) -> &VirtioConsoleReceiveRequest {
        &self.request
    }

    pub fn into_request(self) -> VirtioConsoleReceiveRequest {
        self.request
    }

    fn data_writes(&self, data: &[u8]) -> Result<Vec<VirtioConsoleDescriptorWrite>, VirtioError> {
        if data.len() as u64 > self.request.capacity() {
            return Err(VirtioError::VirtioConsolePayloadLengthOverflow);
        }
        let mut cursor = data;
        let mut writes = Vec::new();
        for target in &self.writable_descriptors {
            if cursor.is_empty() {
                break;
            }
            let bytes = cursor.len().min(target.length() as usize);
            writes.push(VirtioConsoleDescriptorWrite {
                descriptor: target.index(),
                address: target.address(),
                offset: 0,
                bytes: cursor[..bytes].to_vec(),
            });
            cursor = &cursor[bytes..];
        }
        if !cursor.is_empty() {
            return Err(VirtioError::VirtioConsolePayloadLengthOverflow);
        }
        Ok(writes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioConsoleDecodedTransmit {
    request: VirtioConsoleTransmitRequest,
}

impl VirtioConsoleDecodedTransmit {
    pub const fn request(&self) -> &VirtioConsoleTransmitRequest {
        &self.request
    }

    pub fn into_request(self) -> VirtioConsoleTransmitRequest {
        self.request
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VirtioConsoleWritableDescriptor {
    index: u16,
    address: Option<Address>,
    length: u32,
}

impl VirtioConsoleWritableDescriptor {
    const fn index(self) -> u16 {
        self.index
    }

    const fn address(self) -> Option<Address> {
        self.address
    }

    const fn length(self) -> u32 {
        self.length
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioConsoleDescriptorWrite {
    descriptor: u16,
    address: Option<Address>,
    offset: u32,
    bytes: Vec<u8>,
}

impl VirtioConsoleDescriptorWrite {
    pub const fn descriptor(&self) -> u16 {
        self.descriptor
    }

    pub const fn address(&self) -> Option<Address> {
        self.address
    }

    pub const fn offset(&self) -> u32 {
        self.offset
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioConsoleQueueCompletionWrite {
    data_writes: Vec<VirtioConsoleDescriptorWrite>,
    used_slot: u16,
    used_index: u16,
    used_element: VirtioSplitUsedElement,
}

impl VirtioConsoleQueueCompletionWrite {
    pub fn data_writes(&self) -> &[VirtioConsoleDescriptorWrite] {
        &self.data_writes
    }

    pub const fn used_slot(&self) -> u16 {
        self.used_slot
    }

    pub const fn used_index(&self) -> u16 {
        self.used_index
    }

    pub const fn used_element(&self) -> VirtioSplitUsedElement {
        self.used_element
    }
}

impl VirtioSplitDescriptorChain {
    pub fn decode_console_receive_request(
        &self,
        queue: VirtioQueueIndex,
    ) -> Result<VirtioConsoleDecodedReceive, VirtioError> {
        let mut capacity = 0_u64;
        let mut writable_descriptors = Vec::new();
        for descriptor in self.descriptors() {
            if !descriptor.is_writable() {
                return Err(VirtioError::InvalidVirtioConsoleReceiveDescriptor {
                    index: descriptor.index(),
                });
            }
            capacity = capacity
                .checked_add(u64::from(descriptor.length()))
                .ok_or(VirtioError::VirtioConsolePayloadLengthOverflow)?;
            writable_descriptors.push(VirtioConsoleWritableDescriptor {
                index: descriptor.index(),
                address: descriptor.address(),
                length: descriptor.length(),
            });
        }
        let request = VirtioConsoleReceiveRequest::new(
            VirtioConsoleRequestId::new(u64::from(self.head())),
            queue,
            capacity,
        )?;
        Ok(VirtioConsoleDecodedReceive {
            request,
            writable_descriptors,
        })
    }

    pub fn decode_console_transmit_request(
        &self,
        queue: VirtioQueueIndex,
    ) -> Result<VirtioConsoleDecodedTransmit, VirtioError> {
        let mut data = Vec::new();
        for descriptor in self.descriptors() {
            if descriptor.is_writable() {
                return Err(VirtioError::InvalidVirtioConsoleTransmitDescriptor {
                    index: descriptor.index(),
                });
            }
            data.extend_from_slice(descriptor.bytes());
        }
        let request = VirtioConsoleTransmitRequest::new(
            VirtioConsoleRequestId::new(u64::from(self.head())),
            queue,
            data,
        );
        Ok(VirtioConsoleDecodedTransmit { request })
    }
}

impl VirtioSplitQueue {
    pub fn consume_available_console_receive(
        &mut self,
        guest: &mut VirtioGuestMemory<'_>,
        queue: VirtioQueueIndex,
    ) -> Result<Option<VirtioConsoleDecodedReceive>, VirtioError> {
        let Some(chain) = self.consume_available_chain(guest)? else {
            return Ok(None);
        };
        let decoded = chain.decode_console_receive_request(queue)?;
        self.advance_available_index();
        Ok(Some(decoded))
    }

    pub fn consume_available_console_transmit(
        &mut self,
        guest: &mut VirtioGuestMemory<'_>,
        queue: VirtioQueueIndex,
    ) -> Result<Option<VirtioConsoleDecodedTransmit>, VirtioError> {
        let Some(chain) = self.consume_available_chain(guest)? else {
            return Ok(None);
        };
        let decoded = chain.decode_console_transmit_request(queue)?;
        self.advance_available_index();
        Ok(Some(decoded))
    }

    pub fn complete_console_receive(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioConsoleDecodedReceive,
        completion: &VirtioConsoleCompletion,
    ) -> Result<VirtioConsoleQueueCompletionWrite, VirtioError> {
        let used_index = guest.read_u16(add_address(self.used_ring(), 2)?)?;
        let mut used_ring = VirtioSplitUsedRing::new(self.queue_size(), used_index)?;
        let writeback = used_ring.complete_console_receive(decoded, completion)?;
        self.write_console_completion(guest, &writeback)?;
        Ok(writeback)
    }

    pub fn complete_console_transmit(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioConsoleDecodedTransmit,
        completion: &VirtioConsoleCompletion,
    ) -> Result<VirtioConsoleQueueCompletionWrite, VirtioError> {
        let used_index = guest.read_u16(add_address(self.used_ring(), 2)?)?;
        let mut used_ring = VirtioSplitUsedRing::new(self.queue_size(), used_index)?;
        let writeback = used_ring.complete_console_transmit(decoded, completion)?;
        self.write_console_completion(guest, &writeback)?;
        Ok(writeback)
    }

    pub fn complete_console_receive_and_raise_isr(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioConsoleDecodedReceive,
        completion: &VirtioConsoleCompletion,
        isr: &VirtioPciIsrDevice,
    ) -> Result<VirtioConsoleQueueCompletionWrite, VirtioError> {
        let writeback = self.complete_console_receive(guest, decoded, completion)?;
        if self.console_completion_interrupt_enabled(guest, writeback.used_index())? {
            isr.raise_queue_interrupt(completion.tick());
        }
        Ok(writeback)
    }

    pub fn complete_console_transmit_and_raise_isr(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioConsoleDecodedTransmit,
        completion: &VirtioConsoleCompletion,
        isr: &VirtioPciIsrDevice,
    ) -> Result<VirtioConsoleQueueCompletionWrite, VirtioError> {
        let writeback = self.complete_console_transmit(guest, decoded, completion)?;
        if self.console_completion_interrupt_enabled(guest, writeback.used_index())? {
            isr.raise_queue_interrupt(completion.tick());
        }
        Ok(writeback)
    }

    fn write_console_completion(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        writeback: &VirtioConsoleQueueCompletionWrite,
    ) -> Result<(), VirtioError> {
        for write in writeback.data_writes() {
            let address =
                write
                    .address()
                    .ok_or_else(|| VirtioError::PciTransportRuntimeConfig {
                        message: format!(
                            "VirtIO console descriptor {} has no guest address for writeback",
                            write.descriptor()
                        ),
                    })?;
            guest.write_exact(
                add_address(address, u64::from(write.offset()))?,
                write.bytes(),
            )?;
        }
        guest.write_exact(
            add_address(self.used_ring(), 4 + u64::from(writeback.used_slot()) * 8)?,
            &writeback.used_element().to_le_bytes(),
        )?;
        guest.write_u16(add_address(self.used_ring(), 2)?, writeback.used_index())
    }

    fn console_completion_interrupt_enabled(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        used_index: u16,
    ) -> Result<bool, VirtioError> {
        if self.event_index_enabled() {
            let used_event = guest.read_u16(add_address(
                self.available_ring(),
                4 + u64::from(self.queue_size()) * 2,
            )?)?;
            let previous_used_index = used_index.wrapping_sub(1);
            return Ok(previous_used_index == used_event);
        }
        let flags = guest.read_u16(self.available_ring())?;
        Ok(flags & crate::VIRTIO_SPLIT_AVAIL_F_NO_INTERRUPT == 0)
    }
}

impl VirtioSplitUsedRing {
    pub fn complete_console_receive(
        &mut self,
        decoded: &VirtioConsoleDecodedReceive,
        completion: &VirtioConsoleCompletion,
    ) -> Result<VirtioConsoleQueueCompletionWrite, VirtioError> {
        if completion.kind() != VirtioConsoleTransferKind::Receive
            || completion.request() != decoded.request().id()
            || completion.queue() != decoded.request().queue()
        {
            return Err(VirtioError::PciTransportRuntimeConfig {
                message:
                    "VirtIO console receive completion does not match decoded descriptor chain"
                        .into(),
            });
        }
        let data_writes = decoded.data_writes(completion.bytes())?;
        self.complete_console_request(
            decoded.request().id(),
            completion.used_length(),
            data_writes,
        )
    }

    pub fn complete_console_transmit(
        &mut self,
        decoded: &VirtioConsoleDecodedTransmit,
        completion: &VirtioConsoleCompletion,
    ) -> Result<VirtioConsoleQueueCompletionWrite, VirtioError> {
        if completion.kind() != VirtioConsoleTransferKind::Transmit
            || completion.request() != decoded.request().id()
            || completion.queue() != decoded.request().queue()
        {
            return Err(VirtioError::PciTransportRuntimeConfig {
                message:
                    "VirtIO console transmit completion does not match decoded descriptor chain"
                        .into(),
            });
        }
        self.complete_console_request(decoded.request().id(), 0, Vec::new())
    }

    fn complete_console_request(
        &mut self,
        id: VirtioConsoleRequestId,
        used_length: u32,
        data_writes: Vec<VirtioConsoleDescriptorWrite>,
    ) -> Result<VirtioConsoleQueueCompletionWrite, VirtioError> {
        let used_slot = self.index() % self.queue_size();
        let used_index = self.index().wrapping_add(1);
        let used_element = VirtioSplitUsedElement::new(
            u32::try_from(id.get()).map_err(|_| VirtioError::VirtioConsolePayloadLengthOverflow)?,
            used_length,
        );
        self.set_entry(used_slot, used_element);
        self.set_index(used_index);
        Ok(VirtioConsoleQueueCompletionWrite {
            data_writes,
            used_slot,
            used_index,
            used_element,
        })
    }
}
