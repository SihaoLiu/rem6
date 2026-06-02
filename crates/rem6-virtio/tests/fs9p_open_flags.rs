use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_DEFAULT_MSIZE, VIRTIO_9P_EBADF, VIRTIO_9P_EINVAL,
    VIRTIO_9P_ENOENT, VIRTIO_9P_LOPEN_APPEND, VIRTIO_9P_LOPEN_TRUNCATE, VIRTIO_9P_NOFID,
    VIRTIO_9P_OPEN_APPEND, VIRTIO_9P_OPEN_EXECUTE_ONLY, VIRTIO_9P_OPEN_READ_ONLY,
    VIRTIO_9P_OPEN_READ_WRITE, VIRTIO_9P_OPEN_REMOVE_ON_CLOSE, VIRTIO_9P_OPEN_TRUNCATE,
    VIRTIO_9P_OPEN_WRITE_ONLY, VIRTIO_9P_QTFILE, VIRTIO_9P_RCLUNK, VIRTIO_9P_RLERROR,
    VIRTIO_9P_RLINK, VIRTIO_9P_RLOPEN, VIRTIO_9P_RMKDIR, VIRTIO_9P_ROPEN, VIRTIO_9P_RREAD,
    VIRTIO_9P_RREADDIR, VIRTIO_9P_RWALK, VIRTIO_9P_RWRITE, VIRTIO_9P_TATTACH, VIRTIO_9P_TCLUNK,
    VIRTIO_9P_TLINK, VIRTIO_9P_TLOPEN, VIRTIO_9P_TMKDIR, VIRTIO_9P_TOPEN, VIRTIO_9P_TREAD,
    VIRTIO_9P_TREADDIR, VIRTIO_9P_TWALK, VIRTIO_9P_TWRITE,
};

mod support;

use support::fs9p::*;

const OVERSIZED_VECTOR_LENGTH: u64 = isize::MAX as u64 + 1;

fn attached_device_with_file(name: &str, data: &[u8]) -> Virtio9pDevice {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file(name, data.to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    device
}

fn open_payload(message_type: u8, fid: u32, mode: u32) -> Vec<u8> {
    match message_type {
        VIRTIO_9P_TLOPEN => p9_lopen_payload(fid, mode),
        VIRTIO_9P_TOPEN => p9_open_payload(fid, u8::try_from(mode).unwrap()),
        _ => unreachable!("unsupported open message type"),
    }
}

#[test]
fn virtio_9p_device_rejects_oversized_write_offset_before_resizing_file() {
    let device = attached_device_with_file("alpha.txt", b"alpha");

    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    device.execute_at(11, walk).unwrap();
    let open = decoded_request(
        VIRTIO_9P_TLOPEN,
        3,
        p9_lopen_payload(2, u32::from(VIRTIO_9P_OPEN_READ_WRITE)),
    );
    assert_eq!(
        device.execute_at(12, open).unwrap().message_type(),
        VIRTIO_9P_RLOPEN
    );

    let write = decoded_request(
        VIRTIO_9P_TWRITE,
        4,
        p9_write_payload(2, OVERSIZED_VECTOR_LENGTH, b"!"),
    );
    let write_completion = device.execute_at(13, write).unwrap();
    assert_eq!(write_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(write_completion.payload(), VIRTIO_9P_EINVAL.to_le_bytes());

    let read = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(14, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"alpha");
}

#[test]
fn virtio_9p_device_supports_legacy_open_for_walked_files() {
    let device = attached_device_with_file("legacy.txt", b"legacy open");
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
    let device = attached_device_with_file("alpha.txt", b"alpha");

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
    let device = attached_device_with_file("legacy.txt", b"legacy");
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
fn virtio_9p_lopen_truncate_clears_existing_file() {
    let device = attached_device_with_file("log.txt", b"old-data");
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"log.txt"]));
    device.execute_at(11, walk).unwrap();

    let open = decoded_request(
        VIRTIO_9P_TLOPEN,
        3,
        p9_lopen_payload(
            2,
            u32::from(VIRTIO_9P_OPEN_READ_WRITE) | VIRTIO_9P_LOPEN_TRUNCATE,
        ),
    );
    assert_eq!(
        device.execute_at(12, open).unwrap().message_type(),
        VIRTIO_9P_RLOPEN
    );

    let read_empty = decoded_request(VIRTIO_9P_TREAD, 4, p9_read_payload(2, 0, 16));
    let read_empty_completion = device.execute_at(13, read_empty).unwrap();
    assert_eq!(read_empty_completion.message_type(), VIRTIO_9P_RREAD);
    assert!(read_counted_data(read_empty_completion.payload()).is_empty());

    let write = decoded_request(VIRTIO_9P_TWRITE, 5, p9_write_payload(2, 0, b"fresh"));
    let write_completion = device.execute_at(14, write).unwrap();
    assert_eq!(write_completion.message_type(), VIRTIO_9P_RWRITE);
    assert_eq!(write_completion.payload(), 5_u32.to_le_bytes());

    let read_fresh = decoded_request(VIRTIO_9P_TREAD, 6, p9_read_payload(2, 0, 16));
    let read_fresh_completion = device.execute_at(15, read_fresh).unwrap();
    assert_eq!(read_fresh_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_fresh_completion.payload()), b"fresh");
}

#[test]
fn virtio_9p_lopen_append_writes_at_file_end() {
    let device = attached_device_with_file("log.txt", b"head");
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"log.txt"]));
    device.execute_at(11, walk).unwrap();

    let open = decoded_request(
        VIRTIO_9P_TLOPEN,
        3,
        p9_lopen_payload(
            2,
            u32::from(VIRTIO_9P_OPEN_READ_WRITE) | VIRTIO_9P_LOPEN_APPEND,
        ),
    );
    assert_eq!(
        device.execute_at(12, open).unwrap().message_type(),
        VIRTIO_9P_RLOPEN
    );

    let write = decoded_request(VIRTIO_9P_TWRITE, 4, p9_write_payload(2, 0, b"tail"));
    let write_completion = device.execute_at(13, write).unwrap();
    assert_eq!(write_completion.message_type(), VIRTIO_9P_RWRITE);
    assert_eq!(write_completion.payload(), 4_u32.to_le_bytes());

    let read = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(14, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"headtail");
}

#[test]
fn virtio_9p_lopen_execute_only_denies_file_reads_and_writes() {
    let device = attached_device_with_file("exec.txt", b"run");
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"exec.txt"]));
    device.execute_at(11, walk).unwrap();

    let open = decoded_request(
        VIRTIO_9P_TLOPEN,
        3,
        p9_lopen_payload(2, u32::from(VIRTIO_9P_OPEN_EXECUTE_ONLY)),
    );
    assert_eq!(
        device.execute_at(12, open).unwrap().message_type(),
        VIRTIO_9P_RLOPEN
    );

    let denied_read = decoded_request(VIRTIO_9P_TREAD, 4, p9_read_payload(2, 0, 16));
    let denied_read_completion = device.execute_at(13, denied_read).unwrap();
    assert_eq!(denied_read_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        denied_read_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );

    let denied_write = decoded_request(VIRTIO_9P_TWRITE, 5, p9_write_payload(2, 0, b"!"));
    let denied_write_completion = device.execute_at(14, denied_write).unwrap();
    assert_eq!(denied_write_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        denied_write_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
}

#[test]
fn virtio_9p_open_rejects_write_mode_directories_without_opening_fids() {
    for (message_type, reply_type) in [
        (VIRTIO_9P_TLOPEN, VIRTIO_9P_RLOPEN),
        (VIRTIO_9P_TOPEN, VIRTIO_9P_ROPEN),
    ] {
        for rejected_mode in [
            u32::from(VIRTIO_9P_OPEN_WRITE_ONLY),
            u32::from(VIRTIO_9P_OPEN_READ_WRITE),
        ] {
            for use_child_directory in [false, true] {
                let device = attached_device_with_file("alpha.txt", b"alpha");
                let directory_fid = if use_child_directory {
                    let mkdir = decoded_request(
                        VIRTIO_9P_TMKDIR,
                        2,
                        p9_mkdir_payload(1, b"tmp", 0o040755, 0),
                    );
                    let mkdir_completion = device.execute_at(11, mkdir).unwrap();
                    assert_eq!(mkdir_completion.message_type(), VIRTIO_9P_RMKDIR);

                    let walk_tmp =
                        decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"tmp"]));
                    device.execute_at(12, walk_tmp).unwrap();
                    2
                } else {
                    1
                };

                let rejected = decoded_request(
                    message_type,
                    4,
                    open_payload(message_type, directory_fid, rejected_mode),
                );
                let rejected_completion = device.execute_at(13, rejected).unwrap();
                assert_eq!(rejected_completion.message_type(), VIRTIO_9P_RLERROR);
                assert_eq!(rejected_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

                let read_only = decoded_request(
                    message_type,
                    5,
                    open_payload(
                        message_type,
                        directory_fid,
                        u32::from(VIRTIO_9P_OPEN_READ_ONLY),
                    ),
                );
                let read_only_completion = device.execute_at(14, read_only).unwrap();
                assert_eq!(read_only_completion.message_type(), reply_type);

                let readdir = decoded_request(
                    VIRTIO_9P_TREADDIR,
                    6,
                    p9_readdir_payload(directory_fid, 0, 512),
                );
                let readdir_completion = device.execute_at(15, readdir).unwrap();
                assert_eq!(readdir_completion.message_type(), VIRTIO_9P_RREADDIR);
                let entries = read_dir_entries(readdir_completion.payload());
                let expected_name = if use_child_directory {
                    "."
                } else {
                    "alpha.txt"
                };
                assert!(entries.iter().any(|entry| entry.name == expected_name));
            }
        }
    }
}

#[test]
fn virtio_9p_legacy_open_rejects_remove_on_close_write_mode_directory_without_mutating_fid() {
    let device = attached_device_with_file("alpha.txt", b"alpha");
    let mkdir = decoded_request(
        VIRTIO_9P_TMKDIR,
        2,
        p9_mkdir_payload(1, b"tmp", 0o040755, 0),
    );
    let mkdir_completion = device.execute_at(11, mkdir).unwrap();
    assert_eq!(mkdir_completion.message_type(), VIRTIO_9P_RMKDIR);

    let walk_tmp = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"tmp"]));
    device.execute_at(12, walk_tmp).unwrap();

    let rejected = decoded_request(
        VIRTIO_9P_TOPEN,
        4,
        p9_open_payload(
            2,
            VIRTIO_9P_OPEN_READ_WRITE | VIRTIO_9P_OPEN_REMOVE_ON_CLOSE,
        ),
    );
    let rejected_completion = device.execute_at(13, rejected).unwrap();
    assert_eq!(rejected_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(rejected_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let read_only = decoded_request(
        VIRTIO_9P_TOPEN,
        5,
        p9_open_payload(2, VIRTIO_9P_OPEN_READ_ONLY),
    );
    let read_only_completion = device.execute_at(14, read_only).unwrap();
    assert_eq!(read_only_completion.message_type(), VIRTIO_9P_ROPEN);

    let clunk = decoded_request(VIRTIO_9P_TCLUNK, 6, p9_clunk_payload(2));
    let clunk_completion = device.execute_at(15, clunk).unwrap();
    assert_eq!(clunk_completion.message_type(), VIRTIO_9P_RCLUNK);

    let walk_surviving = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(1, 3, &[b"tmp"]));
    let surviving_completion = device.execute_at(16, walk_surviving).unwrap();
    assert_eq!(surviving_completion.message_type(), VIRTIO_9P_RWALK);
}

#[test]
fn virtio_9p_lopen_rejects_reopening_fids_without_changing_access_mode() {
    let device = attached_device_with_file("locked.txt", b"locked");
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"locked.txt"]));
    device.execute_at(11, walk).unwrap();

    let open_read_only = decoded_request(
        VIRTIO_9P_TLOPEN,
        3,
        p9_lopen_payload(2, u32::from(VIRTIO_9P_OPEN_READ_ONLY)),
    );
    assert_eq!(
        device
            .execute_at(12, open_read_only)
            .unwrap()
            .message_type(),
        VIRTIO_9P_RLOPEN
    );

    let reopen_read_write = decoded_request(
        VIRTIO_9P_TLOPEN,
        4,
        p9_lopen_payload(2, u32::from(VIRTIO_9P_OPEN_READ_WRITE)),
    );
    let reopen_completion = device.execute_at(13, reopen_read_write).unwrap();
    assert_eq!(reopen_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(reopen_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let denied_write = decoded_request(VIRTIO_9P_TWRITE, 5, p9_write_payload(2, 0, b"open"));
    let denied_write_completion = device.execute_at(14, denied_write).unwrap();
    assert_eq!(denied_write_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        denied_write_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
}

#[test]
fn virtio_9p_lopen_truncate_rejects_stale_non_file_and_non_writable_fids() {
    let device = attached_device_with_file("log.txt", b"old-data");

    let stale = decoded_request(
        VIRTIO_9P_TLOPEN,
        2,
        p9_lopen_payload(
            99,
            u32::from(VIRTIO_9P_OPEN_WRITE_ONLY) | VIRTIO_9P_LOPEN_TRUNCATE,
        ),
    );
    let stale_completion = device.execute_at(11, stale).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let directory = decoded_request(
        VIRTIO_9P_TLOPEN,
        3,
        p9_lopen_payload(
            1,
            u32::from(VIRTIO_9P_OPEN_WRITE_ONLY) | VIRTIO_9P_LOPEN_TRUNCATE,
        ),
    );
    let directory_completion = device.execute_at(12, directory).unwrap();
    assert_eq!(directory_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        directory_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );

    let walk_read_only = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 2, &[b"log.txt"]));
    device.execute_at(13, walk_read_only).unwrap();
    let read_only_truncate = decoded_request(
        VIRTIO_9P_TLOPEN,
        5,
        p9_lopen_payload(
            2,
            u32::from(VIRTIO_9P_OPEN_READ_ONLY) | VIRTIO_9P_LOPEN_TRUNCATE,
        ),
    );
    let read_only_completion = device.execute_at(14, read_only_truncate).unwrap();
    assert_eq!(read_only_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        read_only_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );

    let walk_check = decoded_request(VIRTIO_9P_TWALK, 6, p9_walk_payload(1, 3, &[b"log.txt"]));
    device.execute_at(15, walk_check).unwrap();
    let open_check = decoded_request(
        VIRTIO_9P_TLOPEN,
        7,
        p9_lopen_payload(3, u32::from(VIRTIO_9P_OPEN_READ_ONLY)),
    );
    assert_eq!(
        device.execute_at(16, open_check).unwrap().message_type(),
        VIRTIO_9P_RLOPEN
    );
    let read_check = decoded_request(VIRTIO_9P_TREAD, 8, p9_read_payload(3, 0, 16));
    let read_check_completion = device.execute_at(17, read_check).unwrap();
    assert_eq!(read_check_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(
        read_counted_data(read_check_completion.payload()),
        b"old-data"
    );
}

#[test]
fn virtio_9p_legacy_open_truncate_clears_existing_file() {
    let device = attached_device_with_file("legacy.txt", b"legacy-data");
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"legacy.txt"]));
    device.execute_at(11, walk).unwrap();

    let open = decoded_request(
        VIRTIO_9P_TOPEN,
        3,
        p9_open_payload(2, VIRTIO_9P_OPEN_READ_WRITE | VIRTIO_9P_OPEN_TRUNCATE),
    );
    assert_eq!(
        device.execute_at(12, open).unwrap().message_type(),
        VIRTIO_9P_ROPEN
    );

    let read = decoded_request(VIRTIO_9P_TREAD, 4, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(13, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert!(read_counted_data(read_completion.payload()).is_empty());
}

#[test]
fn virtio_9p_legacy_open_append_writes_at_file_end() {
    let device = attached_device_with_file("legacy.txt", b"head");
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"legacy.txt"]));
    device.execute_at(11, walk).unwrap();

    let open = decoded_request(
        VIRTIO_9P_TOPEN,
        3,
        p9_open_payload(2, VIRTIO_9P_OPEN_READ_WRITE | VIRTIO_9P_OPEN_APPEND),
    );
    assert_eq!(
        device.execute_at(12, open).unwrap().message_type(),
        VIRTIO_9P_ROPEN
    );

    let write = decoded_request(VIRTIO_9P_TWRITE, 4, p9_write_payload(2, 0, b"tail"));
    let write_completion = device.execute_at(13, write).unwrap();
    assert_eq!(write_completion.message_type(), VIRTIO_9P_RWRITE);
    assert_eq!(write_completion.payload(), 4_u32.to_le_bytes());

    let read = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(14, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"headtail");
}

#[test]
fn virtio_9p_legacy_open_remove_on_close_unlinks_clunked_file() {
    let device = attached_device_with_file("temporary.txt", b"temporary");
    let walk = decoded_request(
        VIRTIO_9P_TWALK,
        2,
        p9_walk_payload(1, 2, &[b"temporary.txt"]),
    );
    device.execute_at(11, walk).unwrap();

    let open = decoded_request(
        VIRTIO_9P_TOPEN,
        3,
        p9_open_payload(
            2,
            VIRTIO_9P_OPEN_READ_WRITE | VIRTIO_9P_OPEN_REMOVE_ON_CLOSE,
        ),
    );
    let open_completion = device.execute_at(12, open).unwrap();
    assert_eq!(open_completion.message_type(), VIRTIO_9P_ROPEN);

    let clunk = decoded_request(VIRTIO_9P_TCLUNK, 4, p9_clunk_payload(2));
    let clunk_completion = device.execute_at(13, clunk).unwrap();
    assert_eq!(clunk_completion.message_type(), VIRTIO_9P_RCLUNK);

    let walk_removed = decoded_request(
        VIRTIO_9P_TWALK,
        5,
        p9_walk_payload(1, 3, &[b"temporary.txt"]),
    );
    let removed_completion = device.execute_at(14, walk_removed).unwrap();
    assert_eq!(removed_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(removed_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
}

#[test]
fn virtio_9p_legacy_open_remove_on_close_removes_only_clunked_hardlink_path() {
    let device = attached_device_with_file("alpha.txt", b"alpha");
    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let alpha_completion = device.execute_at(11, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(alpha_completion.payload(), 2);

    let link = decoded_request(VIRTIO_9P_TLINK, 3, p9_link_payload(1, 2, b"beta.txt"));
    let link_completion = device.execute_at(12, link).unwrap();
    assert_eq!(link_completion.message_type(), VIRTIO_9P_RLINK);

    let walk_beta = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"beta.txt"]));
    let beta_completion = device.execute_at(13, walk_beta).unwrap();
    assert_eq!(read_qid(beta_completion.payload(), 2).2, alpha_path);

    let open_beta = decoded_request(
        VIRTIO_9P_TOPEN,
        5,
        p9_open_payload(
            3,
            VIRTIO_9P_OPEN_READ_WRITE | VIRTIO_9P_OPEN_REMOVE_ON_CLOSE,
        ),
    );
    let open_beta_completion = device.execute_at(14, open_beta).unwrap();
    assert_eq!(open_beta_completion.message_type(), VIRTIO_9P_ROPEN);

    let clunk_beta = decoded_request(VIRTIO_9P_TCLUNK, 6, p9_clunk_payload(3));
    let clunk_completion = device.execute_at(15, clunk_beta).unwrap();
    assert_eq!(clunk_completion.message_type(), VIRTIO_9P_RCLUNK);

    let walk_removed = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(1, 4, &[b"beta.txt"]));
    let removed_completion = device.execute_at(16, walk_removed).unwrap();
    assert_eq!(removed_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(removed_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());

    let open_alpha = decoded_request(
        VIRTIO_9P_TOPEN,
        8,
        p9_open_payload(2, VIRTIO_9P_OPEN_READ_ONLY),
    );
    let open_alpha_completion = device.execute_at(17, open_alpha).unwrap();
    assert_eq!(open_alpha_completion.message_type(), VIRTIO_9P_ROPEN);

    let read_alpha = decoded_request(VIRTIO_9P_TREAD, 9, p9_read_payload(2, 0, 16));
    let read_alpha_completion = device.execute_at(18, read_alpha).unwrap();
    assert_eq!(read_alpha_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_alpha_completion.payload()), b"alpha");
}

#[test]
fn virtio_9p_legacy_open_truncate_rejects_non_writable_fids_without_clobbering() {
    let device = attached_device_with_file("legacy-read.txt", b"legacy-data");
    let walk = decoded_request(
        VIRTIO_9P_TWALK,
        2,
        p9_walk_payload(1, 2, &[b"legacy-read.txt"]),
    );
    device.execute_at(11, walk).unwrap();

    let read_only_truncate = decoded_request(
        VIRTIO_9P_TOPEN,
        3,
        p9_open_payload(2, VIRTIO_9P_OPEN_READ_ONLY | VIRTIO_9P_OPEN_TRUNCATE),
    );
    let read_only_completion = device.execute_at(12, read_only_truncate).unwrap();
    assert_eq!(read_only_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        read_only_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );

    let walk_check = decoded_request(
        VIRTIO_9P_TWALK,
        4,
        p9_walk_payload(1, 3, &[b"legacy-read.txt"]),
    );
    device.execute_at(13, walk_check).unwrap();
    let open_check = decoded_request(
        VIRTIO_9P_TOPEN,
        5,
        p9_open_payload(3, VIRTIO_9P_OPEN_READ_ONLY),
    );
    assert_eq!(
        device.execute_at(14, open_check).unwrap().message_type(),
        VIRTIO_9P_ROPEN
    );
    let read_check = decoded_request(VIRTIO_9P_TREAD, 6, p9_read_payload(3, 0, 16));
    let read_check_completion = device.execute_at(15, read_check).unwrap();
    assert_eq!(read_check_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(
        read_counted_data(read_check_completion.payload()),
        b"legacy-data"
    );
}

#[test]
fn virtio_9p_legacy_open_execute_only_denies_file_reads_and_writes() {
    let device = attached_device_with_file("legacy-exec.txt", b"run");
    let walk = decoded_request(
        VIRTIO_9P_TWALK,
        2,
        p9_walk_payload(1, 2, &[b"legacy-exec.txt"]),
    );
    device.execute_at(11, walk).unwrap();

    let open = decoded_request(
        VIRTIO_9P_TOPEN,
        3,
        p9_open_payload(2, VIRTIO_9P_OPEN_EXECUTE_ONLY),
    );
    assert_eq!(
        device.execute_at(12, open).unwrap().message_type(),
        VIRTIO_9P_ROPEN
    );

    let denied_read = decoded_request(VIRTIO_9P_TREAD, 4, p9_read_payload(2, 0, 16));
    let denied_read_completion = device.execute_at(13, denied_read).unwrap();
    assert_eq!(denied_read_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        denied_read_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );

    let denied_write = decoded_request(VIRTIO_9P_TWRITE, 5, p9_write_payload(2, 0, b"!"));
    let denied_write_completion = device.execute_at(14, denied_write).unwrap();
    assert_eq!(denied_write_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        denied_write_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
}

#[test]
fn virtio_9p_legacy_open_rejects_reopening_fids_without_changing_access_mode() {
    let device = attached_device_with_file("legacy-locked.txt", b"locked");
    let walk = decoded_request(
        VIRTIO_9P_TWALK,
        2,
        p9_walk_payload(1, 2, &[b"legacy-locked.txt"]),
    );
    device.execute_at(11, walk).unwrap();

    let open_read_only = decoded_request(
        VIRTIO_9P_TOPEN,
        3,
        p9_open_payload(2, VIRTIO_9P_OPEN_READ_ONLY),
    );
    assert_eq!(
        device
            .execute_at(12, open_read_only)
            .unwrap()
            .message_type(),
        VIRTIO_9P_ROPEN
    );

    let reopen_read_write = decoded_request(
        VIRTIO_9P_TOPEN,
        4,
        p9_open_payload(2, VIRTIO_9P_OPEN_READ_WRITE),
    );
    let reopen_completion = device.execute_at(13, reopen_read_write).unwrap();
    assert_eq!(reopen_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(reopen_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let denied_write = decoded_request(VIRTIO_9P_TWRITE, 5, p9_write_payload(2, 0, b"open"));
    let denied_write_completion = device.execute_at(14, denied_write).unwrap();
    assert_eq!(denied_write_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        denied_write_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
}
