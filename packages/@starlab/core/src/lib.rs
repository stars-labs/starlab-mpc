// Core FROST implementation shared between WASM and CLI

pub mod traits;
pub mod ed25519;
pub mod secp256k1;
pub mod keystore;
pub mod errors;
pub mod root_secret;
pub mod curve_registry;
pub mod resharing;
pub mod unified_dkg;
pub mod hd_derivation;

// Re-export main types
pub use traits::FrostCurve;
pub use errors::{FrostError, Result};
pub use keystore::{Keystore, KeystoreData, MultiCurveKeystoreData};

// Re-export curve implementations
pub use ed25519::Ed25519Curve;
pub use secp256k1::Secp256k1Curve;

// Re-export unified DKG types
pub use root_secret::RootSecret;
pub use unified_dkg::UnifiedDkg;
pub use hd_derivation::{ChainCode, DerivationPath, DerivedKeys, derive_child_key, derive_child_key_path};