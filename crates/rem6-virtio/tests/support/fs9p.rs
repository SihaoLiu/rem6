use rem6_virtio::{
    Virtio9pRequest, VirtioQueueIndex, VirtioSplitDescriptor, VirtioSplitDescriptorChain,
};

fn queue(index: u16) -> VirtioQueueIndex {
    VirtioQueueIndex::new(index).unwrap()
}

pub(crate) fn p9_string(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend((bytes.len() as u16).to_le_bytes());
    output.extend_from_slice(bytes);
    output
}

fn p9_message(message_type: u8, tag: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend((7_u32 + payload.len() as u32).to_le_bytes());
    bytes.push(message_type);
    bytes.extend(tag.to_le_bytes());
    bytes.extend(payload);
    bytes
}

pub(crate) fn decoded_request(message_type: u8, tag: u16, payload: Vec<u8>) -> Virtio9pRequest {
    let chain = VirtioSplitDescriptorChain::new(
        3,
        [
            VirtioSplitDescriptor::device_readable(
                3,
                p9_message(message_type, tag, &payload),
                Some(4),
            ),
            VirtioSplitDescriptor::device_writable(4, 128, None),
        ],
    )
    .unwrap();
    chain.decode_9p_request(queue(0)).unwrap().into_request()
}

pub(crate) fn p9_version_payload(msize: u32, version: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(msize.to_le_bytes());
    payload.extend(p9_string(version));
    payload
}

pub(crate) fn p9_attach_payload(
    fid: u32,
    afid: u32,
    uname: &[u8],
    aname: &[u8],
    n_uname: u32,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(afid.to_le_bytes());
    payload.extend(p9_string(uname));
    payload.extend(p9_string(aname));
    payload.extend(n_uname.to_le_bytes());
    payload
}

pub(crate) fn p9_walk_payload(fid: u32, newfid: u32, names: &[&[u8]]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(newfid.to_le_bytes());
    payload.extend((names.len() as u16).to_le_bytes());
    for name in names {
        payload.extend(p9_string(name));
    }
    payload
}

pub(crate) fn p9_lopen_payload(fid: u32, flags: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(flags.to_le_bytes());
    payload
}

pub(crate) fn p9_getattr_payload(fid: u32, request_mask: u64) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(request_mask.to_le_bytes());
    payload
}

pub(crate) fn p9_setattr_payload(fid: u32, valid: u32, size: u64) -> Vec<u8> {
    p9_setattr_full_payload(
        fid,
        valid,
        P9SetattrPayload {
            size,
            ..P9SetattrPayload::default()
        },
    )
}

pub(crate) fn p9_setattr_metadata_payload(
    fid: u32,
    valid: u32,
    mode: u32,
    uid: u32,
    gid: u32,
    size: u64,
) -> Vec<u8> {
    p9_setattr_full_payload(
        fid,
        valid,
        P9SetattrPayload {
            mode,
            uid,
            gid,
            size,
            ..P9SetattrPayload::default()
        },
    )
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct P9SetattrPayload {
    pub(crate) mode: u32,
    pub(crate) uid: u32,
    pub(crate) gid: u32,
    pub(crate) size: u64,
    pub(crate) atime_sec: u64,
    pub(crate) atime_nsec: u64,
    pub(crate) mtime_sec: u64,
    pub(crate) mtime_nsec: u64,
}

pub(crate) fn p9_setattr_full_payload(fid: u32, valid: u32, fields: P9SetattrPayload) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(valid.to_le_bytes());
    payload.extend(fields.mode.to_le_bytes());
    payload.extend(fields.uid.to_le_bytes());
    payload.extend(fields.gid.to_le_bytes());
    payload.extend(fields.size.to_le_bytes());
    payload.extend(fields.atime_sec.to_le_bytes());
    payload.extend(fields.atime_nsec.to_le_bytes());
    payload.extend(fields.mtime_sec.to_le_bytes());
    payload.extend(fields.mtime_nsec.to_le_bytes());
    payload
}

pub(crate) fn p9_statfs_payload(fid: u32) -> Vec<u8> {
    fid.to_le_bytes().to_vec()
}

pub(crate) fn p9_symlink_payload(dfid: u32, name: &[u8], target: &[u8], gid: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(dfid.to_le_bytes());
    payload.extend(p9_string(name));
    payload.extend(p9_string(target));
    payload.extend(gid.to_le_bytes());
    payload
}

pub(crate) fn p9_readlink_payload(fid: u32) -> Vec<u8> {
    fid.to_le_bytes().to_vec()
}

pub(crate) fn p9_fsync_payload(fid: u32, datasync: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(datasync.to_le_bytes());
    payload
}

pub(crate) fn p9_lock_payload(
    fid: u32,
    lock_type: u8,
    flags: u32,
    start: u64,
    length: u64,
    proc_id: u32,
    client_id: &[u8],
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.push(lock_type);
    payload.extend(flags.to_le_bytes());
    payload.extend(start.to_le_bytes());
    payload.extend(length.to_le_bytes());
    payload.extend(proc_id.to_le_bytes());
    payload.extend(p9_string(client_id));
    payload
}

pub(crate) fn p9_flush_payload(oldtag: u16) -> Vec<u8> {
    oldtag.to_le_bytes().to_vec()
}

pub(crate) fn p9_lcreate_payload(
    fid: u32,
    name: &[u8],
    flags: u32,
    mode: u32,
    gid: u32,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(p9_string(name));
    payload.extend(flags.to_le_bytes());
    payload.extend(mode.to_le_bytes());
    payload.extend(gid.to_le_bytes());
    payload
}

pub(crate) fn p9_mkdir_payload(dfid: u32, name: &[u8], mode: u32, gid: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(dfid.to_le_bytes());
    payload.extend(p9_string(name));
    payload.extend(mode.to_le_bytes());
    payload.extend(gid.to_le_bytes());
    payload
}

pub(crate) fn p9_mknod_payload(
    dfid: u32,
    name: &[u8],
    mode: u32,
    major: u32,
    minor: u32,
    gid: u32,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(dfid.to_le_bytes());
    payload.extend(p9_string(name));
    payload.extend(mode.to_le_bytes());
    payload.extend(major.to_le_bytes());
    payload.extend(minor.to_le_bytes());
    payload.extend(gid.to_le_bytes());
    payload
}

pub(crate) fn p9_read_payload(fid: u32, offset: u64, count: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(offset.to_le_bytes());
    payload.extend(count.to_le_bytes());
    payload
}

pub(crate) fn p9_readdir_payload(fid: u32, offset: u64, count: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(offset.to_le_bytes());
    payload.extend(count.to_le_bytes());
    payload
}

pub(crate) fn p9_write_payload(fid: u32, offset: u64, data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(offset.to_le_bytes());
    payload.extend((data.len() as u32).to_le_bytes());
    payload.extend(data);
    payload
}

pub(crate) fn p9_clunk_payload(fid: u32) -> Vec<u8> {
    fid.to_le_bytes().to_vec()
}

pub(crate) fn p9_remove_payload(fid: u32) -> Vec<u8> {
    fid.to_le_bytes().to_vec()
}

pub(crate) fn p9_unlinkat_payload(dirfid: u32, name: &[u8], flags: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(dirfid.to_le_bytes());
    payload.extend(p9_string(name));
    payload.extend(flags.to_le_bytes());
    payload
}

pub(crate) fn p9_renameat_payload(
    olddirfid: u32,
    oldname: &[u8],
    newdirfid: u32,
    newname: &[u8],
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(olddirfid.to_le_bytes());
    payload.extend(p9_string(oldname));
    payload.extend(newdirfid.to_le_bytes());
    payload.extend(p9_string(newname));
    payload
}

pub(crate) fn p9_rename_payload(fid: u32, newdirfid: u32, name: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(newdirfid.to_le_bytes());
    payload.extend(p9_string(name));
    payload
}

pub(crate) fn p9_link_payload(dfid: u32, oldfid: u32, newname: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(dfid.to_le_bytes());
    payload.extend(oldfid.to_le_bytes());
    payload.extend(p9_string(newname));
    payload
}

pub(crate) fn read_qid(payload: &[u8], offset: usize) -> (u8, u32, u64) {
    let qtype = payload[offset];
    let version = u32::from_le_bytes(payload[offset + 1..offset + 5].try_into().unwrap());
    let path = u64::from_le_bytes(payload[offset + 5..offset + 13].try_into().unwrap());
    (qtype, version, path)
}

pub(crate) fn read_counted_data(payload: &[u8]) -> &[u8] {
    let count = u32::from_le_bytes(payload[0..4].try_into().unwrap()) as usize;
    &payload[4..4 + count]
}

pub(crate) fn read_u32(payload: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(payload[offset..offset + 4].try_into().unwrap())
}

pub(crate) fn read_u64(payload: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(payload[offset..offset + 8].try_into().unwrap())
}

pub(crate) fn read_string(payload: &[u8], offset: usize) -> &[u8] {
    let count = usize::from(u16::from_le_bytes(
        payload[offset..offset + 2].try_into().unwrap(),
    ));
    &payload[offset + 2..offset + 2 + count]
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ReadDirEntry {
    pub(crate) qtype: u8,
    pub(crate) qpath: u64,
    pub(crate) next_offset: u64,
    pub(crate) dtype: u8,
    pub(crate) name: String,
}

pub(crate) fn read_dir_entries(payload: &[u8]) -> Vec<ReadDirEntry> {
    let data = read_counted_data(payload);
    let mut cursor = 0;
    let mut entries = Vec::new();
    while cursor < data.len() {
        let (qtype, _, qpath) = read_qid(data, cursor);
        cursor += 13;
        let next_offset = read_u64(data, cursor);
        cursor += 8;
        let dtype = data[cursor];
        cursor += 1;
        let name_len = usize::from(u16::from_le_bytes(
            data[cursor..cursor + 2].try_into().unwrap(),
        ));
        cursor += 2;
        let name = std::str::from_utf8(&data[cursor..cursor + name_len])
            .unwrap()
            .to_string();
        cursor += name_len;
        entries.push(ReadDirEntry {
            qtype,
            qpath,
            next_offset,
            dtype,
            name,
        });
    }
    entries
}
