use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VirtioError, VIRTIO_9P_DEFAULT_MSIZE, VIRTIO_9P_EBADF,
    VIRTIO_9P_NOFID, VIRTIO_9P_PROTOCOL_VERSION, VIRTIO_9P_RLERROR, VIRTIO_9P_RLOPEN,
    VIRTIO_9P_RREAD, VIRTIO_9P_RREADDIR, VIRTIO_9P_RVERSION, VIRTIO_9P_TATTACH, VIRTIO_9P_TLOPEN,
    VIRTIO_9P_TREAD, VIRTIO_9P_TREADDIR, VIRTIO_9P_TVERSION, VIRTIO_9P_TWALK,
};

mod support;

use support::fs9p::*;

#[test]
fn virtio_9p_device_negotiates_version_and_records_completion() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let request = decoded_request(
        VIRTIO_9P_TVERSION,
        11,
        p9_version_payload(16384, VIRTIO_9P_PROTOCOL_VERSION),
    );

    let completion = device.execute_at(44, request).unwrap();

    assert_eq!(completion.tick(), 44);
    assert_eq!(completion.message_type(), VIRTIO_9P_RVERSION);
    assert_eq!(completion.tag(), 11);
    assert_eq!(
        completion.payload(),
        p9_version_payload(VIRTIO_9P_DEFAULT_MSIZE, VIRTIO_9P_PROTOCOL_VERSION)
    );
    assert_eq!(device.completions(), vec![completion]);
}

#[test]
fn virtio_9p_device_applies_negotiated_msize_to_io_unit_replies() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("small.txt", b"bounded".to_vec())
        .unwrap();
    let version = decoded_request(
        VIRTIO_9P_TVERSION,
        1,
        p9_version_payload(4096, VIRTIO_9P_PROTOCOL_VERSION),
    );
    let version_completion = device.execute_at(10, version).unwrap();
    assert_eq!(version_completion.message_type(), VIRTIO_9P_RVERSION);
    assert_eq!(version_completion.payload()[0..4], 4096_u32.to_le_bytes());

    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        2,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(11, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"small.txt"]));
    device.execute_at(12, walk).unwrap();
    let open = decoded_request(VIRTIO_9P_TLOPEN, 4, p9_lopen_payload(2, 0));
    let open_completion = device.execute_at(13, open).unwrap();

    assert_eq!(open_completion.message_type(), VIRTIO_9P_RLOPEN);
    assert_eq!(open_completion.payload()[13..17], 4096_u32.to_le_bytes());
}

#[test]
fn virtio_9p_device_resets_session_state_on_version_negotiation() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("reset.txt", b"resettable".to_vec())
        .unwrap();
    let version = decoded_request(
        VIRTIO_9P_TVERSION,
        1,
        p9_version_payload(4096, VIRTIO_9P_PROTOCOL_VERSION),
    );
    device.execute_at(10, version).unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        2,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(11, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"reset.txt"]));
    device.execute_at(12, walk).unwrap();
    let open = decoded_request(VIRTIO_9P_TLOPEN, 4, p9_lopen_payload(2, 0));
    device.execute_at(13, open).unwrap();
    assert_eq!(device.fid_count(), 2);
    assert_eq!(device.attached_fids().len(), 1);

    let renegotiate = decoded_request(
        VIRTIO_9P_TVERSION,
        5,
        p9_version_payload(128, VIRTIO_9P_PROTOCOL_VERSION),
    );
    let version_completion = device.execute_at(14, renegotiate).unwrap();
    assert_eq!(version_completion.message_type(), VIRTIO_9P_RVERSION);
    assert_eq!(version_completion.payload()[0..4], 128_u32.to_le_bytes());
    assert_eq!(device.fid_count(), 0);
    assert!(device.attached_fids().is_empty());

    let stale_read = decoded_request(VIRTIO_9P_TREAD, 6, p9_read_payload(2, 0, 8));
    let stale_completion = device.execute_at(15, stale_read).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let reattach = decoded_request(
        VIRTIO_9P_TATTACH,
        7,
        p9_attach_payload(3, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(16, reattach).unwrap();
    let open_root = decoded_request(VIRTIO_9P_TLOPEN, 8, p9_lopen_payload(3, 0));
    let open_completion = device.execute_at(17, open_root).unwrap();
    assert_eq!(open_completion.payload()[13..17], 128_u32.to_le_bytes());
}

#[test]
fn virtio_9p_device_limits_read_replies_to_negotiated_msize() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("blob.txt", b"abcdefghijklmnopqrstuvwxyz0123456789".to_vec())
        .unwrap();
    let version = decoded_request(
        VIRTIO_9P_TVERSION,
        1,
        p9_version_payload(32, VIRTIO_9P_PROTOCOL_VERSION),
    );
    device.execute_at(10, version).unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        2,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(11, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"blob.txt"]));
    device.execute_at(12, walk).unwrap();
    let open = decoded_request(VIRTIO_9P_TLOPEN, 4, p9_lopen_payload(2, 0));
    device.execute_at(13, open).unwrap();

    let read = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 128));
    let read_completion = device.execute_at(14, read).unwrap();

    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(
        read_counted_data(read_completion.payload()),
        b"abcdefghijklmnopqrstu"
    );
    assert!(7 + read_completion.payload().len() <= 32);
}

#[test]
fn virtio_9p_device_limits_readdir_replies_to_negotiated_msize() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let version = decoded_request(
        VIRTIO_9P_TVERSION,
        1,
        p9_version_payload(64, VIRTIO_9P_PROTOCOL_VERSION),
    );
    device.execute_at(10, version).unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        2,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(11, attach).unwrap();
    let open = decoded_request(VIRTIO_9P_TLOPEN, 3, p9_lopen_payload(1, 0));
    device.execute_at(12, open).unwrap();

    let readdir = decoded_request(VIRTIO_9P_TREADDIR, 4, p9_readdir_payload(1, 0, 512));
    let readdir_completion = device.execute_at(13, readdir).unwrap();
    let entries = read_dir_entries(readdir_completion.payload());
    let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();

    assert_eq!(readdir_completion.message_type(), VIRTIO_9P_RREADDIR);
    assert_eq!(names, [".", ".."]);
    assert!(7 + readdir_completion.payload().len() <= 64);
}

#[test]
fn virtio_9p_device_rejects_malformed_version_payloads_as_typed_errors() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let malformed_version = decoded_request(VIRTIO_9P_TVERSION, 1, vec![1, 2, 3]);

    assert!(matches!(
        device.execute_at(77, malformed_version),
        Err(VirtioError::InvalidVirtio9pPayload {
            message_type: VIRTIO_9P_TVERSION,
            bytes: 3
        })
    ));
    assert!(device.completions().is_empty());
}
