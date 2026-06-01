use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_EBADF, VIRTIO_9P_EEXIST, VIRTIO_9P_EINVAL,
    VIRTIO_9P_ENODATA, VIRTIO_9P_GETATTR_BASIC, VIRTIO_9P_LOPEN_APPEND, VIRTIO_9P_NOFID,
    VIRTIO_9P_OPEN_READ_WRITE, VIRTIO_9P_RCLUNK, VIRTIO_9P_RLERROR, VIRTIO_9P_RLOPEN,
    VIRTIO_9P_RREAD, VIRTIO_9P_RREMOVE, VIRTIO_9P_RWALK, VIRTIO_9P_RWRITE, VIRTIO_9P_RXATTRCREATE,
    VIRTIO_9P_RXATTRWALK, VIRTIO_9P_TATTACH, VIRTIO_9P_TCLUNK, VIRTIO_9P_TGETATTR,
    VIRTIO_9P_TLOPEN, VIRTIO_9P_TREAD, VIRTIO_9P_TREMOVE, VIRTIO_9P_TWALK, VIRTIO_9P_TWRITE,
    VIRTIO_9P_TXATTRCREATE, VIRTIO_9P_TXATTRWALK, VIRTIO_9P_XATTR_CREATE, VIRTIO_9P_XATTR_REPLACE,
};

mod support;

use support::fs9p::*;

#[test]
fn virtio_9p_device_rejects_xattrwalk_to_existing_newfid_without_rebinding_file() {
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
    assert_eq!(
        device.execute_at(11, walk_alpha).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 3, p9_lopen_payload(2, 0));
    assert_eq!(
        device.execute_at(12, open_alpha).unwrap().message_type(),
        VIRTIO_9P_RLOPEN
    );
    let walk_beta = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"beta.txt"]));
    assert_eq!(
        device.execute_at(13, walk_beta).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );

    let xattrwalk = decoded_request(VIRTIO_9P_TXATTRWALK, 5, p9_xattrwalk_payload(2, 3, b""));
    let xattrwalk_completion = device.execute_at(14, xattrwalk).unwrap();
    assert_eq!(xattrwalk_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        xattrwalk_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
    assert_eq!(device.fid_count(), 3);

    let open_beta = decoded_request(VIRTIO_9P_TLOPEN, 6, p9_lopen_payload(3, 0));
    assert_eq!(
        device.execute_at(15, open_beta).unwrap().message_type(),
        VIRTIO_9P_RLOPEN
    );
    let read_beta = decoded_request(VIRTIO_9P_TREAD, 7, p9_read_payload(3, 0, 16));
    let read_beta_completion = device.execute_at(16, read_beta).unwrap();
    assert_eq!(read_beta_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_beta_completion.payload()), b"beta");
}

#[test]
fn virtio_9p_device_rejects_xattrwalk_to_same_fid_without_rebinding_file() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    assert_eq!(
        device.execute_at(11, walk_alpha).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 3, p9_lopen_payload(2, 0));
    assert_eq!(
        device.execute_at(12, open_alpha).unwrap().message_type(),
        VIRTIO_9P_RLOPEN
    );

    let xattrwalk = decoded_request(VIRTIO_9P_TXATTRWALK, 4, p9_xattrwalk_payload(2, 2, b""));
    let xattrwalk_completion = device.execute_at(13, xattrwalk).unwrap();
    assert_eq!(xattrwalk_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        xattrwalk_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
    assert_eq!(device.fid_count(), 2);

    let read_alpha = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 16));
    let read_alpha_completion = device.execute_at(14, read_alpha).unwrap();
    assert_eq!(read_alpha_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_alpha_completion.payload()), b"alpha");
}

#[test]
fn virtio_9p_device_persists_created_xattrs_in_namespace() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let data_fid = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    assert_eq!(
        device.execute_at(11, data_fid).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );
    let xattr_fid = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 3, &[b"alpha.txt"]));
    assert_eq!(
        device.execute_at(12, xattr_fid).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );

    let create = decoded_request(
        VIRTIO_9P_TXATTRCREATE,
        4,
        p9_xattrcreate_payload(3, b"user.color", 4, 0),
    );
    let create_completion = device.execute_at(13, create).unwrap();
    assert_eq!(create_completion.message_type(), VIRTIO_9P_RXATTRCREATE);
    assert!(create_completion.payload().is_empty());

    let write = decoded_request(VIRTIO_9P_TWRITE, 5, p9_write_payload(3, 0, b"blue"));
    let write_completion = device.execute_at(14, write).unwrap();
    assert_eq!(write_completion.message_type(), VIRTIO_9P_RWRITE);
    assert_eq!(
        u32::from_le_bytes(write_completion.payload().try_into().unwrap()),
        4
    );

    let clunk = decoded_request(VIRTIO_9P_TCLUNK, 6, p9_clunk_payload(3));
    assert_eq!(
        device.execute_at(15, clunk).unwrap().message_type(),
        VIRTIO_9P_RCLUNK
    );

    let named_walk = decoded_request(
        VIRTIO_9P_TXATTRWALK,
        7,
        p9_xattrwalk_payload(2, 4, b"user.color"),
    );
    let named_walk_completion = device.execute_at(16, named_walk).unwrap();
    assert_eq!(named_walk_completion.message_type(), VIRTIO_9P_RXATTRWALK);
    assert_eq!(
        u64::from_le_bytes(named_walk_completion.payload().try_into().unwrap()),
        4
    );
    let named_read = decoded_request(VIRTIO_9P_TREAD, 8, p9_read_payload(4, 1, 2));
    let named_read_completion = device.execute_at(17, named_read).unwrap();
    assert_eq!(named_read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(named_read_completion.payload()), b"lu");

    let list_walk = decoded_request(VIRTIO_9P_TXATTRWALK, 9, p9_xattrwalk_payload(2, 5, b""));
    let list_walk_completion = device.execute_at(18, list_walk).unwrap();
    assert_eq!(list_walk_completion.message_type(), VIRTIO_9P_RXATTRWALK);
    assert_eq!(
        u64::from_le_bytes(list_walk_completion.payload().try_into().unwrap()),
        11
    );
    let list_read = decoded_request(VIRTIO_9P_TREAD, 10, p9_read_payload(5, 0, 32));
    let list_read_completion = device.execute_at(19, list_read).unwrap();
    assert_eq!(list_read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(
        read_counted_data(list_read_completion.payload()),
        b"user.color\0"
    );
}

#[test]
fn virtio_9p_append_open_does_not_affect_xattr_write_offsets() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let data_fid = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    assert_eq!(
        device.execute_at(11, data_fid).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );
    let append_open = decoded_request(
        VIRTIO_9P_TLOPEN,
        3,
        p9_lopen_payload(
            2,
            u32::from(VIRTIO_9P_OPEN_READ_WRITE) | VIRTIO_9P_LOPEN_APPEND,
        ),
    );
    assert_eq!(
        device.execute_at(12, append_open).unwrap().message_type(),
        VIRTIO_9P_RLOPEN
    );

    let create = decoded_request(
        VIRTIO_9P_TXATTRCREATE,
        4,
        p9_xattrcreate_payload(2, b"user.note", 4, 0),
    );
    assert_eq!(
        device.execute_at(13, create).unwrap().message_type(),
        VIRTIO_9P_RXATTRCREATE
    );

    let tail = decoded_request(VIRTIO_9P_TWRITE, 5, p9_write_payload(2, 2, b"il"));
    assert_eq!(
        device.execute_at(14, tail).unwrap().payload(),
        2_u32.to_le_bytes()
    );
    let head = decoded_request(VIRTIO_9P_TWRITE, 6, p9_write_payload(2, 0, b"sa"));
    assert_eq!(
        device.execute_at(15, head).unwrap().payload(),
        2_u32.to_le_bytes()
    );
    let clunk = decoded_request(VIRTIO_9P_TCLUNK, 7, p9_clunk_payload(2));
    assert_eq!(
        device.execute_at(16, clunk).unwrap().message_type(),
        VIRTIO_9P_RCLUNK
    );

    let data_fid = decoded_request(VIRTIO_9P_TWALK, 8, p9_walk_payload(1, 3, &[b"alpha.txt"]));
    assert_eq!(
        device.execute_at(17, data_fid).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );
    let xattrwalk = decoded_request(
        VIRTIO_9P_TXATTRWALK,
        9,
        p9_xattrwalk_payload(3, 4, b"user.note"),
    );
    assert_eq!(
        device.execute_at(18, xattrwalk).unwrap().message_type(),
        VIRTIO_9P_RXATTRWALK
    );
    let read = decoded_request(VIRTIO_9P_TREAD, 10, p9_read_payload(4, 0, 16));
    let read_completion = device.execute_at(19, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"sail");
}

#[test]
fn virtio_9p_device_honors_xattr_create_and_replace_flags() {
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
    assert_eq!(
        device.execute_at(11, walk).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );

    let create = decoded_request(
        VIRTIO_9P_TXATTRCREATE,
        3,
        p9_xattrcreate_payload(2, b"user.color", 4, VIRTIO_9P_XATTR_CREATE),
    );
    assert_eq!(
        device.execute_at(12, create).unwrap().message_type(),
        VIRTIO_9P_RXATTRCREATE
    );
    let write = decoded_request(VIRTIO_9P_TWRITE, 4, p9_write_payload(2, 0, b"blue"));
    assert_eq!(
        device.execute_at(13, write).unwrap().message_type(),
        VIRTIO_9P_RWRITE
    );
    let clunk = decoded_request(VIRTIO_9P_TCLUNK, 5, p9_clunk_payload(2));
    assert_eq!(
        device.execute_at(14, clunk).unwrap().message_type(),
        VIRTIO_9P_RCLUNK
    );

    let walk_again = decoded_request(VIRTIO_9P_TWALK, 6, p9_walk_payload(1, 3, &[b"alpha.txt"]));
    assert_eq!(
        device.execute_at(15, walk_again).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );
    let duplicate = decoded_request(
        VIRTIO_9P_TXATTRCREATE,
        7,
        p9_xattrcreate_payload(3, b"user.color", 5, VIRTIO_9P_XATTR_CREATE),
    );
    let duplicate_completion = device.execute_at(16, duplicate).unwrap();
    assert_eq!(duplicate_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        duplicate_completion.payload(),
        VIRTIO_9P_EEXIST.to_le_bytes()
    );

    let missing_replace = decoded_request(
        VIRTIO_9P_TXATTRCREATE,
        8,
        p9_xattrcreate_payload(3, b"user.missing", 3, VIRTIO_9P_XATTR_REPLACE),
    );
    let missing_replace_completion = device.execute_at(17, missing_replace).unwrap();
    assert_eq!(missing_replace_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        missing_replace_completion.payload(),
        VIRTIO_9P_ENODATA.to_le_bytes()
    );

    let replace = decoded_request(
        VIRTIO_9P_TXATTRCREATE,
        9,
        p9_xattrcreate_payload(3, b"user.color", 5, VIRTIO_9P_XATTR_REPLACE),
    );
    assert_eq!(
        device.execute_at(18, replace).unwrap().message_type(),
        VIRTIO_9P_RXATTRCREATE
    );
    let replace_write = decoded_request(VIRTIO_9P_TWRITE, 10, p9_write_payload(3, 0, b"green"));
    assert_eq!(
        device.execute_at(19, replace_write).unwrap().message_type(),
        VIRTIO_9P_RWRITE
    );
    let replace_clunk = decoded_request(VIRTIO_9P_TCLUNK, 11, p9_clunk_payload(3));
    assert_eq!(
        device.execute_at(20, replace_clunk).unwrap().message_type(),
        VIRTIO_9P_RCLUNK
    );

    let walk_for_read =
        decoded_request(VIRTIO_9P_TWALK, 12, p9_walk_payload(1, 4, &[b"alpha.txt"]));
    assert_eq!(
        device.execute_at(21, walk_for_read).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );
    let xattrwalk = decoded_request(
        VIRTIO_9P_TXATTRWALK,
        13,
        p9_xattrwalk_payload(4, 5, b"user.color"),
    );
    assert_eq!(
        device.execute_at(22, xattrwalk).unwrap().message_type(),
        VIRTIO_9P_RXATTRWALK
    );
    let read = decoded_request(VIRTIO_9P_TREAD, 14, p9_read_payload(5, 0, 16));
    let read_completion = device.execute_at(23, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"green");
}

#[test]
fn virtio_9p_device_rejects_invalid_xattrcreate_flags_without_rebinding_fid() {
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
    assert_eq!(
        device.execute_at(11, walk).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );
    let open = decoded_request(VIRTIO_9P_TLOPEN, 3, p9_lopen_payload(2, 0));
    assert_eq!(
        device.execute_at(12, open).unwrap().message_type(),
        VIRTIO_9P_RLOPEN
    );

    let invalid = decoded_request(
        VIRTIO_9P_TXATTRCREATE,
        4,
        p9_xattrcreate_payload(
            2,
            b"user.color",
            4,
            VIRTIO_9P_XATTR_CREATE | VIRTIO_9P_XATTR_REPLACE,
        ),
    );
    let invalid_completion = device.execute_at(13, invalid).unwrap();
    assert_eq!(invalid_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(invalid_completion.payload(), VIRTIO_9P_EINVAL.to_le_bytes());

    let read = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(14, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"alpha");
}

#[test]
fn virtio_9p_device_remove_clunks_pending_xattr_write_without_committing() {
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
    assert_eq!(
        device.execute_at(11, walk).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );
    let create = decoded_request(
        VIRTIO_9P_TXATTRCREATE,
        3,
        p9_xattrcreate_payload(2, b"user.note", 4, 0),
    );
    assert_eq!(
        device.execute_at(12, create).unwrap().message_type(),
        VIRTIO_9P_RXATTRCREATE
    );
    let write = decoded_request(VIRTIO_9P_TWRITE, 4, p9_write_payload(2, 0, b"blue"));
    assert_eq!(
        device.execute_at(13, write).unwrap().message_type(),
        VIRTIO_9P_RWRITE
    );

    let remove = decoded_request(VIRTIO_9P_TREMOVE, 5, p9_remove_payload(2));
    let remove_completion = device.execute_at(14, remove).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(remove_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
    assert_eq!(device.fid_count(), 1);

    let clunk_removed = decoded_request(VIRTIO_9P_TCLUNK, 6, p9_clunk_payload(2));
    let clunk_completion = device.execute_at(15, clunk_removed).unwrap();
    assert_eq!(clunk_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(clunk_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let walk_again = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(1, 3, &[b"alpha.txt"]));
    assert_eq!(
        device.execute_at(16, walk_again).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );
    let xattrwalk = decoded_request(
        VIRTIO_9P_TXATTRWALK,
        8,
        p9_xattrwalk_payload(3, 4, b"user.note"),
    );
    let xattrwalk_completion = device.execute_at(17, xattrwalk).unwrap();
    assert_eq!(xattrwalk_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        xattrwalk_completion.payload(),
        VIRTIO_9P_ENODATA.to_le_bytes()
    );
}

#[test]
fn virtio_9p_device_remove_drops_pending_xattr_write_for_deleted_file() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let xattr_fid = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    assert_eq!(
        device.execute_at(11, xattr_fid).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );
    let remove_fid = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 3, &[b"alpha.txt"]));
    assert_eq!(
        device.execute_at(12, remove_fid).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );
    let create = decoded_request(
        VIRTIO_9P_TXATTRCREATE,
        4,
        p9_xattrcreate_payload(2, b"user.note", 4, 0),
    );
    assert_eq!(
        device.execute_at(13, create).unwrap().message_type(),
        VIRTIO_9P_RXATTRCREATE
    );
    let write = decoded_request(VIRTIO_9P_TWRITE, 5, p9_write_payload(2, 0, b"blue"));
    assert_eq!(
        device.execute_at(14, write).unwrap().message_type(),
        VIRTIO_9P_RWRITE
    );

    let remove = decoded_request(VIRTIO_9P_TREMOVE, 6, p9_remove_payload(3));
    assert_eq!(
        device.execute_at(15, remove).unwrap().message_type(),
        VIRTIO_9P_RREMOVE
    );
    assert_eq!(device.fid_count(), 1);

    let stale_write = decoded_request(VIRTIO_9P_TWRITE, 7, p9_write_payload(2, 0, b"gray"));
    let stale_write_completion = device.execute_at(16, stale_write).unwrap();
    assert_eq!(stale_write_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        stale_write_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
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
fn virtio_9p_device_accepts_slashes_in_xattr_names() {
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

    let create = decoded_request(
        VIRTIO_9P_TXATTRCREATE,
        3,
        p9_xattrcreate_payload(2, b"user.attr/with/slash", 5, 0),
    );
    let create_completion = device.execute_at(12, create).unwrap();
    assert_eq!(create_completion.message_type(), VIRTIO_9P_RXATTRCREATE);
    assert!(create_completion.payload().is_empty());

    let write = decoded_request(VIRTIO_9P_TWRITE, 4, p9_write_payload(2, 0, b"value"));
    let write_completion = device.execute_at(13, write).unwrap();
    assert_eq!(write_completion.message_type(), VIRTIO_9P_RWRITE);
    assert_eq!(write_completion.payload(), 5_u32.to_le_bytes());

    let clunk = decoded_request(VIRTIO_9P_TCLUNK, 5, p9_clunk_payload(2));
    let clunk_completion = device.execute_at(14, clunk).unwrap();
    assert_eq!(clunk_completion.message_type(), VIRTIO_9P_RCLUNK);

    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        6,
        p9_attach_payload(3, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(15, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(3, 4, &[b"alpha.txt"]));
    device.execute_at(16, walk).unwrap();
    let xattrwalk = decoded_request(
        VIRTIO_9P_TXATTRWALK,
        8,
        p9_xattrwalk_payload(4, 5, b"user.attr/with/slash"),
    );
    let xattrwalk_completion = device.execute_at(17, xattrwalk).unwrap();
    assert_eq!(xattrwalk_completion.message_type(), VIRTIO_9P_RXATTRWALK);
    assert_eq!(xattrwalk_completion.payload(), 5_u64.to_le_bytes());

    let read = decoded_request(VIRTIO_9P_TREAD, 9, p9_read_payload(5, 0, 16));
    let read_completion = device.execute_at(18, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"value");
}
