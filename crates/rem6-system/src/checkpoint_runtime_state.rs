use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const STORED_RUNTIME_STATE_MAGIC: &[u8; 4] = b"CRS1";
const STORED_RUNTIME_STATE_BYTES: usize = 20;
static NEXT_STORED_RUNTIME_NAMESPACE: AtomicU64 = AtomicU64::new(1);

type RuntimeStateCapture = Arc<dyn Fn() -> Result<Vec<u8>, String> + Send + Sync>;
type RuntimeStateValidate = Arc<dyn Fn(&[u8]) -> Result<(), String> + Send + Sync>;
type RuntimeStateRestore = Arc<dyn Fn(&[u8]) + Send + Sync>;

#[derive(Clone)]
pub struct CheckpointRuntimeState {
    capture: RuntimeStateCapture,
    validate: RuntimeStateValidate,
    restore: RuntimeStateRestore,
}

impl fmt::Debug for CheckpointRuntimeState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckpointRuntimeState")
            .finish_non_exhaustive()
    }
}

impl CheckpointRuntimeState {
    pub fn new<C, V, R>(capture: C, validate: V, restore: R) -> Self
    where
        C: Fn() -> Result<Vec<u8>, String> + Send + Sync + 'static,
        V: Fn(&[u8]) -> Result<(), String> + Send + Sync + 'static,
        R: Fn(&[u8]) + Send + Sync + 'static,
    {
        Self {
            capture: Arc::new(capture),
            validate: Arc::new(validate),
            restore: Arc::new(restore),
        }
    }

    pub fn stored<S, C, V, R>(capture: C, validate: V, restore: R) -> Self
    where
        S: Clone + Send + 'static,
        C: Fn() -> Result<S, String> + Send + Sync + 'static,
        V: Fn(&S) -> Result<(), String> + Send + Sync + 'static,
        R: Fn(S) + Send + Sync + 'static,
    {
        let namespace = NEXT_STORED_RUNTIME_NAMESPACE.fetch_add(1, Ordering::Relaxed);
        let snapshots = Arc::new(Mutex::new((0_u64, BTreeMap::<u64, S>::new())));
        let capture_snapshots = Arc::clone(&snapshots);
        let validate_snapshots = Arc::clone(&snapshots);
        let restore_snapshots = Arc::clone(&snapshots);
        Self::new(
            move || {
                let snapshot = capture()?;
                let mut snapshots = capture_snapshots
                    .lock()
                    .map_err(|error| format!("runtime snapshot lock poisoned: {error}"))?;
                let id = snapshots.0;
                snapshots.0 = id
                    .checked_add(1)
                    .ok_or_else(|| "runtime snapshot identifier exhausted".to_string())?;
                snapshots.1.insert(id, snapshot);
                Ok(encode_stored_runtime_state(namespace, id))
            },
            move |payload| {
                let id = decode_stored_runtime_state(payload, namespace)?;
                let snapshots = validate_snapshots
                    .lock()
                    .map_err(|error| format!("runtime snapshot lock poisoned: {error}"))?;
                let snapshot = snapshots
                    .1
                    .get(&id)
                    .ok_or_else(|| format!("runtime snapshot {id} is not available"))?;
                validate(snapshot)
            },
            move |payload| {
                let id = decode_stored_runtime_state(payload, namespace)
                    .expect("validated stored runtime checkpoint payload");
                let snapshot = restore_snapshots
                    .lock()
                    .expect("validated runtime snapshot lock")
                    .1
                    .get(&id)
                    .cloned()
                    .expect("validated retained runtime snapshot");
                restore(snapshot);
            },
        )
    }

    pub(crate) fn capture(&self) -> Result<Vec<u8>, String> {
        (self.capture)()
    }

    pub(crate) fn validate(&self, payload: &[u8]) -> Result<(), String> {
        (self.validate)(payload)
    }

    pub(crate) fn restore(&self, payload: &[u8]) {
        (self.restore)(payload);
    }
}

fn encode_stored_runtime_state(namespace: u64, id: u64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(STORED_RUNTIME_STATE_BYTES);
    payload.extend_from_slice(STORED_RUNTIME_STATE_MAGIC);
    payload.extend_from_slice(&namespace.to_le_bytes());
    payload.extend_from_slice(&id.to_le_bytes());
    payload
}

fn decode_stored_runtime_state(payload: &[u8], namespace: u64) -> Result<u64, String> {
    if payload.len() != STORED_RUNTIME_STATE_BYTES
        || payload.get(..4) != Some(STORED_RUNTIME_STATE_MAGIC)
    {
        return Err("invalid stored runtime checkpoint payload".to_string());
    }
    let stored_namespace = u64::from_le_bytes(payload[4..12].try_into().unwrap());
    if stored_namespace != namespace {
        return Err("stored runtime checkpoint belongs to another runtime".to_string());
    }
    Ok(u64::from_le_bytes(payload[12..20].try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn stored_runtime_state_restores_retained_snapshots_in_either_order() {
        let live = Arc::new(Mutex::new(vec![1_u64]));
        let capture = Arc::clone(&live);
        let restore = Arc::clone(&live);
        let state = CheckpointRuntimeState::stored(
            move || Ok(capture.lock().unwrap().clone()),
            |_| Ok(()),
            move |snapshot| {
                *restore.lock().unwrap() = snapshot;
            },
        );

        let early = state.capture().unwrap();
        live.lock().unwrap().extend([2, 3]);
        let late = state.capture().unwrap();
        live.lock().unwrap().push(4);

        state.validate(&early).unwrap();
        state.restore(&early);
        assert_eq!(*live.lock().unwrap(), [1]);
        state.validate(&late).unwrap();
        state.restore(&late);
        assert_eq!(*live.lock().unwrap(), [1, 2, 3]);

        let mut corrupt = late;
        corrupt.push(0);
        assert!(state.validate(&corrupt).is_err());
    }
}
