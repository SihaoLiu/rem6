use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_EBADF, VIRTIO_9P_NOFID, VIRTIO_9P_RLERROR,
    VIRTIO_9P_RLOPEN, VIRTIO_9P_RREAD, VIRTIO_9P_RWALK, VIRTIO_9P_TATTACH, VIRTIO_9P_TLOPEN,
    VIRTIO_9P_TREAD, VIRTIO_9P_TWALK, VIRTIO_9P_TXATTRWALK,
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
