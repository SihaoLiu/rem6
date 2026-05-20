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
        let mut restored = Vec::new();
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
            restored.push((component, chunks));
        }

        for (component, chunks) in restored {
            self.components.insert(component, chunks);
        }
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
