use crate::fs9p_namespace::Virtio9pXattrWritePolicy;
use crate::fs9p_protocol::{
    VIRTIO_9P_EINVAL, VIRTIO_9P_LOCK_FLAGS_BLOCK, VIRTIO_9P_LOCK_FLAGS_RECLAIM,
    VIRTIO_9P_LOCK_TYPE_RDLCK, VIRTIO_9P_LOCK_TYPE_UNLCK, VIRTIO_9P_LOCK_TYPE_WRLCK,
    VIRTIO_9P_XATTR_CREATE, VIRTIO_9P_XATTR_REPLACE,
};

pub(crate) const fn valid_lock_type(lock_type: u8) -> bool {
    matches!(
        lock_type,
        VIRTIO_9P_LOCK_TYPE_RDLCK | VIRTIO_9P_LOCK_TYPE_WRLCK | VIRTIO_9P_LOCK_TYPE_UNLCK
    )
}

pub(crate) const fn valid_lock_flags(flags: u32) -> bool {
    let known_flags = VIRTIO_9P_LOCK_FLAGS_BLOCK | VIRTIO_9P_LOCK_FLAGS_RECLAIM;
    flags & !known_flags == 0
}

pub(crate) const fn xattr_write_policy(flags: u32) -> Result<Virtio9pXattrWritePolicy, u32> {
    match flags {
        0 => Ok(Virtio9pXattrWritePolicy::Any),
        VIRTIO_9P_XATTR_CREATE => Ok(Virtio9pXattrWritePolicy::Create),
        VIRTIO_9P_XATTR_REPLACE => Ok(Virtio9pXattrWritePolicy::Replace),
        _ => Err(VIRTIO_9P_EINVAL),
    }
}

pub(crate) fn read_data_slice(data: &[u8], offset: u64, count: u32) -> Option<Vec<u8>> {
    let start = usize::try_from(offset).ok()?;
    if start >= data.len() {
        return Some(Vec::new());
    }
    let count = usize::try_from(count).ok()?;
    let end = start.saturating_add(count).min(data.len());
    Some(data[start..end].to_vec())
}

pub(crate) fn counted_data_payload(data: Vec<u8>) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend((data.len() as u32).to_le_bytes());
    payload.extend(data);
    payload
}
