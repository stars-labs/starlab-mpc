// Real FROST DKG + Signing demonstration for offline mode
// Uses actual FROST cryptographic protocol, not mock data

use frost_secp256k1::{
    Identifier, 
    keys::dkg::{self, round1, round2},
    keys::{KeyPackage, PublicKeyPackage},
    round1::{SigningCommitments, SigningNonces},
    round2::SignatureShare,
    SigningPackage, Signature,
};
// Use OsRng from frost_ed25519 for compatibility
use frost_ed25519::rand_core::OsRng;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::fs;
use tempfile::TempDir;
use serde::{Serialize, Deserialize};

/// Simulated SD card for offline data exchange
#[derive(Clone)]
struct MockSDCard {
    base_dir: PathBuf,
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MockSDCard {
    fn new(base_dir: PathBuf) -> Self {
        fs::create_dir_all(&base_dir).unwrap();
        Self {
            base_dir,
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    fn export(&self, filename: &str, data: Vec<u8>) {
        let mut files = self.files.lock().unwrap();
        files.insert(filename.to_string(), data.clone());
        let filepath = self.base_dir.join(filename);
        fs::write(filepath, data).unwrap();
        println!("  📤 Exported: {}", filename);
    }
    
    fn import(&self, filename: &str) -> Option<Vec<u8>> {
        let files = self.files.lock().unwrap();
        if let Some(data) = files.get(filename) {
            println!("  📥 Imported: {}", filename);
            Some(data.clone())
        } else {
            None
        }
    }
    
    fn clear_signing_data(&self) {
        let mut files = self.files.lock().unwrap();
        let signing_files: Vec<String> = files.keys()
            .filter(|k| k.contains("signing") || k.contains("nonce") || k.contains("signature"))
            .cloned()
            .collect();
        
        for file in signing_files {
            files.remove(&file);
        }
        println!("  🗑️ Cleared previous signing data");
    }
}

/// Serializable wrapper for DKG data exchange
#[derive(Serialize, Deserialize)]
struct DKGRound1Data {
    participant_id: u16,
    package: Vec<u8>,  // Serialized round1::Package
}

#[derive(Serialize, Deserialize)]
struct DKGRound2Data {
    from_id: u16,
    to_id: u16,
    package: Vec<u8>,  // Serialized round2::Package
}

#[derive(Serialize, Deserialize)]
struct SigningCommitmentData {
    participant_id: u16,
    commitments: Vec<u8>,  // Serialized SigningCommitments
}

#[derive(Serialize, Deserialize)]
struct SignatureShareData {
    participant_id: u16,
    share: Vec<u8>,  // Serialized SignatureShare
}

/// Participant in the FROST DKG and signing protocol
struct FrostParticipant {
    id: u16,
    identifier: Identifier,
    is_coordinator: bool,
    sd_card: MockSDCard,
    
    // DKG state
    round1_secret: Option<round1::SecretPackage>,
    round2_secret: Option<round2::SecretPackage>,
    key_package: Option<KeyPackage>,
    pubkey_package: Option<PublicKeyPackage>,
    
    // Signing state
    signing_nonces: Option<SigningNonces>,
}

impl FrostParticipant {
    fn new(id: u16, is_coordinator: bool, sd_card: MockSDCard) -> Self {
        let identifier = Identifier::try_from(id).expect("Invalid identifier");
        Self {
            id,
            identifier,
            is_coordinator,
            sd_card,
            round1_secret: None,
            round2_secret: None,
            key_package: None,
            pubkey_package: None,
            signing_nonces: None,
        }
    }
    
    // ============= DKG PROCESS =============
    
    fn dkg_round1(&mut self, threshold: u16, total_participants: u16) {
        println!("\n[P{}] 🔑 DKG Round 1: Generating commitments", self.id);
        
        let rng = OsRng;
        
        // Generate round 1 packages using real FROST
        let (secret_package, public_package) = dkg::part1(
            self.identifier,
            total_participants,
            threshold,
            rng,
        ).expect("Failed to generate DKG round 1");
        
        // Store secret for later rounds
        self.round1_secret = Some(secret_package);
        
        // Export public package for distribution
        let package_bytes = public_package.serialize()
            .expect("Failed to serialize round1 package");
        
        let data = DKGRound1Data {
            participant_id: self.id,
            package: package_bytes,
        };
        
        let filename = format!("dkg_round1_p{}.json", self.id);
        self.sd_card.export(&filename, serde_json::to_vec(&data).unwrap());
        println!("  ✅ Generated and exported Round 1 commitment");
    }
    
    fn collect_round1_packages(&self, total_participants: u16) -> BTreeMap<Identifier, round1::Package> {
        println!("\n[P{}] 📦 Collecting Round 1 packages", self.id);
        
        let mut packages = BTreeMap::new();
        
        for p_id in 1..=total_participants {
            let filename = format!("dkg_round1_p{}.json", p_id);
            if let Some(data) = self.sd_card.import(&filename) {
                let round1_data: DKGRound1Data = serde_json::from_slice(&data).unwrap();
                let identifier = Identifier::try_from(round1_data.participant_id).unwrap();
                let package = round1::Package::deserialize(&round1_data.package)
                    .expect("Failed to deserialize round1 package");
                packages.insert(identifier, package);
            }
        }
        
        println!("  ✅ Collected {} Round 1 packages", packages.len());
        packages
    }
    
    fn dkg_round2(&mut self, round1_packages: BTreeMap<Identifier, round1::Package>) {
        println!("\n[P{}] 🔐 DKG Round 2: Generating shares", self.id);
        
        // Remove our own package (as per FROST spec)
        let mut others_packages = round1_packages.clone();
        others_packages.remove(&self.identifier);
        
        // Generate round 2 packages using real FROST
        let (secret_package, public_packages) = dkg::part2(
            self.round1_secret.clone().expect("Missing round1 secret"),
            &others_packages,
        ).expect("Failed to generate DKG round 2");
        
        // Store secret for part3
        self.round2_secret = Some(secret_package);
        
        // Export packages for each other participant
        for (to_identifier, package) in public_packages {
            let package_bytes = package.serialize()
                .expect("Failed to serialize round2 package");
            
            let data = DKGRound2Data {
                from_id: self.id,
                // We need to track the identifier mapping ourselves
                // Since we can't easily convert back from Identifier to u16
                to_id: {
                    // For this demo, we'll extract from the loop context
                    // In real implementation, you'd maintain a mapping
                    if to_identifier == Identifier::try_from(1).unwrap() { 1 }
                    else if to_identifier == Identifier::try_from(2).unwrap() { 2 }
                    else { 3 }
                },
                package: package_bytes,
            };
            
            let to_id_num = {
                if to_identifier == Identifier::try_from(1).unwrap() { 1 }
                else if to_identifier == Identifier::try_from(2).unwrap() { 2 }
                else { 3 }
            };
            let filename = format!("dkg_round2_from_p{}_to_p{}.json", self.id, to_id_num);
            self.sd_card.export(&filename, serde_json::to_vec(&data).unwrap());
        }
        
        println!("  ✅ Generated and exported Round 2 shares");
    }
    
    fn collect_round2_packages(&self) -> BTreeMap<Identifier, round2::Package> {
        println!("\n[P{}] 📦 Collecting Round 2 packages for me", self.id);
        
        let mut packages = BTreeMap::new();
        
        // Look for packages addressed to us
        for p_id in 1..=3 {
            if p_id != self.id {
                let filename = format!("dkg_round2_from_p{}_to_p{}.json", p_id, self.id);
                if let Some(data) = self.sd_card.import(&filename) {
                    let round2_data: DKGRound2Data = serde_json::from_slice(&data).unwrap();
                    let from_identifier = Identifier::try_from(round2_data.from_id).unwrap();
                    let package = round2::Package::deserialize(&round2_data.package)
                        .expect("Failed to deserialize round2 package");
                    packages.insert(from_identifier, package);
                }
            }
        }
        
        println!("  ✅ Collected {} Round 2 packages", packages.len());
        packages
    }
    
    fn dkg_finalize(&mut self, round1_packages: BTreeMap<Identifier, round1::Package>) {
        println!("\n[P{}] ✨ DKG Finalization", self.id);
        
        // Remove our own round1 package
        let mut others_round1 = round1_packages.clone();
        others_round1.remove(&self.identifier);
        
        // Collect round2 packages meant for us
        let round2_packages = self.collect_round2_packages();
        
        // Run FROST part3 to complete DKG
        let (key_package, pubkey_package) = dkg::part3(
            self.round2_secret.as_ref().expect("Missing round2 secret"),
            &others_round1,
            &round2_packages,
        ).expect("Failed to complete DKG part3");
        
        // Store key material
        self.key_package = Some(key_package.clone());
        self.pubkey_package = Some(pubkey_package.clone());
        
        // Export public key for verification
        let verifying_key = pubkey_package.verifying_key();
        let _verifying_key_bytes = verifying_key.serialize().expect("Failed to serialize key");
        
        if self.is_coordinator {
            // Generate Ethereum address from public key
            use sha3::{Digest, Keccak256};
            
            // Get verifying key bytes
            let verifying_key_bytes = verifying_key.serialize().expect("Failed to serialize key");
            
            // Generate a simple ETH address representation from the public key
            // (In production, you'd properly decompress and hash the key)
            let mut hasher = Keccak256::new();
            hasher.update(&verifying_key_bytes);
            let hash = hasher.finalize();
            let eth_address = format!("0x{}", hex::encode(&hash[12..]));
            
            println!("  ✅ DKG Complete!");
            println!("  🔑 Group Public Key: {}", hex::encode(&verifying_key_bytes));
            println!("  💼 Ethereum Address: {}", eth_address);
            println!("  📊 Threshold: {}/{}", key_package.min_signers(), 3);
        } else {
            println!("  ✅ Key share stored securely");
            println!("  🔑 My identifier: {:?}", self.identifier);
        }
    }
    
    // ============= SIGNING PROCESS =============
    
    fn generate_signing_nonces(&mut self) -> SigningCommitments {
        println!("\n[P{}] 🎲 Generating signing nonces", self.id);
        
        let mut rng = OsRng;
        let (nonces, commitments) = frost_secp256k1::round1::commit(
            self.key_package.as_ref().expect("Missing key package").signing_share(),
            &mut rng,
        );
        
        self.signing_nonces = Some(nonces);
        
        // Export commitments for coordinator
        let commitment_bytes = commitments.serialize()
            .expect("Failed to serialize commitments");
        
        let data = SigningCommitmentData {
            participant_id: self.id,
            commitments: commitment_bytes,
        };
        
        let filename = format!("signing_commitment_p{}.json", self.id);
        self.sd_card.export(&filename, serde_json::to_vec(&data).unwrap());
        
        println!("  ✅ Generated nonces and exported commitments");
        commitments
    }
    
    #[allow(dead_code)] // Referenced below in a block that's not wired up yet.
    fn collect_signing_commitments(&self, signers: &[u16]) -> BTreeMap<Identifier, SigningCommitments> {
        println!("\n[P{}] 📦 Collecting signing commitments", self.id);
        
        let mut commitments = BTreeMap::new();
        
        for &signer_id in signers {
            let filename = format!("signing_commitment_p{}.json", signer_id);
            if let Some(data) = self.sd_card.import(&filename) {
                let commitment_data: SigningCommitmentData = serde_json::from_slice(&data).unwrap();
                let identifier = Identifier::try_from(commitment_data.participant_id).unwrap();
                let commitment = SigningCommitments::deserialize(&commitment_data.commitments)
                    .expect("Failed to deserialize commitment");
                commitments.insert(identifier, commitment);
            }
        }
        
        println!("  ✅ Collected {} signing commitments", commitments.len());
        commitments
    }
    
    fn generate_signature_share(&self, message: &[u8], signing_commitments: &BTreeMap<Identifier, SigningCommitments>) -> SignatureShare {
        println!("\n[P{}] ✍️ Generating signature share", self.id);
        
        // Create signing package
        let signing_package = SigningPackage::new(signing_commitments.clone(), message);
        
        // Generate signature share using real FROST
        let signature_share = frost_secp256k1::round2::sign(
            &signing_package,
            self.signing_nonces.as_ref().expect("Missing signing nonces"),
            self.key_package.as_ref().expect("Missing key package"),
        ).expect("Failed to generate signature share");
        
        // Export signature share
        let share_bytes = signature_share.serialize();
        
        let data = SignatureShareData {
            participant_id: self.id,
            share: share_bytes.clone(),
        };
        
        let filename = format!("signature_share_p{}.json", self.id);
        self.sd_card.export(&filename, serde_json::to_vec(&data).unwrap());
        
        println!("  ✅ Generated signature share");
        println!("  🔐 Share: {}", hex::encode(&share_bytes[..8.min(share_bytes.len())]));
        
        signature_share
    }
    
    fn aggregate_signatures(&self, message: &[u8], signing_commitments: BTreeMap<Identifier, SigningCommitments>, signers: &[u16]) -> Signature {
        println!("\n[P{}] 🔗 Aggregating signature shares", self.id);
        
        // Collect signature shares
        let mut signature_shares = BTreeMap::new();
        
        for &signer_id in signers {
            let filename = format!("signature_share_p{}.json", signer_id);
            if let Some(data) = self.sd_card.import(&filename) {
                let share_data: SignatureShareData = serde_json::from_slice(&data).unwrap();
                let identifier = Identifier::try_from(share_data.participant_id).unwrap();
                let share = SignatureShare::deserialize(&share_data.share)
                    .expect("Failed to deserialize signature share");
                signature_shares.insert(identifier, share);
            }
        }
        
        println!("  📊 Collected {} signature shares", signature_shares.len());
        
        // Create signing package for aggregation
        let signing_package = SigningPackage::new(signing_commitments, message);
        
        // Aggregate signature using real FROST
        let group_signature = frost_secp256k1::aggregate(
            &signing_package,
            &signature_shares,
            self.pubkey_package.as_ref().expect("Missing pubkey package"),
        ).expect("Failed to aggregate signature");
        
        let signature_bytes = group_signature.serialize().expect("Failed to serialize signature");
        println!("  ✅ Aggregated signature successfully!");
        println!("  📝 Signature: {}", hex::encode(&signature_bytes));
        
        // Verify the signature
        let verifying_key = self.pubkey_package.as_ref().unwrap().verifying_key();
        verifying_key.verify(message, &group_signature)
            .expect("Signature verification failed!");
        println!("  ✅ Signature verified successfully!");
        
        group_signature
    }
}

/// Run complete offline FROST DKG + Signing demo
fn run_frost_demo() {
    println!("🚀 Real FROST DKG + Signing (Offline Mode)");
    println!("==========================================\n");
    
    // Configuration
    let threshold = 2u16;
    let total_participants = 3u16;
    
    println!("📊 Configuration:");
    println!("  • Protocol: FROST (secp256k1)");
    println!("  • Threshold: {}-of-{}", threshold, total_participants);
    println!("  • Mode: Offline (SD Card Exchange)");
    
    // Setup SD card
    let temp_dir = TempDir::new().unwrap();
    let sd_card = MockSDCard::new(temp_dir.path().to_path_buf());
    
    // Create participants
    let mut p1 = FrostParticipant::new(1, true, sd_card.clone());
    let mut p2 = FrostParticipant::new(2, false, sd_card.clone());
    let mut p3 = FrostParticipant::new(3, false, sd_card.clone());
    
    // ============================================
    // PART 1: DKG CEREMONY
    // ============================================
    
    println!("\n╔════════════════════════════════════════╗");
    println!("║        PART 1: DKG CEREMONY            ║");
    println!("╚════════════════════════════════════════╝");
    
    // DKG Round 1
    println!("\n━━━━━━━━━━ DKG ROUND 1 ━━━━━━━━━━");
    p1.dkg_round1(threshold, total_participants);
    p2.dkg_round1(threshold, total_participants);
    p3.dkg_round1(threshold, total_participants);
    
    // Collect Round 1 packages
    let round1_packages = p1.collect_round1_packages(total_participants);
    
    // DKG Round 2
    println!("\n━━━━━━━━━━ DKG ROUND 2 ━━━━━━━━━━");
    p1.dkg_round2(round1_packages.clone());
    p2.dkg_round2(round1_packages.clone());
    p3.dkg_round2(round1_packages.clone());
    
    // DKG Finalization
    println!("\n━━━━━━━━━━ DKG FINALIZATION ━━━━━━━━━━");
    p1.dkg_finalize(round1_packages.clone());
    p2.dkg_finalize(round1_packages.clone());
    p3.dkg_finalize(round1_packages.clone());
    
    println!("\n✅ DKG COMPLETE - Real FROST key shares distributed!");
    
    // ============================================
    // PART 2: TRANSACTION SIGNING
    // ============================================
    
    println!("\n╔════════════════════════════════════════╗");
    println!("║      PART 2: TRANSACTION SIGNING       ║");
    println!("╚════════════════════════════════════════╝");
    
    // Clear previous signing data
    sd_card.clear_signing_data();
    
    // Message to sign (transaction hash)
    let message = b"Transfer 1.5 ETH to 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb7";
    println!("\n📋 Message: {}", String::from_utf8_lossy(message));
    
    // Select signers (2-of-3 threshold)
    let signers = vec![1u16, 2u16];  // P1 and P2 will sign, P3 is offline
    println!("👥 Selected signers: P1, P2 (threshold met: 2/3)");
    
    // Generate nonces and commitments
    println!("\n━━━━━━━━━━ SIGNING ROUND 1: COMMITMENTS ━━━━━━━━━━");
    let c1 = p1.generate_signing_nonces();
    let c2 = p2.generate_signing_nonces();
    println!("\n[P3] ⚠️ Participant offline - proceeding with 2-of-3");
    
    // Coordinator collects commitments
    let mut signing_commitments = BTreeMap::new();
    signing_commitments.insert(p1.identifier, c1);
    signing_commitments.insert(p2.identifier, c2);
    
    // Generate signature shares
    println!("\n━━━━━━━━━━ SIGNING ROUND 2: SHARES ━━━━━━━━━━");
    let _share1 = p1.generate_signature_share(message, &signing_commitments);
    let _share2 = p2.generate_signature_share(message, &signing_commitments);
    
    // Aggregate signatures
    println!("\n━━━━━━━━━━ SIGNATURE AGGREGATION ━━━━━━━━━━");
    let final_signature = p1.aggregate_signatures(message, signing_commitments, &signers);
    
    // ============================================
    // SUMMARY
    // ============================================
    
    println!("\n╔════════════════════════════════════════╗");
    println!("║              SUMMARY                   ║");
    println!("╚════════════════════════════════════════╝");
    
    println!("\n🎉 SUCCESS - Real FROST Protocol Execution!");
    println!("\n📊 Results:");
    println!("  ✅ DKG: {} participants completed real FROST DKG", total_participants);
    println!("  ✅ Key Shares: Distributed using FROST part1, part2, part3");
    println!("  ✅ Signing: {}-of-{} threshold signature generated", signers.len(), total_participants);
    println!("  ✅ Signature: Verified using FROST aggregate()");
    println!("  ✅ Cryptography: Real FROST, not mock data!");
    
    println!("\n🔒 Security Properties:");
    println!("  • No single party knows the full private key");
    println!("  • Any {} parties can sign", threshold);
    println!("  • Fewer than {} parties cannot sign", threshold);
    println!("  • All operations performed offline (air-gapped)");
    
    // Verify final signature one more time for demonstration
    let verifying_key = p1.pubkey_package.as_ref().unwrap().verifying_key();
    match verifying_key.verify(message, &final_signature) {
        Ok(_) => println!("\n✅ Final verification: Signature is valid!"),
        Err(e) => println!("\n❌ Final verification failed: {:?}", e),
    }
}

fn main() {
    run_frost_demo();
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_real_frost_dkg_and_signing() {
        run_frost_demo();
    }
}