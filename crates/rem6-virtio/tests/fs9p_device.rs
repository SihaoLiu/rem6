use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, Virtio9pRequest, VirtioError, VirtioQueueIndex,
    VirtioSplitDescriptor, VirtioSplitDescriptorChain, VIRTIO_9P_DEFAULT_MSIZE, VIRTIO_9P_EBADF,
    VIRTIO_9P_ENOENT, VIRTIO_9P_ENOTSUP, VIRTIO_9P_NOFID, VIRTIO_9P_PROTOCOL_VERSION,
    VIRTIO_9P_QTDIR, VIRTIO_9P_QTFILE, VIRTIO_9P_RATTACH, VIRTIO_9P_RCLUNK, VIRTIO_9P_RLERROR,
    VIRTIO_9P_RLOPEN, VIRTIO_9P_RREAD, VIRTIO_9P_RVERSION, VIRTIO_9P_RWALK, VIRTIO_9P_TATTACH,
    VIRTIO_9P_TCLUNK, VIRTIO_9P_TLOPEN, VIRTIO_9P_TREAD, VIRTIO_9P_TVERSION, VIRTIO_9P_TWALK,
};

fn queue(index: u16) -> VirtioQueueIndex {
    VirtioQueueIndex::new(index).unwrap()
}

fn p9_string(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend((bytes.len() as u16).to_le_bytes());
    output.extend_from_slice(bytes);
    output
}

fn p9_message(message_type: u8, tag: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend((7_u32 + payload.len() as u32).to_le_bytes());
    bytes.push(message_type);
    bytes.extend(tag.to_le_bytes());
    bytes.extend(payload);
    bytes
}

fn decoded_request(message_type: u8, tag: u16, payload: Vec<u8>) -> Virtio9pRequest {
    let chain = VirtioSplitDescriptorChain::new(
        3,
        [
            VirtioSplitDescriptor::device_readable(
                3,
                p9_message(message_type, tag, &payload),
                Some(4),
            ),
            VirtioSplitDescriptor::device_writable(4, 128, None),
        ],
    )
    .unwrap();
    chain.decode_9p_request(queue(0)).unwrap().into_request()
}

fn p9_version_payload(msize: u32, version: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(msize.to_le_bytes());
    payload.extend(p9_string(version));
    payload
}

fn p9_attach_payload(fid: u32, afid: u32, uname: &[u8], aname: &[u8], n_uname: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(afid.to_le_bytes());
    payload.extend(p9_string(uname));
    payload.extend(p9_string(aname));
    payload.extend(n_uname.to_le_bytes());
    payload
}

fn p9_walk_payload(fid: u32, newfid: u32, names: &[&[u8]]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(newfid.to_le_bytes());
    payload.extend((names.len() as u16).to_le_bytes());
    for name in names {
        payload.extend(p9_string(name));
    }
    payload
}

fn p9_lopen_payload(fid: u32, flags: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(flags.to_le_bytes());
    payload
}

fn p9_read_payload(fid: u32, offset: u64, count: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(fid.to_le_bytes());
    payload.extend(offset.to_le_bytes());
    payload.extend(count.to_le_bytes());
    payload
}

fn p9_clunk_payload(fid: u32) -> Vec<u8> {
    fid.to_le_bytes().to_vec()
}

fn read_qid(payload: &[u8], offset: usize) -> (u8, u32, u64) {
    let qtype = payload[offset];
    let version = u32::from_le_bytes(payload[offset + 1..offset + 5].try_into().unwrap());
    let path = u64::from_le_bytes(payload[offset + 5..offset + 13].try_into().unwrap());
    (qtype, version, path)
}

fn read_counted_data(payload: &[u8]) -> &[u8] {
    let count = u32::from_le_bytes(payload[0..4].try_into().unwrap()) as usize;
    &payload[4..4 + count]
}

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
fn virtio_9p_device_accepts_attach_and_returns_root_qid() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let request = decoded_request(
        VIRTIO_9P_TATTACH,
        21,
        p9_attach_payload(42, VIRTIO_9P_NOFID, b"root", b"", 0),
    );

    let completion = device.execute_at(55, request).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RATTACH);
    assert_eq!(completion.tag(), 21);
    assert_eq!(completion.payload().len(), 13);
    assert_eq!(completion.payload()[0], VIRTIO_9P_QTDIR);
    assert_eq!(completion.payload()[1..5], 0_u32.to_le_bytes());
    assert_eq!(completion.payload()[5..13], 1_u64.to_le_bytes());

    let attachments = device.attached_fids();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].fid(), 42);
    assert_eq!(attachments[0].afid(), VIRTIO_9P_NOFID);
    assert_eq!(attachments[0].uname(), "root");
    assert_eq!(attachments[0].aname(), "");
    assert_eq!(attachments[0].n_uname(), 0);
}

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
fn virtio_9p_device_returns_lerror_for_unsupported_messages() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let request = decoded_request(112, 31, Vec::new());

    let completion = device.execute_at(66, request).unwrap();

    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.tag(), 31);
    assert_eq!(completion.payload(), VIRTIO_9P_ENOTSUP.to_le_bytes());
    assert_eq!(device.completions(), vec![completion]);
}

#[test]
fn virtio_9p_device_rejects_malformed_protocol_payloads_as_typed_errors() {
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

    let malformed_attach = decoded_request(VIRTIO_9P_TATTACH, 2, vec![0; 9]);
    assert!(matches!(
        device.execute_at(78, malformed_attach),
        Err(VirtioError::InvalidVirtio9pPayload {
            message_type: VIRTIO_9P_TATTACH,
            bytes: 9
        })
    ));
}
