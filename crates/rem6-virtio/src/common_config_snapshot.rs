use std::collections::BTreeMap;

use super::{
    validate_queue_size, VirtioError, VirtioPciCommonSnapshot, VirtioPciCommonState,
    VirtioQueueState,
};

const VIRTIO_COMMON_SNAPSHOT_MAGIC: &[u8; 8] = b"VIOCOMM1";
const VIRTIO_COMMON_SNAPSHOT_VERSION: u16 = 1;
const VIRTIO_COMMON_FEATURE_ENTRY_BYTES: usize = 8;
const VIRTIO_COMMON_QUEUE_ENTRY_BYTES: usize = 35;
const U16_BYTES: usize = 2;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

impl VirtioPciCommonSnapshot {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(VIRTIO_COMMON_SNAPSHOT_MAGIC);
        payload.extend_from_slice(&VIRTIO_COMMON_SNAPSHOT_VERSION.to_le_bytes());
        encode_feature_map(&mut payload, &self.state.device_features);
        encode_feature_map(&mut payload, &self.state.driver_features);
        payload.extend_from_slice(&self.state.device_feature_select.to_le_bytes());
        payload.extend_from_slice(&self.state.driver_feature_select.to_le_bytes());
        payload.extend_from_slice(&self.state.config_msix_vector.to_le_bytes());
        payload.push(self.state.device_status);
        payload.push(self.state.config_generation);
        payload.extend_from_slice(&self.state.queue_select.to_le_bytes());
        payload.extend_from_slice(&self.state.admin_queue_index.to_le_bytes());
        payload.extend_from_slice(&self.state.admin_queue_num.to_le_bytes());
        payload.extend_from_slice(&(self.state.queues.len() as u64).to_le_bytes());
        for queue in &self.state.queues {
            payload.extend_from_slice(&queue.max_size.to_le_bytes());
            payload.extend_from_slice(&queue.size.to_le_bytes());
            payload.extend_from_slice(&queue.notify_offset.to_le_bytes());
            payload.extend_from_slice(&queue.notify_config_data.to_le_bytes());
            payload.extend_from_slice(&queue.msix_vector.to_le_bytes());
            payload.push(u8::from(queue.enabled));
            payload.extend_from_slice(&queue.desc_address.to_le_bytes());
            payload.extend_from_slice(&queue.driver_address.to_le_bytes());
            payload.extend_from_slice(&queue.device_address.to_le_bytes());
        }
        payload
    }

    pub fn from_bytes(payload: &[u8]) -> Result<Self, VirtioError> {
        let mut cursor = VirtioPciCommonSnapshotCursor::new(payload);
        cursor.read_magic()?;
        if cursor.read_u16()? != VIRTIO_COMMON_SNAPSHOT_VERSION {
            return Err(VirtioError::InvalidCommonConfigSnapshot);
        }
        let device_features = cursor.read_feature_map()?;
        let driver_features = cursor.read_feature_map()?;
        let device_feature_select = cursor.read_u32()?;
        let driver_feature_select = cursor.read_u32()?;
        let config_msix_vector = cursor.read_u16()?;
        let device_status = cursor.read_u8()?;
        let config_generation = cursor.read_u8()?;
        let queue_select = cursor.read_u16()?;
        let admin_queue_index = cursor.read_u16()?;
        let admin_queue_num = cursor.read_u16()?;
        let queue_count = usize::try_from(cursor.read_u64()?)
            .map_err(|_| VirtioError::InvalidCommonConfigSnapshot)?;
        if queue_count > u16::MAX as usize
            || queue_count > cursor.remaining() / VIRTIO_COMMON_QUEUE_ENTRY_BYTES
        {
            return Err(VirtioError::InvalidCommonConfigSnapshot);
        }
        let mut queues = Vec::with_capacity(queue_count);
        for index in 0..queue_count {
            queues.push(cursor.read_queue_state(index as u16)?);
        }
        cursor.finish()?;
        Ok(Self {
            state: VirtioPciCommonState {
                device_features,
                driver_features,
                device_feature_select,
                driver_feature_select,
                config_msix_vector,
                device_status,
                config_generation,
                queue_select,
                queues,
                admin_queue_index,
                admin_queue_num,
            },
        })
    }
}

fn encode_feature_map(payload: &mut Vec<u8>, features: &BTreeMap<u32, u32>) {
    payload.extend_from_slice(&(features.len() as u64).to_le_bytes());
    for (page, value) in features {
        payload.extend_from_slice(&page.to_le_bytes());
        payload.extend_from_slice(&value.to_le_bytes());
    }
}

struct VirtioPciCommonSnapshotCursor<'a> {
    payload: &'a [u8],
    offset: usize,
}

impl<'a> VirtioPciCommonSnapshotCursor<'a> {
    fn new(payload: &'a [u8]) -> Self {
        Self { payload, offset: 0 }
    }

    fn read_magic(&mut self) -> Result<(), VirtioError> {
        if self.read_exact(VIRTIO_COMMON_SNAPSHOT_MAGIC.len())? == VIRTIO_COMMON_SNAPSHOT_MAGIC {
            Ok(())
        } else {
            Err(VirtioError::InvalidCommonConfigSnapshot)
        }
    }

    fn read_feature_map(&mut self) -> Result<BTreeMap<u32, u32>, VirtioError> {
        let count = usize::try_from(self.read_u64()?)
            .map_err(|_| VirtioError::InvalidCommonConfigSnapshot)?;
        if count > self.remaining() / VIRTIO_COMMON_FEATURE_ENTRY_BYTES {
            return Err(VirtioError::InvalidCommonConfigSnapshot);
        }
        let mut features = BTreeMap::new();
        for _ in 0..count {
            let page = self.read_u32()?;
            let value = self.read_u32()?;
            if features.insert(page, value).is_some() {
                return Err(VirtioError::InvalidCommonConfigSnapshot);
            }
        }
        Ok(features)
    }

    fn read_queue_state(&mut self, index: u16) -> Result<VirtioQueueState, VirtioError> {
        let queue = VirtioQueueState {
            max_size: self.read_u16()?,
            size: self.read_u16()?,
            notify_offset: self.read_u16()?,
            notify_config_data: self.read_u16()?,
            msix_vector: self.read_u16()?,
            enabled: match self.read_u8()? {
                0 => false,
                1 => true,
                _ => return Err(VirtioError::InvalidCommonConfigSnapshot),
            },
            desc_address: self.read_u64()?,
            driver_address: self.read_u64()?,
            device_address: self.read_u64()?,
        };
        validate_decoded_queue_state(index, &queue)?;
        Ok(queue)
    }

    fn read_u8(&mut self) -> Result<u8, VirtioError> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, VirtioError> {
        let bytes = self.read_exact(U16_BYTES)?;
        Ok(u16::from_le_bytes(
            bytes.try_into().expect("snapshot u16 width is fixed"),
        ))
    }

    fn read_u32(&mut self) -> Result<u32, VirtioError> {
        let bytes = self.read_exact(U32_BYTES)?;
        Ok(u32::from_le_bytes(
            bytes.try_into().expect("snapshot u32 width is fixed"),
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
            .ok_or(VirtioError::InvalidCommonConfigSnapshot)?;
        let bytes = self
            .payload
            .get(self.offset..end)
            .ok_or(VirtioError::InvalidCommonConfigSnapshot)?;
        self.offset = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), VirtioError> {
        if self.offset == self.payload.len() {
            Ok(())
        } else {
            Err(VirtioError::InvalidCommonConfigSnapshot)
        }
    }

    fn remaining(&self) -> usize {
        self.payload.len() - self.offset
    }
}

fn validate_decoded_queue_state(index: u16, queue: &VirtioQueueState) -> Result<(), VirtioError> {
    if queue.max_size == 0 || !queue.max_size.is_power_of_two() {
        return Err(VirtioError::InvalidCommonConfigSnapshot);
    }
    if queue.size == 0 || !queue.size.is_power_of_two() || queue.size > queue.max_size {
        return Err(VirtioError::InvalidCommonConfigSnapshot);
    }
    validate_queue_size(index, queue.max_size).map_err(|_| VirtioError::InvalidCommonConfigSnapshot)
}
