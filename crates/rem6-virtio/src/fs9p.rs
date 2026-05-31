use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_kernel::Tick;
use rem6_memory::ByteMask;

use crate::fs9p_namespace::{
    getattr_payload, qid_payload, Virtio9pFidState, Virtio9pNamespace, Virtio9pNodeId,
    Virtio9pTimestamp,
};
use crate::fs9p_protocol::{
    lock_payload, parse_attach_request, parse_clunk_request, parse_create_request,
    parse_flush_request, parse_fsync_request, parse_getattr_request, parse_getlock_request,
    parse_lcreate_request, parse_link_request, parse_lock_request, parse_lopen_request,
    parse_mkdir_request, parse_mknod_request, parse_open_request, parse_read_request,
    parse_readdir_request, parse_readlink_request, parse_remove_request, parse_rename_request,
    parse_renameat_request, parse_setattr_request, parse_statfs_request, parse_symlink_request,
    parse_unlinkat_request, parse_version_request, parse_walk_request, parse_write_request,
    parse_xattrcreate_request, parse_xattrwalk_request, string_payload, version_payload,
};
use crate::{
    modern_feature_pages, Virtio9pCompletion, Virtio9pRequest, VirtioError,
    VirtioPciCommonConfigDevice, VirtioPciDeviceConfigDevice, VirtioPciDeviceConfigSpec,
    VirtioPciNotifyDevice, VirtioQueueIndex, VirtioQueueNotifySpec, VirtioQueueSpec,
};

pub const VIRTIO_9P_DEVICE_ID: u16 = 9;
pub const VIRTIO_9P_F_MOUNT_TAG: u32 = 1;
pub const VIRTIO_9P_REQUEST_QUEUE_INDEX: u16 = 0;
pub const VIRTIO_9P_DEFAULT_QUEUE_SIZE: u16 = 32;
pub const VIRTIO_9P_DEFAULT_MSIZE: u32 = 8192;
pub const VIRTIO_9P_PROTOCOL_VERSION: &[u8] = b"9P2000.L";
pub const VIRTIO_9P_TSTATFS: u8 = 8;
pub const VIRTIO_9P_RSTATFS: u8 = 9;
pub const VIRTIO_9P_TVERSION: u8 = 100;
pub const VIRTIO_9P_RVERSION: u8 = 101;
pub const VIRTIO_9P_TATTACH: u8 = 104;
pub const VIRTIO_9P_RATTACH: u8 = 105;
pub const VIRTIO_9P_TLCREATE: u8 = 14;
pub const VIRTIO_9P_RLCREATE: u8 = 15;
pub const VIRTIO_9P_TSYMLINK: u8 = 16;
pub const VIRTIO_9P_RSYMLINK: u8 = 17;
pub const VIRTIO_9P_TMKNOD: u8 = 18;
pub const VIRTIO_9P_RMKNOD: u8 = 19;
pub const VIRTIO_9P_TRENAME: u8 = 20;
pub const VIRTIO_9P_RRENAME: u8 = 21;
pub const VIRTIO_9P_TREADLINK: u8 = 22;
pub const VIRTIO_9P_RREADLINK: u8 = 23;
pub const VIRTIO_9P_TGETATTR: u8 = 24;
pub const VIRTIO_9P_RGETATTR: u8 = 25;
pub const VIRTIO_9P_TSETATTR: u8 = 26;
pub const VIRTIO_9P_RSETATTR: u8 = 27;
pub const VIRTIO_9P_TXATTRWALK: u8 = 30;
pub const VIRTIO_9P_RXATTRWALK: u8 = 31;
pub const VIRTIO_9P_TXATTRCREATE: u8 = 32;
pub const VIRTIO_9P_RXATTRCREATE: u8 = 33;
pub const VIRTIO_9P_TREADDIR: u8 = 40;
pub const VIRTIO_9P_RREADDIR: u8 = 41;
pub const VIRTIO_9P_TFSYNC: u8 = 50;
pub const VIRTIO_9P_RFSYNC: u8 = 51;
pub const VIRTIO_9P_TLOCK: u8 = 52;
pub const VIRTIO_9P_RLOCK: u8 = 53;
pub const VIRTIO_9P_TGETLOCK: u8 = 54;
pub const VIRTIO_9P_RGETLOCK: u8 = 55;
pub const VIRTIO_9P_TLINK: u8 = 70;
pub const VIRTIO_9P_RLINK: u8 = 71;
pub const VIRTIO_9P_TMKDIR: u8 = 72;
pub const VIRTIO_9P_RMKDIR: u8 = 73;
pub const VIRTIO_9P_TRENAMEAT: u8 = 74;
pub const VIRTIO_9P_RRENAMEAT: u8 = 75;
pub const VIRTIO_9P_TUNLINKAT: u8 = 76;
pub const VIRTIO_9P_RUNLINKAT: u8 = 77;
pub const VIRTIO_9P_TFLUSH: u8 = 108;
pub const VIRTIO_9P_RFLUSH: u8 = 109;
pub const VIRTIO_9P_TWALK: u8 = 110;
pub const VIRTIO_9P_RWALK: u8 = 111;
pub const VIRTIO_9P_TOPEN: u8 = 112;
pub const VIRTIO_9P_ROPEN: u8 = 113;
pub const VIRTIO_9P_TCREATE: u8 = 114;
pub const VIRTIO_9P_RCREATE: u8 = 115;
pub const VIRTIO_9P_TLOPEN: u8 = 12;
pub const VIRTIO_9P_RLOPEN: u8 = 13;
pub const VIRTIO_9P_TREAD: u8 = 116;
pub const VIRTIO_9P_RREAD: u8 = 117;
pub const VIRTIO_9P_TWRITE: u8 = 118;
pub const VIRTIO_9P_RWRITE: u8 = 119;
pub const VIRTIO_9P_TCLUNK: u8 = 120;
pub const VIRTIO_9P_RCLUNK: u8 = 121;
pub const VIRTIO_9P_TREMOVE: u8 = 122;
pub const VIRTIO_9P_RREMOVE: u8 = 123;
pub const VIRTIO_9P_RLERROR: u8 = 7;
pub const VIRTIO_9P_NOFID: u32 = u32::MAX;
pub const VIRTIO_9P_EBADF: u32 = 9;
pub const VIRTIO_9P_EEXIST: u32 = 17;
pub const VIRTIO_9P_ENOENT: u32 = 2;
pub const VIRTIO_9P_ENODATA: u32 = 61;
pub const VIRTIO_9P_ENOTEMPTY: u32 = 39;
pub const VIRTIO_9P_ENOTSUP: u32 = 95;
pub const VIRTIO_9P_QTFILE: u8 = 0;
pub const VIRTIO_9P_QTSYMLINK: u8 = 0x02;
pub const VIRTIO_9P_QTDIR: u8 = 0x80;
pub const VIRTIO_9P_DTCHR: u8 = 2;
pub const VIRTIO_9P_DTDIR: u8 = 4;
pub const VIRTIO_9P_DTBLK: u8 = 6;
pub const VIRTIO_9P_DTREG: u8 = 8;
pub const VIRTIO_9P_DTSYMLINK: u8 = 10;
pub const VIRTIO_9P_GETATTR_BASIC: u64 = 0x0000_07ff;
pub const VIRTIO_9P_SETATTR_MODE: u32 = 0x0000_0001;
pub const VIRTIO_9P_SETATTR_UID: u32 = 0x0000_0002;
pub const VIRTIO_9P_SETATTR_GID: u32 = 0x0000_0004;
pub const VIRTIO_9P_SETATTR_SIZE: u32 = 0x0000_0008;
pub const VIRTIO_9P_SETATTR_ATIME: u32 = 0x0000_0010;
pub const VIRTIO_9P_SETATTR_MTIME: u32 = 0x0000_0020;
pub const VIRTIO_9P_SETATTR_ATIME_SET: u32 = 0x0000_0080;
pub const VIRTIO_9P_SETATTR_MTIME_SET: u32 = 0x0000_0100;
pub const VIRTIO_9P_LOCK_SUCCESS: u8 = 0;
pub const VIRTIO_9P_LOCK_TYPE_RDLCK: u8 = 0;
pub const VIRTIO_9P_LOCK_TYPE_WRLCK: u8 = 1;
pub const VIRTIO_9P_LOCK_TYPE_UNLCK: u8 = 2;
pub const VIRTIO_9P_AT_REMOVEDIR: u32 = 0x200;
pub const VIRTIO_9P_STATFS_TYPE: u32 = 0x0102_1997;
pub const VIRTIO_9P_STATFS_BLOCK_SIZE: u32 = 4096;
pub const VIRTIO_9P_NAME_MAX: u32 = 255;
pub const VIRTIO_9P_CONFIG_TAG_LENGTH_OFFSET: u64 = 0;
pub const VIRTIO_9P_CONFIG_TAG_OFFSET: u64 = 2;

const VIRTIO_9P_CONFIG_LENGTH_BYTES: usize = 2;

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
}

impl Virtio9pDevice {
    pub fn new(config: Virtio9pConfig) -> Self {
        Self {
            config,
            completions: Arc::new(Mutex::new(Vec::new())),
            attached_fids: Arc::new(Mutex::new(Vec::new())),
            namespace: Arc::new(Mutex::new(Virtio9pNamespace::new())),
            fids: Arc::new(Mutex::new(BTreeMap::new())),
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
            VIRTIO_9P_TSTATFS => match self.handle_statfs(&request)? {
                Ok(payload) => (VIRTIO_9P_RSTATFS, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TVERSION => {
                let version = parse_version_request(&request)?;
                let response_version = if version == VIRTIO_9P_PROTOCOL_VERSION {
                    VIRTIO_9P_PROTOCOL_VERSION
                } else {
                    b"unknown"
                };
                (
                    VIRTIO_9P_RVERSION,
                    version_payload(VIRTIO_9P_DEFAULT_MSIZE, response_version),
                )
            }
            VIRTIO_9P_TATTACH => {
                let attached = parse_attach_request(&request)?;
                let root_qid = self
                    .namespace
                    .lock()
                    .expect("virtio 9p namespace lock")
                    .root_qid();
                self.fids
                    .lock()
                    .expect("virtio 9p fid lock")
                    .insert(attached.fid(), Virtio9pFidState::new(Virtio9pNodeId::Root));
                self.attached_fids
                    .lock()
                    .expect("virtio 9p attached fid lock")
                    .push(attached);
                (VIRTIO_9P_RATTACH, qid_payload(root_qid))
            }
            VIRTIO_9P_TWALK => match self.handle_walk(&request)? {
                Ok(payload) => (VIRTIO_9P_RWALK, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TOPEN => match self.handle_open(&request)? {
                Ok(payload) => (VIRTIO_9P_ROPEN, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TLOPEN => match self.handle_lopen(&request)? {
                Ok(payload) => (VIRTIO_9P_RLOPEN, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TLCREATE => match self.handle_lcreate(&request)? {
                Ok(payload) => (VIRTIO_9P_RLCREATE, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TCREATE => match self.handle_create(&request)? {
                Ok(payload) => (VIRTIO_9P_RCREATE, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TSYMLINK => match self.handle_symlink(&request)? {
                Ok(payload) => (VIRTIO_9P_RSYMLINK, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TMKNOD => match self.handle_mknod(&request)? {
                Ok(payload) => (VIRTIO_9P_RMKNOD, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TREADLINK => match self.handle_readlink(&request)? {
                Ok(payload) => (VIRTIO_9P_RREADLINK, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TGETATTR => match self.handle_getattr(&request)? {
                Ok(payload) => (VIRTIO_9P_RGETATTR, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TSETATTR => match self.handle_setattr(&request)? {
                Ok(()) => (VIRTIO_9P_RSETATTR, Vec::new()),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TXATTRWALK => match self.handle_xattrwalk(&request)? {
                Ok(payload) => (VIRTIO_9P_RXATTRWALK, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TXATTRCREATE => match self.handle_xattrcreate(&request)? {
                Ok(()) => (VIRTIO_9P_RXATTRCREATE, Vec::new()),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TREADDIR => match self.handle_readdir(&request)? {
                Ok(payload) => (VIRTIO_9P_RREADDIR, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TFSYNC => match self.handle_fsync(&request)? {
                Ok(()) => (VIRTIO_9P_RFSYNC, Vec::new()),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TLOCK => match self.handle_lock(&request)? {
                Ok(status) => (VIRTIO_9P_RLOCK, vec![status]),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TGETLOCK => match self.handle_getlock(&request)? {
                Ok(payload) => (VIRTIO_9P_RGETLOCK, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TLINK => match self.handle_link(&request)? {
                Ok(()) => (VIRTIO_9P_RLINK, Vec::new()),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TMKDIR => match self.handle_mkdir(&request)? {
                Ok(payload) => (VIRTIO_9P_RMKDIR, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TRENAME => match self.handle_rename(&request)? {
                Ok(()) => (VIRTIO_9P_RRENAME, Vec::new()),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TRENAMEAT => match self.handle_renameat(&request)? {
                Ok(()) => (VIRTIO_9P_RRENAMEAT, Vec::new()),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TUNLINKAT => match self.handle_unlinkat(&request)? {
                Ok(()) => (VIRTIO_9P_RUNLINKAT, Vec::new()),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TREAD => match self.handle_read(&request)? {
                Ok(payload) => (VIRTIO_9P_RREAD, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TWRITE => match self.handle_write(&request)? {
                Ok(payload) => (VIRTIO_9P_RWRITE, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TCLUNK => match self.handle_clunk(&request)? {
                Ok(()) => (VIRTIO_9P_RCLUNK, Vec::new()),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TREMOVE => match self.handle_remove(&request)? {
                Ok(()) => (VIRTIO_9P_RREMOVE, Vec::new()),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
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

    fn fid_node(&self, fid: u32) -> Option<Virtio9pNodeId> {
        self.fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&fid)
            .and_then(Virtio9pFidState::node)
    }

    fn handle_statfs(
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

    fn handle_walk(&self, request: &Virtio9pRequest) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let walk = parse_walk_request(request)?;
        let Some(start) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&walk.fid)
            .cloned()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(mut node) = start.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let mut qids = Vec::new();
        for name in &walk.names {
            let Some(next) = namespace.walk(node, name) else {
                return Ok(Err(VIRTIO_9P_ENOENT));
            };
            node = next;
            qids.push(namespace.qid(node));
        }
        drop(namespace);

        self.fids
            .lock()
            .expect("virtio 9p fid lock")
            .insert(walk.newfid, Virtio9pFidState::new(node));
        let mut payload = Vec::new();
        payload.extend((qids.len() as u16).to_le_bytes());
        for qid in qids {
            payload.extend(qid.to_le_bytes());
        }
        Ok(Ok(payload))
    }

    fn handle_lopen(&self, request: &Virtio9pRequest) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let open = parse_lopen_request(request)?;
        self.open_fid_payload(open.fid)
    }

    fn handle_open(&self, request: &Virtio9pRequest) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let open = parse_open_request(request)?;
        self.open_fid_payload(open.fid)
    }

    fn open_fid_payload(&self, fid: u32) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let mut fids = self.fids.lock().expect("virtio 9p fid lock");
        let Some(fid) = fids.get_mut(&fid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        if namespace.metadata(node).is_none() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        if fid.open().is_none() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let mut payload = namespace.qid(node).to_le_bytes().to_vec();
        payload.extend(VIRTIO_9P_DEFAULT_MSIZE.to_le_bytes());
        Ok(Ok(payload))
    }

    fn handle_lcreate(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let create = parse_lcreate_request(request)?;
        self.create_fid_payload(create.fid, create.name)
    }

    fn handle_create(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let create = parse_create_request(request)?;
        self.create_fid_payload(create.fid, create.name)
    }

    fn create_fid_payload(
        &self,
        fid: u32,
        name: String,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let mut fids = self.fids.lock().expect("virtio 9p fid lock");
        let Some(fid) = fids.get_mut(&fid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(parent) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let node = match namespace.create_file(parent, name)? {
            Ok(node) => node,
            Err(errno) => return Ok(Err(errno)),
        };
        *fid = Virtio9pFidState::opened(node);
        let mut payload = namespace.qid(node).to_le_bytes().to_vec();
        payload.extend(VIRTIO_9P_DEFAULT_MSIZE.to_le_bytes());
        Ok(Ok(payload))
    }

    fn handle_symlink(
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

    fn handle_mknod(&self, request: &Virtio9pRequest) -> Result<Result<Vec<u8>, u32>, VirtioError> {
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

    fn handle_readlink(
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

    fn handle_getattr(
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

    fn handle_setattr(&self, request: &Virtio9pRequest) -> Result<Result<(), u32>, VirtioError> {
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

    fn handle_xattrwalk(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let xattrwalk = parse_xattrwalk_request(request)?;
        let Some(node) = self.fid_node(xattrwalk.fid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if self
            .namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .metadata(node)
            .is_none()
        {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        if !xattrwalk.name.is_empty() {
            return Ok(Err(VIRTIO_9P_ENODATA));
        }
        self.fids
            .lock()
            .expect("virtio 9p fid lock")
            .insert(xattrwalk.newfid, Virtio9pFidState::xattr(Vec::new()));
        Ok(Ok(0_u64.to_le_bytes().to_vec()))
    }

    fn handle_xattrcreate(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<(), u32>, VirtioError> {
        let fid = parse_xattrcreate_request(request)?;
        let Some(node) = self.fid_node(fid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if self
            .namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .metadata(node)
            .is_none()
        {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        Ok(Err(VIRTIO_9P_ENOTSUP))
    }

    fn handle_readdir(
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
        if !fid.is_open() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let Some(payload) = namespace.readdir_payload(node, readdir.offset, readdir.count) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        Ok(Ok(payload))
    }

    fn handle_fsync(&self, request: &Virtio9pRequest) -> Result<Result<(), u32>, VirtioError> {
        let fsync = parse_fsync_request(request)?;
        if self.fid_node(fsync.fid).is_some() {
            Ok(Ok(()))
        } else {
            Ok(Err(VIRTIO_9P_EBADF))
        }
    }

    fn handle_lock(&self, request: &Virtio9pRequest) -> Result<Result<u8, u32>, VirtioError> {
        let lock = parse_lock_request(request)?;
        if !valid_lock_type(lock.lock_type) {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        if self.lockable_fid(lock.fid).is_none() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        Ok(Ok(VIRTIO_9P_LOCK_SUCCESS))
    }

    fn handle_getlock(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let lock = parse_getlock_request(request)?;
        if !valid_lock_type(lock.lock_type) {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        if self.lockable_fid(lock.fid).is_none() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        Ok(Ok(lock_payload(
            VIRTIO_9P_LOCK_TYPE_UNLCK,
            lock.flags,
            lock.start,
            lock.length,
            lock.proc_id,
            &lock.client_id,
        )))
    }

    fn handle_mkdir(&self, request: &Virtio9pRequest) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let mkdir = parse_mkdir_request(request)?;
        let Some(parent) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&mkdir.dfid)
            .cloned()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(parent) = parent.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        match namespace.mkdir(parent, mkdir.name)? {
            Ok(node) => Ok(Ok(qid_payload(namespace.qid(node)))),
            Err(errno) => Ok(Err(errno)),
        }
    }

    fn handle_link(&self, request: &Virtio9pRequest) -> Result<Result<(), u32>, VirtioError> {
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

    fn handle_renameat(&self, request: &Virtio9pRequest) -> Result<Result<(), u32>, VirtioError> {
        let rename = parse_renameat_request(request)?;
        let fids = self.fids.lock().expect("virtio 9p fid lock");
        let Some(old_dirfid) = fids.get(&rename.olddirfid).cloned() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(new_dirfid) = fids.get(&rename.newdirfid).cloned() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(old_dir) = old_dirfid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(new_dir) = new_dirfid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
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
        if let Some(replaced) = rename.replaced {
            if self.node_is_removed(replaced) {
                self.remove_fids_for_node(replaced);
            }
        }
        Ok(Ok(()))
    }

    fn handle_rename(&self, request: &Virtio9pRequest) -> Result<Result<(), u32>, VirtioError> {
        let rename = parse_rename_request(request)?;
        let fids = self.fids.lock().expect("virtio 9p fid lock");
        let Some(fid) = fids.get(&rename.fid).cloned() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(new_dirfid) = fids.get(&rename.newdirfid).cloned() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(new_dir) = new_dirfid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        drop(fids);
        let rename = match self
            .namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .rename_node(node, new_dir, &rename.name)?
        {
            Ok(rename) => rename,
            Err(errno) => return Ok(Err(errno)),
        };
        if let Some(replaced) = rename.replaced {
            if self.node_is_removed(replaced) {
                self.remove_fids_for_node(replaced);
            }
        }
        Ok(Ok(()))
    }

    fn handle_unlinkat(&self, request: &Virtio9pRequest) -> Result<Result<(), u32>, VirtioError> {
        let unlink = parse_unlinkat_request(request)?;
        let Some(fid) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&unlink.dirfid)
            .cloned()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(dir_node) = fid.node() else {
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

    fn handle_read(&self, request: &Virtio9pRequest) -> Result<Result<Vec<u8>, u32>, VirtioError> {
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
            let data = read_data_slice(data, read.offset, read.count)
                .ok_or(VirtioError::Virtio9pPayloadLengthOverflow)?;
            let mut payload = Vec::new();
            payload.extend((data.len() as u32).to_le_bytes());
            payload.extend(data);
            return Ok(Ok(payload));
        }
        if !fid.is_open() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let Some(data) = namespace.read_file(node, read.offset, read.count) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let mut payload = Vec::new();
        payload.extend((data.len() as u32).to_le_bytes());
        payload.extend(data);
        Ok(Ok(payload))
    }

    fn handle_write(&self, request: &Virtio9pRequest) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let write = parse_write_request(request)?;
        let Some(fid) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&write.fid)
            .cloned()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if !fid.is_open() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let Some(bytes) = namespace.write_file(node, write.offset, &write.data) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        Ok(Ok(bytes.to_le_bytes().to_vec()))
    }

    fn handle_clunk(&self, request: &Virtio9pRequest) -> Result<Result<(), u32>, VirtioError> {
        let fid = parse_clunk_request(request)?;
        if self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .remove(&fid)
            .is_some()
        {
            Ok(Ok(()))
        } else {
            Ok(Err(VIRTIO_9P_EBADF))
        }
    }

    fn handle_remove(&self, request: &Virtio9pRequest) -> Result<Result<(), u32>, VirtioError> {
        let remove_fid = parse_remove_request(request)?;
        let Some(fid) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&remove_fid)
            .cloned()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(node) = fid.node() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if node == Virtio9pNodeId::Root {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        if self
            .namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .remove_file_by_node(node)
        {
            self.fids
                .lock()
                .expect("virtio 9p fid lock")
                .remove(&remove_fid);
            if self.node_is_removed(node) {
                self.remove_fids_for_node(node);
            }
            Ok(Ok(()))
        } else {
            Ok(Err(VIRTIO_9P_EBADF))
        }
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
            .retain(|_, fid| fid.node() != Some(node));
    }

    fn lockable_fid(&self, fid: u32) -> Option<Virtio9pFidState> {
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
        Some(fid)
    }
}

impl Default for Virtio9pDevice {
    fn default() -> Self {
        Self::new(Virtio9pConfig::default())
    }
}

const fn valid_lock_type(lock_type: u8) -> bool {
    matches!(
        lock_type,
        VIRTIO_9P_LOCK_TYPE_RDLCK | VIRTIO_9P_LOCK_TYPE_WRLCK | VIRTIO_9P_LOCK_TYPE_UNLCK
    )
}

fn read_data_slice(data: &[u8], offset: u64, count: u32) -> Option<Vec<u8>> {
    let start = usize::try_from(offset).ok()?;
    if start >= data.len() {
        return Some(Vec::new());
    }
    let count = usize::try_from(count).ok()?;
    let end = start.saturating_add(count).min(data.len());
    Some(data[start..end].to_vec())
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
