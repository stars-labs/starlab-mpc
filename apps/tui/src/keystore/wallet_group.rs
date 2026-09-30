//! One wallet = every curve file the keystore holds for a wallet id.
//!
//! A secp256k1 DKG persists the same id twice (FROST Taproot key under
//! `secp256k1/`, threshold-ECDSA key under `secp256k1-ecdsa/`), a unified
//! DKG adds `ed25519/`. The keystore lists one [`WalletMetadata`] per curve
//! file; UIs list [`WalletGroup`]s — one row per wallet id — whose accounts
//! span every curve: Ethereum from the ECDSA key, Bitcoin from the Taproot
//! key, Solana from ed25519.

use super::WalletMetadata;
use starlab_core::ecdsa::ECDSA_CURVE;

/// All curve entries of one wallet id (never empty), in a fixed order:
/// ECDSA (Ethereum) first, then FROST secp256k1 (Bitcoin), then ed25519.
#[derive(Debug, Clone, PartialEq)]
pub struct WalletGroup {
    entries: Vec<WalletMetadata>,
}

/// Canonical curve order inside a group (unknown curves last).
fn curve_rank(curve: &str) -> u8 {
    match curve {
        ECDSA_CURVE => 0,
        "secp256k1" => 1,
        "ed25519" => 2,
        _ => 3,
    }
}

/// Group per-curve keystore entries by wallet id, keeping the order in
/// which ids first appear.
pub fn group_wallets(entries: &[WalletMetadata]) -> Vec<WalletGroup> {
    let mut groups: Vec<WalletGroup> = Vec::new();
    for entry in entries {
        match groups.iter_mut().find(|g| g.id() == entry.session_id) {
            Some(group) => {
                // A curve listed twice (stale cache) counts once.
                if group.entry(&entry.curve_type).is_none() {
                    group.entries.push(entry.clone());
                }
            }
            None => groups.push(WalletGroup {
                entries: vec![entry.clone()],
            }),
        }
    }
    for group in &mut groups {
        group.entries.sort_by_key(|e| curve_rank(&e.curve_type));
    }
    groups
}

impl WalletGroup {
    /// The wallet id shared by every entry.
    pub fn id(&self) -> &str {
        &self.entries[0].session_id
    }

    /// The first entry (canonical order) — for the fields every curve
    /// shares: threshold, total, label, device, creation time.
    pub fn primary(&self) -> &WalletMetadata {
        &self.entries[0]
    }

    pub fn entries(&self) -> &[WalletMetadata] {
        &self.entries
    }

    pub fn display_name(&self) -> &str {
        self.entries
            .iter()
            .find_map(|e| e.label.as_deref())
            .unwrap_or_else(|| self.id())
    }

    pub fn threshold(&self) -> u16 {
        self.primary().threshold
    }

    pub fn total_participants(&self) -> u16 {
        self.primary().total_participants
    }

    /// The entry of `curve`, if this wallet has that key.
    pub fn entry(&self, curve: &str) -> Option<&WalletMetadata> {
        self.entries.iter().find(|e| e.curve_type == curve)
    }

    /// Account `account`'s `(chain, path, address)` on every curve, derived
    /// public-only from the root keys (Ethereum first).
    pub fn accounts(&self, account: u32) -> Vec<(String, String, String)> {
        self.entries
            .iter()
            .filter_map(|e| {
                let group = hex::decode(&e.group_public_key).ok()?;
                starlab_core::accounts::account_addresses(&e.curve_type, &group, account).ok()
            })
            .flatten()
            .collect()
    }

    /// Short chain list for a list row, e.g. "ETH BTC SOL SUI".
    pub fn chains_label(&self) -> String {
        let label = self
            .accounts(0)
            .iter()
            .map(|(chain, _, _)| match chain.as_str() {
                "Ethereum" => "ETH".to_string(),
                "Bitcoin" => "BTC".to_string(),
                "Solana" => "SOL".to_string(),
                other => other.to_uppercase(),
            })
            .collect::<Vec<_>>()
            .join(" ");
        if label.is_empty() {
            "(no accounts)".to_string()
        } else {
            label
        }
    }

    /// Chains this wallet can sign for on a node running the FROST suite
    /// `runner_curve` (`"secp256k1"` or `"ed25519"`): Ethereum needs the
    /// ECDSA key, Bitcoin the Taproot key, Solana the ed25519 key.
    pub fn sign_chains(&self, runner_curve: &str) -> Vec<&'static str> {
        let mut chains = Vec::new();
        if runner_curve == "secp256k1" {
            if self.entry(ECDSA_CURVE).is_some() {
                chains.push("ethereum");
            }
            if self.entry("secp256k1").is_some() {
                chains.push("bitcoin");
            }
        } else if self.entry("ed25519").is_some() {
            chains.push("solana");
        }
        chains
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECP_KEY: &str = "021de2d69979f0a03ea413e7ed6a32ad02111b90d1f03793649157d3e4ee952143";
    const ED_KEY: &str = "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29";

    fn entry(id: &str, curve: &str) -> WalletMetadata {
        let key = if curve == "ed25519" { ED_KEY } else { SECP_KEY };
        WalletMetadata::new(
            id.to_string(),
            "dev".to_string(),
            curve.to_string(),
            2,
            3,
            1,
            key.to_string(),
        )
    }

    #[test]
    fn curve_files_of_one_id_form_one_wallet_with_every_account() {
        let groups = group_wallets(&[
            entry("w1", "ed25519"),
            entry("w1", "secp256k1"),
            entry("w1", ECDSA_CURVE),
        ]);
        assert_eq!(groups.len(), 1);
        let chains: Vec<String> = groups[0]
            .accounts(0)
            .into_iter()
            .map(|(chain, _, _)| chain)
            .collect();
        assert_eq!(chains[0], "Ethereum", "the ECDSA key comes first");
        assert!(chains.contains(&"Bitcoin".to_string()));
        assert!(chains.contains(&"Solana".to_string()));
        assert_eq!(groups[0].chains_label(), "ETH BTC SOL SUI");
    }

    #[test]
    fn ethereum_comes_from_the_ecdsa_key_only() {
        let group = &group_wallets(&[entry("w1", "secp256k1")])[0];
        assert_eq!(group.chains_label(), "BTC");
        assert_eq!(group.sign_chains("secp256k1"), vec!["bitcoin"]);
    }

    #[test]
    fn ed25519_only_wallet_is_solana() {
        let group = &group_wallets(&[entry("w1", "ed25519")])[0];
        assert_eq!(group.chains_label(), "SOL SUI");
        assert_eq!(group.sign_chains("ed25519"), vec!["solana"]);
        assert!(group.sign_chains("secp256k1").is_empty());
    }

    #[test]
    fn two_wallets_stay_two_rows_in_first_seen_order() {
        let groups = group_wallets(&[
            entry("b", "secp256k1"),
            entry("a", "secp256k1"),
            entry("b", ECDSA_CURVE),
            entry("a", ECDSA_CURVE),
        ]);
        let ids: Vec<&str> = groups.iter().map(|g| g.id()).collect();
        assert_eq!(ids, ["b", "a"]);
        assert_eq!(
            groups[0].sign_chains("secp256k1"),
            vec!["ethereum", "bitcoin"]
        );
    }

    #[test]
    fn label_of_any_entry_names_the_wallet() {
        let mut labelled = entry("w1", "secp256k1");
        labelled.label = Some("Savings".into());
        let group = &group_wallets(&[entry("w1", ECDSA_CURVE), labelled])[0];
        assert_eq!(group.display_name(), "Savings");
    }

    #[test]
    fn a_duplicated_curve_entry_counts_once() {
        let groups = group_wallets(&[entry("w1", "secp256k1"), entry("w1", "secp256k1")]);
        assert_eq!(groups[0].entries().len(), 1);
    }
}
