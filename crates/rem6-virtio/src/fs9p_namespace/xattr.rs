use crate::fs9p_protocol::{VIRTIO_9P_EEXIST, VIRTIO_9P_ENODATA};

use super::Virtio9pNodeId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Virtio9pXattrWritePolicy {
    Any,
    Create,
    Replace,
}

impl Virtio9pXattrWritePolicy {
    pub(crate) const fn validate_exists(self, exists: bool) -> Result<(), u32> {
        match (self, exists) {
            (Self::Create, true) => Err(VIRTIO_9P_EEXIST),
            (Self::Replace, false) => Err(VIRTIO_9P_ENODATA),
            (Self::Any, _) | (Self::Create, false) | (Self::Replace, true) => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pXattrCommit {
    pub(crate) node: Virtio9pNodeId,
    pub(crate) name: String,
    pub(crate) data: Vec<u8>,
    pub(crate) policy: Virtio9pXattrWritePolicy,
}
