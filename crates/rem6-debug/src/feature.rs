#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GdbRemoteFeature {
    name: Vec<u8>,
    value: GdbRemoteFeatureValue,
}

impl GdbRemoteFeature {
    pub const fn new(name: Vec<u8>, value: GdbRemoteFeatureValue) -> Self {
        Self { name, value }
    }

    pub fn name(&self) -> &[u8] {
        &self.name
    }

    pub const fn value(&self) -> &GdbRemoteFeatureValue {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GdbRemoteFeatureValue {
    Supported,
    Unsupported,
    AutoDetect,
    Value(Vec<u8>),
    Bare,
}

pub(crate) fn parse_supported_features(
    features: &[u8],
    allow_probe_suffix: bool,
) -> Vec<GdbRemoteFeature> {
    features
        .split(|byte| *byte == b';')
        .filter(|feature| !feature.is_empty())
        .map(|feature| parse_supported_feature(feature, allow_probe_suffix))
        .collect()
}

fn parse_supported_feature(feature: &[u8], allow_probe_suffix: bool) -> GdbRemoteFeature {
    if let Some(separator) = feature.iter().position(|byte| *byte == b'=') {
        return GdbRemoteFeature::new(
            feature[..separator].to_vec(),
            GdbRemoteFeatureValue::Value(feature[separator + 1..].to_vec()),
        );
    }

    match feature.last() {
        Some(b'+') => GdbRemoteFeature::new(
            feature[..feature.len() - 1].to_vec(),
            GdbRemoteFeatureValue::Supported,
        ),
        Some(b'-') => GdbRemoteFeature::new(
            feature[..feature.len() - 1].to_vec(),
            GdbRemoteFeatureValue::Unsupported,
        ),
        Some(b'?') if allow_probe_suffix => GdbRemoteFeature::new(
            feature[..feature.len() - 1].to_vec(),
            GdbRemoteFeatureValue::AutoDetect,
        ),
        _ => GdbRemoteFeature::new(feature.to_vec(), GdbRemoteFeatureValue::Bare),
    }
}

pub(crate) fn encode_supported_features(features: &[GdbRemoteFeature]) -> Vec<u8> {
    let mut encoded = Vec::new();

    for (index, feature) in features.iter().enumerate() {
        if index > 0 {
            encoded.push(b';');
        }
        encoded.extend_from_slice(feature.name());
        match feature.value() {
            GdbRemoteFeatureValue::Supported => encoded.push(b'+'),
            GdbRemoteFeatureValue::Unsupported => encoded.push(b'-'),
            GdbRemoteFeatureValue::AutoDetect => encoded.push(b'?'),
            GdbRemoteFeatureValue::Value(value) => {
                encoded.push(b'=');
                encoded.extend_from_slice(value);
            }
            GdbRemoteFeatureValue::Bare => {}
        }
    }

    encoded
}
