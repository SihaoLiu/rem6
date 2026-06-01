use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_kernel::Tick;
use rem6_memory::ByteMask;

use crate::fs9p_lock::Virtio9pLockTable;
use crate::fs9p_namespace::{Virtio9pFidPath, Virtio9pFidState, Virtio9pNamespace, Virtio9pNodeId};
use crate::fs9p_protocol::*;
use crate::{
    modern_feature_pages, Virtio9pCompletion, Virtio9pRequest, VirtioError,
    VirtioPciCommonConfigDevice, VirtioPciDeviceConfigDevice, VirtioPciDeviceConfigSpec,
    VirtioPciNotifyDevice, VirtioQueueIndex, VirtioQueueNotifySpec, VirtioQueueSpec,
};

mod ops;

pub const VIRTIO_9P_DEVICE_ID: u16 = 9;
pub const VIRTIO_9P_F_MOUNT_TAG: u32 = 1;
pub const VIRTIO_9P_REQUEST_QUEUE_INDEX: u16 = 0;
pub const VIRTIO_9P_DEFAULT_QUEUE_SIZE: u16 = 32;
pub const VIRTIO_9P_DEFAULT_MSIZE: u32 = 8192;
pub const VIRTIO_9P_CONFIG_TAG_LENGTH_OFFSET: u64 = 0;
pub const VIRTIO_9P_CONFIG_TAG_OFFSET: u64 = 2;

const VIRTIO_9P_CONFIG_LENGTH_BYTES: usize = 2;
const VIRTIO_9P_HEADER_BYTES: u32 = 7;
const VIRTIO_9P_COUNT_PREFIX_BYTES: u32 = 4;

fn reply_payload(message_type: u8, result: Result<Vec<u8>, u32>) -> (u8, Vec<u8>) {
    match result {
        Ok(payload) => (message_type, payload),
        Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
    }
}

fn empty_reply_payload(message_type: u8, result: Result<(), u32>) -> (u8, Vec<u8>) {
    reply_payload(message_type, result.map(|()| Vec::new()))
}

fn byte_reply_payload(message_type: u8, result: Result<u8, u32>) -> (u8, Vec<u8>) {
    reply_payload(message_type, result.map(|byte| vec![byte]))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Virtio9pConfig {
    mount_tag: Vec<u8>,
}

impl Virtio9pConfig {
    pub fn new(mount_tag: impl Into<Vec<u8>>) -> Result<Self, VirtioError> {
        let mount_tag = mount_tag.into();
        if mount_tag.len() > usize::from(u16::MAX) {
            return Err(VirtioError::Virtio9pMountTagTooLong {
                bytes: mount_tag.len(),
            });
        }
        Ok(Self { mount_tag })
    }

    pub fn mount_tag(&self) -> &[u8] {
        &self.mount_tag
    }

    pub fn config_size(&self) -> u64 {
        (VIRTIO_9P_CONFIG_LENGTH_BYTES + self.mount_tag.len()) as u64
    }

    pub fn to_le_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.config_size() as usize);
        bytes.extend_from_slice(&(self.mount_tag.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&self.mount_tag);
        bytes
    }

    pub fn device_config_spec(&self) -> Result<VirtioPciDeviceConfigSpec, VirtioError> {
        let bytes = self.to_le_bytes();
        VirtioPciDeviceConfigSpec::new(
            bytes.clone(),
            ByteMask::from_bits(vec![false; bytes.len()]).expect("nonempty 9p config write mask"),
        )
    }

    pub fn build_device_config(&self) -> Result<VirtioPciDeviceConfigDevice, VirtioError> {
        self.device_config_spec()
            .map(VirtioPciDeviceConfigDevice::new)
    }
}

impl Default for Virtio9pConfig {
    fn default() -> Self {
        Self {
            mount_tag: b"gem5".to_vec(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Virtio9pDevice {
    config: Virtio9pConfig,
    completions: Arc<Mutex<Vec<Virtio9pCompletion>>>,
    attached_fids: Arc<Mutex<Vec<Virtio9pAttachedFid>>>,
    namespace: Arc<Mutex<Virtio9pNamespace>>,
    fids: Arc<Mutex<BTreeMap<u32, Virtio9pFidState>>>,
    negotiated_msize: Arc<Mutex<u32>>,
    locks: Arc<Mutex<Virtio9pLockTable>>,
}

impl Virtio9pDevice {
    pub fn new(config: Virtio9pConfig) -> Self {
        Self {
            config,
            completions: Arc::new(Mutex::new(Vec::new())),
            attached_fids: Arc::new(Mutex::new(Vec::new())),
            namespace: Arc::new(Mutex::new(Virtio9pNamespace::new())),
            fids: Arc::new(Mutex::new(BTreeMap::new())),
            negotiated_msize: Arc::new(Mutex::new(VIRTIO_9P_DEFAULT_MSIZE)),
            locks: Arc::new(Mutex::new(Virtio9pLockTable::default())),
        }
    }

    pub fn with_file(self, name: impl Into<String>, data: Vec<u8>) -> Result<Self, VirtioError> {
        self.namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .insert_file(name.into(), data)?;
        Ok(self)
    }

    pub fn feature_pages(&self) -> Vec<(u32, u32)> {
        modern_feature_pages([(0, VIRTIO_9P_F_MOUNT_TAG)])
    }

    pub fn queue_specs(&self) -> [VirtioQueueSpec; 1] {
        [VirtioQueueSpec::available(VIRTIO_9P_DEFAULT_QUEUE_SIZE, 0)]
    }

    pub fn notify_specs(&self) -> [VirtioQueueNotifySpec; 1] {
        [VirtioQueueNotifySpec::new(
            VirtioQueueIndex::new(VIRTIO_9P_REQUEST_QUEUE_INDEX).expect("9p request queue index"),
            0,
        )]
    }

    pub fn device_config_spec(&self) -> Result<VirtioPciDeviceConfigSpec, VirtioError> {
        self.config.device_config_spec()
    }

    pub fn build_device_config(&self) -> Result<VirtioPciDeviceConfigDevice, VirtioError> {
        self.config.build_device_config()
    }

    pub fn build_common_config(&self) -> Result<VirtioPciCommonConfigDevice, VirtioError> {
        VirtioPciCommonConfigDevice::new(self.feature_pages(), self.queue_specs())
    }

    pub fn build_notify_device(
        &self,
        notify_off_multiplier: u32,
    ) -> Result<VirtioPciNotifyDevice, VirtioError> {
        VirtioPciNotifyDevice::new(notify_off_multiplier, self.notify_specs())
    }

    pub fn config_size(&self) -> u64 {
        self.config.config_size()
    }

    pub fn config_bytes(&self) -> Vec<u8> {
        self.config.to_le_bytes()
    }

    pub fn config(&self) -> &Virtio9pConfig {
        &self.config
    }

    pub fn execute_at(
        &self,
        tick: Tick,
        request: Virtio9pRequest,
    ) -> Result<Virtio9pCompletion, VirtioError> {
        let (message_type, payload) = match request.message_type() {
            VIRTIO_9P_TSTATFS => reply_payload(VIRTIO_9P_RSTATFS, self.handle_statfs(&request)?),
            VIRTIO_9P_TVERSION => {
                let version = parse_version_request(&request)?;
                let response_version = if version.version == VIRTIO_9P_PROTOCOL_VERSION {
                    VIRTIO_9P_PROTOCOL_VERSION
                } else {
                    b"unknown"
                };
                let response_msize = version_response_msize(version.msize, response_version)?;
                self.reset_session(response_msize);
                (
                    VIRTIO_9P_RVERSION,
                    version_payload(response_msize, response_version),
                )
            }
            VIRTIO_9P_TAUTH => {
                parse_auth_request(&request)?;
                (VIRTIO_9P_RLERROR, VIRTIO_9P_ENOTSUP.to_le_bytes().to_vec())
            }
            VIRTIO_9P_TATTACH => reply_payload(VIRTIO_9P_RATTACH, self.handle_attach(&request)?),
            VIRTIO_9P_TWALK => reply_payload(VIRTIO_9P_RWALK, self.handle_walk(&request)?),
            VIRTIO_9P_TOPEN => reply_payload(VIRTIO_9P_ROPEN, self.handle_open(&request)?),
            VIRTIO_9P_TLOPEN => reply_payload(VIRTIO_9P_RLOPEN, self.handle_lopen(&request)?),
            VIRTIO_9P_TLCREATE => reply_payload(VIRTIO_9P_RLCREATE, self.handle_lcreate(&request)?),
            VIRTIO_9P_TCREATE => reply_payload(VIRTIO_9P_RCREATE, self.handle_create(&request)?),
            VIRTIO_9P_TSYMLINK => reply_payload(VIRTIO_9P_RSYMLINK, self.handle_symlink(&request)?),
            VIRTIO_9P_TMKNOD => reply_payload(VIRTIO_9P_RMKNOD, self.handle_mknod(&request)?),
            VIRTIO_9P_TREADLINK => {
                reply_payload(VIRTIO_9P_RREADLINK, self.handle_readlink(&request)?)
            }
            VIRTIO_9P_TGETATTR => reply_payload(VIRTIO_9P_RGETATTR, self.handle_getattr(&request)?),
            VIRTIO_9P_TSETATTR => {
                empty_reply_payload(VIRTIO_9P_RSETATTR, self.handle_setattr(&request)?)
            }
            VIRTIO_9P_TSTAT => reply_payload(VIRTIO_9P_RSTAT, self.handle_stat(&request)?),
            VIRTIO_9P_TWSTAT => empty_reply_payload(VIRTIO_9P_RWSTAT, self.handle_wstat(&request)?),
            VIRTIO_9P_TXATTRWALK => {
                reply_payload(VIRTIO_9P_RXATTRWALK, self.handle_xattrwalk(&request)?)
            }
            VIRTIO_9P_TXATTRCREATE => {
                empty_reply_payload(VIRTIO_9P_RXATTRCREATE, self.handle_xattrcreate(&request)?)
            }
            VIRTIO_9P_TREADDIR => reply_payload(VIRTIO_9P_RREADDIR, self.handle_readdir(&request)?),
            VIRTIO_9P_TFSYNC => empty_reply_payload(VIRTIO_9P_RFSYNC, self.handle_fsync(&request)?),
            VIRTIO_9P_TLOCK => byte_reply_payload(VIRTIO_9P_RLOCK, self.handle_lock(&request)?),
            VIRTIO_9P_TGETLOCK => reply_payload(VIRTIO_9P_RGETLOCK, self.handle_getlock(&request)?),
            VIRTIO_9P_TLINK => empty_reply_payload(VIRTIO_9P_RLINK, self.handle_link(&request)?),
            VIRTIO_9P_TMKDIR => reply_payload(VIRTIO_9P_RMKDIR, self.handle_mkdir(&request)?),
            VIRTIO_9P_TRENAME => {
                empty_reply_payload(VIRTIO_9P_RRENAME, self.handle_rename(&request)?)
            }
            VIRTIO_9P_TRENAMEAT => {
                empty_reply_payload(VIRTIO_9P_RRENAMEAT, self.handle_renameat(&request)?)
            }
            VIRTIO_9P_TUNLINKAT => {
                empty_reply_payload(VIRTIO_9P_RUNLINKAT, self.handle_unlinkat(&request)?)
            }
            VIRTIO_9P_TREAD => reply_payload(VIRTIO_9P_RREAD, self.handle_read(&request)?),
            VIRTIO_9P_TWRITE => reply_payload(VIRTIO_9P_RWRITE, self.handle_write(&request)?),
            VIRTIO_9P_TCLUNK => empty_reply_payload(VIRTIO_9P_RCLUNK, self.handle_clunk(&request)?),
            VIRTIO_9P_TREMOVE => {
                empty_reply_payload(VIRTIO_9P_RREMOVE, self.handle_remove(&request)?)
            }
            VIRTIO_9P_TFLUSH => {
                parse_flush_request(&request)?;
                (VIRTIO_9P_RFLUSH, Vec::new())
            }
            _ => (VIRTIO_9P_RLERROR, VIRTIO_9P_ENOTSUP.to_le_bytes().to_vec()),
        };
        let completion = Virtio9pCompletion::new(
            request.id(),
            request.queue(),
            tick,
            message_type,
            request.tag(),
            payload,
        )?;
        self.completions
            .lock()
            .expect("virtio 9p completion lock")
            .push(completion.clone());
        Ok(completion)
    }

    pub fn completions(&self) -> Vec<Virtio9pCompletion> {
        self.completions
            .lock()
            .expect("virtio 9p completion lock")
            .clone()
    }

    pub fn attached_fids(&self) -> Vec<Virtio9pAttachedFid> {
        self.attached_fids
            .lock()
            .expect("virtio 9p attached fid lock")
            .clone()
    }

    pub fn fid_count(&self) -> usize {
        self.fids.lock().expect("virtio 9p fid lock").len()
    }

    fn reset_session(&self, negotiated_msize: u32) {
        *self
            .negotiated_msize
            .lock()
            .expect("virtio 9p negotiated msize lock") = negotiated_msize;
        self.fids.lock().expect("virtio 9p fid lock").clear();
        self.attached_fids
            .lock()
            .expect("virtio 9p attached fid lock")
            .clear();
        self.locks.lock().expect("virtio 9p lock table").clear();
    }

    fn fid_node(&self, fid: u32) -> Option<Virtio9pNodeId> {
        self.fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&fid)
            .and_then(Virtio9pFidState::node)
    }

    fn negotiated_msize(&self) -> u32 {
        *self
            .negotiated_msize
            .lock()
            .expect("virtio 9p negotiated msize lock")
    }

    fn counted_data_limit(&self, requested: u32) -> u32 {
        requested.min(
            self.negotiated_msize()
                .saturating_sub(VIRTIO_9P_HEADER_BYTES + VIRTIO_9P_COUNT_PREFIX_BYTES),
        )
    }

    fn node_is_removed(&self, node: Virtio9pNodeId) -> bool {
        self.namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .metadata(node)
            .is_none()
    }

    fn remove_fids_for_node(&self, node: Virtio9pNodeId) {
        self.fids
            .lock()
            .expect("virtio 9p fid lock")
            .retain(|_, fid| fid.removal_node() != Some(node));
        self.locks
            .lock()
            .expect("virtio 9p lock table")
            .remove_node(node);
    }

    fn remove_node_for_fid_path(
        &self,
        node: Virtio9pNodeId,
        fid_path: Option<&Virtio9pFidPath>,
    ) -> Result<(), u32> {
        if node == Virtio9pNodeId::Root {
            return Err(VIRTIO_9P_EBADF);
        }
        self.namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .remove_node_by_fid_path(node, fid_path)?;
        if self.node_is_removed(node) {
            self.remove_fids_for_node(node);
        }
        Ok(())
    }

    fn move_fid_paths(&self, old_path: &Virtio9pFidPath, new_path: &Virtio9pFidPath) {
        for fid in self.fids.lock().expect("virtio 9p fid lock").values_mut() {
            fid.move_path(old_path, new_path);
        }
    }

    fn lockable_node(&self, fid: u32) -> Option<Virtio9pNodeId> {
        let fid = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&fid)
            .cloned()?;
        if !fid.is_open() {
            return None;
        }
        let node = fid.node()?;
        self.namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .read_file(node, 0, 0)?;
        Some(node)
    }
}

impl Default for Virtio9pDevice {
    fn default() -> Self {
        Self::new(Virtio9pConfig::default())
    }
}

fn version_response_msize(requested: u32, version: &[u8]) -> Result<u32, VirtioError> {
    let version_bytes =
        u32::try_from(version.len()).map_err(|_| VirtioError::Virtio9pPayloadLengthOverflow)?;
    let minimum = VIRTIO_9P_HEADER_BYTES
        .checked_add(4)
        .and_then(|bytes| bytes.checked_add(2))
        .and_then(|bytes| bytes.checked_add(version_bytes))
        .ok_or(VirtioError::Virtio9pPayloadLengthOverflow)?;
    Ok(requested.min(VIRTIO_9P_DEFAULT_MSIZE).max(minimum))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Virtio9pAttachedFid {
    fid: u32,
    afid: u32,
    uname: String,
    aname: String,
    n_uname: u32,
}

impl Virtio9pAttachedFid {
    pub(crate) const fn new(
        fid: u32,
        afid: u32,
        uname: String,
        aname: String,
        n_uname: u32,
    ) -> Self {
        Self {
            fid,
            afid,
            uname,
            aname,
            n_uname,
        }
    }

    pub const fn fid(&self) -> u32 {
        self.fid
    }

    pub const fn afid(&self) -> u32 {
        self.afid
    }

    pub fn uname(&self) -> &str {
        &self.uname
    }

    pub fn aname(&self) -> &str {
        &self.aname
    }

    pub const fn n_uname(&self) -> u32 {
        self.n_uname
    }
}
