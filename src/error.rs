use crate::{
    generator::{CheckSocError, GeneratorOffError, GeneratorOnError},
    AcLimitError,
};

impl From<rumqttc::ClientError> for AcLimitError {
    fn from(error: rumqttc::ClientError) -> Self {
        Self::RumqttcClientError(error)
    }
}

impl From<AcLimitError> for GeneratorOffError {
    fn from(error: AcLimitError) -> Self {
        Self::SetAcLimit(error)
    }
}

impl From<lci_gateway::SetError> for GeneratorOffError {
    fn from(error: lci_gateway::SetError) -> Self {
        Self::SetError(error)
    }
}

impl From<prowl::CreationError> for GeneratorOnError {
    fn from(error: prowl::CreationError) -> Self {
        Self::Creation(error)
    }
}

impl From<prowl::AddError> for GeneratorOnError {
    fn from(error: prowl::AddError) -> Self {
        Self::AddError(error)
    }
}

impl From<lci_gateway::SetError> for GeneratorOnError {
    fn from(error: lci_gateway::SetError) -> Self {
        Self::SetError(error)
    }
}

impl From<AcLimitError> for GeneratorOnError {
    fn from(error: AcLimitError) -> Self {
        Self::AcLimitError(error)
    }
}

impl From<GeneratorOffError> for CheckSocError {
    fn from(error: GeneratorOffError) -> Self {
        Self::GeneratorOff(error)
    }
}

impl From<GeneratorOnError> for CheckSocError {
    fn from(error: GeneratorOnError) -> Self {
        Self::GeneratorOn(error)
    }
}

impl From<std::num::ParseFloatError> for CheckSocError {
    fn from(error: std::num::ParseFloatError) -> Self {
        Self::ParseFloat(error)
    }
}

impl From<lci_gateway::GeneratorStateConversionError> for GeneratorOnError {
    fn from(error: lci_gateway::GeneratorStateConversionError) -> Self {
        Self::GeneratorStateConversionError(error)
    }
}

impl From<lci_gateway::GeneratorStateConversionError> for GeneratorOffError {
    fn from(error: lci_gateway::GeneratorStateConversionError) -> Self {
        Self::GeneratorStateConversionError(error)
    }
}
