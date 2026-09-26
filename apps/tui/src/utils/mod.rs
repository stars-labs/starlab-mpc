pub mod appstate_compat;
pub mod device;
pub mod erc20_encoder;
pub mod eth_helper;
pub mod performance;
pub mod solana_encoder;
pub mod state;
// Maps a FROST `Ciphersuite` generic to the "secp256k1" / "ed25519"
// string names the blockchain helpers expect. Previously orphaned in
// the tree — wired up so `protocal::dkg::process_dkg_round2` can derive
// the real curve name from `C` instead of the session's "unified" label.
pub mod curve_traits;
