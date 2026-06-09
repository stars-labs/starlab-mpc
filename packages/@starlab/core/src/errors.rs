use thiserror::Error;

#[derive(Error, Debug)]
pub enum FrostError {
    #[error("Invalid identifier: {0}")]
    InvalidIdentifier(String),
    
    #[error("DKG error: {0}")]
    DkgError(String),
    
    #[error("Signing error: {0}")]
    SigningError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Keystore error: {0}")]
    KeystoreError(String),
    
    #[error("Invalid state: {0}")]
    InvalidState(String),
    
    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Derivation error: {0}")]
    DerivationError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, FrostError>;