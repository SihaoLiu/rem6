use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_EBADF, VIRTIO_9P_NOFID, VIRTIO_9P_RFLUSH,
    VIRTIO_9P_RFSYNC, VIRTIO_9P_RLERROR, VIRTIO_9P_RREAD, VIRTIO_9P_TATTACH, VIRTIO_9P_TFLUSH,
    VIRTIO_9P_TFSYNC, VIRTIO_9P_TLOPEN, VIRTIO_9P_TREAD, VIRTIO_9P_TWALK,
};

mod support;

use support::fs9p::*;

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
