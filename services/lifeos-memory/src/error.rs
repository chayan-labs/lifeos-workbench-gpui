//! Crate error type. Small on purpose: callers (lifeos-api, lifeos-drain)
//! map it into their own error surfaces.

#[derive(Debug)]
pub enum MemoryError {
    Db(libsql::Error),
    Storage(lifeos_vcs::BackendError),
    Model(String),
    Other(String),
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryError::Db(e) => write!(f, "db error: {e}"),
            MemoryError::Storage(e) => write!(f, "storage error: {e}"),
            MemoryError::Model(m) => write!(f, "model error: {m}"),
            MemoryError::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for MemoryError {}

impl From<libsql::Error> for MemoryError {
    fn from(e: libsql::Error) -> Self {
        MemoryError::Db(e)
    }
}

impl From<lifeos_vcs::BackendError> for MemoryError {
    fn from(e: lifeos_vcs::BackendError) -> Self {
        MemoryError::Storage(e)
    }
}
