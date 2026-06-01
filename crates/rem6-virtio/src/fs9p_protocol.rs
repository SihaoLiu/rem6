use crate::{fs9p::Virtio9pAttachedFid, fs9p_queue::Virtio9pRequest, VirtioError};

pub const VIRTIO_9P_PROTOCOL_VERSION: &[u8] = b"9P2000.L";
pub const VIRTIO_9P_TSTATFS: u8 = 8;
pub const VIRTIO_9P_RSTATFS: u8 = 9;
pub const VIRTIO_9P_TVERSION: u8 = 100;
pub const VIRTIO_9P_RVERSION: u8 = 101;
pub const VIRTIO_9P_TAUTH: u8 = 102;
pub const VIRTIO_9P_RAUTH: u8 = 103;
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
pub const VIRTIO_9P_TSTAT: u8 = 124;
pub const VIRTIO_9P_RSTAT: u8 = 125;
pub const VIRTIO_9P_TWSTAT: u8 = 126;
pub const VIRTIO_9P_RWSTAT: u8 = 127;
pub const VIRTIO_9P_RLERROR: u8 = 7;
pub const VIRTIO_9P_NOFID: u32 = u32::MAX;
pub const VIRTIO_9P_EBADF: u32 = 9;
pub const VIRTIO_9P_EEXIST: u32 = 17;
pub const VIRTIO_9P_ENOENT: u32 = 2;
pub const VIRTIO_9P_ENODATA: u32 = 61;
pub const VIRTIO_9P_ENOTEMPTY: u32 = 39;
pub const VIRTIO_9P_ENOTSUP: u32 = 95;
pub const VIRTIO_9P_EINVAL: u32 = 22;
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
pub const VIRTIO_9P_LOCK_BLOCKED: u8 = 1;
pub const VIRTIO_9P_LOCK_TYPE_RDLCK: u8 = 0;
pub const VIRTIO_9P_LOCK_TYPE_WRLCK: u8 = 1;
pub const VIRTIO_9P_LOCK_TYPE_UNLCK: u8 = 2;
pub const VIRTIO_9P_LOCK_FLAGS_BLOCK: u32 = 1;
pub const VIRTIO_9P_LOCK_FLAGS_RECLAIM: u32 = 2;
pub const VIRTIO_9P_AT_REMOVEDIR: u32 = 0x200;
pub const VIRTIO_9P_XATTR_CREATE: u32 = 0x1;
pub const VIRTIO_9P_XATTR_REPLACE: u32 = 0x2;
pub const VIRTIO_9P_OPEN_READ_ONLY: u8 = 0;
pub const VIRTIO_9P_OPEN_WRITE_ONLY: u8 = 1;
pub const VIRTIO_9P_OPEN_READ_WRITE: u8 = 2;
pub const VIRTIO_9P_OPEN_EXECUTE_ONLY: u8 = 3;
pub const VIRTIO_9P_OPEN_ACCESS_MASK: u8 = 0x3;
pub const VIRTIO_9P_OPEN_TRUNCATE: u8 = 0x10;
pub const VIRTIO_9P_OPEN_REMOVE_ON_CLOSE: u8 = 0x40;
pub const VIRTIO_9P_OPEN_APPEND: u8 = 0x80;
pub const VIRTIO_9P_LOPEN_TRUNCATE: u32 = 0x0000_0200;
pub const VIRTIO_9P_LOPEN_APPEND: u32 = 0x0000_0400;
pub const VIRTIO_9P_STATFS_TYPE: u32 = 0x0102_1997;
pub const VIRTIO_9P_STATFS_BLOCK_SIZE: u32 = 4096;
pub const VIRTIO_9P_NAME_MAX: u32 = 255;
const VIRTIO_9P_MAX_WALK_ELEMENTS: u16 = 16;

pub(crate) fn parse_version_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pVersionRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let msize = reader.read_u32()?;
    let version = reader.read_string()?;
    reader.finish()?;
    Ok(Virtio9pVersionRequest { msize, version })
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
    if name_count > VIRTIO_9P_MAX_WALK_ELEMENTS {
        return Err(VirtioError::InvalidVirtio9pPayload {
            message_type: request.message_type(),
            bytes: request.payload().len(),
        });
    }
    let mut names = Vec::with_capacity(usize::from(name_count));
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
    let flags = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pOpenRequest {
        fid,
        mode: (flags & u32::from(VIRTIO_9P_OPEN_ACCESS_MASK)) as u8,
        truncate: flags & VIRTIO_9P_LOPEN_TRUNCATE != 0,
        remove_on_clunk: false,
        append: flags & VIRTIO_9P_LOPEN_APPEND != 0,
    })
}

pub(crate) fn parse_open_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pOpenRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let mode = reader.read_u8()?;
    reader.finish()?;
    Ok(Virtio9pOpenRequest {
        fid,
        mode: mode & VIRTIO_9P_OPEN_ACCESS_MASK,
        truncate: mode & VIRTIO_9P_OPEN_TRUNCATE != 0,
        remove_on_clunk: mode & VIRTIO_9P_OPEN_REMOVE_ON_CLOSE != 0,
        append: mode & VIRTIO_9P_OPEN_APPEND != 0,
    })
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
    let flags = reader.read_u32()?;
    let _mode = reader.read_u32()?;
    let _gid = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pCreateRequest {
        fid,
        name,
        mode: (flags & u32::from(VIRTIO_9P_OPEN_ACCESS_MASK)) as u8,
        append: flags & VIRTIO_9P_LOPEN_APPEND != 0,
    })
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
    let raw_mode = reader.read_u8()?;
    let mode = raw_mode & VIRTIO_9P_OPEN_ACCESS_MASK;
    reader.finish()?;
    Ok(Virtio9pCreateRequest {
        fid,
        name,
        mode,
        append: raw_mode & VIRTIO_9P_OPEN_APPEND != 0,
    })
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

pub(crate) fn parse_xattrcreate_request(
    request: &Virtio9pRequest,
) -> Result<Virtio9pXattrcreateRequest, VirtioError> {
    let mut reader = Virtio9pPayloadReader::new(request.message_type(), request.payload());
    let fid = reader.read_u32()?;
    let name = string_from_9p(
        request.message_type(),
        reader.read_string()?,
        request.payload(),
    )?;
    let attr_size = reader.read_u64()?;
    let flags = reader.read_u32()?;
    reader.finish()?;
    Ok(Virtio9pXattrcreateRequest {
        fid,
        name,
        attr_size,
        flags,
    })
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
pub(crate) struct Virtio9pVersionRequest {
    pub(crate) msize: u32,
    pub(crate) version: Vec<u8>,
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
    pub(crate) mode: u8,
    pub(crate) truncate: bool,
    pub(crate) remove_on_clunk: bool,
    pub(crate) append: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pCreateRequest {
    pub(crate) fid: u32,
    pub(crate) name: String,
    pub(crate) mode: u8,
    pub(crate) append: bool,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pXattrcreateRequest {
    pub(crate) fid: u32,
    pub(crate) name: String,
    pub(crate) attr_size: u64,
    pub(crate) flags: u32,
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
