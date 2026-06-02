use crate::fs9p_device_helpers::{counted_data_payload, read_data_slice};
use crate::fs9p_protocol::*;
use crate::{Virtio9pRequest, VirtioError};

use super::super::Virtio9pDevice;

impl Virtio9pDevice {
    pub(in crate::fs9p) fn handle_read(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let read = parse_read_request(request)?;
        let Some(fid) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&read.fid)
            .cloned()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if let Some(data) = fid.xattr_data() {
            let data = read_data_slice(data, read.offset, self.counted_data_limit(read.count))
                .ok_or(VirtioError::Virtio9pPayloadLengthOverflow)?;
            return Ok(Ok(counted_data_payload(data)));
        }
        if !fid.can_read() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let Some(data) =
            namespace.read_file(node, read.offset, self.counted_data_limit(read.count))
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        Ok(Ok(counted_data_payload(data)))
    }

    pub(in crate::fs9p) fn handle_write(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let write = parse_write_request(request)?;
        let fid = {
            let mut fids = self.fids.lock().expect("virtio 9p fid lock");
            let Some(fid) = fids.get_mut(&write.fid) else {
                return Ok(Err(VIRTIO_9P_EBADF));
            };
            match fid.write_xattr_data(write.offset, &write.data) {
                Ok(Some(bytes)) => return Ok(Ok(bytes.to_le_bytes().to_vec())),
                Ok(None) => {}
                Err(errno) => return Ok(Err(errno)),
            }
            fid.clone()
        };
        if !fid.can_write() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let offset = if fid.append_writes() {
            let Some(metadata) = namespace.metadata(node) else {
                return Ok(Err(VIRTIO_9P_EBADF));
            };
            metadata.size()
        } else {
            write.offset
        };
        let bytes = match namespace.write_file(node, offset, &write.data) {
            Ok(bytes) => bytes,
            Err(errno) => return Ok(Err(errno)),
        };
        Ok(Ok(bytes.to_le_bytes().to_vec()))
    }

    pub(in crate::fs9p) fn handle_clunk(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<(), u32>, VirtioError> {
        let fid = parse_clunk_request(request)?;
        let removed = self.fids.lock().expect("virtio 9p fid lock").remove(&fid);
        let Some(removed) = removed else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        self.locks
            .lock()
            .expect("virtio 9p lock table")
            .remove_fid(fid);
        if removed.remove_on_clunk() {
            let Some(node) = removed.node() else {
                return Ok(Err(VIRTIO_9P_EBADF));
            };
            return Ok(self.remove_node_for_fid_path(node, removed.path()));
        }
        let commit = match removed.into_xattr_commit() {
            Ok(commit) => commit,
            Err(errno) => return Ok(Err(errno)),
        };
        if let Some(commit) = commit {
            if let Err(errno) = self
                .namespace
                .lock()
                .expect("virtio 9p namespace lock")
                .write_xattr(commit.node, commit.name, commit.data, commit.policy)
            {
                return Ok(Err(errno));
            }
        }
        Ok(Ok(()))
    }

    pub(in crate::fs9p) fn handle_remove(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<(), u32>, VirtioError> {
        let remove_fid = parse_remove_request(request)?;
        let Some(fid) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .remove(&remove_fid)
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        self.locks
            .lock()
            .expect("virtio 9p lock table")
            .remove_fid(remove_fid);
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        Ok(self.remove_node_for_fid_path(node, fid.path()))
    }

    pub(in crate::fs9p) fn handle_readdir(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let readdir = parse_readdir_request(request)?;
        let Some(fid) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&readdir.fid)
            .cloned()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if !fid.can_read() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let count = self.counted_data_limit(readdir.count);
        let Some(payload) = namespace.readdir_payload(node, readdir.offset, count) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        Ok(Ok(payload))
    }

    pub(in crate::fs9p) fn handle_fsync(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<(), u32>, VirtioError> {
        let fsync = parse_fsync_request(request)?;
        if self.fid_node(fsync.fid).is_some() {
            Ok(Ok(()))
        } else {
            Ok(Err(VIRTIO_9P_EBADF))
        }
    }
}
