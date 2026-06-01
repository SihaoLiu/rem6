use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_EBADF, VIRTIO_9P_ENOTSUP, VIRTIO_9P_GETATTR_BASIC,
    VIRTIO_9P_NAME_MAX, VIRTIO_9P_NOFID, VIRTIO_9P_QTDIR, VIRTIO_9P_QTFILE, VIRTIO_9P_RGETATTR,
    VIRTIO_9P_RLERROR, VIRTIO_9P_RREAD, VIRTIO_9P_RSETATTR, VIRTIO_9P_RSTATFS,
    VIRTIO_9P_SETATTR_ATIME, VIRTIO_9P_SETATTR_ATIME_SET, VIRTIO_9P_SETATTR_GID,
    VIRTIO_9P_SETATTR_MODE, VIRTIO_9P_SETATTR_MTIME, VIRTIO_9P_SETATTR_MTIME_SET,
    VIRTIO_9P_SETATTR_SIZE, VIRTIO_9P_SETATTR_UID, VIRTIO_9P_STATFS_BLOCK_SIZE,
    VIRTIO_9P_STATFS_TYPE, VIRTIO_9P_TATTACH, VIRTIO_9P_TGETATTR, VIRTIO_9P_TLOPEN,
    VIRTIO_9P_TREAD, VIRTIO_9P_TSETATTR, VIRTIO_9P_TSTATFS, VIRTIO_9P_TWALK,
};

mod support;

use support::fs9p::*;

const P9_SETATTR_CTIME: u32 = 0x0000_0040;

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
fn virtio_9p_device_setattr_resizes_file_data_and_metadata() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"abcdef".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    device.execute_at(11, walk).unwrap();
    let open = decoded_request(VIRTIO_9P_TLOPEN, 3, p9_lopen_payload(2, 0));
    device.execute_at(12, open).unwrap();

    let shrink = decoded_request(
        VIRTIO_9P_TSETATTR,
        4,
        p9_setattr_payload(2, VIRTIO_9P_SETATTR_SIZE, 3),
    );
    let shrink_completion = device.execute_at(13, shrink).unwrap();
    assert_eq!(shrink_completion.message_type(), VIRTIO_9P_RSETATTR);
    assert!(shrink_completion.payload().is_empty());
    let read_shrunk = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 16));
    let shrunk_completion = device.execute_at(14, read_shrunk).unwrap();
    assert_eq!(shrunk_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(shrunk_completion.payload()), b"abc");

    let grow = decoded_request(
        VIRTIO_9P_TSETATTR,
        6,
        p9_setattr_payload(2, VIRTIO_9P_SETATTR_SIZE, 8),
    );
    let grow_completion = device.execute_at(15, grow).unwrap();
    assert_eq!(grow_completion.message_type(), VIRTIO_9P_RSETATTR);
    assert!(grow_completion.payload().is_empty());
    let read_grown = decoded_request(VIRTIO_9P_TREAD, 7, p9_read_payload(2, 0, 16));
    let grown_completion = device.execute_at(16, read_grown).unwrap();
    assert_eq!(grown_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(
        read_counted_data(grown_completion.payload()),
        &[b'a', b'b', b'c', 0, 0, 0, 0, 0]
    );

    let getattr = decoded_request(
        VIRTIO_9P_TGETATTR,
        8,
        p9_getattr_payload(2, VIRTIO_9P_GETATTR_BASIC),
    );
    let getattr_completion = device.execute_at(17, getattr).unwrap();
    assert_eq!(getattr_completion.message_type(), VIRTIO_9P_RGETATTR);
    assert_eq!(read_u64(getattr_completion.payload(), 49), 8);
}

#[test]
fn virtio_9p_device_rejects_setattr_size_on_stale_and_directory_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let stale = decoded_request(
        VIRTIO_9P_TSETATTR,
        1,
        p9_setattr_payload(7, VIRTIO_9P_SETATTR_SIZE, 4),
    );
    let stale_completion = device.execute_at(10, stale).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        2,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(11, attach).unwrap();
    let directory = decoded_request(
        VIRTIO_9P_TSETATTR,
        3,
        p9_setattr_payload(1, VIRTIO_9P_SETATTR_SIZE, 4),
    );
    let directory_completion = device.execute_at(12, directory).unwrap();
    assert_eq!(directory_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        directory_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
}

#[test]
fn virtio_9p_device_setattr_updates_mode_uid_and_gid_metadata() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"abcdef".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    device.execute_at(11, walk).unwrap();

    let valid = VIRTIO_9P_SETATTR_MODE | VIRTIO_9P_SETATTR_UID | VIRTIO_9P_SETATTR_GID;
    let setattr = decoded_request(
        VIRTIO_9P_TSETATTR,
        3,
        p9_setattr_metadata_payload(2, valid, 0o100600, 1000, 1001, 0),
    );
    let setattr_completion = device.execute_at(12, setattr).unwrap();
    assert_eq!(setattr_completion.message_type(), VIRTIO_9P_RSETATTR);
    assert!(setattr_completion.payload().is_empty());

    let getattr = decoded_request(
        VIRTIO_9P_TGETATTR,
        4,
        p9_getattr_payload(2, VIRTIO_9P_GETATTR_BASIC),
    );
    let getattr_completion = device.execute_at(13, getattr).unwrap();
    assert_eq!(getattr_completion.message_type(), VIRTIO_9P_RGETATTR);
    assert_eq!(read_u32(getattr_completion.payload(), 21), 0o100600);
    assert_eq!(read_u32(getattr_completion.payload(), 25), 1000);
    assert_eq!(read_u32(getattr_completion.payload(), 29), 1001);
    assert_eq!(read_u64(getattr_completion.payload(), 49), 6);
}

#[test]
fn virtio_9p_device_setattr_updates_explicit_timestamps() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"abcdef".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    device.execute_at(11, walk).unwrap();

    let valid = VIRTIO_9P_SETATTR_ATIME
        | VIRTIO_9P_SETATTR_ATIME_SET
        | VIRTIO_9P_SETATTR_MTIME
        | VIRTIO_9P_SETATTR_MTIME_SET;
    let setattr = decoded_request(
        VIRTIO_9P_TSETATTR,
        3,
        p9_setattr_full_payload(
            2,
            valid,
            P9SetattrPayload {
                atime_sec: 11,
                atime_nsec: 22,
                mtime_sec: 33,
                mtime_nsec: 44,
                ..P9SetattrPayload::default()
            },
        ),
    );
    let setattr_completion = device.execute_at(12, setattr).unwrap();
    assert_eq!(setattr_completion.message_type(), VIRTIO_9P_RSETATTR);
    assert!(setattr_completion.payload().is_empty());

    let getattr = decoded_request(
        VIRTIO_9P_TGETATTR,
        4,
        p9_getattr_payload(2, VIRTIO_9P_GETATTR_BASIC),
    );
    let getattr_completion = device.execute_at(13, getattr).unwrap();
    assert_eq!(getattr_completion.message_type(), VIRTIO_9P_RGETATTR);
    assert_eq!(read_u64(getattr_completion.payload(), 73), 11);
    assert_eq!(read_u64(getattr_completion.payload(), 81), 22);
    assert_eq!(read_u64(getattr_completion.payload(), 89), 33);
    assert_eq!(read_u64(getattr_completion.payload(), 97), 44);
}

#[test]
fn virtio_9p_device_rejects_unsupported_setattr_fields() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"abcdef".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    device.execute_at(11, walk).unwrap();

    let setattr = decoded_request(
        VIRTIO_9P_TSETATTR,
        3,
        p9_setattr_payload(2, P9_SETATTR_CTIME, 0),
    );
    let completion = device.execute_at(12, setattr).unwrap();
    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.payload(), VIRTIO_9P_ENOTSUP.to_le_bytes());
}
