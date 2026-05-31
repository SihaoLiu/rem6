use rem6_memory::Address;

use crate::{
    block_queue::add_address, VirtioError, VirtioGuestMemory, VirtioPciIsrDevice, VirtioQueueIndex,
    VirtioSplitDescriptorChain, VirtioSplitQueue, VirtioSplitUsedElement, VirtioSplitUsedRing,
};

pub const VIRTIO_9P_HEADER_BYTES: usize = 7;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Virtio9pRequestId(u64);

impl Virtio9pRequestId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Virtio9pRequest {
    id: Virtio9pRequestId,
    queue: VirtioQueueIndex,
    message_type: u8,
    tag: u16,
    payload: Vec<u8>,
}

impl Virtio9pRequest {
    fn new(
        id: Virtio9pRequestId,
        queue: VirtioQueueIndex,
        message_type: u8,
        tag: u16,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            id,
            queue,
            message_type,
            tag,
            payload,
        }
    }

    pub const fn id(&self) -> Virtio9pRequestId {
        self.id
    }

    pub const fn queue(&self) -> VirtioQueueIndex {
        self.queue
    }

    pub const fn message_type(&self) -> u8 {
        self.message_type
    }

    pub const fn tag(&self) -> u16 {
        self.tag
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Virtio9pCompletion {
    request: Virtio9pRequestId,
    queue: VirtioQueueIndex,
    tick: u64,
    message_type: u8,
    tag: u16,
    payload: Vec<u8>,
}

impl Virtio9pCompletion {
    pub fn new(
        request: Virtio9pRequestId,
        queue: VirtioQueueIndex,
        tick: u64,
        message_type: u8,
        tag: u16,
        payload: Vec<u8>,
    ) -> Result<Self, VirtioError> {
        let len = VIRTIO_9P_HEADER_BYTES
            .checked_add(payload.len())
            .ok_or(VirtioError::Virtio9pPayloadLengthOverflow)?;
        u32::try_from(len).map_err(|_| VirtioError::Virtio9pPayloadLengthOverflow)?;
        Ok(Self {
            request,
            queue,
            tick,
            message_type,
            tag,
            payload,
        })
    }

    pub const fn request(&self) -> Virtio9pRequestId {
        self.request
    }

    pub const fn queue(&self) -> VirtioQueueIndex {
        self.queue
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn message_type(&self) -> u8 {
        self.message_type
    }

    pub const fn tag(&self) -> u16 {
        self.tag
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    fn to_message_bytes(&self) -> Vec<u8> {
        let len = VIRTIO_9P_HEADER_BYTES + self.payload.len();
        let mut bytes = Vec::with_capacity(len);
        bytes.extend((len as u32).to_le_bytes());
        bytes.push(self.message_type);
        bytes.extend(self.tag.to_le_bytes());
        bytes.extend_from_slice(&self.payload);
        bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Virtio9pDecodedRequest {
    request: Virtio9pRequest,
    writable_descriptors: Vec<Virtio9pWritableDescriptor>,
    writable_data_bytes: u64,
}

impl Virtio9pDecodedRequest {
    pub const fn request(&self) -> &Virtio9pRequest {
        &self.request
    }

    pub const fn writable_data_bytes(&self) -> u64 {
        self.writable_data_bytes
    }

    pub fn into_request(self) -> Virtio9pRequest {
        self.request
    }

    fn data_writes(&self, data: &[u8]) -> Result<Vec<Virtio9pDescriptorWrite>, VirtioError> {
        if data.len() as u64 > self.writable_data_bytes {
            return Err(VirtioError::Virtio9pPayloadLengthOverflow);
        }
        let mut cursor = data;
        let mut writes = Vec::new();
        for target in &self.writable_descriptors {
            if cursor.is_empty() {
                break;
            }
            let bytes = cursor.len().min(target.length() as usize);
            writes.push(Virtio9pDescriptorWrite {
                descriptor: target.index(),
                address: target.address(),
                offset: 0,
                bytes: cursor[..bytes].to_vec(),
            });
            cursor = &cursor[bytes..];
        }
        if !cursor.is_empty() {
            return Err(VirtioError::Virtio9pPayloadLengthOverflow);
        }
        Ok(writes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Virtio9pWritableDescriptor {
    index: u16,
    address: Option<Address>,
    length: u32,
}

impl Virtio9pWritableDescriptor {
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
pub struct Virtio9pDescriptorWrite {
    descriptor: u16,
    address: Option<Address>,
    offset: u32,
    bytes: Vec<u8>,
}

impl Virtio9pDescriptorWrite {
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
pub struct Virtio9pQueueCompletionWrite {
    data_writes: Vec<Virtio9pDescriptorWrite>,
    used_slot: u16,
    used_index: u16,
    used_element: VirtioSplitUsedElement,
}

impl Virtio9pQueueCompletionWrite {
    pub fn data_writes(&self) -> &[Virtio9pDescriptorWrite] {
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
    pub fn decode_9p_request(
        &self,
        queue: VirtioQueueIndex,
    ) -> Result<Virtio9pDecodedRequest, VirtioError> {
        let mut message = Vec::new();
        let mut writable_descriptors = Vec::new();
        let mut writable_data_bytes = 0_u64;
        let mut declared_message_len = None;
        let mut request_complete = false;

        for descriptor in self.descriptors() {
            if request_complete {
                if !descriptor.is_writable() {
                    return Err(VirtioError::InvalidVirtio9pWritableDescriptor {
                        index: descriptor.index(),
                    });
                }
                writable_data_bytes = writable_data_bytes
                    .checked_add(u64::from(descriptor.length()))
                    .ok_or(VirtioError::Virtio9pPayloadLengthOverflow)?;
                writable_descriptors.push(Virtio9pWritableDescriptor {
                    index: descriptor.index(),
                    address: descriptor.address(),
                    length: descriptor.length(),
                });
                continue;
            }

            if descriptor.is_writable() {
                if message.len() < VIRTIO_9P_HEADER_BYTES {
                    return Err(VirtioError::ShortVirtio9pHeader {
                        bytes: message.len() as u64,
                    });
                }
                let declared = declared_message_len.unwrap_or_else(|| {
                    u32::from_le_bytes(message[0..4].try_into().unwrap()) as usize
                });
                let actual = u32::try_from(message.len())
                    .map_err(|_| VirtioError::Virtio9pPayloadLengthOverflow)?;
                if message.len() != declared {
                    return Err(VirtioError::InvalidVirtio9pMessageLength {
                        declared: declared as u32,
                        actual,
                    });
                }
                request_complete = true;
                writable_data_bytes = writable_data_bytes
                    .checked_add(u64::from(descriptor.length()))
                    .ok_or(VirtioError::Virtio9pPayloadLengthOverflow)?;
                writable_descriptors.push(Virtio9pWritableDescriptor {
                    index: descriptor.index(),
                    address: descriptor.address(),
                    length: descriptor.length(),
                });
                continue;
            }

            let before = message.len();
            message.extend_from_slice(descriptor.bytes());
            if declared_message_len.is_none() && message.len() >= 4 {
                let declared = u32::from_le_bytes(message[0..4].try_into().unwrap()) as usize;
                declared_message_len = Some(declared);
            }
            if let Some(declared) = declared_message_len {
                if message.len() > declared {
                    if before >= declared {
                        return Err(VirtioError::InvalidVirtio9pWritableDescriptor {
                            index: descriptor.index(),
                        });
                    }
                    let actual = u32::try_from(message.len())
                        .map_err(|_| VirtioError::Virtio9pPayloadLengthOverflow)?;
                    return Err(VirtioError::InvalidVirtio9pMessageLength {
                        declared: declared as u32,
                        actual,
                    });
                }
                request_complete = message.len() == declared;
            }
        }
        if writable_descriptors.is_empty() {
            return Err(VirtioError::MissingVirtio9pWritableDescriptor);
        }
        if message.len() < VIRTIO_9P_HEADER_BYTES {
            return Err(VirtioError::ShortVirtio9pHeader {
                bytes: message.len() as u64,
            });
        }

        let declared = u32::from_le_bytes(message[0..4].try_into().unwrap());
        let actual =
            u32::try_from(message.len()).map_err(|_| VirtioError::Virtio9pPayloadLengthOverflow)?;
        if declared != actual {
            return Err(VirtioError::InvalidVirtio9pMessageLength { declared, actual });
        }
        let message_type = message[4];
        let tag = u16::from_le_bytes(message[5..7].try_into().unwrap());
        let payload = message[VIRTIO_9P_HEADER_BYTES..].to_vec();
        let request = Virtio9pRequest::new(
            Virtio9pRequestId::new(u64::from(self.head())),
            queue,
            message_type,
            tag,
            payload,
        );
        Ok(Virtio9pDecodedRequest {
            request,
            writable_descriptors,
            writable_data_bytes,
        })
    }
}

impl VirtioSplitQueue {
    pub fn consume_available_9p(
        &mut self,
        guest: &mut VirtioGuestMemory<'_>,
        queue: VirtioQueueIndex,
    ) -> Result<Option<Virtio9pDecodedRequest>, VirtioError> {
        let Some(chain) = self.consume_available_chain(guest)? else {
            return Ok(None);
        };
        let decoded = chain.decode_9p_request(queue)?;
        self.advance_available_index();
        Ok(Some(decoded))
    }

    pub fn complete_9p_request(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &Virtio9pDecodedRequest,
        completion: &Virtio9pCompletion,
    ) -> Result<Virtio9pQueueCompletionWrite, VirtioError> {
        let used_index = guest.read_u16(add_address(self.used_ring(), 2)?)?;
        let mut used_ring = VirtioSplitUsedRing::new(self.queue_size(), used_index)?;
        let writeback = used_ring.complete_9p_request(decoded, completion)?;
        self.write_9p_completion(guest, &writeback)?;
        Ok(writeback)
    }

    pub fn complete_9p_request_and_raise_isr(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &Virtio9pDecodedRequest,
        completion: &Virtio9pCompletion,
        isr: &VirtioPciIsrDevice,
    ) -> Result<Virtio9pQueueCompletionWrite, VirtioError> {
        let writeback = self.complete_9p_request(guest, decoded, completion)?;
        if self.completion_interrupt_enabled(guest, writeback.used_index())? {
            isr.raise_queue_interrupt(completion.tick());
        }
        Ok(writeback)
    }

    fn write_9p_completion(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        writeback: &Virtio9pQueueCompletionWrite,
    ) -> Result<(), VirtioError> {
        for write in writeback.data_writes() {
            let address =
                write
                    .address()
                    .ok_or_else(|| VirtioError::PciTransportRuntimeConfig {
                        message: format!(
                            "VirtIO 9p descriptor {} has no guest address for writeback",
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
}

impl VirtioSplitUsedRing {
    pub fn complete_9p_request(
        &mut self,
        decoded: &Virtio9pDecodedRequest,
        completion: &Virtio9pCompletion,
    ) -> Result<Virtio9pQueueCompletionWrite, VirtioError> {
        if completion.request() != decoded.request().id()
            || completion.queue() != decoded.request().queue()
            || completion.tag() != decoded.request().tag()
        {
            return Err(VirtioError::PciTransportRuntimeConfig {
                message: "VirtIO 9p completion does not match decoded descriptor chain".into(),
            });
        }
        let message = completion.to_message_bytes();
        let data_writes = decoded.data_writes(&message)?;
        let used_slot = self.index() % self.queue_size();
        let used_index = self.index().wrapping_add(1);
        let used_element = VirtioSplitUsedElement::new(
            u32::try_from(decoded.request().id().get())
                .map_err(|_| VirtioError::Virtio9pPayloadLengthOverflow)?,
            u32::try_from(message.len()).map_err(|_| VirtioError::Virtio9pPayloadLengthOverflow)?,
        );
        self.set_entry(used_slot, used_element);
        self.set_index(used_index);
        Ok(Virtio9pQueueCompletionWrite {
            data_writes,
            used_slot,
            used_index,
            used_element,
        })
    }
}
