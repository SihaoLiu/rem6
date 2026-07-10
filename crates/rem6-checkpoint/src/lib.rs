use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckpointComponentId(String);

impl CheckpointComponentId {
    pub fn new(value: impl Into<String>) -> Result<Self, CheckpointError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CheckpointError::EmptyComponentId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointChunk {
    name: String,
    payload: Vec<u8>,
}

impl CheckpointChunk {
    pub fn new(name: impl Into<String>, payload: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            payload,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointState {
    component: CheckpointComponentId,
    chunks: Vec<CheckpointChunk>,
}

impl CheckpointState {
    pub fn new(component: CheckpointComponentId, chunks: Vec<CheckpointChunk>) -> Self {
        Self { component, chunks }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn chunks(&self) -> &[CheckpointChunk] {
        &self.chunks
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointChunkSummary {
    name: String,
    payload_bytes: usize,
}

impl CheckpointChunkSummary {
    pub fn new(name: impl Into<String>, payload_bytes: usize) -> Self {
        Self {
            name: name.into(),
            payload_bytes,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointComponentSummary {
    component: CheckpointComponentId,
    chunk_count: usize,
    payload_bytes: usize,
    chunk_summaries: Vec<CheckpointChunkSummary>,
}

impl CheckpointComponentSummary {
    pub fn new(component: CheckpointComponentId, chunk_count: usize, payload_bytes: usize) -> Self {
        Self {
            component,
            chunk_count,
            payload_bytes,
            chunk_summaries: Vec::new(),
        }
    }

    pub fn with_chunk_summaries(
        component: CheckpointComponentId,
        chunk_summaries: Vec<CheckpointChunkSummary>,
    ) -> Self {
        let chunk_count = chunk_summaries.len();
        let payload_bytes = chunk_summaries
            .iter()
            .map(CheckpointChunkSummary::payload_bytes)
            .sum();
        Self {
            component,
            chunk_count,
            payload_bytes,
            chunk_summaries,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub const fn chunk_count(&self) -> usize {
        self.chunk_count
    }

    pub const fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }

    pub fn chunk_summaries(&self) -> &[CheckpointChunkSummary] {
        &self.chunk_summaries
    }

    pub fn chunk_summary(&self, name: &str) -> Option<&CheckpointChunkSummary> {
        self.chunk_summaries
            .iter()
            .find(|summary| summary.name() == name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointManifestSummary {
    component_summaries: Vec<CheckpointComponentSummary>,
    chunk_count: usize,
    payload_bytes: usize,
}

impl CheckpointManifestSummary {
    pub fn new(component_summaries: Vec<CheckpointComponentSummary>) -> Self {
        let chunk_count = component_summaries
            .iter()
            .map(CheckpointComponentSummary::chunk_count)
            .sum();
        let payload_bytes = component_summaries
            .iter()
            .map(CheckpointComponentSummary::payload_bytes)
            .sum();
        Self {
            component_summaries,
            chunk_count,
            payload_bytes,
        }
    }

    pub fn component_summaries(&self) -> &[CheckpointComponentSummary] {
        &self.component_summaries
    }

    pub fn component_summary(
        &self,
        component: &CheckpointComponentId,
    ) -> Option<&CheckpointComponentSummary> {
        self.component_summaries
            .iter()
            .find(|summary| summary.component() == component)
    }

    pub fn component_count(&self) -> usize {
        self.component_summaries.len()
    }

    pub const fn chunk_count(&self) -> usize {
        self.chunk_count
    }

    pub const fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointManifest {
    label: String,
    tick: Tick,
    states: Vec<CheckpointState>,
}

impl CheckpointManifest {
    pub fn new(label: impl Into<String>, tick: Tick, states: Vec<CheckpointState>) -> Self {
        Self {
            label: label.into(),
            tick,
            states,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub fn states(&self) -> &[CheckpointState] {
        &self.states
    }

    pub fn summary(&self) -> CheckpointManifestSummary {
        CheckpointManifestSummary::new(
            self.states
                .iter()
                .map(|state| {
                    let mut chunk_summaries = state
                        .chunks()
                        .iter()
                        .map(|chunk| {
                            CheckpointChunkSummary::new(
                                chunk.name().to_string(),
                                chunk.payload().len(),
                            )
                        })
                        .collect::<Vec<_>>();
                    chunk_summaries.sort_by(|left, right| left.name().cmp(right.name()));
                    CheckpointComponentSummary::with_chunk_summaries(
                        state.component().clone(),
                        chunk_summaries,
                    )
                })
                .collect(),
        )
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CheckpointRegistry {
    components: BTreeMap<CheckpointComponentId, BTreeMap<String, Vec<u8>>>,
}

impl CheckpointRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, component: CheckpointComponentId) -> Result<(), CheckpointError> {
        if self.components.contains_key(&component) {
            return Err(CheckpointError::DuplicateComponent { component });
        }
        self.components.insert(component, BTreeMap::new());
        Ok(())
    }

    pub fn write_chunk(
        &mut self,
        component: &CheckpointComponentId,
        name: impl Into<String>,
        payload: Vec<u8>,
    ) -> Result<(), CheckpointError> {
        let name = name.into();
        if name.is_empty() {
            return Err(CheckpointError::EmptyChunkName {
                component: component.clone(),
            });
        }

        let chunks = self.components.get_mut(component).ok_or_else(|| {
            CheckpointError::UnknownComponent {
                component: component.clone(),
            }
        })?;
        chunks.insert(name, payload);
        Ok(())
    }

    pub fn chunk(&self, component: &CheckpointComponentId, name: &str) -> Option<&[u8]> {
        self.components.get(component)?.get(name).map(Vec::as_slice)
    }

    pub fn contains_component(&self, component: &CheckpointComponentId) -> bool {
        self.components.contains_key(component)
    }

    pub fn capture(
        &self,
        label: impl Into<String>,
        tick: Tick,
    ) -> Result<CheckpointManifest, CheckpointError> {
        let label = label.into();
        if label.is_empty() {
            return Err(CheckpointError::EmptyLabel);
        }

        let states = self
            .components
            .iter()
            .map(|(component, chunks)| {
                CheckpointState::new(
                    component.clone(),
                    chunks
                        .iter()
                        .map(|(name, payload)| CheckpointChunk::new(name.clone(), payload.clone()))
                        .collect(),
                )
            })
            .collect();
        Ok(CheckpointManifest::new(label, tick, states))
    }

    pub fn restore(&mut self, manifest: &CheckpointManifest) -> Result<(), CheckpointError> {
        if manifest.label().is_empty() {
            return Err(CheckpointError::EmptyLabel);
        }

        let mut seen_components = BTreeSet::new();
        let mut restored = BTreeMap::new();
        for state in manifest.states() {
            let component = state.component().clone();
            if !self.components.contains_key(&component) {
                return Err(CheckpointError::UnknownComponent { component });
            }
            if !seen_components.insert(component.clone()) {
                return Err(CheckpointError::DuplicateComponent { component });
            }

            let mut seen_chunks = BTreeSet::new();
            let mut chunks = BTreeMap::new();
            for chunk in state.chunks() {
                if chunk.name().is_empty() {
                    return Err(CheckpointError::EmptyChunkName {
                        component: component.clone(),
                    });
                }
                if !seen_chunks.insert(chunk.name().to_string()) {
                    return Err(CheckpointError::DuplicateChunk {
                        component,
                        name: chunk.name().to_string(),
                    });
                }
                chunks.insert(chunk.name().to_string(), chunk.payload().to_vec());
            }
            restored.insert(component, chunks);
        }

        for component in self.components.keys() {
            restored.entry(component.clone()).or_default();
        }
        self.components = restored;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckpointError {
    EmptyComponentId,
    EmptyLabel,
    DuplicateComponent {
        component: CheckpointComponentId,
    },
    UnknownComponent {
        component: CheckpointComponentId,
    },
    ComponentNotQuiescent {
        component: CheckpointComponentId,
    },
    EmptyChunkName {
        component: CheckpointComponentId,
    },
    DuplicateChunk {
        component: CheckpointComponentId,
        name: String,
    },
}

impl fmt::Display for CheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyComponentId => {
                write!(formatter, "checkpoint component id must not be empty")
            }
            Self::EmptyLabel => write!(formatter, "checkpoint label must not be empty"),
            Self::DuplicateComponent { component } => {
                write!(
                    formatter,
                    "checkpoint component already exists: {}",
                    component.as_str()
                )
            }
            Self::UnknownComponent { component } => {
                write!(
                    formatter,
                    "unknown checkpoint component: {}",
                    component.as_str()
                )
            }
            Self::ComponentNotQuiescent { component } => write!(
                formatter,
                "checkpoint component is not quiescent: {}",
                component.as_str()
            ),
            Self::EmptyChunkName { component } => {
                write!(
                    formatter,
                    "checkpoint chunk name for component {} must not be empty",
                    component.as_str()
                )
            }
            Self::DuplicateChunk { component, name } => write!(
                formatter,
                "checkpoint component {} has duplicate chunk {name}",
                component.as_str()
            ),
        }
    }
}

impl Error for CheckpointError {}

#[cfg(test)]
mod tests {
    use super::{
        CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointRegistry,
        CheckpointState,
    };

    #[test]
    fn restore_clears_registered_components_absent_from_manifest() {
        let cpu = CheckpointComponentId::new("cpu0").unwrap();
        let memory = CheckpointComponentId::new("memory0").unwrap();
        let mut registry = CheckpointRegistry::new();
        registry.register(cpu.clone()).unwrap();
        registry.register(memory.clone()).unwrap();
        registry.write_chunk(&cpu, "pc", vec![1]).unwrap();
        registry.write_chunk(&memory, "store", vec![2]).unwrap();
        let manifest = CheckpointManifest::new(
            "cpu-only",
            12,
            vec![CheckpointState::new(
                cpu.clone(),
                vec![CheckpointChunk::new("pc", vec![3])],
            )],
        );

        registry.restore(&manifest).unwrap();

        assert_eq!(registry.chunk(&cpu, "pc"), Some(&[3][..]));
        assert_eq!(registry.chunk(&memory, "store"), None);
    }
}
