use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_DEFAULT_MSIZE, VIRTIO_9P_EBADF, VIRTIO_9P_EEXIST,
    VIRTIO_9P_NOFID, VIRTIO_9P_QTFILE, VIRTIO_9P_RCREATE, VIRTIO_9P_RLCREATE, VIRTIO_9P_RLERROR,
    VIRTIO_9P_RLOPEN, VIRTIO_9P_RREAD, VIRTIO_9P_RWALK, VIRTIO_9P_RWRITE, VIRTIO_9P_TATTACH,
    VIRTIO_9P_TCREATE, VIRTIO_9P_TLCREATE, VIRTIO_9P_TLOPEN, VIRTIO_9P_TREAD, VIRTIO_9P_TWALK,
    VIRTIO_9P_TWRITE,
};

mod support;

use support::fs9p::*;

#[test]
fn virtio_9p_device_supports_legacy_create_for_attached_directories() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let create = decoded_request(
        VIRTIO_9P_TCREATE,
        2,
        p9_create_payload(1, b"legacy-new.txt", 0o100644, 0),
    );
    let create_completion = device.execute_at(11, create).unwrap();
    assert_eq!(create_completion.message_type(), VIRTIO_9P_RCREATE);
    let (create_qtype, create_version, create_path) = read_qid(create_completion.payload(), 0);
    assert_eq!(create_qtype, VIRTIO_9P_QTFILE);
    assert_eq!(create_version, 0);
    assert_ne!(create_path, 1);
    assert_eq!(
        create_completion.payload()[13..17],
        VIRTIO_9P_DEFAULT_MSIZE.to_le_bytes()
    );

    let write = decoded_request(VIRTIO_9P_TWRITE, 3, p9_write_payload(1, 0, b"created"));
    let write_completion = device.execute_at(12, write).unwrap();
    assert_eq!(write_completion.message_type(), VIRTIO_9P_RWRITE);
    assert_eq!(write_completion.payload(), 7_u32.to_le_bytes());

    let read = decoded_request(VIRTIO_9P_TREAD, 4, p9_read_payload(1, 0, 16));
    let read_completion = device.execute_at(13, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"created");
}

#[test]
fn virtio_9p_device_rejects_legacy_create_duplicate_files_without_clobbering() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let create = decoded_request(
        VIRTIO_9P_TCREATE,
        2,
        p9_create_payload(1, b"alpha.txt", 0o100644, 0),
    );
    let create_completion = device.execute_at(11, create).unwrap();
    assert_eq!(create_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(create_completion.payload(), VIRTIO_9P_EEXIST.to_le_bytes());

    let walk = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    assert_eq!(
        device.execute_at(12, walk).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );
    let open = decoded_request(VIRTIO_9P_TLOPEN, 4, p9_lopen_payload(2, 0));
    assert_eq!(
        device.execute_at(13, open).unwrap().message_type(),
        VIRTIO_9P_RLOPEN
    );
    let read = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(14, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"alpha");
}

#[test]
fn virtio_9p_device_rejects_legacy_create_on_stale_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let create = decoded_request(
        VIRTIO_9P_TCREATE,
        1,
        p9_create_payload(7, b"legacy-new.txt", 0o100644, 0),
    );

    let completion = device.execute_at(10, create).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
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
fn virtio_9p_device_rejects_lcreate_duplicate_files_without_clobbering() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("note.txt", b"existing".to_vec())
        .unwrap();
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
    assert_eq!(create_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(create_completion.payload(), VIRTIO_9P_EEXIST.to_le_bytes());

    let walk = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"note.txt"]));
    assert_eq!(
        device.execute_at(12, walk).unwrap().message_type(),
        VIRTIO_9P_RWALK
    );
    let open = decoded_request(VIRTIO_9P_TLOPEN, 4, p9_lopen_payload(2, 0));
    assert_eq!(
        device.execute_at(13, open).unwrap().message_type(),
        VIRTIO_9P_RLOPEN
    );
    let read = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(14, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"existing");
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
