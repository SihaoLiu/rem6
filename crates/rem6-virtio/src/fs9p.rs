use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_kernel::Tick;
use rem6_memory::ByteMask;

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
pub const VIRTIO_9P_TGETATTR: u8 = 24;
pub const VIRTIO_9P_RGETATTR: u8 = 25;
pub const VIRTIO_9P_TREADDIR: u8 = 40;
pub const VIRTIO_9P_RREADDIR: u8 = 41;
pub const VIRTIO_9P_TMKDIR: u8 = 72;
pub const VIRTIO_9P_RMKDIR: u8 = 73;
pub const VIRTIO_9P_TRENAMEAT: u8 = 74;
pub const VIRTIO_9P_RRENAMEAT: u8 = 75;
pub const VIRTIO_9P_TUNLINKAT: u8 = 76;
pub const VIRTIO_9P_RUNLINKAT: u8 = 77;
pub const VIRTIO_9P_TWALK: u8 = 110;
pub const VIRTIO_9P_RWALK: u8 = 111;
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
pub const VIRTIO_9P_ENOTSUP: u32 = 95;
pub const VIRTIO_9P_QTFILE: u8 = 0;
pub const VIRTIO_9P_QTDIR: u8 = 0x80;
pub const VIRTIO_9P_DTDIR: u8 = 4;
pub const VIRTIO_9P_DTREG: u8 = 8;
pub const VIRTIO_9P_GETATTR_BASIC: u64 = 0x0000_07ff;
pub const VIRTIO_9P_STATFS_TYPE: u32 = 0x0102_1997;
pub const VIRTIO_9P_STATFS_BLOCK_SIZE: u32 = 4096;
pub const VIRTIO_9P_NAME_MAX: u32 = 255;
pub const VIRTIO_9P_CONFIG_TAG_LENGTH_OFFSET: u64 = 0;
pub const VIRTIO_9P_CONFIG_TAG_OFFSET: u64 = 2;

const VIRTIO_9P_CONFIG_LENGTH_BYTES: usize = 2;
const VIRTIO_9P_QID_BYTES: usize = 13;
const VIRTIO_9P_STATFS_BLOCKS: u64 = 1024;
const VIRTIO_9P_STATFS_FSID: u64 = 0x7265_6d36;

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
            VIRTIO_9P_TLOPEN => match self.handle_lopen(&request)? {
                Ok(payload) => (VIRTIO_9P_RLOPEN, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TLCREATE => match self.handle_lcreate(&request)? {
                Ok(payload) => (VIRTIO_9P_RLCREATE, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TGETATTR => match self.handle_getattr(&request)? {
                Ok(payload) => (VIRTIO_9P_RGETATTR, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TREADDIR => match self.handle_readdir(&request)? {
                Ok(payload) => (VIRTIO_9P_RREADDIR, payload),
                Err(errno) => (VIRTIO_9P_RLERROR, errno.to_le_bytes().to_vec()),
            },
            VIRTIO_9P_TMKDIR => match self.handle_mkdir(&request)? {
                Ok(payload) => (VIRTIO_9P_RMKDIR, payload),
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

    fn handle_statfs(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let fid = parse_statfs_request(request)?;
        if !self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .contains_key(&fid)
        {
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
            .copied()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let mut node = start.node();
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
        let mut fids = self.fids.lock().expect("virtio 9p fid lock");
        let Some(fid) = fids.get_mut(&open.fid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        if namespace.metadata(fid.node()).is_none() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        fid.open();
        let mut payload = namespace.qid(fid.node()).to_le_bytes().to_vec();
        payload.extend(VIRTIO_9P_DEFAULT_MSIZE.to_le_bytes());
        Ok(Ok(payload))
    }

    fn handle_lcreate(
        &self,
        request: &Virtio9pRequest,
    ) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let create = parse_lcreate_request(request)?;
        let mut fids = self.fids.lock().expect("virtio 9p fid lock");
        let Some(fid) = fids.get_mut(&create.fid) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let parent = fid.node();
        let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let node = match namespace.create_file(parent, create.name)? {
            Ok(node) => node,
            Err(errno) => return Ok(Err(errno)),
        };
        *fid = Virtio9pFidState::opened(node);
        let mut payload = namespace.qid(node).to_le_bytes().to_vec();
        payload.extend(VIRTIO_9P_DEFAULT_MSIZE.to_le_bytes());
        Ok(Ok(payload))
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
            .copied()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let Some(metadata) = namespace.metadata(fid.node()) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        Ok(Ok(getattr_payload(metadata, getattr.request_mask)))
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
            .copied()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if !fid.is_open() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let Some(payload) = namespace.readdir_payload(fid.node(), readdir.offset, readdir.count)
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        Ok(Ok(payload))
    }

    fn handle_mkdir(&self, request: &Virtio9pRequest) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let mkdir = parse_mkdir_request(request)?;
        let Some(parent) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&mkdir.dfid)
            .copied()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        match namespace.mkdir(parent.node(), mkdir.name)? {
            Ok(node) => Ok(Ok(qid_payload(namespace.qid(node)))),
            Err(errno) => Ok(Err(errno)),
        }
    }

    fn handle_renameat(&self, request: &Virtio9pRequest) -> Result<Result<(), u32>, VirtioError> {
        let rename = parse_renameat_request(request)?;
        let fids = self.fids.lock().expect("virtio 9p fid lock");
        let Some(old_dirfid) = fids.get(&rename.olddirfid).copied() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let Some(new_dirfid) = fids.get(&rename.newdirfid).copied() else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        drop(fids);
        let rename = match self
            .namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .rename_file(
                old_dirfid.node(),
                &rename.oldname,
                new_dirfid.node(),
                &rename.newname,
            )? {
            Ok(rename) => rename,
            Err(errno) => return Ok(Err(errno)),
        };
        if let Some(replaced) = rename.replaced {
            self.remove_fids_for_node(replaced);
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
            .copied()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let node = match self
            .namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .remove_file_by_name(fid.node(), &unlink.name)?
        {
            Ok(node) => node,
            Err(errno) => return Ok(Err(errno)),
        };
        self.remove_fids_for_node(node);
        Ok(Ok(()))
    }

    fn handle_read(&self, request: &Virtio9pRequest) -> Result<Result<Vec<u8>, u32>, VirtioError> {
        let read = parse_read_request(request)?;
        let Some(fid) = self
            .fids
            .lock()
            .expect("virtio 9p fid lock")
            .get(&read.fid)
            .copied()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if !fid.is_open() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let Some(data) = namespace.read_file(fid.node(), read.offset, read.count) else {
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
            .copied()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if !fid.is_open() {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let mut namespace = self.namespace.lock().expect("virtio 9p namespace lock");
        let Some(bytes) = namespace.write_file(fid.node(), write.offset, &write.data) else {
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
            .copied()
        else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        let node = fid.node();
        if node == Virtio9pNodeId::Root {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        if self
            .namespace
            .lock()
            .expect("virtio 9p namespace lock")
            .remove_file_by_node(node)
        {
            self.remove_fids_for_node(node);
            Ok(Ok(()))
        } else {
            Ok(Err(VIRTIO_9P_EBADF))
        }
    }

    fn remove_fids_for_node(&self, node: Virtio9pNodeId) {
        self.fids
            .lock()
            .expect("virtio 9p fid lock")
            .retain(|_, fid| fid.node() != node);
    }
}

impl Default for Virtio9pDevice {
    fn default() -> Self {
        Self::new(Virtio9pConfig::default())
    }
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
    const fn new(fid: u32, afid: u32, uname: String, aname: String, n_uname: u32) -> Self {
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

fn parse_version_request(request: &Virtio9pRequest) -> Result<Vec<u8>, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let _msize = reader.read_u32()?;
    let version = reader.read_string()?;
    reader.finish()?;
    Ok(version)
}

fn parse_attach_request(request: &Virtio9pRequest) -> Result<Virtio9pAttachedFid, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let afid = reader.read_u32()?;
    let uname = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let aname = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let n_uname = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pAttachedFid::new(fid, afid, uname, aname, n_uname))
}

fn parse_statfs_request(request: &Virtio9pRequest) -> Result<u32, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    reader.finish()?;
    Ok(fid)
}

fn parse_walk_request(request: &Virtio9pRequest) -> Result<Virtio9pWalkRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let newfid = reader.read_u32()?;
    let name_count = reader.read_u16()?;
    let mut names = Vec::new();
    for _ in 0..name_count {
        names.push(string_from_9p(
            request.message_type(),
            reader.read_string()?,
            request.payload(),
        )?);
    }
    reader.finish()?;
    Ok(Virtio9pWalkRequest { fid, newfid, names })
}

fn parse_lopen_request(request: &Virtio9pRequest) -> Result<Virtio9pOpenRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let _flags = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pOpenRequest { fid })
}

fn parse_lcreate_request(request: &Virtio9pRequest) -> Result<Virtio9pCreateRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let name = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let _flags = reader.read_u32()?;
    let _mode = reader.read_u32()?;
    let _gid = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pCreateRequest { fid, name })
}

fn parse_mkdir_request(request: &Virtio9pRequest) -> Result<Virtio9pMkdirRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let dfid = reader.read_u32()?;
    let name = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let _mode = reader.read_u32()?;
    let _gid = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pMkdirRequest { dfid, name })
}

fn parse_getattr_request(request: &Virtio9pRequest) -> Result<Virtio9pGetattrRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let request_mask = reader.read_u64()?;
    reader.finish()?;
    Ok(Virtio9pGetattrRequest { fid, request_mask })
}

fn parse_readdir_request(request: &Virtio9pRequest) -> Result<Virtio9pReaddirRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let offset = reader.read_u64()?;
    let count = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pReaddirRequest { fid, offset, count })
}

fn parse_renameat_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pRenameatRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let olddirfid = reader.read_u32()?;
    let oldname = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let newdirfid = reader.read_u32()?;
    let newname = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    reader.finish()?;
    Ok(Virtio9pRenameatRequest {
        olddirfid,
        oldname,
        newdirfid,
        newname,
    })
}

fn parse_unlinkat_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pUnlinkatRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let dirfid = reader.read_u32()?;
    let name = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let _flags = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pUnlinkatRequest { dirfid, name })
}

fn parse_read_request(request: &Virtio9pRequest) -> Result<Virtio9pReadRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let offset = reader.read_u64()?;
    let count = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pReadRequest { fid, offset, count })
}

fn parse_write_request(request: &Virtio9pRequest) -> Result<Virtio9pWriteRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let offset = reader.read_u64()?;
    let count = reader.read_u32()?;
    let data = reader.read_counted_bytes(count)?;
    reader.finish()?;
    Ok(Virtio9pWriteRequest { fid, offset, data })
}

fn parse_clunk_request(request: &Virtio9pRequest) -> Result<u32, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    reader.finish()?;
    Ok(fid)
}

fn parse_remove_request(request: &Virtio9pRequest) -> Result<u32, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    reader.finish()?;
    Ok(fid)
}

fn version_payload(msize: u32, version: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(msize.to_le_bytes());
    payload.extend((version.len() as u16).to_le_bytes());
    payload.extend_from_slice(version);
    payload
}

fn string_from_9p(
    message_type: u8,
    bytes: Vec<u8>,
    original_payload: &[u8],
) -> Result<String, VirtioError> {
    String::from_utf8(bytes).map_err(|_| VirtioError::InvalidVirtio9pPayload {
        message_type,
        bytes: original_payload.len(),
    })
}

struct Virtio9pPayloadReader<'a> {
    message_type: u8,
    payload: &'a [u8],
    cursor: usize,
}

impl<'a> Virtio9pPayloadReader<'a> {
    const fn new(message_type: u8, payload: &'a [u8]) -> Self {
        Self {
            message_type,
            payload,
            cursor: 0,
        }
    }

    fn read_u16(&mut self) -> Result<u16, VirtioError> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_u32(&mut self) -> Result<u32, VirtioError> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_u64(&mut self) -> Result<u64, VirtioError> {
        let bytes = self.read_exact(8)?;
        Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_string(&mut self) -> Result<Vec<u8>, VirtioError> {
        let len = usize::from(self.read_u16()?);
        Ok(self.read_exact(len)?.to_vec())
    }

    fn read_counted_bytes(&mut self, count: u32) -> Result<Vec<u8>, VirtioError> {
        let len = usize::try_from(count).map_err(|_| VirtioError::Virtio9pPayloadLengthOverflow)?;
        Ok(self.read_exact(len)?.to_vec())
    }

    fn read_exact(&mut self, bytes: usize) -> Result<&'a [u8], VirtioError> {
        let end = self
            .cursor
            .checked_add(bytes)
            .ok_or(VirtioError::Virtio9pPayloadLengthOverflow)?;
        let data =
            self.payload
                .get(self.cursor..end)
                .ok_or(VirtioError::InvalidVirtio9pPayload {
                    message_type: self.message_type,
                    bytes: self.payload.len(),
                })?;
        self.cursor = end;
        Ok(data)
    }

    fn finish(self) -> Result<(), VirtioError> {
        if self.cursor == self.payload.len() {
            Ok(())
        } else {
            Err(VirtioError::InvalidVirtio9pPayload {
                message_type: self.message_type,
                bytes: self.payload.len(),
            })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Virtio9pQid {
    qtype: u8,
    version: u32,
    path: u64,
}

impl Virtio9pQid {
    const fn new(qtype: u8, path: u64) -> Self {
        Self {
            qtype,
            version: 0,
            path,
        }
    }

    fn to_le_bytes(self) -> [u8; VIRTIO_9P_QID_BYTES] {
        let version = self.version.to_le_bytes();
        let path = self.path.to_le_bytes();
        [
            self.qtype, version[0], version[1], version[2], version[3], path[0], path[1], path[2],
            path[3], path[4], path[5], path[6], path[7],
        ]
    }
}

fn qid_payload(qid: Virtio9pQid) -> Vec<u8> {
    qid.to_le_bytes().to_vec()
}

fn getattr_payload(metadata: Virtio9pNodeMetadata, request_mask: u64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(153);
    payload.extend((request_mask & VIRTIO_9P_GETATTR_BASIC).to_le_bytes());
    payload.extend(metadata.qid.to_le_bytes());
    payload.extend(metadata.mode.to_le_bytes());
    payload.extend(0_u32.to_le_bytes());
    payload.extend(0_u32.to_le_bytes());
    payload.extend(metadata.nlink.to_le_bytes());
    payload.extend(0_u64.to_le_bytes());
    payload.extend(metadata.size.to_le_bytes());
    payload.extend(u64::from(VIRTIO_9P_STATFS_BLOCK_SIZE).to_le_bytes());
    payload.extend(metadata.blocks.to_le_bytes());
    for _ in 0..10 {
        payload.extend(0_u64.to_le_bytes());
    }
    payload
}

fn counted_payload(data: Vec<u8>) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4 + data.len());
    payload.extend((data.len() as u32).to_le_bytes());
    payload.extend(data);
    payload
}

fn readdir_entry_bytes(qid: Virtio9pQid, next_offset: u64, dtype: u8, name: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24 + name.len());
    bytes.extend(qid.to_le_bytes());
    bytes.extend(next_offset.to_le_bytes());
    bytes.push(dtype);
    bytes.extend((name.len() as u16).to_le_bytes());
    bytes.extend(name.as_bytes());
    bytes
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Virtio9pNodeId {
    Root,
    File(u64),
    Directory(u64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Virtio9pNamespace {
    entries: BTreeMap<String, Virtio9pNode>,
    next_path: u64,
}

impl Virtio9pNamespace {
    fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            next_path: 2,
        }
    }

    const fn root_qid(&self) -> Virtio9pQid {
        Virtio9pQid::new(VIRTIO_9P_QTDIR, 1)
    }

    fn insert_file(&mut self, name: String, data: Vec<u8>) -> Result<(), VirtioError> {
        validate_file_name(VIRTIO_9P_TWALK, &name)?;
        let path = self.allocate_path()?;
        self.entries.insert(
            name,
            Virtio9pNode::File(Virtio9pFileNode {
                qid_path: path,
                data,
            }),
        );
        Ok(())
    }

    fn create_file(
        &mut self,
        parent: Virtio9pNodeId,
        name: String,
    ) -> Result<Result<Virtio9pNodeId, u32>, VirtioError> {
        validate_file_name(VIRTIO_9P_TLCREATE, &name)?;
        let Some(entries) = self.directory_entries(parent) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if matches!(entries.get(&name), Some(Virtio9pNode::Directory(_))) {
            return Ok(Err(VIRTIO_9P_EEXIST));
        }
        let path = self.allocate_path()?;
        let entries = self
            .directory_entries_mut(parent)
            .expect("prevalidated 9p directory parent");
        entries.insert(
            name,
            Virtio9pNode::File(Virtio9pFileNode {
                qid_path: path,
                data: Vec::new(),
            }),
        );
        Ok(Ok(Virtio9pNodeId::File(path)))
    }

    fn mkdir(
        &mut self,
        parent: Virtio9pNodeId,
        name: String,
    ) -> Result<Result<Virtio9pNodeId, u32>, VirtioError> {
        validate_file_name(VIRTIO_9P_TMKDIR, &name)?;
        let Some(entries) = self.directory_entries(parent) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if entries.contains_key(&name) {
            return Ok(Err(VIRTIO_9P_EEXIST));
        }
        let path = self.allocate_path()?;
        let entries = self
            .directory_entries_mut(parent)
            .expect("prevalidated 9p directory parent");
        entries.insert(
            name,
            Virtio9pNode::Directory(Virtio9pDirectoryNode {
                qid_path: path,
                entries: BTreeMap::new(),
            }),
        );
        Ok(Ok(Virtio9pNodeId::Directory(path)))
    }

    fn remove_file_by_name(
        &mut self,
        parent: Virtio9pNodeId,
        name: &str,
    ) -> Result<Result<Virtio9pNodeId, u32>, VirtioError> {
        validate_file_name(VIRTIO_9P_TUNLINKAT, name)?;
        let Some(entries) = self.directory_entries_mut(parent) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        match entries.get(name) {
            Some(Virtio9pNode::File(_)) => {
                let Some(Virtio9pNode::File(file)) = entries.remove(name) else {
                    unreachable!("prevalidated 9p file entry")
                };
                Ok(Ok(Virtio9pNodeId::File(file.qid_path)))
            }
            Some(Virtio9pNode::Directory(_)) => Ok(Err(VIRTIO_9P_EBADF)),
            None => Ok(Err(VIRTIO_9P_ENOENT)),
        }
    }

    fn remove_file_by_node(&mut self, node: Virtio9pNodeId) -> bool {
        let Virtio9pNodeId::File(path) = node else {
            return false;
        };
        remove_file_by_path(&mut self.entries, path)
    }

    fn rename_file(
        &mut self,
        old_parent: Virtio9pNodeId,
        oldname: &str,
        new_parent: Virtio9pNodeId,
        newname: &str,
    ) -> Result<Result<Virtio9pRenameOutcome, u32>, VirtioError> {
        validate_file_name(VIRTIO_9P_TRENAMEAT, oldname)?;
        validate_file_name(VIRTIO_9P_TRENAMEAT, newname)?;
        if old_parent != new_parent {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let Some(entries) = self.directory_entries_mut(old_parent) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if oldname == newname {
            return match entries.get(oldname) {
                Some(Virtio9pNode::File(_)) => Ok(Ok(Virtio9pRenameOutcome { replaced: None })),
                Some(Virtio9pNode::Directory(_)) => Ok(Err(VIRTIO_9P_EBADF)),
                None => Ok(Err(VIRTIO_9P_ENOENT)),
            };
        }
        match entries.get(newname) {
            Some(Virtio9pNode::Directory(_)) => return Ok(Err(VIRTIO_9P_EEXIST)),
            Some(Virtio9pNode::File(_)) | None => {}
        }
        let Some(Virtio9pNode::File(file)) = entries.remove(oldname) else {
            return Ok(Err(VIRTIO_9P_ENOENT));
        };
        let replaced = entries
            .insert(newname.to_string(), Virtio9pNode::File(file))
            .and_then(|node| match node {
                Virtio9pNode::File(file) => Some(Virtio9pNodeId::File(file.qid_path)),
                Virtio9pNode::Directory(_) => None,
            });
        Ok(Ok(Virtio9pRenameOutcome { replaced }))
    }

    fn walk(&self, node: Virtio9pNodeId, name: &str) -> Option<Virtio9pNodeId> {
        self.directory_entries(node)?
            .get(name)
            .map(Virtio9pNode::id)
    }

    fn qid(&self, node: Virtio9pNodeId) -> Virtio9pQid {
        match node {
            Virtio9pNodeId::Root => self.root_qid(),
            Virtio9pNodeId::File(path) => Virtio9pQid::new(VIRTIO_9P_QTFILE, path),
            Virtio9pNodeId::Directory(path) => Virtio9pQid::new(VIRTIO_9P_QTDIR, path),
        }
    }

    fn metadata(&self, node: Virtio9pNodeId) -> Option<Virtio9pNodeMetadata> {
        match node {
            Virtio9pNodeId::Root => Some(Virtio9pNodeMetadata {
                qid: self.root_qid(),
                mode: 0o040755,
                nlink: 2 + self.entries.len() as u64,
                size: 0,
                blocks: 0,
            }),
            Virtio9pNodeId::File(path) => {
                let file = find_file(&self.entries, path)?;
                let size = file.data.len() as u64;
                Some(Virtio9pNodeMetadata {
                    qid: Virtio9pQid::new(VIRTIO_9P_QTFILE, path),
                    mode: 0o100644,
                    nlink: 1,
                    size,
                    blocks: size.div_ceil(512),
                })
            }
            Virtio9pNodeId::Directory(path) => {
                let entries = self.directory_entries(node)?;
                Some(Virtio9pNodeMetadata {
                    qid: Virtio9pQid::new(VIRTIO_9P_QTDIR, path),
                    mode: 0o040755,
                    nlink: 2 + entries.len() as u64,
                    size: 0,
                    blocks: 0,
                })
            }
        }
    }

    fn statfs_payload(&self) -> Vec<u8> {
        let files = 1 + count_nodes(&self.entries);
        let mut payload = Vec::with_capacity(60);
        payload.extend(VIRTIO_9P_STATFS_TYPE.to_le_bytes());
        payload.extend(VIRTIO_9P_STATFS_BLOCK_SIZE.to_le_bytes());
        payload.extend(VIRTIO_9P_STATFS_BLOCKS.to_le_bytes());
        payload.extend(VIRTIO_9P_STATFS_BLOCKS.to_le_bytes());
        payload.extend(VIRTIO_9P_STATFS_BLOCKS.to_le_bytes());
        payload.extend(files.to_le_bytes());
        payload.extend(VIRTIO_9P_STATFS_BLOCKS.saturating_sub(files).to_le_bytes());
        payload.extend(VIRTIO_9P_STATFS_FSID.to_le_bytes());
        payload.extend(VIRTIO_9P_NAME_MAX.to_le_bytes());
        payload
    }

    fn readdir_payload(&self, node: Virtio9pNodeId, offset: u64, count: u32) -> Option<Vec<u8>> {
        let children = self.directory_entries(node)?;
        let start = usize::try_from(offset).ok()?;
        let budget = usize::try_from(count).ok()?;
        let mut entries = Vec::with_capacity(2 + children.len());
        let mut next_offset = 0_u64;

        for (qid, dtype, name) in [
            (self.qid(node), VIRTIO_9P_DTDIR, "."),
            (self.root_qid(), VIRTIO_9P_DTDIR, ".."),
        ] {
            let entry_len = 24 + name.len();
            next_offset = next_offset.checked_add(entry_len as u64)?;
            entries.push(readdir_entry_bytes(qid, next_offset, dtype, name));
        }

        for (name, child) in children {
            let entry_len = 24 + name.len();
            next_offset = next_offset.checked_add(entry_len as u64)?;
            entries.push(readdir_entry_bytes(
                child.qid(),
                next_offset,
                child.dtype(),
                name,
            ));
        }

        let mut full_offset = 0_usize;
        let mut data = Vec::new();
        for entry in entries {
            let entry_start = full_offset;
            let entry_end = entry_start.checked_add(entry.len())?;
            full_offset = entry_end;
            if entry_start < start {
                continue;
            }
            if data.len().checked_add(entry.len())? > budget {
                break;
            }
            data.extend(entry);
        }
        Some(counted_payload(data))
    }

    fn read_file(&self, node: Virtio9pNodeId, offset: u64, count: u32) -> Option<Vec<u8>> {
        let Virtio9pNodeId::File(path) = node else {
            return None;
        };
        let file = find_file(&self.entries, path)?;
        let start = usize::try_from(offset).ok()?;
        if start >= file.data.len() {
            return Some(Vec::new());
        }
        let count = usize::try_from(count).ok()?;
        let end = start.saturating_add(count).min(file.data.len());
        Some(file.data[start..end].to_vec())
    }

    fn write_file(&mut self, node: Virtio9pNodeId, offset: u64, data: &[u8]) -> Option<u32> {
        let Virtio9pNodeId::File(path) = node else {
            return None;
        };
        let file = find_file_mut(&mut self.entries, path)?;
        let start = usize::try_from(offset).ok()?;
        let end = start.checked_add(data.len())?;
        if file.data.len() < end {
            file.data.resize(end, 0);
        }
        file.data[start..end].copy_from_slice(data);
        u32::try_from(data.len()).ok()
    }

    fn directory_entries(&self, node: Virtio9pNodeId) -> Option<&BTreeMap<String, Virtio9pNode>> {
        match node {
            Virtio9pNodeId::Root => Some(&self.entries),
            Virtio9pNodeId::Directory(path) => {
                find_directory(&self.entries, path).map(|directory| &directory.entries)
            }
            Virtio9pNodeId::File(_) => None,
        }
    }

    fn directory_entries_mut(
        &mut self,
        node: Virtio9pNodeId,
    ) -> Option<&mut BTreeMap<String, Virtio9pNode>> {
        match node {
            Virtio9pNodeId::Root => Some(&mut self.entries),
            Virtio9pNodeId::Directory(path) => {
                find_directory_mut(&mut self.entries, path).map(|directory| &mut directory.entries)
            }
            Virtio9pNodeId::File(_) => None,
        }
    }

    fn allocate_path(&mut self) -> Result<u64, VirtioError> {
        let path = self.next_path;
        self.next_path = self
            .next_path
            .checked_add(1)
            .ok_or(VirtioError::Virtio9pPayloadLengthOverflow)?;
        Ok(path)
    }
}

fn count_nodes(entries: &BTreeMap<String, Virtio9pNode>) -> u64 {
    entries
        .values()
        .map(|node| match node {
            Virtio9pNode::File(_) => 1,
            Virtio9pNode::Directory(directory) => 1 + count_nodes(&directory.entries),
        })
        .sum()
}

fn find_file(entries: &BTreeMap<String, Virtio9pNode>, path: u64) -> Option<&Virtio9pFileNode> {
    for node in entries.values() {
        match node {
            Virtio9pNode::File(file) if file.qid_path == path => return Some(file),
            Virtio9pNode::File(_) => {}
            Virtio9pNode::Directory(directory) => {
                if let Some(file) = find_file(&directory.entries, path) {
                    return Some(file);
                }
            }
        }
    }
    None
}

fn find_file_mut(
    entries: &mut BTreeMap<String, Virtio9pNode>,
    path: u64,
) -> Option<&mut Virtio9pFileNode> {
    for node in entries.values_mut() {
        match node {
            Virtio9pNode::File(file) if file.qid_path == path => return Some(file),
            Virtio9pNode::File(_) => {}
            Virtio9pNode::Directory(directory) => {
                if let Some(file) = find_file_mut(&mut directory.entries, path) {
                    return Some(file);
                }
            }
        }
    }
    None
}

fn find_directory(
    entries: &BTreeMap<String, Virtio9pNode>,
    path: u64,
) -> Option<&Virtio9pDirectoryNode> {
    for node in entries.values() {
        let Virtio9pNode::Directory(directory) = node else {
            continue;
        };
        if directory.qid_path == path {
            return Some(directory);
        }
        if let Some(directory) = find_directory(&directory.entries, path) {
            return Some(directory);
        }
    }
    None
}

fn find_directory_mut(
    entries: &mut BTreeMap<String, Virtio9pNode>,
    path: u64,
) -> Option<&mut Virtio9pDirectoryNode> {
    for node in entries.values_mut() {
        let Virtio9pNode::Directory(directory) = node else {
            continue;
        };
        if directory.qid_path == path {
            return Some(directory);
        }
        if let Some(directory) = find_directory_mut(&mut directory.entries, path) {
            return Some(directory);
        }
    }
    None
}

fn remove_file_by_path(entries: &mut BTreeMap<String, Virtio9pNode>, path: u64) -> bool {
    if let Some(name) = entries.iter().find_map(|(name, node)| match node {
        Virtio9pNode::File(file) if file.qid_path == path => Some(name.clone()),
        Virtio9pNode::File(_) | Virtio9pNode::Directory(_) => None,
    }) {
        return entries.remove(&name).is_some();
    }
    entries.values_mut().any(|node| match node {
        Virtio9pNode::Directory(directory) => remove_file_by_path(&mut directory.entries, path),
        Virtio9pNode::File(_) => false,
    })
}

fn validate_file_name(message_type: u8, name: &str) -> Result<(), VirtioError> {
    if name.is_empty() || name.contains('/') {
        return Err(VirtioError::InvalidVirtio9pPayload {
            message_type,
            bytes: name.len(),
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Virtio9pFileNode {
    qid_path: u64,
    data: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Virtio9pDirectoryNode {
    qid_path: u64,
    entries: BTreeMap<String, Virtio9pNode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Virtio9pNode {
    File(Virtio9pFileNode),
    Directory(Virtio9pDirectoryNode),
}

impl Virtio9pNode {
    const fn id(&self) -> Virtio9pNodeId {
        match self {
            Self::File(file) => Virtio9pNodeId::File(file.qid_path),
            Self::Directory(directory) => Virtio9pNodeId::Directory(directory.qid_path),
        }
    }

    const fn qid(&self) -> Virtio9pQid {
        match self {
            Self::File(file) => Virtio9pQid::new(VIRTIO_9P_QTFILE, file.qid_path),
            Self::Directory(directory) => Virtio9pQid::new(VIRTIO_9P_QTDIR, directory.qid_path),
        }
    }

    const fn dtype(&self) -> u8 {
        match self {
            Self::File(_) => VIRTIO_9P_DTREG,
            Self::Directory(_) => VIRTIO_9P_DTDIR,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Virtio9pRenameOutcome {
    replaced: Option<Virtio9pNodeId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Virtio9pNodeMetadata {
    qid: Virtio9pQid,
    mode: u32,
    nlink: u64,
    size: u64,
    blocks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Virtio9pFidState {
    node: Virtio9pNodeId,
    open: bool,
}

impl Virtio9pFidState {
    const fn new(node: Virtio9pNodeId) -> Self {
        Self { node, open: false }
    }

    const fn node(self) -> Virtio9pNodeId {
        self.node
    }

    fn open(&mut self) {
        self.open = true;
    }

    const fn opened(node: Virtio9pNodeId) -> Self {
        Self { node, open: true }
    }

    const fn is_open(self) -> bool {
        self.open
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Virtio9pWalkRequest {
    fid: u32,
    newfid: u32,
    names: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Virtio9pOpenRequest {
    fid: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Virtio9pCreateRequest {
    fid: u32,
    name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Virtio9pMkdirRequest {
    dfid: u32,
    name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Virtio9pGetattrRequest {
    fid: u32,
    request_mask: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Virtio9pReaddirRequest {
    fid: u32,
    offset: u64,
    count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Virtio9pRenameatRequest {
    olddirfid: u32,
    oldname: String,
    newdirfid: u32,
    newname: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Virtio9pUnlinkatRequest {
    dirfid: u32,
    name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Virtio9pReadRequest {
    fid: u32,
    offset: u64,
    count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Virtio9pWriteRequest {
    fid: u32,
    offset: u64,
    data: Vec<u8>,
}
