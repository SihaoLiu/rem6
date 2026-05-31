use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, SchedulerContext, Tick};

use crate::{
    VirtioError, VirtioPciCommonConfigDevice, VirtioPciNotifyDevice, VirtioQueueIndex,
    VirtioQueueNotifySpec, VirtioQueueSpec,
};

pub const VIRTIO_RNG_DEVICE_ID: u16 = 4;
pub const VIRTIO_RNG_REQUEST_QUEUE_INDEX: u16 = 0;
pub const VIRTIO_RNG_DEFAULT_QUEUE_SIZE: u16 = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioRngByteSource {
    bytes: Vec<u8>,
    cursor: usize,
}

impl VirtioRngByteSource {
    pub fn repeating(bytes: Vec<u8>) -> Result<Self, VirtioError> {
        if bytes.is_empty() {
            return Err(VirtioError::EmptyRngEntropySource);
        }
        Ok(Self { bytes, cursor: 0 })
    }

    fn fill(&mut self, output: &mut [u8]) {
        for byte in output {
            *byte = self.bytes[self.cursor];
            self.cursor = (self.cursor + 1) % self.bytes.len();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VirtioRngRequestId(u64);

impl VirtioRngRequestId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioRngRequest {
    id: VirtioRngRequestId,
    queue: VirtioQueueIndex,
    bytes: u64,
}

impl VirtioRngRequest {
    pub fn new(
        id: VirtioRngRequestId,
        queue: VirtioQueueIndex,
        bytes: u64,
    ) -> Result<Self, VirtioError> {
        if bytes == 0 {
            return Err(VirtioError::MissingVirtioRngWritableDescriptor);
        }
        Ok(Self { id, queue, bytes })
    }

    pub const fn id(&self) -> VirtioRngRequestId {
        self.id
    }

    pub const fn queue(&self) -> VirtioQueueIndex {
        self.queue
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioRngCompletion {
    request: VirtioRngRequestId,
    queue: VirtioQueueIndex,
    tick: Tick,
    bytes: Vec<u8>,
}

impl VirtioRngCompletion {
    fn new(
        request: VirtioRngRequestId,
        queue: VirtioQueueIndex,
        tick: Tick,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            request,
            queue,
            tick,
            bytes,
        }
    }

    pub const fn request(&self) -> VirtioRngRequestId {
        self.request
    }

    pub const fn queue(&self) -> VirtioQueueIndex {
        self.queue
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Debug)]
pub struct VirtioRngDevice {
    source: Arc<Mutex<VirtioRngByteSource>>,
    completions: Arc<Mutex<Vec<VirtioRngCompletion>>>,
}

impl VirtioRngDevice {
    pub fn new(source: VirtioRngByteSource) -> Self {
        Self {
            source: Arc::new(Mutex::new(source)),
            completions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn feature_pages(&self) -> Vec<(u32, u32)> {
        Vec::new()
    }

    pub fn queue_specs(&self) -> [VirtioQueueSpec; 1] {
        [VirtioQueueSpec::available(VIRTIO_RNG_DEFAULT_QUEUE_SIZE, 0)]
    }

    pub fn notify_specs(&self) -> [VirtioQueueNotifySpec; 1] {
        [VirtioQueueNotifySpec::new(
            VirtioQueueIndex::new(VIRTIO_RNG_REQUEST_QUEUE_INDEX).expect("rng request queue index"),
            0,
        )]
    }

    pub fn build_common_config(&self) -> Result<VirtioPciCommonConfigDevice, VirtioError> {
        VirtioPciCommonConfigDevice::new(self.feature_pages(), self.queue_specs())
    }

    pub fn build_notify_device(
        &self,
        notify_off_multiplier: u32,
    ) -> Result<VirtioPciNotifyDevice, VirtioError> {
        VirtioPciNotifyDevice::new(notify_off_multiplier, self.notify_specs())
    }

    pub const fn config_size(&self) -> u64 {
        0
    }

    pub fn execute(
        &self,
        context: &mut SchedulerContext<'_>,
        request: VirtioRngRequest,
    ) -> Result<VirtioRngCompletion, VirtioError> {
        self.execute_at(context.now(), request)
    }

    pub fn execute_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: VirtioRngRequest,
    ) -> Result<VirtioRngCompletion, VirtioError> {
        self.execute_at(context.now(), request)
    }

    pub fn execute_at(
        &self,
        tick: Tick,
        request: VirtioRngRequest,
    ) -> Result<VirtioRngCompletion, VirtioError> {
        let bytes_len = usize::try_from(request.bytes())
            .map_err(|_| VirtioError::VirtioRngPayloadLengthOverflow)?;
        let mut bytes = vec![0; bytes_len];
        self.source
            .lock()
            .expect("virtio rng source lock")
            .fill(&mut bytes);
        let completion = VirtioRngCompletion::new(request.id(), request.queue(), tick, bytes);
        self.completions
            .lock()
            .expect("virtio rng completion lock")
            .push(completion.clone());
        Ok(completion)
    }

    pub fn completions(&self) -> Vec<VirtioRngCompletion> {
        self.completions
            .lock()
            .expect("virtio rng completion lock")
            .clone()
    }
}
