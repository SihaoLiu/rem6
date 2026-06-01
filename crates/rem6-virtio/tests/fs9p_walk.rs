use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VirtioError, VIRTIO_9P_EBADF, VIRTIO_9P_ENOENT,
    VIRTIO_9P_NAME_MAX, VIRTIO_9P_NOFID, VIRTIO_9P_RLERROR, VIRTIO_9P_RLOPEN, VIRTIO_9P_RMKDIR,
    VIRTIO_9P_RREAD, VIRTIO_9P_RWALK, VIRTIO_9P_TATTACH, VIRTIO_9P_TLOPEN, VIRTIO_9P_TMKDIR,
    VIRTIO_9P_TREAD, VIRTIO_9P_TWALK,
};

mod support;

use support::fs9p::*;

#[test]
fn virtio_9p_device_rejects_walk_to_existing_newfid_without_rebinding_fids() {
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
    assert_eq!(alpha_completion.message_type(), VIRTIO_9P_RWALK);
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 3, p9_lopen_payload(2, 0));
    let open_alpha_completion = device.execute_at(12, open_alpha).unwrap();
    assert_eq!(open_alpha_completion.message_type(), VIRTIO_9P_RLOPEN);

    let walk_beta = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"beta.txt"]));
    let beta_completion = device.execute_at(13, walk_beta).unwrap();
    assert_eq!(beta_completion.message_type(), VIRTIO_9P_RWALK);

    let occupied_walk = decoded_request(VIRTIO_9P_TWALK, 5, p9_walk_payload(1, 2, &[b"beta.txt"]));
    let occupied_completion = device.execute_at(14, occupied_walk).unwrap();
    assert_eq!(occupied_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(occupied_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
    assert_eq!(device.fid_count(), 3);

    let read_alpha = decoded_request(VIRTIO_9P_TREAD, 6, p9_read_payload(2, 0, 16));
    let read_alpha_completion = device.execute_at(15, read_alpha).unwrap();
    assert_eq!(read_alpha_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_alpha_completion.payload()), b"alpha");
}

#[test]
fn virtio_9p_device_rejects_non_empty_walk_to_same_fid_without_rebinding_root() {
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

    let same_fid_walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 1, &[b"alpha.txt"]));
    let same_fid_completion = device.execute_at(11, same_fid_walk).unwrap();
    assert_eq!(same_fid_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(same_fid_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
    assert_eq!(device.fid_count(), 1);

    let walk_beta = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"beta.txt"]));
    let beta_completion = device.execute_at(12, walk_beta).unwrap();
    assert_eq!(beta_completion.message_type(), VIRTIO_9P_RWALK);
}

#[test]
fn virtio_9p_device_allows_empty_walk_to_same_fid_without_extra_fid() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let same_fid_walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 1, &[]));
    let same_fid_completion = device.execute_at(11, same_fid_walk).unwrap();
    assert_eq!(same_fid_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(same_fid_completion.payload(), 0_u16.to_le_bytes());
    assert_eq!(device.fid_count(), 1);
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
fn virtio_9p_device_rejects_walk_from_open_file_fids_without_binding_newfid() {
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
    let walk_completion = device.execute_at(11, walk_alpha).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 3, p9_lopen_payload(2, 0));
    let open_completion = device.execute_at(12, open_alpha).unwrap();
    assert_eq!(open_completion.message_type(), VIRTIO_9P_RLOPEN);

    let open_walk = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(2, 3, &[]));
    let open_walk_completion = device.execute_at(13, open_walk).unwrap();
    assert_eq!(open_walk_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        open_walk_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
    assert_eq!(device.fid_count(), 2);

    let open_newfid = decoded_request(VIRTIO_9P_TLOPEN, 5, p9_lopen_payload(3, 0));
    let newfid_completion = device.execute_at(14, open_newfid).unwrap();
    assert_eq!(newfid_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(newfid_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let read_alpha = decoded_request(VIRTIO_9P_TREAD, 6, p9_read_payload(2, 0, 16));
    let read_alpha_completion = device.execute_at(15, read_alpha).unwrap();
    assert_eq!(read_alpha_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_alpha_completion.payload()), b"alpha");
}

#[test]
fn virtio_9p_device_walks_dot_and_dotdot_directory_components() {
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
    let mkdir_completion = device.execute_at(11, mkdir).unwrap();
    assert_eq!(mkdir_completion.message_type(), VIRTIO_9P_RMKDIR);
    let (tmp_qtype, _, tmp_qpath) = read_qid(mkdir_completion.payload(), 0);

    let self_walk = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"tmp", b"."]));
    let self_completion = device.execute_at(12, self_walk).unwrap();
    assert_eq!(self_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(self_completion.payload()[0..2], 2_u16.to_le_bytes());
    assert_eq!(
        read_qid(self_completion.payload(), 2),
        (tmp_qtype, 0, tmp_qpath)
    );
    assert_eq!(
        read_qid(self_completion.payload(), 15),
        (tmp_qtype, 0, tmp_qpath)
    );

    let parent_walk = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"tmp", b".."]));
    let parent_completion = device.execute_at(13, parent_walk).unwrap();
    assert_eq!(parent_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(parent_completion.payload()[0..2], 2_u16.to_le_bytes());
    assert_eq!(
        read_qid(parent_completion.payload(), 2),
        (tmp_qtype, 0, tmp_qpath)
    );
    assert_eq!(read_qid(parent_completion.payload(), 15).2, 1);

    let root_parent = decoded_request(VIRTIO_9P_TWALK, 5, p9_walk_payload(1, 4, &[b".."]));
    let root_completion = device.execute_at(14, root_parent).unwrap();
    assert_eq!(root_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(root_completion.payload()[0..2], 1_u16.to_le_bytes());
    assert_eq!(read_qid(root_completion.payload(), 2).2, 1);
}

#[test]
fn virtio_9p_device_returns_partial_walk_without_binding_newfid() {
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
    let mkdir_completion = device.execute_at(11, mkdir).unwrap();
    assert_eq!(mkdir_completion.message_type(), VIRTIO_9P_RMKDIR);
    let tmp_qid = read_qid(mkdir_completion.payload(), 0);

    let partial_walk = decoded_request(
        VIRTIO_9P_TWALK,
        3,
        p9_walk_payload(1, 2, &[b"tmp", b"missing"]),
    );
    let partial_completion = device.execute_at(12, partial_walk).unwrap();
    assert_eq!(partial_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(partial_completion.payload()[0..2], 1_u16.to_le_bytes());
    assert_eq!(read_qid(partial_completion.payload(), 2), tmp_qid);
    assert_eq!(device.fid_count(), 1);

    let open_partial_fid = decoded_request(VIRTIO_9P_TLOPEN, 4, p9_lopen_payload(2, 0));
    let open_completion = device.execute_at(13, open_partial_fid).unwrap();
    assert_eq!(open_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(open_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_rejects_walk_with_too_many_name_elements() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let names = vec![b"." as &[u8]; 17];
    let payload = p9_walk_payload(1, 2, &names);
    let payload_len = payload.len();
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, payload);

    assert!(matches!(
        device.execute_at(11, walk),
        Err(VirtioError::InvalidVirtio9pPayload {
            message_type: VIRTIO_9P_TWALK,
            bytes
        }) if bytes == payload_len
    ));
    assert_eq!(device.fid_count(), 1);
    assert_eq!(device.completions().len(), 1);
}

#[test]
fn virtio_9p_device_rejects_walk_names_longer_than_statfs_limit() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let oversized_name = vec![b'a'; VIRTIO_9P_NAME_MAX as usize + 1];
    let walk = decoded_request(
        VIRTIO_9P_TWALK,
        2,
        p9_walk_payload(1, 2, &[&oversized_name]),
    );

    assert!(matches!(
        device.execute_at(11, walk),
        Err(VirtioError::InvalidVirtio9pPayload {
            message_type: VIRTIO_9P_TWALK,
            bytes
        }) if bytes == oversized_name.len()
    ));
    assert_eq!(device.fid_count(), 1);
    assert_eq!(device.completions().len(), 1);
}

#[test]
fn virtio_9p_device_accepts_maximum_walk_name_elements() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let names = vec![b"." as &[u8]; 16];
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &names));
    let completion = device.execute_at(11, walk).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(completion.payload()[0..2], 16_u16.to_le_bytes());
    assert_eq!(read_qid(completion.payload(), 2).2, 1);
    assert_eq!(read_qid(completion.payload(), 2 + 15 * 13).2, 1);
    assert_eq!(device.fid_count(), 2);
}
