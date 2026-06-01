use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VirtioError, VIRTIO_9P_DEFAULT_MSIZE, VIRTIO_9P_EBADF,
    VIRTIO_9P_ENOENT, VIRTIO_9P_ENOTSUP, VIRTIO_9P_GETATTR_BASIC, VIRTIO_9P_NAME_MAX,
    VIRTIO_9P_NOFID, VIRTIO_9P_OPEN_READ_WRITE, VIRTIO_9P_QTDIR, VIRTIO_9P_QTFILE,
    VIRTIO_9P_RATTACH, VIRTIO_9P_RCLUNK, VIRTIO_9P_RGETATTR, VIRTIO_9P_RLERROR, VIRTIO_9P_RLINK,
    VIRTIO_9P_RLOPEN, VIRTIO_9P_RREAD, VIRTIO_9P_RSETATTR, VIRTIO_9P_RSTATFS, VIRTIO_9P_RUNLINKAT,
    VIRTIO_9P_RWALK, VIRTIO_9P_SETATTR_ATIME, VIRTIO_9P_SETATTR_ATIME_SET, VIRTIO_9P_SETATTR_GID,
    VIRTIO_9P_SETATTR_MODE, VIRTIO_9P_SETATTR_MTIME, VIRTIO_9P_SETATTR_MTIME_SET,
    VIRTIO_9P_SETATTR_SIZE, VIRTIO_9P_SETATTR_UID, VIRTIO_9P_STATFS_BLOCK_SIZE,
    VIRTIO_9P_STATFS_TYPE, VIRTIO_9P_TATTACH, VIRTIO_9P_TCLUNK, VIRTIO_9P_TGETATTR,
    VIRTIO_9P_TLINK, VIRTIO_9P_TLOPEN, VIRTIO_9P_TREAD, VIRTIO_9P_TSETATTR, VIRTIO_9P_TSTATFS,
    VIRTIO_9P_TUNLINKAT, VIRTIO_9P_TWALK, VIRTIO_9P_TWRITE,
};

mod support;

use support::fs9p::*;

const P9_SETATTR_CTIME: u32 = 0x0000_0040;

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
fn virtio_9p_device_rejects_attach_on_occupied_fid_without_replacing_it() {
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
    device.execute_at(11, walk).unwrap();
    let open = decoded_request(VIRTIO_9P_TLOPEN, 3, p9_lopen_payload(2, 0));
    device.execute_at(12, open).unwrap();

    let duplicate_attach = decoded_request(
        VIRTIO_9P_TATTACH,
        4,
        p9_attach_payload(2, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    let duplicate_completion = device.execute_at(13, duplicate_attach).unwrap();
    assert_eq!(duplicate_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        duplicate_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
    assert_eq!(device.attached_fids().len(), 1);
    assert_eq!(device.fid_count(), 2);

    let read = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 5));
    let read_completion = device.execute_at(14, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"hello");
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
#[test]
fn virtio_9p_device_preserves_linked_file_identity() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
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

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let alpha_completion = device.execute_at(12, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(alpha_completion.payload(), 2);
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 4, p9_lopen_payload(2, 0));
    device.execute_at(13, open_alpha).unwrap();

    let link = decoded_request(VIRTIO_9P_TLINK, 5, p9_link_payload(1, 2, b"beta.txt"));
    let link_completion = device.execute_at(14, link).unwrap();
    assert_eq!(link_completion.message_type(), VIRTIO_9P_RLINK);
    assert!(link_completion.payload().is_empty());

    let walk_beta = decoded_request(VIRTIO_9P_TWALK, 6, p9_walk_payload(1, 3, &[b"beta.txt"]));
    let beta_completion = device.execute_at(15, walk_beta).unwrap();
    assert_eq!(beta_completion.message_type(), VIRTIO_9P_RWALK);
    let (_, _, beta_path) = read_qid(beta_completion.payload(), 2);
    assert_eq!(beta_path, alpha_path);
    let open_beta = decoded_request(
        VIRTIO_9P_TLOPEN,
        7,
        p9_lopen_payload(3, u32::from(VIRTIO_9P_OPEN_READ_WRITE)),
    );
    device.execute_at(16, open_beta).unwrap();

    let getattr = decoded_request(
        VIRTIO_9P_TGETATTR,
        8,
        p9_getattr_payload(2, VIRTIO_9P_GETATTR_BASIC),
    );
    let getattr_completion = device.execute_at(17, getattr).unwrap();
    assert_eq!(read_u64(getattr_completion.payload(), 33), 2);

    let write_beta = decoded_request(VIRTIO_9P_TWRITE, 9, p9_write_payload(3, 5, b" linked"));
    device.execute_at(18, write_beta).unwrap();
    let read_alpha = decoded_request(VIRTIO_9P_TREAD, 10, p9_read_payload(2, 0, 32));
    let read_completion = device.execute_at(19, read_alpha).unwrap();
    assert_eq!(
        read_counted_data(read_completion.payload()),
        b"alpha linked"
    );

    let unlink_alpha = decoded_request(
        VIRTIO_9P_TUNLINKAT,
        11,
        p9_unlinkat_payload(1, b"alpha.txt", 0),
    );
    let unlink_completion = device.execute_at(20, unlink_alpha).unwrap();
    assert_eq!(unlink_completion.message_type(), VIRTIO_9P_RUNLINKAT);

    let read_after_unlink = decoded_request(VIRTIO_9P_TREAD, 12, p9_read_payload(2, 0, 32));
    let old_fid_completion = device.execute_at(21, read_after_unlink).unwrap();
    assert_eq!(
        read_counted_data(old_fid_completion.payload()),
        b"alpha linked"
    );

    let old_walk = decoded_request(VIRTIO_9P_TWALK, 13, p9_walk_payload(1, 4, &[b"alpha.txt"]));
    let old_completion = device.execute_at(22, old_walk).unwrap();
    assert_eq!(old_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(old_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());

    let beta_read = decoded_request(VIRTIO_9P_TREAD, 14, p9_read_payload(3, 0, 32));
    let beta_completion = device.execute_at(23, beta_read).unwrap();
    assert_eq!(
        read_counted_data(beta_completion.payload()),
        b"alpha linked"
    );
}
#[test]
fn virtio_9p_device_returns_lerror_for_unsupported_messages() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let request = decoded_request(200, 31, Vec::new());

    let completion = device.execute_at(66, request).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.tag(), 31);
    assert_eq!(completion.payload(), VIRTIO_9P_ENOTSUP.to_le_bytes());
    assert_eq!(device.completions(), vec![completion]);
}

#[test]
fn virtio_9p_device_rejects_malformed_attach_payloads_as_typed_errors() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let malformed_attach = decoded_request(VIRTIO_9P_TATTACH, 2, vec![0; 9]);

    assert!(matches!(
        device.execute_at(78, malformed_attach),
        Err(VirtioError::InvalidVirtio9pPayload {
            message_type: VIRTIO_9P_TATTACH,
            bytes: 9
        })
    ));
}
