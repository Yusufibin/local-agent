use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Serialize for HostError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<String> for HostError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}

impl From<&str> for HostError {
    fn from(value: &str) -> Self {
        Self::Message(value.to_string())
    }
}

pub type HostResult<T> = Result<T, HostError>;
