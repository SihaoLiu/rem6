use crate::{InOrderPipelineConfig, InOrderPipelineStage, InOrderPipelineStageWidth};

pub(crate) fn default_riscv_in_order_pipeline_config() -> InOrderPipelineConfig {
    InOrderPipelineConfig::new([
        InOrderPipelineStageWidth::new(InOrderPipelineStage::Fetch1, 1)
            .expect("default RISC-V fetch1 width is valid"),
        InOrderPipelineStageWidth::new(InOrderPipelineStage::Fetch2, 1)
            .expect("default RISC-V fetch2 width is valid"),
        InOrderPipelineStageWidth::new(InOrderPipelineStage::Decode, 1)
            .expect("default RISC-V decode width is valid"),
        InOrderPipelineStageWidth::new(InOrderPipelineStage::Execute, 1)
            .expect("default RISC-V execute width is valid"),
        InOrderPipelineStageWidth::new(InOrderPipelineStage::Commit, 1)
            .expect("default RISC-V commit width is valid"),
    ])
    .expect("default RISC-V in-order pipeline config covers every stage")
}
