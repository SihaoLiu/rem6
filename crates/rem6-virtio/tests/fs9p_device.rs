use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, Virtio9pRequest, VirtioError, VirtioQueueIndex,
    VirtioSplitDescriptor, VirtioSplitDescriptorChain, VIRTIO_9P_DEFAULT_MSIZE, VIRTIO_9P_DTDIR,
    VIRTIO_9P_DTREG, VIRTIO_9P_EBADF, VIRTIO_9P_ENOENT, VIRTIO_9P_ENOTSUP, VIRTIO_9P_GETATTR_BASIC,
    VIRTIO_9P_NAME_MAX, VIRTIO_9P_NOFID, VIRTIO_9P_PROTOCOL_VERSION, VIRTIO_9P_QTDIR,
    VIRTIO_9P_QTFILE, VIRTIO_9P_RATTACH, VIRTIO_9P_RCLUNK, VIRTIO_9P_RGETATTR, VIRTIO_9P_RLCREATE,
    VIRTIO_9P_RLERROR, VIRTIO_9P_RLOPEN, VIRTIO_9P_RREAD, VIRTIO_9P_RREADDIR, VIRTIO_9P_RREMOVE,
    VIRTIO_9P_RSTATFS, VIRTIO_9P_RUNLINKAT, VIRTIO_9P_RVERSION, VIRTIO_9P_RWALK, VIRTIO_9P_RWRITE,
    VIRTIO_9P_STATFS_BLOCK_SIZE, VIRTIO_9P_STATFS_TYPE, VIRTIO_9P_TATTACH, VIRTIO_9P_TCLUNK,
    VIRTIO_9P_TGETATTR, VIRTIO_9P_TLCREATE, VIRTIO_9P_TLOPEN, VIRTIO_9P_TREAD, VIRTIO_9P_TREADDIR,
    VIRTIO_9P_TREMOVE, VIRTIO_9P_TSTATFS, VIRTIO_9P_TUNLINKAT, VIRTIO_9P_TVERSION, VIRTIO_9P_TWALK,
    VIRTIO_9P_TWRITE,
};

fn queue(index: u16) -> VirtioQueueIndex {
    VirtioQueueIndex::new(index).unwrap()
}

fn p9_string(bytes: &[u8]) -> Vec<u8> {
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

fn decoded_request(message_type: u8, tag: u16, payload: Vec<u8>) -> Virtio9pRequest {
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

fn p9_version_payload(msize: u32, version: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(msize.to_le_bytes());
    payload.extend(p9_string(version));
    payload
}

fn p9_attach_payload(fid: u32, afid: u32, uname: &[u8], aname: &[u8], n_uname: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(afid.to_le_bytes());
    payload.extend(p9_string(uname));
    payload.extend(p9_string(aname));
    payload.extend(n_uname.to_le_bytes());
    payload
}

fn p9_walk_payload(fid: u32, newfid: u32, names: &[&[u8]]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(newfid.to_le_bytes());
    payload.extend((names.len() as u16).to_le_bytes());
    for name in names {
        payload.extend(p9_string(name));
    }
    payload
}

fn p9_lopen_payload(fid: u32, flags: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(flags.to_le_bytes());
    payload
}

fn p9_getattr_payload(fid: u32, request_mask: u64) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(request_mask.to_le_bytes());
    payload
}

fn p9_statfs_payload(fid: u32) -> Vec<u8> {
    fid.to_le_bytes().to_vec()
}

fn p9_lcreate_payload(fid: u32, name: &[u8], flags: u32, mode: u32, gid: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(p9_string(name));
    payload.extend(flags.to_le_bytes());
    payload.extend(mode.to_le_bytes());
    payload.extend(gid.to_le_bytes());
    payload
}

fn p9_read_payload(fid: u32, offset: u64, count: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(offset.to_le_bytes());
    payload.extend(count.to_le_bytes());
    payload
}

fn p9_readdir_payload(fid: u32, offset: u64, count: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(offset.to_le_bytes());
    payload.extend(count.to_le_bytes());
    payload
}

fn p9_write_payload(fid: u32, offset: u64, data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(offset.to_le_bytes());
    payload.extend((data.len() as u32).to_le_bytes());
    payload.extend(data);
    payload
}

fn p9_clunk_payload(fid: u32) -> Vec<u8> {
    fid.to_le_bytes().to_vec()
}

fn p9_remove_payload(fid: u32) -> Vec<u8> {
    fid.to_le_bytes().to_vec()
}

fn p9_unlinkat_payload(dirfid: u32, name: &[u8], flags: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(dirfid.to_le_bytes());
    payload.extend(p9_string(name));
    payload.extend(flags.to_le_bytes());
    payload
}

fn read_qid(payload: &[u8], offset: usize) -> (u8, u32, u64) {
    let qtype = payload[offset];
    let version = u32::from_le_bytes(payload[offset + 1..offset + 5].try_into().unwrap());
    let path = u64::from_le_bytes(payload[offset + 5..offset + 13].try_into().unwrap());
    (qtype, version, path)
}

fn read_counted_data(payload: &[u8]) -> &[u8] {
    let count = u32::from_le_bytes(payload[0..4].try_into().unwrap()) as usize;
    &payload[4..4 + count]
}

fn read_u32(payload: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(payload[offset..offset + 4].try_into().unwrap())
}

fn read_u64(payload: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(payload[offset..offset + 8].try_into().unwrap())
}

#[derive(Debug, Eq, PartialEq)]
struct ReadDirEntry {
    qtype: u8,
    qpath: u64,
    next_offset: u64,
    dtype: u8,
    name: String,
}

fn read_dir_entries(payload: &[u8]) -> Vec<ReadDirEntry> {
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

#[test]
fn virtio_9p_device_negotiates_version_and_records_completion() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let request = decoded_request(
        VIRTIO_9P_TVERSION,
        11,
        p9_version_payload(16384, VIRTIO_9P_PROTOCOL_VERSION),
    );

    let completion = device.execute_at(44, request).unwrap();

    assert_eq!(completion.tick(), 44);
    assert_eq!(completion.message_type(), VIRTIO_9P_RVERSION);
    assert_eq!(completion.tag(), 11);
    assert_eq!(
        completion.payload(),
        p9_version_payload(VIRTIO_9P_DEFAULT_MSIZE, VIRTIO_9P_PROTOCOL_VERSION)
    );
    assert_eq!(device.completions(), vec![completion]);
}

#[test]
fn virtio_9p_device_accepts_attach_and_returns_root_qid() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let request = decoded_request(
        VIRTIO_9P_TATTACH,
        21,
        p9_attach_payload(42, VIRTIO_9P_NOFID, b"root", b"", 0),
    );

    let completion = device.execute_at(55, request).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RATTACH);
    assert_eq!(completion.tag(), 21);
    assert_eq!(completion.payload().len(), 13);
    assert_eq!(completion.payload()[0], VIRTIO_9P_QTDIR);
    assert_eq!(completion.payload()[1..5], 0_u32.to_le_bytes());
    assert_eq!(completion.payload()[5..13], 1_u64.to_le_bytes());

    let attachments = device.attached_fids();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].fid(), 42);
    assert_eq!(attachments[0].afid(), VIRTIO_9P_NOFID);
    assert_eq!(attachments[0].uname(), "root");
    assert_eq!(attachments[0].aname(), "");
    assert_eq!(attachments[0].n_uname(), 0);
}

#[test]
fn virtio_9p_device_walks_opens_reads_and_clunks_in_memory_files() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("hello.txt", b"hello rem6".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"hello.txt"]));
    let walk_completion = device.execute_at(11, walk).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(walk_completion.payload()[0..2], 1_u16.to_le_bytes());
    let (walk_qtype, walk_version, walk_path) = read_qid(walk_completion.payload(), 2);
    assert_eq!(walk_qtype, VIRTIO_9P_QTFILE);
    assert_eq!(walk_version, 0);
    assert_ne!(walk_path, 1);

    let open = decoded_request(VIRTIO_9P_TLOPEN, 3, p9_lopen_payload(2, 0));
    let open_completion = device.execute_at(12, open).unwrap();
    assert_eq!(open_completion.message_type(), VIRTIO_9P_RLOPEN);
    let (open_qtype, open_version, open_path) = read_qid(open_completion.payload(), 0);
    assert_eq!(open_qtype, VIRTIO_9P_QTFILE);
    assert_eq!(open_version, 0);
    assert_eq!(open_path, walk_path);
    assert_eq!(
        open_completion.payload()[13..17],
        VIRTIO_9P_DEFAULT_MSIZE.to_le_bytes()
    );

    let read = decoded_request(VIRTIO_9P_TREAD, 4, p9_read_payload(2, 6, 16));
    let read_completion = device.execute_at(13, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"rem6");

    let clunk = decoded_request(VIRTIO_9P_TCLUNK, 5, p9_clunk_payload(2));
    let clunk_completion = device.execute_at(14, clunk).unwrap();
    assert_eq!(clunk_completion.message_type(), VIRTIO_9P_RCLUNK);
    assert!(clunk_completion.payload().is_empty());

    let read_after_clunk = decoded_request(VIRTIO_9P_TREAD, 6, p9_read_payload(2, 0, 4));
    let error_completion = device.execute_at(15, read_after_clunk).unwrap();
    assert_eq!(error_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(error_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_reports_lerror_for_missing_walk_targets() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"missing"]));
    let completion = device.execute_at(11, walk).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
    assert_eq!(device.fid_count(), 1);
}

#[test]
fn virtio_9p_device_reports_getattr_for_root_and_files() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("hello.txt", b"hello rem6".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let getattr_root = decoded_request(
        VIRTIO_9P_TGETATTR,
        2,
        p9_getattr_payload(1, VIRTIO_9P_GETATTR_BASIC),
    );
    let root_completion = device.execute_at(11, getattr_root).unwrap();
    assert_eq!(root_completion.message_type(), VIRTIO_9P_RGETATTR);
    assert_eq!(root_completion.payload().len(), 153);
    assert_eq!(
        read_u64(root_completion.payload(), 0),
        VIRTIO_9P_GETATTR_BASIC
    );
    let (root_qtype, root_version, root_path) = read_qid(root_completion.payload(), 8);
    assert_eq!(root_qtype, VIRTIO_9P_QTDIR);
    assert_eq!(root_version, 0);
    assert_eq!(root_path, 1);
    assert_eq!(read_u32(root_completion.payload(), 21), 0o040755);
    assert_eq!(read_u32(root_completion.payload(), 25), 0);
    assert_eq!(read_u32(root_completion.payload(), 29), 0);
    assert_eq!(read_u64(root_completion.payload(), 33), 3);
    assert_eq!(read_u64(root_completion.payload(), 49), 0);
    assert_eq!(
        read_u64(root_completion.payload(), 57),
        u64::from(VIRTIO_9P_STATFS_BLOCK_SIZE)
    );
    assert_eq!(read_u64(root_completion.payload(), 65), 0);

    let walk = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"hello.txt"]));
    let walk_completion = device.execute_at(12, walk).unwrap();
    let (_, _, file_path) = read_qid(walk_completion.payload(), 2);

    let getattr_file = decoded_request(
        VIRTIO_9P_TGETATTR,
        4,
        p9_getattr_payload(2, VIRTIO_9P_GETATTR_BASIC),
    );
    let file_completion = device.execute_at(13, getattr_file).unwrap();
    assert_eq!(file_completion.message_type(), VIRTIO_9P_RGETATTR);
    assert_eq!(
        read_u64(file_completion.payload(), 0),
        VIRTIO_9P_GETATTR_BASIC
    );
    let (file_qtype, file_version, getattr_file_path) = read_qid(file_completion.payload(), 8);
    assert_eq!(file_qtype, VIRTIO_9P_QTFILE);
    assert_eq!(file_version, 0);
    assert_eq!(getattr_file_path, file_path);
    assert_eq!(read_u32(file_completion.payload(), 21), 0o100644);
    assert_eq!(read_u64(file_completion.payload(), 33), 1);
    assert_eq!(read_u64(file_completion.payload(), 49), 10);
    assert_eq!(read_u64(file_completion.payload(), 65), 1);
}

#[test]
fn virtio_9p_device_reports_statfs_for_attached_namespace() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("hello.txt", b"hello".to_vec())
        .unwrap()
        .with_file("note.txt", b"note".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let statfs = decoded_request(VIRTIO_9P_TSTATFS, 2, p9_statfs_payload(1));
    let completion = device.execute_at(11, statfs).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RSTATFS);
    assert_eq!(completion.payload().len(), 60);
    assert_eq!(read_u32(completion.payload(), 0), VIRTIO_9P_STATFS_TYPE);
    assert_eq!(
        read_u32(completion.payload(), 4),
        VIRTIO_9P_STATFS_BLOCK_SIZE
    );
    assert_ne!(read_u64(completion.payload(), 8), 0);
    assert_eq!(read_u64(completion.payload(), 32), 3);
    assert_eq!(read_u32(completion.payload(), 56), VIRTIO_9P_NAME_MAX);
}

#[test]
fn virtio_9p_device_rejects_metadata_queries_on_stale_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());

    let getattr = decoded_request(
        VIRTIO_9P_TGETATTR,
        1,
        p9_getattr_payload(7, VIRTIO_9P_GETATTR_BASIC),
    );
    let getattr_completion = device.execute_at(10, getattr).unwrap();
    assert_eq!(getattr_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(getattr_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let statfs = decoded_request(VIRTIO_9P_TSTATFS, 2, p9_statfs_payload(7));
    let statfs_completion = device.execute_at(11, statfs).unwrap();
    assert_eq!(statfs_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(statfs_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_opens_and_reads_root_directory_entries() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("beta.txt", b"beta".to_vec())
        .unwrap()
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let open_root = decoded_request(VIRTIO_9P_TLOPEN, 2, p9_lopen_payload(1, 0));
    let open_completion = device.execute_at(11, open_root).unwrap();
    assert_eq!(open_completion.message_type(), VIRTIO_9P_RLOPEN);
    let (open_qtype, _, open_path) = read_qid(open_completion.payload(), 0);
    assert_eq!(open_qtype, VIRTIO_9P_QTDIR);
    assert_eq!(open_path, 1);
    assert_eq!(
        open_completion.payload()[13..17],
        VIRTIO_9P_DEFAULT_MSIZE.to_le_bytes()
    );

    let readdir = decoded_request(VIRTIO_9P_TREADDIR, 3, p9_readdir_payload(1, 0, 512));
    let completion = device.execute_at(12, readdir).unwrap();
    assert_eq!(completion.message_type(), VIRTIO_9P_RREADDIR);
    let entries = read_dir_entries(completion.payload());
    let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, [".", "..", "alpha.txt", "beta.txt"]);
    assert_eq!(entries[0].qtype, VIRTIO_9P_QTDIR);
    assert_eq!(entries[0].qpath, 1);
    assert_eq!(entries[0].dtype, VIRTIO_9P_DTDIR);
    assert_eq!(entries[1].qtype, VIRTIO_9P_QTDIR);
    assert_eq!(entries[1].qpath, 1);
    assert_eq!(entries[1].dtype, VIRTIO_9P_DTDIR);
    assert_eq!(entries[2].qtype, VIRTIO_9P_QTFILE);
    assert_eq!(entries[2].dtype, VIRTIO_9P_DTREG);
    assert_eq!(entries[3].qtype, VIRTIO_9P_QTFILE);
    assert_eq!(entries[3].dtype, VIRTIO_9P_DTREG);
    assert!(entries
        .windows(2)
        .all(|pair| pair[0].next_offset < pair[1].next_offset));

    let resume = decoded_request(
        VIRTIO_9P_TREADDIR,
        4,
        p9_readdir_payload(1, entries[0].next_offset, 512),
    );
    let resumed_completion = device.execute_at(13, resume).unwrap();
    let resumed_entries = read_dir_entries(resumed_completion.payload());
    let resumed_names: Vec<_> = resumed_entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(resumed_names, ["..", "alpha.txt", "beta.txt"]);

    let too_small = decoded_request(VIRTIO_9P_TREADDIR, 5, p9_readdir_payload(1, 0, 1));
    let too_small_completion = device.execute_at(14, too_small).unwrap();
    assert_eq!(too_small_completion.message_type(), VIRTIO_9P_RREADDIR);
    assert!(read_counted_data(too_small_completion.payload()).is_empty());
}

#[test]
fn virtio_9p_device_rejects_readdir_on_stale_unopened_and_file_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("hello.txt", b"hello".to_vec())
        .unwrap();

    let stale = decoded_request(VIRTIO_9P_TREADDIR, 1, p9_readdir_payload(7, 0, 128));
    let stale_completion = device.execute_at(10, stale).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        2,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(11, attach).unwrap();

    let unopened = decoded_request(VIRTIO_9P_TREADDIR, 3, p9_readdir_payload(1, 0, 128));
    let unopened_completion = device.execute_at(12, unopened).unwrap();
    assert_eq!(unopened_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(unopened_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let walk = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 2, &[b"hello.txt"]));
    device.execute_at(13, walk).unwrap();
    let open_file = decoded_request(VIRTIO_9P_TLOPEN, 5, p9_lopen_payload(2, 0));
    device.execute_at(14, open_file).unwrap();

    let file = decoded_request(VIRTIO_9P_TREADDIR, 6, p9_readdir_payload(2, 0, 128));
    let file_completion = device.execute_at(15, file).unwrap();
    assert_eq!(file_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(file_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_creates_writes_and_reads_in_memory_files() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let create = decoded_request(
        VIRTIO_9P_TLCREATE,
        2,
        p9_lcreate_payload(1, b"note.txt", 0, 0o100644, 0),
    );
    let create_completion = device.execute_at(11, create).unwrap();
    assert_eq!(create_completion.message_type(), VIRTIO_9P_RLCREATE);
    let (created_qtype, created_version, created_path) = read_qid(create_completion.payload(), 0);
    assert_eq!(created_qtype, VIRTIO_9P_QTFILE);
    assert_eq!(created_version, 0);
    assert_ne!(created_path, 1);
    assert_eq!(
        create_completion.payload()[13..17],
        VIRTIO_9P_DEFAULT_MSIZE.to_le_bytes()
    );

    let write = decoded_request(VIRTIO_9P_TWRITE, 3, p9_write_payload(1, 0, b"hello"));
    let write_completion = device.execute_at(12, write).unwrap();
    assert_eq!(write_completion.message_type(), VIRTIO_9P_RWRITE);
    assert_eq!(write_completion.payload(), 5_u32.to_le_bytes());

    let overwrite = decoded_request(VIRTIO_9P_TWRITE, 4, p9_write_payload(1, 2, b"rem6"));
    let overwrite_completion = device.execute_at(13, overwrite).unwrap();
    assert_eq!(overwrite_completion.message_type(), VIRTIO_9P_RWRITE);
    assert_eq!(overwrite_completion.payload(), 4_u32.to_le_bytes());

    let read = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(1, 0, 16));
    let read_completion = device.execute_at(14, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"herem6");

    let attach_root = decoded_request(
        VIRTIO_9P_TATTACH,
        6,
        p9_attach_payload(10, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(15, attach_root).unwrap();

    let walk = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(10, 2, &[b"note.txt"]));
    let walk_completion = device.execute_at(16, walk).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
    let (_, _, walked_path) = read_qid(walk_completion.payload(), 2);
    assert_eq!(walked_path, created_path);
}

#[test]
fn virtio_9p_device_rejects_create_and_write_on_stale_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let create = decoded_request(
        VIRTIO_9P_TLCREATE,
        1,
        p9_lcreate_payload(7, b"note.txt", 0, 0o100644, 0),
    );
    let create_completion = device.execute_at(10, create).unwrap();
    assert_eq!(create_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(create_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let write = decoded_request(VIRTIO_9P_TWRITE, 2, p9_write_payload(7, 0, b"data"));
    let write_completion = device.execute_at(11, write).unwrap();
    assert_eq!(write_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(write_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_unlinks_named_files_from_root_directory() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("beta.txt", b"beta".to_vec())
        .unwrap()
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let open_root = decoded_request(VIRTIO_9P_TLOPEN, 2, p9_lopen_payload(1, 0));
    device.execute_at(11, open_root).unwrap();

    let initial = decoded_request(VIRTIO_9P_TREADDIR, 3, p9_readdir_payload(1, 0, 512));
    let initial_completion = device.execute_at(12, initial).unwrap();
    let initial_entries = read_dir_entries(initial_completion.payload());
    let initial_names: Vec<_> = initial_entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(initial_names, [".", "..", "alpha.txt", "beta.txt"]);

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    device.execute_at(13, walk_alpha).unwrap();
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 5, p9_lopen_payload(2, 0));
    device.execute_at(14, open_alpha).unwrap();

    let unlink = decoded_request(
        VIRTIO_9P_TUNLINKAT,
        6,
        p9_unlinkat_payload(1, b"alpha.txt", 0),
    );
    let unlink_completion = device.execute_at(15, unlink).unwrap();
    assert_eq!(unlink_completion.message_type(), VIRTIO_9P_RUNLINKAT);
    assert!(unlink_completion.payload().is_empty());

    let read_deleted = decoded_request(VIRTIO_9P_TREAD, 7, p9_read_payload(2, 0, 8));
    let read_completion = device.execute_at(16, read_deleted).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(read_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let after = decoded_request(VIRTIO_9P_TREADDIR, 8, p9_readdir_payload(1, 0, 512));
    let after_completion = device.execute_at(17, after).unwrap();
    let after_entries = read_dir_entries(after_completion.payload());
    let after_names: Vec<_> = after_entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(after_names, [".", "..", "beta.txt"]);

    let walk_removed = decoded_request(VIRTIO_9P_TWALK, 9, p9_walk_payload(1, 3, &[b"alpha.txt"]));
    let removed_completion = device.execute_at(18, walk_removed).unwrap();
    assert_eq!(removed_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(removed_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
}

#[test]
fn virtio_9p_device_removes_file_fids_and_rejects_deleted_access() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("hello.txt", b"hello".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"hello.txt"]));
    device.execute_at(11, walk).unwrap();
    let open = decoded_request(VIRTIO_9P_TLOPEN, 3, p9_lopen_payload(2, 0));
    device.execute_at(12, open).unwrap();

    let remove = decoded_request(VIRTIO_9P_TREMOVE, 4, p9_remove_payload(2));
    let remove_completion = device.execute_at(13, remove).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RREMOVE);
    assert!(remove_completion.payload().is_empty());
    assert_eq!(device.fid_count(), 1);

    let read_after_remove = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 8));
    let read_completion = device.execute_at(14, read_after_remove).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(read_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let walk_removed = decoded_request(VIRTIO_9P_TWALK, 6, p9_walk_payload(1, 3, &[b"hello.txt"]));
    let removed_completion = device.execute_at(15, walk_removed).unwrap();
    assert_eq!(removed_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(removed_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
}

#[test]
fn virtio_9p_device_rejects_remove_and_unlinkat_on_missing_targets() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("hello.txt", b"hello".to_vec())
        .unwrap();

    let remove_stale = decoded_request(VIRTIO_9P_TREMOVE, 1, p9_remove_payload(7));
    let remove_completion = device.execute_at(10, remove_stale).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(remove_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        2,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(11, attach).unwrap();

    let unlink_missing = decoded_request(
        VIRTIO_9P_TUNLINKAT,
        3,
        p9_unlinkat_payload(1, b"missing.txt", 0),
    );
    let missing_completion = device.execute_at(12, unlink_missing).unwrap();
    assert_eq!(missing_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(missing_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());

    let unlink_stale = decoded_request(
        VIRTIO_9P_TUNLINKAT,
        4,
        p9_unlinkat_payload(7, b"hello.txt", 0),
    );
    let stale_completion = device.execute_at(13, unlink_stale).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_returns_lerror_for_unsupported_messages() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let request = decoded_request(112, 31, Vec::new());

    let completion = device.execute_at(66, request).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.tag(), 31);
    assert_eq!(completion.payload(), VIRTIO_9P_ENOTSUP.to_le_bytes());
    assert_eq!(device.completions(), vec![completion]);
}

#[test]
fn virtio_9p_device_rejects_malformed_protocol_payloads_as_typed_errors() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let malformed_version = decoded_request(VIRTIO_9P_TVERSION, 1, vec![1, 2, 3]);

    assert!(matches!(
        device.execute_at(77, malformed_version),
        Err(VirtioError::InvalidVirtio9pPayload {
            message_type: VIRTIO_9P_TVERSION,
            bytes: 3
        })
    ));
    assert!(device.completions().is_empty());

    let malformed_attach = decoded_request(VIRTIO_9P_TATTACH, 2, vec![0; 9]);
    assert!(matches!(
        device.execute_at(78, malformed_attach),
        Err(VirtioError::InvalidVirtio9pPayload {
            message_type: VIRTIO_9P_TATTACH,
            bytes: 9
        })
    ));
}
