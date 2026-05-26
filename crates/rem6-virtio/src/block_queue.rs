use std::collections::{BTreeMap, BTreeSet};

use crate::{
    VirtioBlockRequest, VirtioBlockRequestId, VirtioBlockRequestKind, VirtioError,
    VirtioQueueIndex, VIRTIO_BLOCK_T_FLUSH, VIRTIO_BLOCK_T_GET_ID, VIRTIO_BLOCK_T_IN,
    VIRTIO_BLOCK_T_OUT,
};

pub const VIRTIO_SPLIT_DESC_F_NEXT: u16 = 1;
pub const VIRTIO_SPLIT_DESC_F_WRITE: u16 = 2;
pub const VIRTIO_SPLIT_DESC_F_INDIRECT: u16 = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioSplitDescriptor {
    index: u16,
    bytes: Vec<u8>,
    length: u32,
    writable: bool,
    next: Option<u16>,
}

impl VirtioSplitDescriptor {
    pub fn device_readable(index: u16, bytes: Vec<u8>, next: Option<u16>) -> Self {
        Self {
            index,
            length: bytes.len() as u32,
            bytes,
            writable: false,
            next,
        }
    }

    pub const fn device_writable(index: u16, length: u32, next: Option<u16>) -> Self {
        Self {
            index,
            bytes: Vec::new(),
            length,
            writable: true,
            next,
        }
    }

    pub const fn index(&self) -> u16 {
        self.index
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn length(&self) -> u32 {
        self.length
    }

    pub const fn is_writable(&self) -> bool {
        self.writable
    }

    pub const fn next(&self) -> Option<u16> {
        self.next
    }

    pub const fn flags(&self) -> u16 {
        let mut flags = 0;
        if self.next.is_some() {
            flags |= VIRTIO_SPLIT_DESC_F_NEXT;
        }
        if self.writable {
            flags |= VIRTIO_SPLIT_DESC_F_WRITE;
        }
        flags
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioSplitDescriptorChain {
    head: u16,
    descriptors: Vec<VirtioSplitDescriptor>,
}

impl VirtioSplitDescriptorChain {
    pub fn new(
        head: u16,
        descriptors: impl IntoIterator<Item = VirtioSplitDescriptor>,
    ) -> Result<Self, VirtioError> {
        let mut descriptor_map = BTreeMap::new();
        for descriptor in descriptors {
            let index = descriptor.index();
            if descriptor_map.insert(index, descriptor).is_some() {
                return Err(VirtioError::DuplicateVirtioDescriptor { index });
            }
        }
        let mut ordered = Vec::new();
        let mut visited = BTreeSet::new();
        let mut cursor = head;
        loop {
            if !visited.insert(cursor) {
                return Err(VirtioError::VirtioDescriptorLoop { index: cursor });
            }
            let descriptor = descriptor_map
                .get(&cursor)
                .cloned()
                .ok_or(VirtioError::MissingVirtioDescriptor { index: cursor })?;
            let next = descriptor.next();
            ordered.push(descriptor);
            let Some(next) = next else {
                break;
            };
            cursor = next;
        }
        Ok(Self {
            head,
            descriptors: ordered,
        })
    }

    pub const fn head(&self) -> u16 {
        self.head
    }

    pub fn descriptors(&self) -> &[VirtioSplitDescriptor] {
        &self.descriptors
    }

    pub fn decode_block_request(
        &self,
        queue: VirtioQueueIndex,
    ) -> Result<VirtioBlockDecodedRequest, VirtioError> {
        let (raw_type, sector) = self.block_header()?;
        let status = self.status_descriptor()?;
        let payload = &self.descriptors[1..self.descriptors.len() - 1];
        let request = match raw_type {
            VIRTIO_BLOCK_T_IN => {
                let bytes = self.writable_payload_bytes(raw_type, payload)?;
                VirtioBlockRequest::read(
                    VirtioBlockRequestId::new(u64::from(self.head)),
                    queue,
                    sector,
                    bytes,
                )?
            }
            VIRTIO_BLOCK_T_OUT => {
                let data = self.readable_payload_bytes(raw_type, payload)?;
                VirtioBlockRequest::write(
                    VirtioBlockRequestId::new(u64::from(self.head)),
                    queue,
                    sector,
                    data,
                )?
            }
            VIRTIO_BLOCK_T_FLUSH => {
                self.require_no_payload(raw_type, payload)?;
                VirtioBlockRequest::flush(VirtioBlockRequestId::new(u64::from(self.head)), queue)?
            }
            VIRTIO_BLOCK_T_GET_ID => {
                let bytes = self.writable_payload_bytes(raw_type, payload)?;
                if bytes < 20 {
                    return Err(VirtioError::InvalidVirtioBlockDeviceIdOutput { bytes });
                }
                VirtioBlockRequest::get_id(VirtioBlockRequestId::new(u64::from(self.head)), queue)
            }
            _ => VirtioBlockRequest::unsupported(
                VirtioBlockRequestId::new(u64::from(self.head)),
                queue,
                raw_type,
                self.readable_payload_bytes_lossy(payload),
            )?,
        };
        let writable_data_bytes = match request.kind() {
            VirtioBlockRequestKind::Read { bytes } => *bytes,
            VirtioBlockRequestKind::GetId => self.writable_payload_bytes(raw_type, payload)?,
            _ => 0,
        };
        Ok(VirtioBlockDecodedRequest {
            request,
            status_descriptor: status.index(),
            writable_data_bytes,
        })
    }

    fn block_header(&self) -> Result<(u32, u64), VirtioError> {
        let Some(first) = self.descriptors.first() else {
            return Err(VirtioError::ShortVirtioBlockHeader { bytes: 0 });
        };
        if first.is_writable() {
            return Err(VirtioError::InvalidVirtioBlockReadableDescriptor {
                raw_type: 0,
                index: first.index(),
            });
        }
        if first.bytes().len() < 16 {
            return Err(VirtioError::ShortVirtioBlockHeader {
                bytes: first.bytes().len() as u64,
            });
        }
        let raw_type = u32::from_le_bytes(first.bytes()[0..4].try_into().unwrap());
        let sector = u64::from_le_bytes(first.bytes()[8..16].try_into().unwrap());
        Ok((raw_type, sector))
    }

    fn status_descriptor(&self) -> Result<&VirtioSplitDescriptor, VirtioError> {
        let Some(status) = self.descriptors.last() else {
            return Err(VirtioError::MissingVirtioBlockStatusDescriptor);
        };
        if !status.is_writable() || status.length() < 1 {
            return Err(VirtioError::InvalidVirtioBlockStatusDescriptor {
                index: status.index(),
                length: status.length(),
                writable: status.is_writable(),
            });
        }
        if self.descriptors.len() < 2 {
            return Err(VirtioError::MissingVirtioBlockStatusDescriptor);
        }
        Ok(status)
    }

    fn readable_payload_bytes(
        &self,
        raw_type: u32,
        payload: &[VirtioSplitDescriptor],
    ) -> Result<Vec<u8>, VirtioError> {
        let mut bytes = Vec::new();
        for descriptor in payload {
            if descriptor.is_writable() {
                return Err(VirtioError::InvalidVirtioBlockReadableDescriptor {
                    raw_type,
                    index: descriptor.index(),
                });
            }
            bytes.extend_from_slice(descriptor.bytes());
        }
        Ok(bytes)
    }

    fn readable_payload_bytes_lossy(&self, payload: &[VirtioSplitDescriptor]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for descriptor in payload {
            if !descriptor.is_writable() {
                bytes.extend_from_slice(descriptor.bytes());
            }
        }
        bytes
    }

    fn writable_payload_bytes(
        &self,
        raw_type: u32,
        payload: &[VirtioSplitDescriptor],
    ) -> Result<u64, VirtioError> {
        let mut bytes = 0_u64;
        for descriptor in payload {
            if !descriptor.is_writable() {
                return Err(VirtioError::InvalidVirtioBlockWritableDescriptor {
                    raw_type,
                    index: descriptor.index(),
                });
            }
            bytes = bytes
                .checked_add(u64::from(descriptor.length()))
                .ok_or(VirtioError::VirtioBlockPayloadLengthOverflow { raw_type })?;
        }
        Ok(bytes)
    }

    fn require_no_payload(
        &self,
        raw_type: u32,
        payload: &[VirtioSplitDescriptor],
    ) -> Result<(), VirtioError> {
        if let Some(descriptor) = payload.first() {
            if descriptor.is_writable() {
                return Err(VirtioError::InvalidVirtioBlockReadableDescriptor {
                    raw_type,
                    index: descriptor.index(),
                });
            }
            return Err(VirtioError::InvalidVirtioBlockWritableDescriptor {
                raw_type,
                index: descriptor.index(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioBlockDecodedRequest {
    request: VirtioBlockRequest,
    status_descriptor: u16,
    writable_data_bytes: u64,
}

impl VirtioBlockDecodedRequest {
    pub const fn request(&self) -> &VirtioBlockRequest {
        &self.request
    }

    pub const fn status_descriptor(&self) -> u16 {
        self.status_descriptor
    }

    pub const fn writable_data_bytes(&self) -> u64 {
        self.writable_data_bytes
    }

    pub fn into_request(self) -> VirtioBlockRequest {
        self.request
    }
}
