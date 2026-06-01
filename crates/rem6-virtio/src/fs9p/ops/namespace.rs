use crate::fs9p_namespace::{
    qid_payload, Virtio9pFidPath, Virtio9pFidState, Virtio9pNodeId, Virtio9pRenameOutcome,
};
use crate::fs9p_protocol::*;
use crate::{Virtio9pRequest, VirtioError};

use super::super::Virtio9pDevice;

impl Virtio9pDevice {
    pub(in crate::fs9p) fn handle_mkdir(
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

    pub(in crate::fs9p) fn handle_link(
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

    pub(in crate::fs9p) fn handle_renameat(
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

    pub(in crate::fs9p) fn handle_rename(
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

    pub(in crate::fs9p) fn handle_unlinkat(
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
