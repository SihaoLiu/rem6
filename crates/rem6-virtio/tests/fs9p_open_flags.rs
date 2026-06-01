use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_LOPEN_APPEND, VIRTIO_9P_LOPEN_TRUNCATE,
    VIRTIO_9P_NOFID, VIRTIO_9P_OPEN_READ_WRITE, VIRTIO_9P_OPEN_TRUNCATE, VIRTIO_9P_RLOPEN,
    VIRTIO_9P_ROPEN, VIRTIO_9P_RREAD, VIRTIO_9P_RWRITE, VIRTIO_9P_TATTACH, VIRTIO_9P_TLOPEN,
    VIRTIO_9P_TOPEN, VIRTIO_9P_TREAD, VIRTIO_9P_TWALK, VIRTIO_9P_TWRITE,
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
