use crate::{fs9p::Virtio9pAttachedFid, fs9p_queue::Virtio9pRequest, VirtioError};

pub(crate) fn parse_version_request(request: &Virtio9pRequest) -> Result<Vec<u8>, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let _msize = reader.read_u32()?;
    let version = reader.read_string()?;
    reader.finish()?;
    Ok(version)
}

pub(crate) fn parse_attach_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pAttachedFid, VirtioError> {
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

pub(crate) fn parse_auth_request(request: &Virtio9pRequest) -> Result<(), VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let _afid = reader.read_u32()?;
    let _uname = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let _aname = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let _n_uname = reader.read_u32()?;
    reader.finish()
}

pub(crate) fn parse_statfs_request(request: &Virtio9pRequest) -> Result<u32, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    reader.finish()?;
    Ok(fid)
}

pub(crate) fn parse_walk_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pWalkRequest, VirtioError> {
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

pub(crate) fn parse_lopen_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pOpenRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let _flags = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pOpenRequest { fid })
}

pub(crate) fn parse_open_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pOpenRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let _mode = reader.read_u8()?;
    reader.finish()?;
    Ok(Virtio9pOpenRequest { fid })
}

pub(crate) fn parse_lcreate_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pCreateRequest, VirtioError> {
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

pub(crate) fn parse_create_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pCreateRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let name = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let _perm = reader.read_u32()?;
    let _mode = reader.read_u8()?;
    reader.finish()?;
    Ok(Virtio9pCreateRequest { fid, name })
}

pub(crate) fn parse_symlink_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pSymlinkRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let dfid = reader.read_u32()?;
    let name = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let target = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let _gid = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pSymlinkRequest { dfid, name, target })
}

pub(crate) fn parse_mknod_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pMknodRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let dfid = reader.read_u32()?;
    let name = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let mode = reader.read_u32()?;
    let major = reader.read_u32()?;
    let minor = reader.read_u32()?;
    let _gid = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pMknodRequest {
        dfid,
        name,
        mode,
        major,
        minor,
    })
}

pub(crate) fn parse_readlink_request(request: &Virtio9pRequest) -> Result<u32, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    reader.finish()?;
    Ok(fid)
}

pub(crate) fn parse_mkdir_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pMkdirRequest, VirtioError> {
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

pub(crate) fn parse_link_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pLinkRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let dfid = reader.read_u32()?;
    let oldfid = reader.read_u32()?;
    let newname = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    reader.finish()?;
    Ok(Virtio9pLinkRequest {
        dfid,
        oldfid,
        newname,
    })
}

pub(crate) fn parse_getattr_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pGetattrRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let request_mask = reader.read_u64()?;
    reader.finish()?;
    Ok(Virtio9pGetattrRequest { fid, request_mask })
}

pub(crate) fn parse_setattr_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pSetattrRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let valid = reader.read_u32()?;
    let mode = reader.read_u32()?;
    let uid = reader.read_u32()?;
    let gid = reader.read_u32()?;
    let size = reader.read_u64()?;
    let atime_sec = reader.read_u64()?;
    let atime_nsec = reader.read_u64()?;
    let mtime_sec = reader.read_u64()?;
    let mtime_nsec = reader.read_u64()?;
    reader.finish()?;
    Ok(Virtio9pSetattrRequest {
        fid,
        valid,
        mode,
        uid,
        gid,
        size,
        atime_sec,
        atime_nsec,
        mtime_sec,
        mtime_nsec,
    })
}

pub(crate) fn parse_stat_request(request: &Virtio9pRequest) -> Result<u32, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    reader.finish()?;
    Ok(fid)
}

pub(crate) fn parse_wstat_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pWstatRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let stat_len = reader.read_u16()?;
    let stat = reader.read_counted_bytes(u32::from(stat_len))?;
    reader.finish()?;

    let mut stat_reader = Virtio9pPayloadReader::new(request.message_type(), &stat);
    let _file_type = stat_reader.read_u16()?;
    let _dev = stat_reader.read_u32()?;
    let _qid = stat_reader.read_exact(13)?;
    let mode = stat_reader.read_u32()?;
    let atime_sec = stat_reader.read_u32()?;
    let mtime_sec = stat_reader.read_u32()?;
    let length = stat_reader.read_u64()?;
    let name = string_from_9p(
        request.message_type(),
        stat_reader.read_string()?,
        request.payload(),
    )?;
    let uid = string_from_9p(
        request.message_type(),
        stat_reader.read_string()?,
        request.payload(),
    )?;
    let gid = string_from_9p(
        request.message_type(),
        stat_reader.read_string()?,
        request.payload(),
    )?;
    let _muid = string_from_9p(
        request.message_type(),
        stat_reader.read_string()?,
        request.payload(),
    )?;
    stat_reader.finish()?;

    Ok(Virtio9pWstatRequest {
        fid,
        name: nonempty_string(name),
        mode: (mode != u32::MAX).then_some(mode),
        uid: parse_optional_u32_string(request, uid)?,
        gid: parse_optional_u32_string(request, gid)?,
        atime_sec: (atime_sec != u32::MAX).then_some(atime_sec),
        mtime_sec: (mtime_sec != u32::MAX).then_some(mtime_sec),
        length: (length != u64::MAX).then_some(length),
    })
}

pub(crate) fn parse_xattrwalk_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pXattrwalkRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let newfid = reader.read_u32()?;
    let name = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    reader.finish()?;
    Ok(Virtio9pXattrwalkRequest { fid, newfid, name })
}

pub(crate) fn parse_xattrcreate_request(request: &Virtio9pRequest) -> Result<u32, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let _name = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let _attr_size = reader.read_u64()?;
    let _flags = reader.read_u32()?;
    reader.finish()?;
    Ok(fid)
}

pub(crate) fn parse_readdir_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pReaddirRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let offset = reader.read_u64()?;
    let count = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pReaddirRequest { fid, offset, count })
}

pub(crate) fn parse_fsync_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pFsyncRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let _datasync = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pFsyncRequest { fid })
}

pub(crate) fn parse_lock_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pLockRequest, VirtioError> {
    parse_lock_payload(request)
}

pub(crate) fn parse_getlock_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pLockRequest, VirtioError> {
    parse_lock_payload(request)
}

pub(crate) fn parse_rename_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pRenameRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let newdirfid = reader.read_u32()?;
    let name = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    reader.finish()?;
    Ok(Virtio9pRenameRequest {
        fid,
        newdirfid,
        name,
    })
}

pub(crate) fn parse_renameat_request(
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

pub(crate) fn parse_unlinkat_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pUnlinkatRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let dirfid = reader.read_u32()?;
    let name = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let flags = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pUnlinkatRequest {
        dirfid,
        name,
        flags,
    })
}

pub(crate) fn parse_read_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pReadRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let offset = reader.read_u64()?;
    let count = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pReadRequest { fid, offset, count })
}

pub(crate) fn parse_write_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pWriteRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let offset = reader.read_u64()?;
    let count = reader.read_u32()?;
    let data = reader.read_counted_bytes(count)?;
    reader.finish()?;
    Ok(Virtio9pWriteRequest { fid, offset, data })
}

pub(crate) fn parse_clunk_request(request: &Virtio9pRequest) -> Result<u32, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    reader.finish()?;
    Ok(fid)
}

pub(crate) fn parse_remove_request(request: &Virtio9pRequest) -> Result<u32, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    reader.finish()?;
    Ok(fid)
}

pub(crate) fn parse_flush_request(request: &Virtio9pRequest) -> Result<u16, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let oldtag = reader.read_u16()?;
    reader.finish()?;
    Ok(oldtag)
}

pub(crate) fn version_payload(msize: u32, version: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(msize.to_le_bytes());
    payload.extend(string_payload(version));
    payload
}

pub(crate) fn string_payload(data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(2 + data.len());
    payload.extend((data.len() as u16).to_le_bytes());
    payload.extend_from_slice(data);
    payload
}

pub(crate) fn lock_payload(
    lock_type: u8,
    flags: u32,
    start: u64,
    length: u64,
    proc_id: u32,
    client_id: &str,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(lock_type);
    payload.extend(flags.to_le_bytes());
    payload.extend(start.to_le_bytes());
    payload.extend(length.to_le_bytes());
    payload.extend(proc_id.to_le_bytes());
    payload.extend(string_payload(client_id.as_bytes()));
    payload
}

fn parse_lock_payload(request: &Virtio9pRequest) -> Result<Virtio9pLockRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let lock_type = reader.read_u8()?;
    let flags = reader.read_u32()?;
    let start = reader.read_u64()?;
    let length = reader.read_u64()?;
    let proc_id = reader.read_u32()?;
    let client_id = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    reader.finish()?;
    Ok(Virtio9pLockRequest {
        fid,
        lock_type,
        flags,
        start,
        length,
        proc_id,
        client_id,
    })
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

fn nonempty_string(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn parse_optional_u32_string(
    request: &Virtio9pRequest,
    value: String,
) -> Result<Option<u32>, VirtioError> {
    if value.is_empty() {
        return Ok(None);
    }
    value
        .parse::<u32>()
        .map(Some)
        .map_err(|_| VirtioError::InvalidVirtio9pPayload {
            message_type: request.message_type(),
            bytes: request.payload().len(),
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

    fn read_u8(&mut self) -> Result<u8, VirtioError> {
        Ok(self.read_exact(1)?[0])
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pWalkRequest {
    pub(crate) fid: u32,
    pub(crate) newfid: u32,
    pub(crate) names: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pOpenRequest {
    pub(crate) fid: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pCreateRequest {
    pub(crate) fid: u32,
    pub(crate) name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pSymlinkRequest {
    pub(crate) dfid: u32,
    pub(crate) name: String,
    pub(crate) target: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pMknodRequest {
    pub(crate) dfid: u32,
    pub(crate) name: String,
    pub(crate) mode: u32,
    pub(crate) major: u32,
    pub(crate) minor: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pMkdirRequest {
    pub(crate) dfid: u32,
    pub(crate) name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pLinkRequest {
    pub(crate) dfid: u32,
    pub(crate) oldfid: u32,
    pub(crate) newname: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pGetattrRequest {
    pub(crate) fid: u32,
    pub(crate) request_mask: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pSetattrRequest {
    pub(crate) fid: u32,
    pub(crate) valid: u32,
    pub(crate) mode: u32,
    pub(crate) uid: u32,
    pub(crate) gid: u32,
    pub(crate) size: u64,
    pub(crate) atime_sec: u64,
    pub(crate) atime_nsec: u64,
    pub(crate) mtime_sec: u64,
    pub(crate) mtime_nsec: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pWstatRequest {
    pub(crate) fid: u32,
    pub(crate) name: Option<String>,
    pub(crate) mode: Option<u32>,
    pub(crate) uid: Option<u32>,
    pub(crate) gid: Option<u32>,
    pub(crate) atime_sec: Option<u32>,
    pub(crate) mtime_sec: Option<u32>,
    pub(crate) length: Option<u64>,
}

impl Virtio9pWstatRequest {
    pub(crate) const fn has_metadata_update(&self) -> bool {
        self.mode.is_some()
            || self.uid.is_some()
            || self.gid.is_some()
            || self.atime_sec.is_some()
            || self.mtime_sec.is_some()
            || self.length.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pXattrwalkRequest {
    pub(crate) fid: u32,
    pub(crate) newfid: u32,
    pub(crate) name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pReaddirRequest {
    pub(crate) fid: u32,
    pub(crate) offset: u64,
    pub(crate) count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pFsyncRequest {
    pub(crate) fid: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pLockRequest {
    pub(crate) fid: u32,
    pub(crate) lock_type: u8,
    pub(crate) flags: u32,
    pub(crate) start: u64,
    pub(crate) length: u64,
    pub(crate) proc_id: u32,
    pub(crate) client_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pRenameRequest {
    pub(crate) fid: u32,
    pub(crate) newdirfid: u32,
    pub(crate) name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pRenameatRequest {
    pub(crate) olddirfid: u32,
    pub(crate) oldname: String,
    pub(crate) newdirfid: u32,
    pub(crate) newname: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pUnlinkatRequest {
    pub(crate) dirfid: u32,
    pub(crate) name: String,
    pub(crate) flags: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pReadRequest {
    pub(crate) fid: u32,
    pub(crate) offset: u64,
    pub(crate) count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pWriteRequest {
    pub(crate) fid: u32,
    pub(crate) offset: u64,
    pub(crate) data: Vec<u8>,
}
