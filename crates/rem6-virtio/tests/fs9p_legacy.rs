use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_EBADF, VIRTIO_9P_ENOTSUP, VIRTIO_9P_NOFID,
    VIRTIO_9P_QTFILE, VIRTIO_9P_RLERROR, VIRTIO_9P_RSTAT, VIRTIO_9P_TATTACH, VIRTIO_9P_TAUTH,
    VIRTIO_9P_TSTAT, VIRTIO_9P_TWALK, VIRTIO_9P_TWSTAT,
};

mod support;

use support::fs9p::*;

#[test]
fn virtio_9p_device_reports_legacy_stat_for_walked_files() {
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
    let walk_completion = device.execute_at(11, walk).unwrap();
    let (_, _, walk_path) = read_qid(walk_completion.payload(), 2);

    let stat = decoded_request(VIRTIO_9P_TSTAT, 3, p9_legacy_stat_payload(2));
    let stat_completion = device.execute_at(12, stat).unwrap();
    assert_eq!(stat_completion.message_type(), VIRTIO_9P_RSTAT);
    let payload = stat_completion.payload();
    assert_eq!(read_u16(payload, 0) as usize, payload.len() - 2);
    assert_eq!(read_u16(payload, 2), 0);
    assert_eq!(read_u32(payload, 4), 0);
    let (qtype, qversion, qpath) = read_qid(payload, 8);
    assert_eq!(qtype, VIRTIO_9P_QTFILE);
    assert_eq!(qversion, 0);
    assert_eq!(qpath, walk_path);
    assert_eq!(read_u32(payload, 21), 0o100644);
    assert_eq!(read_u32(payload, 25), 0);
    assert_eq!(read_u32(payload, 29), 0);
    assert_eq!(read_u64(payload, 33), 5);

    let (name, next) = read_string_with_next(payload, 41);
    let (uid, next) = read_string_with_next(payload, next);
    let (gid, next) = read_string_with_next(payload, next);
    let (muid, next) = read_string_with_next(payload, next);
    assert_eq!(name, b"alpha.txt");
    assert_eq!(uid, b"0");
    assert_eq!(gid, b"0");
    assert_eq!(muid, b"");
    assert_eq!(next, payload.len());
}

#[test]
fn virtio_9p_device_rejects_legacy_stat_on_stale_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let stat = decoded_request(VIRTIO_9P_TSTAT, 1, p9_legacy_stat_payload(7));

    let completion = device.execute_at(10, stat).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_rejects_malformed_legacy_stat_payloads() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let stat = decoded_request(VIRTIO_9P_TSTAT, 1, vec![1, 2, 3]);

    assert!(device.execute_at(10, stat).is_err());
    assert!(device.completions().is_empty());
}

#[test]
fn virtio_9p_device_rejects_auth_after_parsing_payloads() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let auth = decoded_request(VIRTIO_9P_TAUTH, 1, p9_auth_payload(7, b"root", b"", 0));

    let completion = device.execute_at(10, auth).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.payload(), VIRTIO_9P_ENOTSUP.to_le_bytes());
    assert_eq!(device.fid_count(), 0);
    assert!(device.attached_fids().is_empty());
}

#[test]
fn virtio_9p_device_rejects_malformed_auth_payloads() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let auth = decoded_request(VIRTIO_9P_TAUTH, 1, vec![7, 0, 0]);

    assert!(device.execute_at(10, auth).is_err());
    assert!(device.completions().is_empty());
}

#[test]
fn virtio_9p_device_rejects_legacy_wstat_after_validating_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("wstat.txt", b"original".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"wstat.txt"]));
    device.execute_at(11, walk).unwrap();
    let stat = decoded_request(VIRTIO_9P_TSTAT, 3, p9_legacy_stat_payload(2));
    let stat_payload = device.execute_at(12, stat).unwrap().payload().to_vec();

    let wstat = decoded_request(
        VIRTIO_9P_TWSTAT,
        4,
        p9_legacy_wstat_payload(2, &stat_payload),
    );
    let completion = device.execute_at(13, wstat).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.payload(), VIRTIO_9P_ENOTSUP.to_le_bytes());
    let restat = decoded_request(VIRTIO_9P_TSTAT, 5, p9_legacy_stat_payload(2));
    let restat_payload = device.execute_at(14, restat).unwrap().payload().to_vec();
    assert_eq!(restat_payload, stat_payload);
}

#[test]
fn virtio_9p_device_rejects_legacy_wstat_on_stale_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let stat = [0_u8, 0];
    let wstat = decoded_request(VIRTIO_9P_TWSTAT, 1, p9_legacy_wstat_payload(7, &stat));

    let completion = device.execute_at(10, wstat).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_rejects_malformed_legacy_wstat_payloads() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let wstat = decoded_request(VIRTIO_9P_TWSTAT, 1, vec![1, 0, 0, 0, 4, 0, 1]);

    assert!(device.execute_at(10, wstat).is_err());
    assert!(device.completions().is_empty());
}

fn read_u16(payload: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(payload[offset..offset + 2].try_into().unwrap())
}

fn read_string_with_next(payload: &[u8], offset: usize) -> (&[u8], usize) {
    let len = usize::from(read_u16(payload, offset));
    let start = offset + 2;
    let end = start + len;
    (&payload[start..end], end)
}
