use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, Virtio9pRequest, VirtioError, VirtioQueueIndex,
    VirtioSplitDescriptor, VirtioSplitDescriptorChain, VIRTIO_9P_DEFAULT_MSIZE, VIRTIO_9P_ENOTSUP,
    VIRTIO_9P_NOFID, VIRTIO_9P_PROTOCOL_VERSION, VIRTIO_9P_QTDIR, VIRTIO_9P_RATTACH,
    VIRTIO_9P_RLERROR, VIRTIO_9P_RVERSION, VIRTIO_9P_TATTACH, VIRTIO_9P_TVERSION,
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
fn virtio_9p_device_returns_lerror_for_unsupported_messages() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let request = decoded_request(110, 31, Vec::new());

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
