pub mod eth_helper;
pub mod negotiation;
pub mod device;
pub mod state;
pub mod appstate_compat;
pub mod performance;
pub mod erc20_encoder;
pub mod solana_encoder;
// Maps a FROST `Ciphersuite` generic to the "secp256k1" / "ed25519"
// string names the blockchain helpers expect. Previously orphaned in
// the tree — wired up so `protocal::dkg::process_dkg_round2` can derive
// the real curve name from `C` instead of the session's "unified" label.
pub mod curve_traits;
