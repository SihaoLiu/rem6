use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VirtioError, VIRTIO_9P_AT_REMOVEDIR, VIRTIO_9P_DEFAULT_MSIZE,
    VIRTIO_9P_DTCHR, VIRTIO_9P_DTDIR, VIRTIO_9P_DTREG, VIRTIO_9P_DTSYMLINK, VIRTIO_9P_EBADF,
    VIRTIO_9P_EEXIST, VIRTIO_9P_ENODATA, VIRTIO_9P_ENOENT, VIRTIO_9P_ENOTEMPTY, VIRTIO_9P_ENOTSUP,
    VIRTIO_9P_GETATTR_BASIC, VIRTIO_9P_LOCK_SUCCESS, VIRTIO_9P_LOCK_TYPE_UNLCK,
    VIRTIO_9P_LOCK_TYPE_WRLCK, VIRTIO_9P_NAME_MAX, VIRTIO_9P_NOFID, VIRTIO_9P_OPEN_READ_ONLY,
    VIRTIO_9P_OPEN_READ_WRITE, VIRTIO_9P_OPEN_WRITE_ONLY, VIRTIO_9P_QTDIR, VIRTIO_9P_QTFILE,
    VIRTIO_9P_QTSYMLINK, VIRTIO_9P_RATTACH, VIRTIO_9P_RCLUNK, VIRTIO_9P_RFLUSH, VIRTIO_9P_RFSYNC,
    VIRTIO_9P_RGETATTR, VIRTIO_9P_RGETLOCK, VIRTIO_9P_RLCREATE, VIRTIO_9P_RLERROR, VIRTIO_9P_RLINK,
    VIRTIO_9P_RLOCK, VIRTIO_9P_RLOPEN, VIRTIO_9P_RMKDIR, VIRTIO_9P_RMKNOD, VIRTIO_9P_ROPEN,
    VIRTIO_9P_RREAD, VIRTIO_9P_RREADDIR, VIRTIO_9P_RREADLINK, VIRTIO_9P_RREMOVE, VIRTIO_9P_RRENAME,
    VIRTIO_9P_RRENAMEAT, VIRTIO_9P_RSETATTR, VIRTIO_9P_RSTATFS, VIRTIO_9P_RSYMLINK,
    VIRTIO_9P_RUNLINKAT, VIRTIO_9P_RWALK, VIRTIO_9P_RWRITE, VIRTIO_9P_RXATTRWALK,
    VIRTIO_9P_SETATTR_ATIME, VIRTIO_9P_SETATTR_ATIME_SET, VIRTIO_9P_SETATTR_GID,
    VIRTIO_9P_SETATTR_MODE, VIRTIO_9P_SETATTR_MTIME, VIRTIO_9P_SETATTR_MTIME_SET,
    VIRTIO_9P_SETATTR_SIZE, VIRTIO_9P_SETATTR_UID, VIRTIO_9P_STATFS_BLOCK_SIZE,
    VIRTIO_9P_STATFS_TYPE, VIRTIO_9P_TATTACH, VIRTIO_9P_TCLUNK, VIRTIO_9P_TFLUSH, VIRTIO_9P_TFSYNC,
    VIRTIO_9P_TGETATTR, VIRTIO_9P_TGETLOCK, VIRTIO_9P_TLCREATE, VIRTIO_9P_TLINK, VIRTIO_9P_TLOCK,
    VIRTIO_9P_TLOPEN, VIRTIO_9P_TMKDIR, VIRTIO_9P_TMKNOD, VIRTIO_9P_TOPEN, VIRTIO_9P_TREAD,
    VIRTIO_9P_TREADDIR, VIRTIO_9P_TREADLINK, VIRTIO_9P_TREMOVE, VIRTIO_9P_TRENAME,
    VIRTIO_9P_TRENAMEAT, VIRTIO_9P_TSETATTR, VIRTIO_9P_TSTATFS, VIRTIO_9P_TSYMLINK,
    VIRTIO_9P_TUNLINKAT, VIRTIO_9P_TWALK, VIRTIO_9P_TWRITE, VIRTIO_9P_TXATTRCREATE,
    VIRTIO_9P_TXATTRWALK,
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
fn virtio_9p_device_supports_legacy_open_for_walked_files() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("legacy.txt", b"legacy open".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"legacy.txt"]));
    let walk_completion = device.execute_at(11, walk).unwrap();
    let (_, _, walk_path) = read_qid(walk_completion.payload(), 2);

    let open = decoded_request(VIRTIO_9P_TOPEN, 3, p9_open_payload(2, 0));
    let open_completion = device.execute_at(12, open).unwrap();
    assert_eq!(open_completion.message_type(), VIRTIO_9P_ROPEN);
    let (open_qtype, open_version, open_path) = read_qid(open_completion.payload(), 0);
    assert_eq!(open_qtype, VIRTIO_9P_QTFILE);
    assert_eq!(open_version, 0);
    assert_eq!(open_path, walk_path);
    assert_eq!(
        open_completion.payload()[13..17],
        VIRTIO_9P_DEFAULT_MSIZE.to_le_bytes()
    );

    let read = decoded_request(VIRTIO_9P_TREAD, 4, p9_read_payload(2, 7, 16));
    let read_completion = device.execute_at(13, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"open");
}

#[test]
fn virtio_9p_device_enforces_lopen_access_modes() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let walk_read = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    device.execute_at(11, walk_read).unwrap();
    let open_read = decoded_request(
        VIRTIO_9P_TLOPEN,
        3,
        p9_lopen_payload(2, u32::from(VIRTIO_9P_OPEN_READ_ONLY)),
    );
    assert_eq!(
        device.execute_at(12, open_read).unwrap().message_type(),
        VIRTIO_9P_RLOPEN
    );
    let denied_write = decoded_request(VIRTIO_9P_TWRITE, 4, p9_write_payload(2, 0, b"!"));
    let denied_write_completion = device.execute_at(13, denied_write).unwrap();
    assert_eq!(denied_write_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        denied_write_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
    let allowed_read = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 16));
    let allowed_read_completion = device.execute_at(14, allowed_read).unwrap();
    assert_eq!(allowed_read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(
        read_counted_data(allowed_read_completion.payload()),
        b"alpha"
    );

    let walk_write = decoded_request(VIRTIO_9P_TWALK, 6, p9_walk_payload(1, 3, &[b"alpha.txt"]));
    device.execute_at(15, walk_write).unwrap();
    let open_write = decoded_request(
        VIRTIO_9P_TLOPEN,
        7,
        p9_lopen_payload(3, u32::from(VIRTIO_9P_OPEN_WRITE_ONLY)),
    );
    assert_eq!(
        device.execute_at(16, open_write).unwrap().message_type(),
        VIRTIO_9P_RLOPEN
    );
    let denied_read = decoded_request(VIRTIO_9P_TREAD, 8, p9_read_payload(3, 0, 16));
    let denied_read_completion = device.execute_at(17, denied_read).unwrap();
    assert_eq!(denied_read_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        denied_read_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
    let allowed_write = decoded_request(VIRTIO_9P_TWRITE, 9, p9_write_payload(3, 5, b" rem6"));
    let allowed_write_completion = device.execute_at(18, allowed_write).unwrap();
    assert_eq!(allowed_write_completion.message_type(), VIRTIO_9P_RWRITE);
    assert_eq!(allowed_write_completion.payload(), 5_u32.to_le_bytes());

    let read_updated = decoded_request(VIRTIO_9P_TREAD, 10, p9_read_payload(2, 0, 16));
    let read_updated_completion = device.execute_at(19, read_updated).unwrap();
    assert_eq!(read_updated_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(
        read_counted_data(read_updated_completion.payload()),
        b"alpha rem6"
    );
}

#[test]
fn virtio_9p_device_enforces_legacy_open_access_modes() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("legacy.txt", b"legacy".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"legacy.txt"]));
    device.execute_at(11, walk).unwrap();
    let open = decoded_request(
        VIRTIO_9P_TOPEN,
        3,
        p9_open_payload(2, VIRTIO_9P_OPEN_READ_WRITE),
    );
    assert_eq!(
        device.execute_at(12, open).unwrap().message_type(),
        VIRTIO_9P_ROPEN
    );

    let write = decoded_request(VIRTIO_9P_TWRITE, 4, p9_write_payload(2, 6, b" open"));
    let write_completion = device.execute_at(13, write).unwrap();
    assert_eq!(write_completion.message_type(), VIRTIO_9P_RWRITE);
    assert_eq!(write_completion.payload(), 5_u32.to_le_bytes());
    let read = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(14, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"legacy open");
}

#[test]
fn virtio_9p_device_rejects_legacy_open_on_stale_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let open = decoded_request(VIRTIO_9P_TOPEN, 1, p9_open_payload(7, 0));

    let completion = device.execute_at(10, open).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_flush_acknowledges_without_mutating_fids() {
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

    let flush = decoded_request(VIRTIO_9P_TFLUSH, 3, p9_flush_payload(2));
    let flush_completion = device.execute_at(12, flush).unwrap();
    assert_eq!(flush_completion.message_type(), VIRTIO_9P_RFLUSH);
    assert_eq!(flush_completion.tag(), 3);
    assert!(flush_completion.payload().is_empty());
    assert_eq!(device.fid_count(), 2);

    let open = decoded_request(VIRTIO_9P_TLOPEN, 4, p9_lopen_payload(2, 0));
    device.execute_at(13, open).unwrap();
    let read = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(14, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"hello rem6");
}

#[test]
fn virtio_9p_device_fsync_acknowledges_existing_fids_only() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let stale = decoded_request(VIRTIO_9P_TFSYNC, 1, p9_fsync_payload(7, 0));
    let stale_completion = device.execute_at(10, stale).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        2,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(11, attach).unwrap();
    let fsync = decoded_request(VIRTIO_9P_TFSYNC, 3, p9_fsync_payload(1, 1));
    let fsync_completion = device.execute_at(12, fsync).unwrap();
    assert_eq!(fsync_completion.message_type(), VIRTIO_9P_RFSYNC);
    assert!(fsync_completion.payload().is_empty());
}

#[test]
fn virtio_9p_device_accepts_advisory_locks_on_open_files() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
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

    let lock = decoded_request(
        VIRTIO_9P_TLOCK,
        4,
        p9_lock_payload(2, VIRTIO_9P_LOCK_TYPE_WRLCK, 0, 0, 5, 42, b"client-a"),
    );
    let lock_completion = device.execute_at(13, lock).unwrap();
    assert_eq!(lock_completion.message_type(), VIRTIO_9P_RLOCK);
    assert_eq!(lock_completion.payload(), [VIRTIO_9P_LOCK_SUCCESS]);

    let stale = decoded_request(
        VIRTIO_9P_TLOCK,
        5,
        p9_lock_payload(7, VIRTIO_9P_LOCK_TYPE_WRLCK, 0, 0, 5, 42, b"client-a"),
    );
    let stale_completion = device.execute_at(14, stale).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_reports_no_advisory_lock_conflicts() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
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

    let getlock = decoded_request(
        VIRTIO_9P_TGETLOCK,
        4,
        p9_lock_payload(2, VIRTIO_9P_LOCK_TYPE_WRLCK, 0, 12, 8, 99, b"client-a"),
    );
    let getlock_completion = device.execute_at(13, getlock).unwrap();
    assert_eq!(getlock_completion.message_type(), VIRTIO_9P_RGETLOCK);
    let payload = getlock_completion.payload();
    assert_eq!(payload[0], VIRTIO_9P_LOCK_TYPE_UNLCK);
    assert_eq!(read_u32(payload, 1), 0);
    assert_eq!(read_u64(payload, 5), 12);
    assert_eq!(read_u64(payload, 13), 8);
    assert_eq!(read_u32(payload, 21), 99);
    assert_eq!(read_string(payload, 25), b"client-a");

    let stale = decoded_request(
        VIRTIO_9P_TGETLOCK,
        5,
        p9_lock_payload(7, VIRTIO_9P_LOCK_TYPE_WRLCK, 0, 12, 8, 99, b"client-a"),
    );
    let stale_completion = device.execute_at(14, stale).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_lists_empty_xattrs_with_readable_xattr_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    device.execute_at(11, walk).unwrap();

    let xattrwalk = decoded_request(VIRTIO_9P_TXATTRWALK, 3, p9_xattrwalk_payload(2, 3, b""));
    let xattrwalk_completion = device.execute_at(12, xattrwalk).unwrap();
    assert_eq!(xattrwalk_completion.message_type(), VIRTIO_9P_RXATTRWALK);
    assert_eq!(xattrwalk_completion.payload(), 0_u64.to_le_bytes());

    let read = decoded_request(VIRTIO_9P_TREAD, 4, p9_read_payload(3, 0, 16));
    let read_completion = device.execute_at(13, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"");

    let getattr = decoded_request(
        VIRTIO_9P_TGETATTR,
        5,
        p9_getattr_payload(3, VIRTIO_9P_GETATTR_BASIC),
    );
    let getattr_completion = device.execute_at(14, getattr).unwrap();
    assert_eq!(getattr_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(getattr_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let clunk = decoded_request(VIRTIO_9P_TCLUNK, 6, p9_clunk_payload(3));
    let clunk_completion = device.execute_at(15, clunk).unwrap();
    assert_eq!(clunk_completion.message_type(), VIRTIO_9P_RCLUNK);
    assert!(clunk_completion.payload().is_empty());
}

#[test]
fn virtio_9p_device_reports_missing_xattrs_and_rejects_stale_xattr_create() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    device.execute_at(11, walk).unwrap();

    let missing = decoded_request(
        VIRTIO_9P_TXATTRWALK,
        3,
        p9_xattrwalk_payload(2, 3, b"user.missing"),
    );
    let missing_completion = device.execute_at(12, missing).unwrap();
    assert_eq!(missing_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        missing_completion.payload(),
        VIRTIO_9P_ENODATA.to_le_bytes()
    );

    let read_missing_fid = decoded_request(VIRTIO_9P_TREAD, 4, p9_read_payload(3, 0, 16));
    let read_missing_completion = device.execute_at(13, read_missing_fid).unwrap();
    assert_eq!(read_missing_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        read_missing_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );

    let stale = decoded_request(
        VIRTIO_9P_TXATTRCREATE,
        5,
        p9_xattrcreate_payload(9, b"user.created", 4, 0),
    );
    let stale_completion = device.execute_at(14, stale).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_creates_walks_and_reads_symlinks() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("target.txt", b"target data".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let symlink = decoded_request(
        VIRTIO_9P_TSYMLINK,
        2,
        p9_symlink_payload(1, b"target.link", b"target.txt", 0),
    );
    let symlink_completion = device.execute_at(11, symlink).unwrap();
    assert_eq!(symlink_completion.message_type(), VIRTIO_9P_RSYMLINK);
    let (symlink_qtype, symlink_version, symlink_path) = read_qid(symlink_completion.payload(), 0);
    assert_eq!(symlink_qtype, VIRTIO_9P_QTSYMLINK);
    assert_eq!(symlink_version, 0);
    assert_ne!(symlink_path, 1);

    let walk = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"target.link"]));
    let walk_completion = device.execute_at(12, walk).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
    let (walk_qtype, _, walk_path) = read_qid(walk_completion.payload(), 2);
    assert_eq!(walk_qtype, VIRTIO_9P_QTSYMLINK);
    assert_eq!(walk_path, symlink_path);

    let readlink = decoded_request(VIRTIO_9P_TREADLINK, 4, p9_readlink_payload(2));
    let readlink_completion = device.execute_at(13, readlink).unwrap();
    assert_eq!(readlink_completion.message_type(), VIRTIO_9P_RREADLINK);
    assert_eq!(readlink_completion.payload(), p9_string(b"target.txt"));

    let open_root = decoded_request(VIRTIO_9P_TLOPEN, 5, p9_lopen_payload(1, 0));
    device.execute_at(14, open_root).unwrap();
    let readdir = decoded_request(VIRTIO_9P_TREADDIR, 6, p9_readdir_payload(1, 0, 512));
    let readdir_completion = device.execute_at(15, readdir).unwrap();
    let entries = read_dir_entries(readdir_completion.payload());
    let link_entry = entries
        .iter()
        .find(|entry| entry.name == "target.link")
        .unwrap();
    assert_eq!(link_entry.qtype, VIRTIO_9P_QTSYMLINK);
    assert_eq!(link_entry.qpath, symlink_path);
    assert_eq!(link_entry.dtype, VIRTIO_9P_DTSYMLINK);
}

#[test]
fn virtio_9p_device_rejects_stale_or_non_symlink_readlink_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let stale = decoded_request(VIRTIO_9P_TREADLINK, 1, p9_readlink_payload(7));
    let stale_completion = device.execute_at(10, stale).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        2,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(11, attach).unwrap();
    let root = decoded_request(VIRTIO_9P_TREADLINK, 3, p9_readlink_payload(1));
    let root_completion = device.execute_at(12, root).unwrap();
    assert_eq!(root_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(root_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
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
fn virtio_9p_device_remove_deletes_empty_directory_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let mkdir = decoded_request(
        VIRTIO_9P_TMKDIR,
        2,
        p9_mkdir_payload(1, b"empty", 0o040755, 0),
    );
    device.execute_at(11, mkdir).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"empty"]));
    device.execute_at(12, walk).unwrap();

    let remove = decoded_request(VIRTIO_9P_TREMOVE, 4, p9_remove_payload(2));
    let remove_completion = device.execute_at(13, remove).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RREMOVE);
    assert!(remove_completion.payload().is_empty());
    assert_eq!(device.fid_count(), 1);

    let stat_removed = decoded_request(VIRTIO_9P_TSTATFS, 5, p9_statfs_payload(2));
    let stat_completion = device.execute_at(14, stat_removed).unwrap();
    assert_eq!(stat_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stat_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let walk_removed = decoded_request(VIRTIO_9P_TWALK, 6, p9_walk_payload(1, 3, &[b"empty"]));
    let removed_completion = device.execute_at(15, walk_removed).unwrap();
    assert_eq!(removed_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(removed_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
}

#[test]
fn virtio_9p_device_remove_rejects_non_empty_directories() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let mkdir = decoded_request(
        VIRTIO_9P_TMKDIR,
        2,
        p9_mkdir_payload(1, b"parent", 0o040755, 0),
    );
    device.execute_at(11, mkdir).unwrap();
    let remove_target = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"parent"]));
    device.execute_at(12, remove_target).unwrap();
    let create_parent = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"parent"]));
    device.execute_at(13, create_parent).unwrap();
    let create_child = decoded_request(
        VIRTIO_9P_TLCREATE,
        5,
        p9_lcreate_payload(3, b"child.txt", 0, 0o100644, 0),
    );
    device.execute_at(14, create_child).unwrap();

    let remove = decoded_request(VIRTIO_9P_TREMOVE, 6, p9_remove_payload(2));
    let remove_completion = device.execute_at(15, remove).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        remove_completion.payload(),
        VIRTIO_9P_ENOTEMPTY.to_le_bytes()
    );

    let walk_parent = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(1, 4, &[b"parent"]));
    let walk_completion = device.execute_at(16, walk_parent).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
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
fn virtio_9p_device_unlinkat_removes_empty_directories_with_remove_dir_flag() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let mkdir = decoded_request(
        VIRTIO_9P_TMKDIR,
        2,
        p9_mkdir_payload(1, b"empty", 0o040755, 0),
    );
    let mkdir_completion = device.execute_at(11, mkdir).unwrap();
    assert_eq!(mkdir_completion.message_type(), VIRTIO_9P_RMKDIR);

    let unlink = decoded_request(
        VIRTIO_9P_TUNLINKAT,
        3,
        p9_unlinkat_payload(1, b"empty", VIRTIO_9P_AT_REMOVEDIR),
    );
    let unlink_completion = device.execute_at(12, unlink).unwrap();
    assert_eq!(unlink_completion.message_type(), VIRTIO_9P_RUNLINKAT);
    assert!(unlink_completion.payload().is_empty());

    let walk_removed = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 2, &[b"empty"]));
    let removed_completion = device.execute_at(13, walk_removed).unwrap();
    assert_eq!(removed_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(removed_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
}

#[test]
fn virtio_9p_device_rejects_unlinkat_remove_dir_for_non_empty_directories() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let mkdir = decoded_request(
        VIRTIO_9P_TMKDIR,
        2,
        p9_mkdir_payload(1, b"parent", 0o040755, 0),
    );
    device.execute_at(11, mkdir).unwrap();
    let walk_parent = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"parent"]));
    device.execute_at(12, walk_parent).unwrap();
    let create_child = decoded_request(
        VIRTIO_9P_TLCREATE,
        4,
        p9_lcreate_payload(2, b"child.txt", 0, 0o100644, 0),
    );
    device.execute_at(13, create_child).unwrap();

    let unlink = decoded_request(
        VIRTIO_9P_TUNLINKAT,
        5,
        p9_unlinkat_payload(1, b"parent", VIRTIO_9P_AT_REMOVEDIR),
    );
    let unlink_completion = device.execute_at(14, unlink).unwrap();
    assert_eq!(unlink_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        unlink_completion.payload(),
        VIRTIO_9P_ENOTEMPTY.to_le_bytes()
    );

    let walk_parent = decoded_request(VIRTIO_9P_TWALK, 6, p9_walk_payload(1, 3, &[b"parent"]));
    let walk_completion = device.execute_at(15, walk_parent).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
}

#[test]
fn virtio_9p_device_makes_root_directories_and_walks_into_them() {
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

    let mkdir = decoded_request(
        VIRTIO_9P_TMKDIR,
        3,
        p9_mkdir_payload(1, b"tmp", 0o040755, 0),
    );
    let mkdir_completion = device.execute_at(12, mkdir).unwrap();
    assert_eq!(mkdir_completion.message_type(), VIRTIO_9P_RMKDIR);
    let (mkdir_qtype, mkdir_version, mkdir_path) = read_qid(mkdir_completion.payload(), 0);
    assert_eq!(mkdir_qtype, VIRTIO_9P_QTDIR);
    assert_eq!(mkdir_version, 0);
    assert_ne!(mkdir_path, 1);

    let root_readdir = decoded_request(VIRTIO_9P_TREADDIR, 4, p9_readdir_payload(1, 0, 512));
    let root_completion = device.execute_at(13, root_readdir).unwrap();
    let root_entries = read_dir_entries(root_completion.payload());
    let root_names: Vec<_> = root_entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(root_names, [".", "..", "alpha.txt", "tmp"]);
    let tmp_entry = root_entries
        .iter()
        .find(|entry| entry.name == "tmp")
        .unwrap();
    assert_eq!(tmp_entry.qtype, VIRTIO_9P_QTDIR);
    assert_eq!(tmp_entry.qpath, mkdir_path);
    assert_eq!(tmp_entry.dtype, VIRTIO_9P_DTDIR);

    let walk_tmp = decoded_request(VIRTIO_9P_TWALK, 5, p9_walk_payload(1, 2, &[b"tmp"]));
    let walk_completion = device.execute_at(14, walk_tmp).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
    let (walk_qtype, _, walk_path) = read_qid(walk_completion.payload(), 2);
    assert_eq!(walk_qtype, VIRTIO_9P_QTDIR);
    assert_eq!(walk_path, mkdir_path);

    let getattr_tmp = decoded_request(
        VIRTIO_9P_TGETATTR,
        6,
        p9_getattr_payload(2, VIRTIO_9P_GETATTR_BASIC),
    );
    let getattr_completion = device.execute_at(15, getattr_tmp).unwrap();
    assert_eq!(getattr_completion.message_type(), VIRTIO_9P_RGETATTR);
    let (getattr_qtype, _, getattr_path) = read_qid(getattr_completion.payload(), 8);
    assert_eq!(getattr_qtype, VIRTIO_9P_QTDIR);
    assert_eq!(getattr_path, mkdir_path);
    assert_eq!(read_u32(getattr_completion.payload(), 21), 0o040755);

    let open_tmp = decoded_request(VIRTIO_9P_TLOPEN, 7, p9_lopen_payload(2, 0));
    device.execute_at(16, open_tmp).unwrap();
    let tmp_readdir = decoded_request(VIRTIO_9P_TREADDIR, 8, p9_readdir_payload(2, 0, 512));
    let tmp_completion = device.execute_at(17, tmp_readdir).unwrap();
    let tmp_entries = read_dir_entries(tmp_completion.payload());
    let tmp_names: Vec<_> = tmp_entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(tmp_names, [".", ".."]);
}

#[test]
fn virtio_9p_device_mknod_creates_lists_and_reports_character_devices() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let mknod = decoded_request(
        VIRTIO_9P_TMKNOD,
        2,
        p9_mknod_payload(1, b"null", 0o020666, 1, 3, 0),
    );
    let mknod_completion = device.execute_at(11, mknod).unwrap();
    assert_eq!(mknod_completion.message_type(), VIRTIO_9P_RMKNOD);
    let (mknod_qtype, mknod_version, mknod_path) = read_qid(mknod_completion.payload(), 0);
    assert_eq!(mknod_qtype, VIRTIO_9P_QTFILE);
    assert_eq!(mknod_version, 0);
    assert_ne!(mknod_path, 1);

    let walk = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"null"]));
    let walk_completion = device.execute_at(12, walk).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
    let (walk_qtype, _, walk_path) = read_qid(walk_completion.payload(), 2);
    assert_eq!(walk_qtype, VIRTIO_9P_QTFILE);
    assert_eq!(walk_path, mknod_path);

    let getattr = decoded_request(
        VIRTIO_9P_TGETATTR,
        4,
        p9_getattr_payload(2, VIRTIO_9P_GETATTR_BASIC),
    );
    let getattr_completion = device.execute_at(13, getattr).unwrap();
    assert_eq!(getattr_completion.message_type(), VIRTIO_9P_RGETATTR);
    assert_eq!(read_u32(getattr_completion.payload(), 21), 0o020666);
    assert_eq!(read_u64(getattr_completion.payload(), 33), 1);
    assert_eq!(read_u64(getattr_completion.payload(), 41), 0x103);
    assert_eq!(read_u64(getattr_completion.payload(), 49), 0);

    let open_root = decoded_request(VIRTIO_9P_TLOPEN, 5, p9_lopen_payload(1, 0));
    device.execute_at(14, open_root).unwrap();
    let readdir = decoded_request(VIRTIO_9P_TREADDIR, 6, p9_readdir_payload(1, 0, 512));
    let readdir_completion = device.execute_at(15, readdir).unwrap();
    let entries = read_dir_entries(readdir_completion.payload());
    let null_entry = entries.iter().find(|entry| entry.name == "null").unwrap();
    assert_eq!(null_entry.qtype, VIRTIO_9P_QTFILE);
    assert_eq!(null_entry.qpath, mknod_path);
    assert_eq!(null_entry.dtype, VIRTIO_9P_DTCHR);

    let open_null = decoded_request(VIRTIO_9P_TLOPEN, 7, p9_lopen_payload(2, 0));
    device.execute_at(16, open_null).unwrap();
    let read_null = decoded_request(VIRTIO_9P_TREAD, 8, p9_read_payload(2, 0, 8));
    let read_completion = device.execute_at(17, read_null).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(read_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_rejects_mknod_on_invalid_parents_and_duplicates() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let stale = decoded_request(
        VIRTIO_9P_TMKNOD,
        1,
        p9_mknod_payload(7, b"null", 0o020666, 1, 3, 0),
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
    let duplicate = decoded_request(
        VIRTIO_9P_TMKNOD,
        3,
        p9_mknod_payload(1, b"alpha.txt", 0o020666, 1, 3, 0),
    );
    let duplicate_completion = device.execute_at(12, duplicate).unwrap();
    assert_eq!(duplicate_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        duplicate_completion.payload(),
        VIRTIO_9P_EEXIST.to_le_bytes()
    );

    let walk_file = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    device.execute_at(13, walk_file).unwrap();
    let file_parent = decoded_request(
        VIRTIO_9P_TMKNOD,
        5,
        p9_mknod_payload(2, b"null", 0o020666, 1, 3, 0),
    );
    let file_parent_completion = device.execute_at(14, file_parent).unwrap();
    assert_eq!(file_parent_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        file_parent_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
}

#[test]
fn virtio_9p_device_creates_reads_and_lists_files_inside_directories() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let mkdir = decoded_request(
        VIRTIO_9P_TMKDIR,
        2,
        p9_mkdir_payload(1, b"tmp", 0o040755, 0),
    );
    device.execute_at(11, mkdir).unwrap();
    let walk_tmp = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"tmp"]));
    device.execute_at(12, walk_tmp).unwrap();

    let create = decoded_request(
        VIRTIO_9P_TLCREATE,
        4,
        p9_lcreate_payload(
            2,
            b"note.txt",
            u32::from(VIRTIO_9P_OPEN_READ_WRITE),
            0o100644,
            0,
        ),
    );
    let create_completion = device.execute_at(13, create).unwrap();
    assert_eq!(create_completion.message_type(), VIRTIO_9P_RLCREATE);
    let (created_qtype, _, created_path) = read_qid(create_completion.payload(), 0);
    assert_eq!(created_qtype, VIRTIO_9P_QTFILE);

    let write = decoded_request(VIRTIO_9P_TWRITE, 5, p9_write_payload(2, 0, b"inside"));
    let write_completion = device.execute_at(14, write).unwrap();
    assert_eq!(write_completion.message_type(), VIRTIO_9P_RWRITE);
    assert_eq!(write_completion.payload(), 6_u32.to_le_bytes());

    let read = decoded_request(VIRTIO_9P_TREAD, 6, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(15, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"inside");

    let attach_root = decoded_request(
        VIRTIO_9P_TATTACH,
        7,
        p9_attach_payload(10, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(16, attach_root).unwrap();
    let walk_file = decoded_request(
        VIRTIO_9P_TWALK,
        8,
        p9_walk_payload(10, 3, &[b"tmp", b"note.txt"]),
    );
    let walk_completion = device.execute_at(17, walk_file).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
    let (_, _, walked_path) = read_qid(walk_completion.payload(), 15);
    assert_eq!(walked_path, created_path);

    let walk_tmp = decoded_request(VIRTIO_9P_TWALK, 9, p9_walk_payload(10, 4, &[b"tmp"]));
    device.execute_at(18, walk_tmp).unwrap();
    let open_tmp = decoded_request(VIRTIO_9P_TLOPEN, 10, p9_lopen_payload(4, 0));
    device.execute_at(19, open_tmp).unwrap();
    let readdir = decoded_request(VIRTIO_9P_TREADDIR, 11, p9_readdir_payload(4, 0, 512));
    let readdir_completion = device.execute_at(20, readdir).unwrap();
    let entries = read_dir_entries(readdir_completion.payload());
    let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, [".", "..", "note.txt"]);
}

#[test]
fn virtio_9p_device_rejects_mkdir_on_stale_file_and_duplicate_targets() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("plain.txt", b"plain".to_vec())
        .unwrap();

    let stale = decoded_request(
        VIRTIO_9P_TMKDIR,
        1,
        p9_mkdir_payload(7, b"tmp", 0o040755, 0),
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
    let mkdir = decoded_request(
        VIRTIO_9P_TMKDIR,
        3,
        p9_mkdir_payload(1, b"tmp", 0o040755, 0),
    );
    device.execute_at(12, mkdir).unwrap();
    let duplicate = decoded_request(
        VIRTIO_9P_TMKDIR,
        4,
        p9_mkdir_payload(1, b"tmp", 0o040755, 0),
    );
    let duplicate_completion = device.execute_at(13, duplicate).unwrap();
    assert_eq!(duplicate_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        duplicate_completion.payload(),
        VIRTIO_9P_EEXIST.to_le_bytes()
    );

    let walk_file = decoded_request(VIRTIO_9P_TWALK, 5, p9_walk_payload(1, 2, &[b"plain.txt"]));
    device.execute_at(14, walk_file).unwrap();
    let mkdir_under_file = decoded_request(
        VIRTIO_9P_TMKDIR,
        6,
        p9_mkdir_payload(2, b"child", 0o040755, 0),
    );
    let file_parent_completion = device.execute_at(15, mkdir_under_file).unwrap();
    assert_eq!(file_parent_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        file_parent_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
}

#[test]
fn virtio_9p_device_renames_root_files_preserving_open_fids() {
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

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let walk_completion = device.execute_at(12, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(walk_completion.payload(), 2);
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 4, p9_lopen_payload(2, 0));
    device.execute_at(13, open_alpha).unwrap();

    let rename = decoded_request(
        VIRTIO_9P_TRENAMEAT,
        5,
        p9_renameat_payload(1, b"alpha.txt", 1, b"gamma.txt"),
    );
    let rename_completion = device.execute_at(14, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAMEAT);
    assert!(rename_completion.payload().is_empty());

    let read_open = decoded_request(VIRTIO_9P_TREAD, 6, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(15, read_open).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"alpha");

    let old_walk = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(1, 3, &[b"alpha.txt"]));
    let old_completion = device.execute_at(16, old_walk).unwrap();
    assert_eq!(old_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(old_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());

    let new_walk = decoded_request(VIRTIO_9P_TWALK, 8, p9_walk_payload(1, 4, &[b"gamma.txt"]));
    let new_completion = device.execute_at(17, new_walk).unwrap();
    assert_eq!(new_completion.message_type(), VIRTIO_9P_RWALK);
    let (_, _, gamma_path) = read_qid(new_completion.payload(), 2);
    assert_eq!(gamma_path, alpha_path);

    let readdir = decoded_request(VIRTIO_9P_TREADDIR, 9, p9_readdir_payload(1, 0, 512));
    let readdir_completion = device.execute_at(18, readdir).unwrap();
    let entries = read_dir_entries(readdir_completion.payload());
    let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, [".", "..", "beta.txt", "gamma.txt"]);
}

#[test]
fn virtio_9p_device_renameat_replaces_existing_root_files() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap()
        .with_file("beta.txt", b"beta".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let alpha_completion = device.execute_at(11, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(alpha_completion.payload(), 2);
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 3, p9_lopen_payload(2, 0));
    device.execute_at(12, open_alpha).unwrap();

    let walk_beta = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"beta.txt"]));
    device.execute_at(13, walk_beta).unwrap();
    let open_beta = decoded_request(VIRTIO_9P_TLOPEN, 5, p9_lopen_payload(3, 0));
    device.execute_at(14, open_beta).unwrap();

    let rename = decoded_request(
        VIRTIO_9P_TRENAMEAT,
        6,
        p9_renameat_payload(1, b"alpha.txt", 1, b"beta.txt"),
    );
    let rename_completion = device.execute_at(15, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAMEAT);

    let read_replaced = decoded_request(VIRTIO_9P_TREAD, 7, p9_read_payload(3, 0, 16));
    let replaced_completion = device.execute_at(16, read_replaced).unwrap();
    assert_eq!(replaced_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(replaced_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let read_alpha_fid = decoded_request(VIRTIO_9P_TREAD, 8, p9_read_payload(2, 0, 16));
    let alpha_fid_completion = device.execute_at(17, read_alpha_fid).unwrap();
    assert_eq!(alpha_fid_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(alpha_fid_completion.payload()), b"alpha");

    let new_walk = decoded_request(VIRTIO_9P_TWALK, 9, p9_walk_payload(1, 4, &[b"beta.txt"]));
    let new_completion = device.execute_at(18, new_walk).unwrap();
    assert_eq!(new_completion.message_type(), VIRTIO_9P_RWALK);
    let (_, _, new_beta_path) = read_qid(new_completion.payload(), 2);
    assert_eq!(new_beta_path, alpha_path);

    let old_walk = decoded_request(VIRTIO_9P_TWALK, 10, p9_walk_payload(1, 5, &[b"alpha.txt"]));
    let old_completion = device.execute_at(19, old_walk).unwrap();
    assert_eq!(old_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(old_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
}

#[test]
fn virtio_9p_device_renames_open_file_fid_into_directory() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let mkdir = decoded_request(
        VIRTIO_9P_TMKDIR,
        2,
        p9_mkdir_payload(1, b"tmp", 0o040755, 0),
    );
    device.execute_at(11, mkdir).unwrap();

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let walk_completion = device.execute_at(12, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(walk_completion.payload(), 2);
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 4, p9_lopen_payload(2, 0));
    device.execute_at(13, open_alpha).unwrap();
    let walk_tmp = decoded_request(VIRTIO_9P_TWALK, 5, p9_walk_payload(1, 3, &[b"tmp"]));
    device.execute_at(14, walk_tmp).unwrap();

    let rename = decoded_request(
        VIRTIO_9P_TRENAME,
        6,
        p9_rename_payload(2, 3, b"renamed.txt"),
    );
    let rename_completion = device.execute_at(15, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAME);
    assert!(rename_completion.payload().is_empty());

    let read_open = decoded_request(VIRTIO_9P_TREAD, 7, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(16, read_open).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"alpha");

    let old_walk = decoded_request(VIRTIO_9P_TWALK, 8, p9_walk_payload(1, 4, &[b"alpha.txt"]));
    let old_completion = device.execute_at(17, old_walk).unwrap();
    assert_eq!(old_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(old_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());

    let new_walk = decoded_request(VIRTIO_9P_TWALK, 9, p9_walk_payload(3, 5, &[b"renamed.txt"]));
    let new_completion = device.execute_at(18, new_walk).unwrap();
    assert_eq!(new_completion.message_type(), VIRTIO_9P_RWALK);
    let (_, _, renamed_path) = read_qid(new_completion.payload(), 2);
    assert_eq!(renamed_path, alpha_path);
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
