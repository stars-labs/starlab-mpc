use clap::{Parser, ValueEnum};
use frost_core::Ciphersuite;
use frost_core::Identifier;
use frost_core::keys::dkg::{part1, part2, part3};
use frost_core::keys::{KeyPackage, PublicKeyPackage};
use frost_ed25519::Ed25519Sha512;
use frost_secp256k1::Secp256K1Sha256;
// Import rand_core directly from frost_ed25519 to ensure version compatibility
use frost_ed25519::rand_core::{CryptoRng, OsRng, RngCore};
use std::collections::BTreeMap;
use std::fmt::Debug;

#[derive(Clone, Debug, ValueEnum)]
enum Curve {
    Secp256k1,
    Ed25519,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Curve to use for DKG
    #[arg(short, long, value_enum, default_value = "secp256k1")]
    curve: Curve,

    /// Maximum number of signers
    #[arg(long, default_value_t = 5)]
    max_signers: u16,

    /// Minimum number of signers required
    #[arg(short, long, default_value_t = 3)]
    min_signers: u16,
}

fn main() {
    let args = Args::parse();

    println!("Starting DKG with:");
    println!("  Curve: {:?}", args.curve);
    println!("  Maximum signers: {}", args.max_signers);
    println!("  Minimum signers: {}", args.min_signers);

    let result = match args.curve {
        Curve::Secp256k1 => run_dkg::<Secp256K1Sha256>(args.max_signers, args.min_signers)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
        Curve::Ed25519 => run_dkg::<Ed25519Sha512>(args.max_signers, args.min_signers)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
    };

    if let Err(e) = result {
        eprintln!("Error in DKG process: {:?}", e);
        std::process::exit(1);
    }
}

// New struct to represent a participant in the DKG protocol
// Update Ciphersuite bound to Ciphersuite (it will have serde capabilities via feature flag)
struct Participant<C: Ciphersuite> {
    id: Identifier<C>,
    round1_secret_package: Option<frost_core::keys::dkg::round1::SecretPackage<C>>,
    round1_package: Option<frost_core::keys::dkg::round1::Package<C>>,
    round2_secret_package: Option<frost_core::keys::dkg::round2::SecretPackage<C>>,
    round1_packages_received: BTreeMap<Identifier<C>, frost_core::keys::dkg::round1::Package<C>>,
    round2_packages_received: BTreeMap<Identifier<C>, frost_core::keys::dkg::round2::Package<C>>,
    key_package: Option<KeyPackage<C>>,
    pubkey_package: Option<PublicKeyPackage<C>>,
}

// Update Ciphersuite bound to Ciphersuite
impl<C: Ciphersuite> Participant<C> {
    fn new(id: Identifier<C>) -> Self {
        Self {
            id,
            round1_secret_package: None,
            round1_package: None,
            round2_secret_package: None,
            round1_packages_received: BTreeMap::new(),
            round2_packages_received: BTreeMap::new(),
            key_package: None,
            pubkey_package: None,
        }
    }

    // Generate and store Round 1 package
    // Fix the trait bounds to match what FROST expects
    fn generate_round1(
        &mut self,
        max_signers: u16,
        min_signers: u16,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> Result<&frost_core::keys::dkg::round1::Package<C>, frost_core::Error<C>> {
        let (secret, package) = part1(self.id, max_signers, min_signers, rng)?;
        self.round1_secret_package = Some(secret);
        self.round1_package = Some(package.clone());
        self.round1_packages_received
            .insert(self.id, package.clone());
        Ok(self.round1_package.as_ref().unwrap())
    }

    // Add a received Round 1 package (now takes serialized package)
    fn add_round1_package(&mut self, sender_id: Identifier<C>, package_json: &str) {
        let package: frost_core::keys::dkg::round1::Package<C> =
            serde_json::from_str(package_json).expect("Failed to deserialize Round 1 package");
        self.round1_packages_received.insert(sender_id, package);
    }

    // Generate Round 2 packages for other participants
    fn generate_round2(
        &mut self,
    ) -> Result<
        BTreeMap<Identifier<C>, frost_core::keys::dkg::round2::Package<C>>,
        frost_core::Error<C>,
    > {
        // Filter out own package
        let round1_packages_from_others: BTreeMap<_, _> = self
            .round1_packages_received
            .iter()
            .filter(|(id, _)| *id != &self.id)
            .map(|(id, pkg)| (*id, pkg.clone()))
            .collect();

        let (secret, packages) = part2(
            self.round1_secret_package.take().unwrap(),
            &round1_packages_from_others,
        )?;

        self.round2_secret_package = Some(secret);
        Ok(packages)
    }

    // Add a received Round 2 package (now takes serialized package)
    fn add_round2_package(&mut self, sender_id: Identifier<C>, package_json: &str) {
        let package: frost_core::keys::dkg::round2::Package<C> =
            serde_json::from_str(package_json).expect("Failed to deserialize Round 2 package");
        self.round2_packages_received.insert(sender_id, package);
    }

    // Finalize the DKG process (Part 3)
    fn finalize_dkg(&mut self) -> Result<(), frost_core::Error<C>> {
        // Filter out own package for round1
        let round1_packages_from_others: BTreeMap<_, _> = self
            .round1_packages_received
            .iter()
            .filter(|(id, _)| *id != &self.id)
            .map(|(id, pkg)| (*id, pkg.clone()))
            .collect();

        let (key_package, pubkey_package) = part3(
            self.round2_secret_package.as_ref().unwrap(),
            &round1_packages_from_others,
            &self.round2_packages_received,
        )?;

        self.key_package = Some(key_package);
        self.pubkey_package = Some(pubkey_package);
        Ok(())
    }
}

// Helper function to distribute Round 1 packages to all participants
// Update Ciphersuite bound
fn distribute_round1_packages<C: Ciphersuite>(
    participants: &mut BTreeMap<Identifier<C>, Participant<C>>,
    packages_json: &BTreeMap<Identifier<C>, String>,
) {
    for (&sender_id, package_str) in packages_json {
        for (receiver_id, receiver) in participants.iter_mut() {
            if *receiver_id != sender_id {
                receiver.add_round1_package(sender_id, package_str.as_str());
            }
        }
    }
}

// Helper function to distribute a Round 2 package to its recipient
// Update Ciphersuite bound
fn distribute_round2_package<C: Ciphersuite>(
    participants: &mut BTreeMap<Identifier<C>, Participant<C>>,
    sender_id: Identifier<C>,
    receiver_id: Identifier<C>,
    package_json: String,
) {
    if let Some(receiver) = participants.get_mut(&receiver_id) {
        receiver.add_round2_package(sender_id, &package_json);
    }
}

// Update Ciphersuite bound
fn run_dkg<C>(max_signers: u16, min_signers: u16) -> Result<(), frost_core::Error<C>>
where
    C: Ciphersuite, // Updated bound
{
    // Use OsRng from the same rand_core version that frost uses
    let mut rng = OsRng;

    println!(
        "\nStarting DKG with max_signers={}, min_signers={}",
        max_signers, min_signers
    );

    // Create participant instances
    let mut participants = BTreeMap::new();
    for i in 1..=max_signers {
        let id = Identifier::try_from(i).expect("should be nonzero");
        participants.insert(id, Participant::new(id));
    }

    // === Round 1: Generate packages ===
    println!("\n=== DKG Round 1 ===");
    let mut round1_packages_json = BTreeMap::new(); // Store serialized packages

    // Each participant generates their Round 1 package
    for (&id, participant) in participants.iter_mut() {
        println!("\nParticipant {:?} generating Round 1 package...", id);
        let package = participant.generate_round1(max_signers, min_signers, &mut rng)?;
        // Serialize the package to JSON
        let package_json =
            serde_json::to_string(&package).expect("Failed to serialize Round 1 package");
        round1_packages_json.insert(id, package_json.clone()); // Store serialized version
        println!(
            "  Participant {:?} generated Round 1 package (serialized): {:?}", // Log serialized version or keep original
            id,
            package_json // package // Or keep original package for logging if preferred
        );
    }

    // Distribute serialized Round 1 packages to all participants
    distribute_round1_packages(&mut participants, &round1_packages_json);

    // === Round 2: Process Round 1 packages and generate Round 2 packages ===
    println!("\n=== DKG Round 2 ===");

    // Each participant processes received Round 1 packages and generates Round 2 packages
    // Collect all (sender_id, receiver_id, serialized_package) into a temporary vector
    let mut round2_to_distribute_json = Vec::new();
    for (&sender_id, sender) in participants.iter_mut() {
        println!(
            "\nParticipant {:?} processing Round 1 packages and generating Round 2 packages...",
            sender_id
        );

        // Generate Round 2 packages
        let round2_packages_map = sender.generate_round2()?;

        println!(
            "  Participant {:?} generated Round 2 packages for others: {:?}",
            sender_id,
            round2_packages_map.keys().collect::<Vec<_>>()
        );

        // Collect and serialize Round 2 packages to distribute later
        for (receiver_id, package) in round2_packages_map {
            let package_json =
                serde_json::to_string(&package).expect("Failed to serialize Round 2 package");
            round2_to_distribute_json.push((sender_id, receiver_id, package_json));
        }
    }

    // Distribute serialized Round 2 packages to each recipient
    for (sender_id, receiver_id, package_json) in round2_to_distribute_json {
        distribute_round2_package(&mut participants, sender_id, receiver_id, package_json);
    }

    // === Part 3: Finalize DKG ===
    println!("\n=== DKG Final Computation ===");

    // Each participant finalizes the DKG process
    for (&id, participant) in participants.iter_mut() {
        println!("\nParticipant {:?} performing final computation...", id);

        participant.finalize_dkg()?;

        println!(
            "  Participant {:?} generated KeyPackage: {:?}",
            id,
            participant.key_package.as_ref().unwrap()
        );
        println!();
    }

    // Collect all final key packages and public key packages
    let mut key_packages = BTreeMap::new();
    let mut pubkey_packages = BTreeMap::new();

    for (&id, participant) in &participants {
        key_packages.insert(id, participant.key_package.clone().unwrap());
        pubkey_packages.insert(id, participant.pubkey_package.clone().unwrap());
    }

    // Verify that all participants have the same public key package
    println!("\n=== DKG Complete ===");
    println!("Generated Key Packages:\n{:#?}", key_packages);
    println!("Generated Public Key Packages:\n{:#?}", pubkey_packages);

    let first_pubkey_package = pubkey_packages.values().next().unwrap();
    for (id, pkp) in &pubkey_packages {
        if pkp != first_pubkey_package {
            panic!(
                "Error: Participant {:?} has a different PublicKeyPackage!",
                id
            );
        }
    }
    println!("\nAll participants generated the same PublicKeyPackage successfully.");
    println!(
        "Group Verifying Key: {:?}",
        first_pubkey_package.verifying_key()
    );

    println!("\nDKG process finished successfully.");
    Ok(())
}
