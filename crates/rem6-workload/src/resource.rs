use crate::WorkloadError;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadResourceId(String);

impl WorkloadResourceId {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkloadError> {
        let value = value.into();
        if value.is_empty() {
            return Err(WorkloadError::EmptyResourceId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadResourceKind {
    Kernel,
    DiskImage,
    Firmware,
    DeviceTree,
    Input,
    Output,
    Initrd,
}

impl WorkloadResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Kernel => "kernel",
            Self::DiskImage => "disk-image",
            Self::Firmware => "firmware",
            Self::DeviceTree => "device-tree",
            Self::Input => "input",
            Self::Output => "output",
            Self::Initrd => "initrd",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadResourceKindField {
    DiskImageConstruction,
}

impl WorkloadResourceKindField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DiskImageConstruction => "disk-image-construction",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadResourceAcquisitionKind {
    LocalFile,
    HostFile,
    ArchiveTar,
    ArchiveZip,
    RemoteUri,
    Generated,
    Preloaded,
}

impl WorkloadResourceAcquisitionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalFile => "local-file",
            Self::HostFile => "host-file",
            Self::ArchiveTar => "archive-tar",
            Self::ArchiveZip => "archive-zip",
            Self::RemoteUri => "remote-uri",
            Self::Generated => "generated",
            Self::Preloaded => "preloaded",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadResourceAcquisitionField {
    Locator,
    Tool,
    Revision,
}

impl WorkloadResourceAcquisitionField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Locator => "locator",
            Self::Tool => "tool",
            Self::Revision => "revision",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadResourceAcquisition {
    kind: WorkloadResourceAcquisitionKind,
    locator: String,
    tool: Option<String>,
    revision: Option<String>,
}

impl WorkloadResourceAcquisition {
    pub fn new(
        kind: WorkloadResourceAcquisitionKind,
        locator: impl Into<String>,
    ) -> Result<Self, WorkloadError> {
        let locator = locator.into();
        validate_acquisition_text(WorkloadResourceAcquisitionField::Locator, &locator)?;
        Ok(Self {
            kind,
            locator,
            tool: None,
            revision: None,
        })
    }

    pub fn with_tool(mut self, tool: impl Into<String>) -> Result<Self, WorkloadError> {
        let tool = tool.into();
        validate_acquisition_text(WorkloadResourceAcquisitionField::Tool, &tool)?;
        self.tool = Some(tool);
        Ok(self)
    }

    pub fn with_revision(mut self, revision: impl Into<String>) -> Result<Self, WorkloadError> {
        let revision = revision.into();
        validate_acquisition_text(WorkloadResourceAcquisitionField::Revision, &revision)?;
        self.revision = Some(revision);
        Ok(self)
    }

    pub const fn kind(&self) -> WorkloadResourceAcquisitionKind {
        self.kind
    }

    pub fn locator(&self) -> &str {
        &self.locator
    }

    pub fn tool(&self) -> Option<&str> {
        self.tool.as_deref()
    }

    pub fn revision(&self) -> Option<&str> {
        self.revision.as_deref()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadResourceConstructionField {
    ImageFormat,
    Tool,
    Operation,
    Input,
    Argument,
}

impl WorkloadResourceConstructionField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ImageFormat => "image-format",
            Self::Tool => "tool",
            Self::Operation => "operation",
            Self::Input => "input",
            Self::Argument => "argument",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadDiskImageConstructionStep {
    tool: String,
    operation: String,
    input: String,
    arguments: Vec<String>,
}

impl WorkloadDiskImageConstructionStep {
    pub fn new(
        tool: impl Into<String>,
        operation: impl Into<String>,
        input: impl Into<String>,
    ) -> Result<Self, WorkloadError> {
        let tool = tool.into();
        validate_construction_text(WorkloadResourceConstructionField::Tool, &tool)?;
        let operation = operation.into();
        validate_construction_text(WorkloadResourceConstructionField::Operation, &operation)?;
        let input = input.into();
        validate_construction_text(WorkloadResourceConstructionField::Input, &input)?;
        Ok(Self {
            tool,
            operation,
            input,
            arguments: Vec::new(),
        })
    }

    pub fn with_argument(mut self, argument: impl Into<String>) -> Result<Self, WorkloadError> {
        let argument = argument.into();
        validate_construction_text(WorkloadResourceConstructionField::Argument, &argument)?;
        self.arguments.push(argument);
        Ok(self)
    }

    pub fn tool(&self) -> &str {
        &self.tool
    }

    pub fn operation(&self) -> &str {
        &self.operation
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadDiskImageConstruction {
    image_format: String,
    virtual_size_bytes: u64,
    steps: Vec<WorkloadDiskImageConstructionStep>,
}

impl WorkloadDiskImageConstruction {
    pub fn new(
        image_format: impl Into<String>,
        virtual_size_bytes: u64,
    ) -> Result<Self, WorkloadError> {
        let image_format = image_format.into();
        validate_construction_text(
            WorkloadResourceConstructionField::ImageFormat,
            &image_format,
        )?;
        if virtual_size_bytes == 0 {
            return Err(WorkloadError::ZeroDiskImageVirtualSizeBytes);
        }
        Ok(Self {
            image_format,
            virtual_size_bytes,
            steps: Vec::new(),
        })
    }

    pub fn with_step(mut self, step: WorkloadDiskImageConstructionStep) -> Self {
        self.steps.push(step);
        self
    }

    pub fn image_format(&self) -> &str {
        &self.image_format
    }

    pub const fn virtual_size_bytes(&self) -> u64 {
        self.virtual_size_bytes
    }

    pub fn steps(&self) -> &[WorkloadDiskImageConstructionStep] {
        &self.steps
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadResource {
    id: WorkloadResourceId,
    kind: WorkloadResourceKind,
    digest: String,
    locator: String,
    acquisition: Option<WorkloadResourceAcquisition>,
    disk_image_construction: Option<WorkloadDiskImageConstruction>,
}

impl WorkloadResource {
    pub fn new(
        id: WorkloadResourceId,
        kind: WorkloadResourceKind,
        digest: impl Into<String>,
        locator: impl Into<String>,
    ) -> Result<Self, WorkloadError> {
        let digest = digest.into();
        if digest.is_empty() {
            return Err(WorkloadError::EmptyResourceDigest {
                resource: id.clone(),
            });
        }

        let locator = locator.into();
        if locator.is_empty() {
            return Err(WorkloadError::EmptyResourceLocator {
                resource: id.clone(),
            });
        }

        Ok(Self {
            id,
            kind,
            digest,
            locator,
            acquisition: None,
            disk_image_construction: None,
        })
    }

    pub fn with_acquisition(mut self, acquisition: WorkloadResourceAcquisition) -> Self {
        self.acquisition = Some(acquisition);
        self
    }

    pub fn with_disk_image_construction(
        mut self,
        construction: WorkloadDiskImageConstruction,
    ) -> Result<Self, WorkloadError> {
        if self.kind != WorkloadResourceKind::DiskImage {
            return Err(WorkloadError::ResourceKindFieldMismatch {
                resource: self.id.clone(),
                field: WorkloadResourceKindField::DiskImageConstruction,
                expected: WorkloadResourceKind::DiskImage,
                actual: self.kind,
            });
        }
        self.disk_image_construction = Some(construction);
        Ok(self)
    }

    pub fn id(&self) -> &WorkloadResourceId {
        &self.id
    }

    pub const fn kind(&self) -> WorkloadResourceKind {
        self.kind
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn locator(&self) -> &str {
        &self.locator
    }

    pub fn acquisition(&self) -> Option<&WorkloadResourceAcquisition> {
        self.acquisition.as_ref()
    }

    pub fn disk_image_construction(&self) -> Option<&WorkloadDiskImageConstruction> {
        self.disk_image_construction.as_ref()
    }
}

fn validate_acquisition_text(
    field: WorkloadResourceAcquisitionField,
    value: &str,
) -> Result<(), WorkloadError> {
    if value.is_empty() {
        return Err(WorkloadError::EmptyResourceAcquisitionField { field });
    }
    Ok(())
}

fn validate_construction_text(
    field: WorkloadResourceConstructionField,
    value: &str,
) -> Result<(), WorkloadError> {
    if value.is_empty() {
        return Err(WorkloadError::EmptyResourceConstructionField { field });
    }
    Ok(())
}
