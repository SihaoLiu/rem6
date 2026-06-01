use crate::fs9p_namespace::{
    getattr_payload, qid_payload, validate_file_name, Virtio9pFidState, Virtio9pNodeId,
    Virtio9pOpenMode, Virtio9pQid, Virtio9pTimestamp,
};
use crate::fs9p_protocol::*;
use crate::{Virtio9pRequest, VirtioError};

use super::super::Virtio9pDevice;

impl Virtio9pDevice {
    pub(in crate::fs9p) fn handle_attach(
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

    pub(in crate::fs9p) fn handle_statfs(
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

    pub(in crate::fs9p) fn handle_walk(
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
        if start.is_open() && !matches!(node, Virtio9pNodeId::Root | Virtio9pNodeId::Directory(_)) {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
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

    pub(in crate::fs9p) fn handle_lopen(
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

    pub(in crate::fs9p) fn handle_open(
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

    pub(in crate::fs9p) fn handle_lcreate(
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

    pub(in crate::fs9p) fn handle_create(
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

    pub(in crate::fs9p) fn handle_symlink(
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

    pub(in crate::fs9p) fn handle_mknod(
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

    pub(in crate::fs9p) fn handle_readlink(
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

    pub(in crate::fs9p) fn handle_getattr(
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

    pub(in crate::fs9p) fn handle_setattr(
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

    pub(in crate::fs9p) fn handle_stat(
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

    pub(in crate::fs9p) fn handle_wstat(
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
}

fn walk_payload(qids: &[Virtio9pQid]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend((qids.len() as u16).to_le_bytes());
    for qid in qids {
        payload.extend(qid.to_le_bytes());
    }
    payload
}
