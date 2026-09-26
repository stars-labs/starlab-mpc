// Core FROST implementation shared between WASM and CLI

pub mod accounts;
pub mod curve_registry;
pub mod ed25519;
pub mod errors;
pub mod hd_derivation;
pub mod keystore;
pub mod resharing;
pub mod rng;
pub mod root_secret;
pub mod secp256k1;
pub mod secp256k1_tr;
pub mod traits;
pub mod unified_dkg;

// Re-export main types
pub use errors::{FrostError, Result};
pub use keystore::{Keystore, KeystoreData, MultiCurveKeystoreData};
pub use traits::FrostCurve;

// Re-export curve implementations
pub use ed25519::Ed25519Curve;
pub use secp256k1::Secp256k1Curve;
pub use secp256k1_tr::Secp256k1TrCurve;

// Re-export unified DKG types
pub use hd_derivation::{
    ChainCode, DerivationPath, DerivedKeys, derive_child_key, derive_child_key_path,
    derive_child_verifying_key_path,
};
pub use resharing::ReshareSession;
pub use root_secret::RootSecret;
pub use unified_dkg::UnifiedDkg;
