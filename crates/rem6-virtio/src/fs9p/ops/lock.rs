use crate::fs9p_device_helpers::{valid_lock_flags, valid_lock_type};
use crate::fs9p_protocol::*;
use crate::{Virtio9pRequest, VirtioError};

use super::super::Virtio9pDevice;

impl Virtio9pDevice {
    pub(in crate::fs9p) fn handle_lock(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<u8, u32>, VirtioError> {
        let lock = parse_lock_request(request)?;
        if !valid_lock_type(lock.lock_type) {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        if !valid_lock_flags(lock.flags) {
            return Ok(Err(VIRTIO_9P_EINVAL));
        }
        let Some(node) = self.lockable_node(lock.fid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let status = self
            .locks
            .lock()
            .expect("virtio 9p lock table")
            .apply(node, &lock);
        Ok(Ok(status))
    }

    pub(in crate::fs9p) fn handle_getlock(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let lock = parse_getlock_request(request)?;
        if !valid_lock_type(lock.lock_type) {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        if !valid_lock_flags(lock.flags) {
            return Ok(Err(VIRTIO_9P_EINVAL));
        }
        let Some(node) = self.lockable_node(lock.fid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        Ok(Ok(self
            .locks
            .lock()
            .expect("virtio 9p lock table")
            .conflict_payload(node, &lock)))
    }
}
