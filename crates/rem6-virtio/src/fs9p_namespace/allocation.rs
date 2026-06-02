use std::collections::BTreeMap;

use crate::fs9p_protocol::VIRTIO_9P_EINVAL;

use super::{Virtio9pFileNode, Virtio9pNode};

const VIRTIO_9P_MAX_VECTOR_BYTES: usize = isize::MAX as usize;

pub(super) fn checked_vector_len(bytes: u64) -> Result<usize, u32> {
    let bytes = usize::try_from(bytes).map_err(|_| VIRTIO_9P_EINVAL)?;
    checked_vector_len_usize(bytes)
}

pub(super) fn checked_vector_len_usize(bytes: usize) -> Result<usize, u32> {
    if bytes > VIRTIO_9P_MAX_VECTOR_BYTES {
        return Err(VIRTIO_9P_EINVAL);
    }
    Ok(bytes)
}

pub(super) fn reserve_vec_len(bytes: &mut Vec<u8>, len: usize) -> Result<(), u32> {
    checked_vector_len_usize(len)?;
    if len > bytes.capacity() {
        bytes
            .try_reserve_exact(len.saturating_sub(bytes.len()))
            .map_err(|_| VIRTIO_9P_EINVAL)?;
    }
    Ok(())
}

pub(super) fn try_for_each_file_mut(
    entries: &mut BTreeMap<String, Virtio9pNode>,
    path: u64,
    update: &mut impl FnMut(&mut Virtio9pFileNode) -> Result<(), u32>,
) -> Result<(), u32> {
    for node in entries.values_mut() {
        match node {
            Virtio9pNode::File(file) if file.qid_path == path => update(file)?,
            Virtio9pNode::File(_) | Virtio9pNode::Symlink(_) | Virtio9pNode::Special(_) => {}
            Virtio9pNode::Directory(directory) => {
                try_for_each_file_mut(&mut directory.entries, path, update)?;
            }
        }
    }
    Ok(())
}
