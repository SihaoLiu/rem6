use crate::fs9p_device_helpers::xattr_write_policy;
use crate::fs9p_namespace::Virtio9pFidState;
use crate::fs9p_protocol::*;
use crate::{Virtio9pRequest, VirtioError};

use super::super::Virtio9pDevice;

impl Virtio9pDevice {
    pub(in crate::fs9p) fn handle_xattrwalk(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let xattrwalk = parse_xattrwalk_request(request)?;
        let node = {
            let fids = self.fids.lock().expect("virtio 9p fid lock");
            if fids.contains_key(&xattrwalk.newfid) {
                return Ok(Err(VIRTIO_9P_EBADF));
            }
            fids.get(&xattrwalk.fid).and_then(Virtio9pFidState::node)
        };
        let Some(node) = node else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let data = {
            let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
            if namespace.metadata(node).is_none() {
                return Ok(Err(VIRTIO_9P_EBADF));
            }
            if xattrwalk.name.is_empty() {
                namespace.xattr_list(node)
            } else {
                namespace
                    .read_xattr(node, &xattrwalk.name)
                    .map(std::borrow::ToOwned::to_owned)
            }
        };
        let Some(data) = data else {
            return Ok(Err(VIRTIO_9P_ENODATA));
        };
        let size =
            u64::try_from(data.len()).map_err(|_| VirtioError::Virtio9pPayloadLengthOverflow)?;
        let mut fids = self.fids.lock().expect("virtio 9p fid lock");
        if fids.contains_key(&xattrwalk.newfid) {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        fids.insert(xattrwalk.newfid, Virtio9pFidState::xattr_read(data));
        Ok(Ok(size.to_le_bytes().to_vec()))
    }

    pub(in crate::fs9p) fn handle_xattrcreate(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<(), u32>, VirtioError> {
        let xattrcreate = parse_xattrcreate_request(request)?;
        let policy = match xattr_write_policy(xattrcreate.flags) {
            Ok(policy) => policy,
            Err(errno) => return Ok(Err(errno)),
        };
        let Some(node) = self.fid_node(xattrcreate.fid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        {
            let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
            if namespace.metadata(node).is_none() {
                return Ok(Err(VIRTIO_9P_EBADF));
            }
            if let Err(errno) = namespace.prepare_xattr_write(node, &xattrcreate.name, policy) {
                return Ok(Err(errno));
            }
        }
        let Some(fid) =
            Virtio9pFidState::xattr_write(node, xattrcreate.name, xattrcreate.attr_size, policy)
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        self.fids
            .lock()
            .expect("virtio 9p fid lock")
            .insert(xattrcreate.fid, fid);
        Ok(Ok(()))
    }
}
