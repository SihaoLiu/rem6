use crate::fs9p_namespace::Virtio9pNodeId;
use crate::fs9p_protocol::{
    lock_payload, Virtio9pLockRequest, VIRTIO_9P_LOCK_BLOCKED, VIRTIO_9P_LOCK_SUCCESS,
    VIRTIO_9P_LOCK_TYPE_UNLCK, VIRTIO_9P_LOCK_TYPE_WRLCK,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Virtio9pLockTable {
    locks: Vec<Virtio9pHeldLock>,
}

impl Virtio9pLockTable {
    pub(crate) fn apply(&mut self, node: Virtio9pNodeId, request: &Virtio9pLockRequest) -> u8 {
        if request.lock_type == VIRTIO_9P_LOCK_TYPE_UNLCK {
            self.unlock(node, request);
            return VIRTIO_9P_LOCK_SUCCESS;
        }
        if self.conflict(node, request).is_some() {
            return VIRTIO_9P_LOCK_BLOCKED;
        }
        self.unlock(node, request);
        self.locks.push(Virtio9pHeldLock::new(node, request));
        VIRTIO_9P_LOCK_SUCCESS
    }

    pub(crate) fn conflict_payload(
        &self,
        node: Virtio9pNodeId,
        request: &Virtio9pLockRequest,
    ) -> Vec<u8> {
        if let Some(lock) = self.conflict(node, request) {
            return lock_payload(
                lock.lock_type,
                lock.flags,
                lock.start,
                lock.length,
                lock.proc_id,
                &lock.client_id,
            );
        }
        lock_payload(
            VIRTIO_9P_LOCK_TYPE_UNLCK,
            request.flags,
            request.start,
            request.length,
            request.proc_id,
            &request.client_id,
        )
    }

    pub(crate) fn clear(&mut self) {
        self.locks.clear();
    }

    pub(crate) fn remove_node(&mut self, node: Virtio9pNodeId) {
        self.locks.retain(|lock| lock.node != node);
    }

    fn unlock(&mut self, node: Virtio9pNodeId, request: &Virtio9pLockRequest) {
        self.locks.retain(|lock| {
            lock.node != node
                || lock.proc_id != request.proc_id
                || lock.client_id != request.client_id
                || !ranges_overlap(lock.start, lock.length, request.start, request.length)
        });
    }

    fn conflict(
        &self,
        node: Virtio9pNodeId,
        request: &Virtio9pLockRequest,
    ) -> Option<&Virtio9pHeldLock> {
        self.locks.iter().find(|lock| {
            lock.node == node
                && (lock.proc_id != request.proc_id || lock.client_id != request.client_id)
                && lock_types_conflict(lock.lock_type, request.lock_type)
                && ranges_overlap(lock.start, lock.length, request.start, request.length)
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Virtio9pHeldLock {
    node: Virtio9pNodeId,
    lock_type: u8,
    flags: u32,
    start: u64,
    length: u64,
    proc_id: u32,
    client_id: String,
}

impl Virtio9pHeldLock {
    fn new(node: Virtio9pNodeId, request: &Virtio9pLockRequest) -> Self {
        Self {
            node,
            lock_type: request.lock_type,
            flags: request.flags,
            start: request.start,
            length: request.length,
            proc_id: request.proc_id,
            client_id: request.client_id.clone(),
        }
    }
}

const fn lock_types_conflict(left: u8, right: u8) -> bool {
    matches!(left, VIRTIO_9P_LOCK_TYPE_WRLCK) || matches!(right, VIRTIO_9P_LOCK_TYPE_WRLCK)
}

fn ranges_overlap(left_start: u64, left_length: u64, right_start: u64, right_length: u64) -> bool {
    let left_end = lock_end(left_start, left_length);
    let right_end = lock_end(right_start, right_length);
    let left_before_right = left_end.is_some_and(|end| end <= right_start);
    let right_before_left = right_end.is_some_and(|end| end <= left_start);
    !(left_before_right || right_before_left)
}

fn lock_end(start: u64, length: u64) -> Option<u64> {
    (length != 0).then(|| start.saturating_add(length))
}
