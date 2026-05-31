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
pub const VIRTIO_9P_TVERSION: u8 = 100;
pub const VIRTIO_9P_RVERSION: u8 = 101;
pub const VIRTIO_9P_TATTACH: u8 = 104;
pub const VIRTIO_9P_RATTACH: u8 = 105;
pub const VIRTIO_9P_RLERROR: u8 = 7;
pub const VIRTIO_9P_NOFID: u32 = u32::MAX;
pub const VIRTIO_9P_ENOTSUP: u32 = 95;
pub const VIRTIO_9P_QTDIR: u8 = 0x80;
pub const VIRTIO_9P_CONFIG_TAG_LENGTH_OFFSET: u64 = 0;
pub const VIRTIO_9P_CONFIG_TAG_OFFSET: u64 = 2;

const VIRTIO_9P_CONFIG_LENGTH_BYTES: usize = 2;
const VIRTIO_9P_QID_BYTES: usize = 13;

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
}

impl Virtio9pDevice {
    pub fn new(config: Virtio9pConfig) -> Self {
        Self {
            config,
            completions: Arc::new(Mutex::new(Vec::new())),
            attached_fids: Arc::new(Mutex::new(Vec::new())),
        }
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
                self.attached_fids
                    .lock()
                    .expect("virtio 9p attached fid lock")
                    .push(attached);
                (VIRTIO_9P_RATTACH, root_qid_payload())
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

fn version_payload(msize: u32, version: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(msize.to_le_bytes());
    payload.extend((version.len() as u16).to_le_bytes());
    payload.extend_from_slice(version);
    payload
}

fn root_qid_payload() -> Vec<u8> {
    let mut payload = Vec::with_capacity(VIRTIO_9P_QID_BYTES);
    payload.push(VIRTIO_9P_QTDIR);
    payload.extend(0_u32.to_le_bytes());
    payload.extend(1_u64.to_le_bytes());
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

    fn read_string(&mut self) -> Result<Vec<u8>, VirtioError> {
        let len = usize::from(self.read_u16()?);
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
