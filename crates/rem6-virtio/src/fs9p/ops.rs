use crate::fs9p_device_helpers::{
    counted_data_payload, read_data_slice, valid_lock_type, xattr_write_policy,
};
use crate::fs9p_namespace::{
    getattr_payload, qid_payload, validate_file_name, Virtio9pFidPath, Virtio9pFidState,
    Virtio9pNodeId, Virtio9pOpenMode, Virtio9pQid, Virtio9pRenameOutcome, Virtio9pTimestamp,
};
use crate::fs9p_protocol::*;
use crate::{Virtio9pRequest, VirtioError};

use super::Virtio9pDevice;

impl Virtio9pDevice {
    pub(super) fn handle_attach(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let attached = parse_attach_request(request)?;
        let root_qid = self
            .namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .root_qid();
        let mut fids = self.fids.lock().expect("virtio 9p fid lock");
        if fids.contains_key(&attached.fid()) {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        fids.insert(attached.fid(), Virtio9pFidState::new(Virtio9pNodeId::Root));
        drop(fids);
        self.attached_fids
            .lock()
            .expect("virtio 9p attached fid lock")
            .push(attached);
        Ok(Ok(qid_payload(root_qid)))
    }

    pub(super) fn handle_statfs(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let fid = parse_statfs_request(request)?;
        if self.fid_node(fid).is_none() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        Ok(Ok(self
            .namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .statfs_payload()))
    }

    pub(super) fn handle_walk(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let walk = parse_walk_request(request)?;
        let start = {
            let fids = self.fids.lock().expect("virtio 9p fid lock");
            let Some(start) = fids.get(&walk.fid).cloned() else {
                return Ok(Err(VIRTIO_9P_EBADF));
            };
            if walk.fid == walk.newfid {
                if !walk.names.is_empty() {
                    return Ok(Err(VIRTIO_9P_EBADF));
                }
            } else if fids.contains_key(&walk.newfid) {
                return Ok(Err(VIRTIO_9P_EBADF));
            }
            start
        };
        let Some(mut node) = start.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(mut path) = start.path().cloned() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        for name in &walk.names {
            validate_file_name(VIRTIO_9P_TWALK, name)?;
        }
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let mut qids = Vec::new();
        let mut completed = true;
        for name in &walk.names {
            let Some(next) = namespace.walk(node, name) else {
                completed = false;
                break;
            };
            node = next;
            path.walk_component(name);
            qids.push(namespace.qid(node));
        }
        if !completed {
            if qids.is_empty() {
                return Ok(Err(VIRTIO_9P_ENOENT));
            }
            return Ok(Ok(walk_payload(&qids)));
        }
        drop(namespace);

        if walk.fid != walk.newfid {
            let mut fids = self.fids.lock().expect("virtio 9p fid lock");
            if fids.contains_key(&walk.newfid) {
                return Ok(Err(VIRTIO_9P_EBADF));
            }
            fids.insert(walk.newfid, Virtio9pFidState::new_at(node, path));
        }
        Ok(Ok(walk_payload(&qids)))
    }

    pub(super) fn handle_lopen(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let open = parse_lopen_request(request)?;
        self.open_fid_payload(
            open.fid,
            Virtio9pOpenMode::from_bits(open.mode),
            open.truncate,
            open.append,
        )
    }

    pub(super) fn handle_open(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let open = parse_open_request(request)?;
        self.open_fid_payload(
            open.fid,
            Virtio9pOpenMode::from_bits(open.mode),
            open.truncate,
            open.append,
        )
    }

    fn open_fid_payload(
        &self,
        fid: u32,
        mode: Virtio9pOpenMode,
        truncate: bool,
        append: bool,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let mut fids = self.fids.lock().expect("virtio 9p fid lock");
        let Some(fid) = fids.get_mut(&fid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if fid.is_open() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        if namespace.metadata(node).is_none() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        if truncate && (!mode.can_write() || namespace.resize_file(node, 0).is_none()) {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        if fid.open(mode, append).is_none() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let mut payload = namespace.qid(node).to_le_bytes().to_vec();
        payload.extend(self.negotiated_msize().to_le_bytes());
        Ok(Ok(payload))
    }

    pub(super) fn handle_lcreate(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let create = parse_lcreate_request(request)?;
        self.create_fid_payload(
            create.fid,
            create.name,
            Virtio9pOpenMode::from_bits(create.mode),
            create.append,
        )
    }

    pub(super) fn handle_create(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let create = parse_create_request(request)?;
        self.create_fid_payload(
            create.fid,
            create.name,
            Virtio9pOpenMode::from_bits(create.mode),
            create.append,
        )
    }

    fn create_fid_payload(
        &self,
        fid: u32,
        name: String,
        mode: Virtio9pOpenMode,
        append: bool,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let mut fids = self.fids.lock().expect("virtio 9p fid lock");
        let Some(fid) = fids.get_mut(&fid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(parent) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(parent_path) = fid.path().cloned() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if fid.is_open() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let node = match namespace.create_file(parent, name.clone())? {
            Ok(node) => node,
            Err(errno) => return Ok(Err(errno)),
        };
        *fid = Virtio9pFidState::opened_at(node, parent_path.child(name), mode, append);
        let mut payload = namespace.qid(node).to_le_bytes().to_vec();
        payload.extend(self.negotiated_msize().to_le_bytes());
        Ok(Ok(payload))
    }

    pub(super) fn handle_symlink(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let symlink = parse_symlink_request(request)?;
        let Some(parent) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&symlink.dfid)
            .cloned()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(parent) = parent.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        match namespace.create_symlink(parent, symlink.name, symlink.target)? {
            Ok(node) => Ok(Ok(qid_payload(namespace.qid(node)))),
            Err(errno) => Ok(Err(errno)),
        }
    }

    pub(super) fn handle_mknod(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let mknod = parse_mknod_request(request)?;
        let Some(parent) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&mknod.dfid)
            .cloned()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(parent) = parent.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        match namespace.create_special(parent, mknod.name, mknod.mode, mknod.major, mknod.minor)? {
            Ok(node) => Ok(Ok(qid_payload(namespace.qid(node)))),
            Err(errno) => Ok(Err(errno)),
        }
    }

    pub(super) fn handle_readlink(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let fid = parse_readlink_request(request)?;
        let Some(fid) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&fid)
            .cloned()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let Some(target) = namespace.readlink(node) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        Ok(Ok(string_payload(target.as_bytes())))
    }

    pub(super) fn handle_getattr(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let getattr = parse_getattr_request(request)?;
        let Some(fid) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&getattr.fid)
            .cloned()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let Some(metadata) = namespace.metadata(node) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        Ok(Ok(getattr_payload(metadata, getattr.request_mask)))
    }

    pub(super) fn handle_setattr(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<(), u32>, VirtioError> {
        let setattr = parse_setattr_request(request)?;
        let Some(fid) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&setattr.fid)
            .cloned()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let supported = VIRTIO_9P_SETATTR_MODE
            | VIRTIO_9P_SETATTR_UID
            | VIRTIO_9P_SETATTR_GID
            | VIRTIO_9P_SETATTR_SIZE
            | VIRTIO_9P_SETATTR_ATIME
            | VIRTIO_9P_SETATTR_MTIME
            | VIRTIO_9P_SETATTR_ATIME_SET
            | VIRTIO_9P_SETATTR_MTIME_SET;
        if setattr.valid & !supported != 0 {
            return Ok(Err(VIRTIO_9P_ENOTSUP));
        }
        let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        if setattr.valid & VIRTIO_9P_SETATTR_SIZE != 0
            && namespace.resize_file(node, setattr.size).is_none()
        {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        if namespace
            .set_metadata_fields(
                node,
                (setattr.valid & VIRTIO_9P_SETATTR_MODE != 0).then_some(setattr.mode),
                (setattr.valid & VIRTIO_9P_SETATTR_UID != 0).then_some(setattr.uid),
                (setattr.valid & VIRTIO_9P_SETATTR_GID != 0).then_some(setattr.gid),
                (setattr.valid & VIRTIO_9P_SETATTR_ATIME != 0).then_some(Virtio9pTimestamp::new(
                    setattr.atime_sec,
                    setattr.atime_nsec,
                )),
                (setattr.valid & VIRTIO_9P_SETATTR_MTIME != 0).then_some(Virtio9pTimestamp::new(
                    setattr.mtime_sec,
                    setattr.mtime_nsec,
                )),
            )
            .is_some()
        {
            return Ok(Ok(()));
        }
        Ok(Err(VIRTIO_9P_EBADF))
    }

    pub(super) fn handle_stat(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let fid = parse_stat_request(request)?;
        let Some(node) = self.fid_node(fid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let Some(payload) = namespace.legacy_stat_payload(node) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        Ok(Ok(payload))
    }

    pub(super) fn handle_wstat(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<(), u32>, VirtioError> {
        let wstat = parse_wstat_request(request)?;
        let Some(fid) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&wstat.fid)
            .cloned()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        if let Some(name) = wstat.name.as_deref() {
            if wstat.has_metadata_update() {
                return Ok(Err(VIRTIO_9P_ENOTSUP));
            }
            let Some(old_path) = fid.path().cloned() else {
                return Ok(Err(VIRTIO_9P_EBADF));
            };
            let Some(new_path) = old_path.sibling(name.to_string()) else {
                return Ok(Err(VIRTIO_9P_EBADF));
            };
            return match namespace.rename_node_in_parent(node, &old_path, name)? {
                Ok(moved) => {
                    drop(namespace);
                    if moved {
                        self.move_fid_paths(&old_path, &new_path);
                    }
                    Ok(Ok(()))
                }
                Err(errno) => Ok(Err(errno)),
            };
        }
        if wstat
            .length
            .is_some_and(|size| namespace.resize_file(node, size).is_none())
        {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        if namespace
            .set_metadata_fields(
                node,
                wstat.mode,
                wstat.uid,
                wstat.gid,
                wstat
                    .atime_sec
                    .map(|seconds| Virtio9pTimestamp::new(u64::from(seconds), 0)),
                wstat
                    .mtime_sec
                    .map(|seconds| Virtio9pTimestamp::new(u64::from(seconds), 0)),
            )
            .is_some()
        {
            return Ok(Ok(()));
        }
        Ok(Err(VIRTIO_9P_EBADF))
    }

    pub(super) fn handle_xattrwalk(
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

    pub(super) fn handle_xattrcreate(
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

    pub(super) fn handle_read(
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

    pub(super) fn handle_write(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let write = parse_write_request(request)?;
        let fid = {
            let mut fids = self.fids.lock().expect("virtio 9p fid lock");
            let Some(fid) = fids.get_mut(&write.fid) else {
                return Ok(Err(VIRTIO_9P_EBADF));
            };
            if let Some(bytes) = fid.write_xattr_data(write.offset, &write.data) {
                return Ok(Ok(bytes.to_le_bytes().to_vec()));
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
        let Some(bytes) = namespace.write_file(node, offset, &write.data) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        Ok(Ok(bytes.to_le_bytes().to_vec()))
    }

    pub(super) fn handle_clunk(
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
        if let Some((node, name, data, policy)) = removed.into_xattr_commit() {
            if let Err(errno) = self
                .namespace
                .lock()
                .expect("virtio 9p namespace lock")
                .write_xattr(node, name, data, policy)
            {
                return Ok(Err(errno));
            }
        }
        Ok(Ok(()))
    }

    pub(super) fn handle_remove(
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
        let remove_result = if node == Virtio9pNodeId::Root {
            Err(VIRTIO_9P_EBADF)
        } else {
            let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
            namespace.remove_node_by_fid_path(node, fid.path())
        };
        match remove_result {
            Ok(_) => {
                if self.node_is_removed(node) {
                    self.remove_fids_for_node(node);
                }
                Ok(Ok(()))
            }
            Err(error) => Ok(Err(error)),
        }
    }

    pub(super) fn handle_readdir(
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

    pub(super) fn handle_fsync(
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

    pub(super) fn handle_lock(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<u8, u32>, VirtioError> {
        let lock = parse_lock_request(request)?;
        if !valid_lock_type(lock.lock_type) {
            return Ok(Err(VIRTIO_9P_EBADF));
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

    pub(super) fn handle_getlock(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let lock = parse_getlock_request(request)?;
        if !valid_lock_type(lock.lock_type) {
            return Ok(Err(VIRTIO_9P_EBADF));
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

    pub(super) fn handle_mkdir(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let mkdir = parse_mkdir_request(request)?;
        let Some(parent) = self.fid_node(mkdir.dfid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        match namespace.mkdir(parent, mkdir.name)? {
            Ok(node) => Ok(Ok(qid_payload(namespace.qid(node)))),
            Err(errno) => Ok(Err(errno)),
        }
    }

    pub(super) fn handle_link(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<(), u32>, VirtioError> {
        let link = parse_link_request(request)?;
        let fids = self.fids.lock().expect("virtio 9p fid lock");
        let Some(parent) = fids.get(&link.dfid).cloned() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(oldfid) = fids.get(&link.oldfid).cloned() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(parent) = parent.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(old_node) = oldfid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        drop(fids);
        self.namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .link_file(parent, old_node, link.newname)
    }

    pub(super) fn handle_renameat(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<(), u32>, VirtioError> {
        let rename = parse_renameat_request(request)?;
        let fids = self.fids.lock().expect("virtio 9p fid lock");
        let Some(old_dirfid) = fids.get(&rename.olddirfid).cloned() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(new_dirfid) = fids.get(&rename.newdirfid).cloned() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some((old_dir, old_dir_path)) = fid_node_and_path(&old_dirfid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some((new_dir, new_dir_path)) = fid_node_and_path(&new_dirfid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let old_path = old_dir_path.child(rename.oldname.clone());
        let new_path = new_dir_path.child(rename.newname.clone());
        drop(fids);
        let rename = match self
            .namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .rename_file(old_dir, &rename.oldname, new_dir, &rename.newname)?
        {
            Ok(rename) => rename,
            Err(errno) => return Ok(Err(errno)),
        };
        self.apply_rename_outcome(&old_path, &new_path, rename);
        Ok(Ok(()))
    }

    pub(super) fn handle_rename(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<(), u32>, VirtioError> {
        let rename = parse_rename_request(request)?;
        let fids = self.fids.lock().expect("virtio 9p fid lock");
        let Some(fid) = fids.get(&rename.fid).cloned() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(new_dirfid) = fids.get(&rename.newdirfid).cloned() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some((node, old_path)) = fid_node_and_path(&fid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some((new_dir, new_dir_path)) = fid_node_and_path(&new_dirfid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let new_path = new_dir_path.child(rename.name.clone());
        drop(fids);
        let rename = match self
            .namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .rename_node(node, &old_path, new_dir, &rename.name)?
        {
            Ok(rename) => rename,
            Err(errno) => return Ok(Err(errno)),
        };
        self.apply_rename_outcome(&old_path, &new_path, rename);
        Ok(Ok(()))
    }

    pub(super) fn handle_unlinkat(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<(), u32>, VirtioError> {
        let unlink = parse_unlinkat_request(request)?;
        let Some(dir_node) = self.fid_node(unlink.dirfid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let node = match self
            .namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .unlink_by_name(
                dir_node,
                &unlink.name,
                unlink.flags & VIRTIO_9P_AT_REMOVEDIR != 0,
            )? {
            Ok(node) => node,
            Err(errno) => return Ok(Err(errno)),
        };
        if self.node_is_removed(node) {
            self.remove_fids_for_node(node);
        }
        Ok(Ok(()))
    }

    fn apply_rename_outcome(
        &self,
        old_path: &Virtio9pFidPath,
        new_path: &Virtio9pFidPath,
        rename: Virtio9pRenameOutcome,
    ) {
        if rename.moved {
            self.move_fid_paths(old_path, new_path);
        }
        if let Some(replaced) = rename.replaced {
            if self.node_is_removed(replaced) {
                self.remove_fids_for_node(replaced);
            }
        }
    }
}

fn fid_node_and_path(fid: &Virtio9pFidState) -> Option<(Virtio9pNodeId, Virtio9pFidPath)> {
    Some((fid.node()?, fid.path()?.clone()))
}

fn walk_payload(qids: &[Virtio9pQid]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend((qids.len() as u16).to_le_bytes());
    for qid in qids {
        payload.extend(qid.to_le_bytes());
    }
    payload
}
