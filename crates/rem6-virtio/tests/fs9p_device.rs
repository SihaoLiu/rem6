use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_DEFAULT_MSIZE, VIRTIO_9P_EBADF, VIRTIO_9P_ENOTSUP,
    VIRTIO_9P_NOFID, VIRTIO_9P_QTFILE, VIRTIO_9P_RCLUNK, VIRTIO_9P_RLERROR, VIRTIO_9P_RLOPEN,
    VIRTIO_9P_RREAD, VIRTIO_9P_RWALK, VIRTIO_9P_TATTACH, VIRTIO_9P_TCLUNK, VIRTIO_9P_TLOPEN,
    VIRTIO_9P_TREAD, VIRTIO_9P_TWALK,
};

mod support;

use support::fs9p::*;

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
fn virtio_9p_device_returns_lerror_for_unsupported_messages() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let request = decoded_request(200, 31, Vec::new());

    let completion = device.execute_at(66, request).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.tag(), 31);
    assert_eq!(completion.payload(), VIRTIO_9P_ENOTSUP.to_le_bytes());
    assert_eq!(device.completions(), vec![completion]);
}
