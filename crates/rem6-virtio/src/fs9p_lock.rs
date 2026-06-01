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
        self.locks
            .push(Virtio9pHeldLock::new(node, request.fid, request));
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

    pub(crate) fn remove_fid(&mut self, fid: u32) {
        self.locks.retain(|lock| lock.fid != fid);
    }

    fn unlock(&mut self, node: Virtio9pNodeId, request: &Virtio9pLockRequest) {
        let mut remaining = Vec::with_capacity(self.locks.len());
        for lock in self.locks.drain(..) {
            if lock.node == node
                && lock.proc_id == request.proc_id
                && lock.client_id == request.client_id
                && ranges_overlap(lock.start, lock.length, request.start, request.length)
            {
                lock.push_unlocked_ranges(request.start, request.length, &mut remaining);
            } else {
                remaining.push(lock);
            }
        }
        self.locks = remaining;
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
    fid: u32,
    lock_type: u8,
    flags: u32,
    start: u64,
    length: u64,
    proc_id: u32,
    client_id: String,
}

impl Virtio9pHeldLock {
    fn new(node: Virtio9pNodeId, fid: u32, request: &Virtio9pLockRequest) -> Self {
        Self {
            node,
            fid,
            lock_type: request.lock_type,
            flags: request.flags,
            start: request.start,
            length: request.length,
            proc_id: request.proc_id,
            client_id: request.client_id.clone(),
        }
    }

    fn push_unlocked_ranges(&self, unlock_start: u64, unlock_length: u64, output: &mut Vec<Self>) {
        let held_end = lock_end(self.start, self.length);
        if self.start < unlock_start {
            push_lock_range(self, self.start, Some(unlock_start), output);
        }
        let Some(unlock_end) = lock_end(unlock_start, unlock_length) else {
            return;
        };
        let has_right_range = match held_end {
            Some(end) => unlock_end < end,
            None => true,
        };
        if has_right_range {
            push_lock_range(self, unlock_end.max(self.start), held_end, output);
        }
    }
}

fn push_lock_range(
    template: &Virtio9pHeldLock,
    start: u64,
    end: Option<u64>,
    output: &mut Vec<Virtio9pHeldLock>,
) {
    let Some(length) = lock_length(start, end) else {
        return;
    };
    output.push(Virtio9pHeldLock {
        node: template.node,
        fid: template.fid,
        lock_type: template.lock_type,
        flags: template.flags,
        start,
        length,
        proc_id: template.proc_id,
        client_id: template.client_id.clone(),
    });
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

fn lock_length(start: u64, end: Option<u64>) -> Option<u64> {
    match end {
        Some(end) if start < end => Some(end - start),
        Some(_) => None,
        None => Some(0),
    }
}
