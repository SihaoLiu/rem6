use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_EBADF, VIRTIO_9P_ENOENT, VIRTIO_9P_LOPEN_APPEND,
    VIRTIO_9P_LOPEN_TRUNCATE, VIRTIO_9P_NOFID, VIRTIO_9P_OPEN_APPEND, VIRTIO_9P_OPEN_EXECUTE_ONLY,
    VIRTIO_9P_OPEN_READ_ONLY, VIRTIO_9P_OPEN_READ_WRITE, VIRTIO_9P_OPEN_REMOVE_ON_CLOSE,
    VIRTIO_9P_OPEN_TRUNCATE, VIRTIO_9P_OPEN_WRITE_ONLY, VIRTIO_9P_RCLUNK, VIRTIO_9P_RLERROR,
    VIRTIO_9P_RLOPEN, VIRTIO_9P_ROPEN, VIRTIO_9P_RREAD, VIRTIO_9P_RREADDIR, VIRTIO_9P_RWRITE,
    VIRTIO_9P_TATTACH, VIRTIO_9P_TCLUNK, VIRTIO_9P_TLOPEN, VIRTIO_9P_TOPEN, VIRTIO_9P_TREAD,
    VIRTIO_9P_TREADDIR, VIRTIO_9P_TWALK, VIRTIO_9P_TWRITE,
};

mod support;

use support::fs9p::*;

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
fn virtio_9p_open_rejects_write_only_directories_without_opening_fids() {
    for (message_type, reply_type, write_only_payload, read_only_payload) in [
        (
            VIRTIO_9P_TLOPEN,
            VIRTIO_9P_RLOPEN,
            p9_lopen_payload(1, u32::from(VIRTIO_9P_OPEN_WRITE_ONLY)),
            p9_lopen_payload(1, u32::from(VIRTIO_9P_OPEN_READ_ONLY)),
        ),
        (
            VIRTIO_9P_TOPEN,
            VIRTIO_9P_ROPEN,
            p9_open_payload(1, VIRTIO_9P_OPEN_WRITE_ONLY),
            p9_open_payload(1, VIRTIO_9P_OPEN_READ_ONLY),
        ),
    ] {
        let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
            .with_file("alpha.txt", b"alpha".to_vec())
            .unwrap();
        let attach = decoded_request(
            VIRTIO_9P_TATTACH,
            1,
            p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
        );
        device.execute_at(10, attach).unwrap();

        let write_only = decoded_request(message_type, 2, write_only_payload);
        let write_only_completion = device.execute_at(11, write_only).unwrap();
        assert_eq!(write_only_completion.message_type(), VIRTIO_9P_RLERROR);
        assert_eq!(
            write_only_completion.payload(),
            VIRTIO_9P_EBADF.to_le_bytes()
        );

        let read_only = decoded_request(message_type, 3, read_only_payload);
        let read_only_completion = device.execute_at(12, read_only).unwrap();
        assert_eq!(read_only_completion.message_type(), reply_type);

        let readdir = decoded_request(VIRTIO_9P_TREADDIR, 4, p9_readdir_payload(1, 0, 512));
        let readdir_completion = device.execute_at(13, readdir).unwrap();
        assert_eq!(readdir_completion.message_type(), VIRTIO_9P_RREADDIR);
        let entries = read_dir_entries(readdir_completion.payload());
        assert!(entries.iter().any(|entry| entry.name == "alpha.txt"));
    }
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
