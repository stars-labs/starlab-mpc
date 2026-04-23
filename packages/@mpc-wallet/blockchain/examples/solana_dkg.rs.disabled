use bincode::serde::{decode_from_slice, encode_to_vec};
use clap::{Parser, arg, command}; // Fix clap imports
use frost::Identifier;
use frost::rand_core::OsRng;
use frost_core::SigningPackage;
use frost_core::keys::dkg::{round1, round2};
use frost_core::keys::{KeyPackage, PublicKeyPackage};
use frost_core::{round1 as frost_round1, round2 as frost_round2};
use frost_ed25519 as frost;
use frost_ed25519::Ed25519Sha512;
use hex;
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message, pubkey::Pubkey, signature::Signature,
    transaction::Transaction,
};
use std::collections::BTreeMap;
use std::convert::TryInto;

use std::error::Error;
use std::fs::{self};
use std::io::{self, Read, Write, stdin};
use std::net::{TcpListener, TcpStream};
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use std::collections::HashSet;

/// Solana DKG Example CLI
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct CliArgs {
    /// Your node index (1-based)
    #[arg(long, short)]
    index: u16,
    /// Total number of participants
    #[arg(long, short)]
    total: u16,
    /// Threshold for signing
    #[arg(short = 'k', long)]
    threshold: u16,
    /// Is this node the initiator?
    #[arg(long, default_value_t = false)]
    is_initiator: bool,
    /// Wait for all nodes to join before proceeding (default: true)
    #[arg(long, default_value_t = true)]
    wait_for_all: bool,
}

// --- New Message Types for Discovery ---
#[derive(Serialize, Deserialize, Clone, Debug)]
struct PingMessage {
    sender_index: u16,
    timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct PongMessage {
    sender_index: u16,
    responding_to: u16,
    timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ReadyMessage {
    sender_index: u16,
    ready_nodes: Vec<u16>,
}
#[derive(Serialize, Deserialize, Clone, Debug)] // Add Debug
struct Round1Message {
    participant_index: u16,
    package: round1::Package<Ed25519Sha512>,
}

#[derive(Serialize, Deserialize, Clone, Debug)] // Add Debug
struct Round2Message {
    participant_index: u16,
    package: round2::Package<Ed25519Sha512>,
}

// --- DKG Message Types ---
#[derive(Serialize, Deserialize, Clone, Debug)] // Add Debug
struct TxMessage {
    message_bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug)] // Add Debug
struct CommitmentMessage {
    sender_identifier: Identifier, // Removed <Ed25519Sha512>
    commitment: frost_round1::SigningCommitments<Ed25519Sha512>,
}

#[derive(Serialize, Deserialize, Clone, Debug)] // Add Debug
struct ShareMessage {
    sender_identifier: Identifier, // Removed <Ed25519Sha512>
    share: frost_round2::SignatureShare<Ed25519Sha512>,
}

// --- Signing Message Types ---
#[derive(Serialize, Deserialize, Clone, Debug)] // Add Debug
struct AggregatedSignatureMessage {
    signature_bytes: Vec<u8>,
}

// --- New Message Type for Signer Selection ---
#[derive(Serialize, Deserialize, Clone, Debug)] // Add Debug
struct SignerSelectionMessage {
    selected_identifiers: Vec<Identifier>,
}

// --- Generic Message Wrapper ---
#[derive(Serialize, Deserialize, Clone, Debug)] // Add Debug
enum MessageWrapper {
    // Discovery messages
    Ping(PingMessage),
    Pong(PongMessage),
    Ready(ReadyMessage),
    // DKG messages
    DkgRound1(Round1Message),
    DkgRound2(Round2Message),
    // Signing messages
    SignTx(TxMessage),
    SignCommitment(CommitmentMessage),
    SignShare(ShareMessage),
    SignAggregated(AggregatedSignatureMessage),
    SignerSelection(SignerSelectionMessage),
}

// Remove parse_args function
// fn parse_args() -> (u16, u16, u16, bool) { ... }

// Simple send/recv helpers for TCP with better error handling
fn send_to(addr: &str, msg: &MessageWrapper) -> bool {
    let data = encode_to_vec(msg, bincode::config::standard()).unwrap();
    match TcpStream::connect(addr) {
        Ok(mut stream) => match stream.write_all(&data) {
            Ok(_) => {
                println!("Successfully sent message to {}", addr);
                true
            }
            Err(e) => {
                // Only print error for non-connection issues
                eprintln!("Failed to write to {}: {}", addr, e);
                false
            }
        },
        Err(_) => {
            // Connection refused is expected when nodes aren't ready - don't spam logs
            false
        }
    }
}

// Try to send with retries but less aggressive
fn send_to_with_retries(addr: &str, msg: &MessageWrapper, max_retries: u32) -> bool {
    for attempt in 0..=max_retries {
        if send_to(addr, msg) {
            return true;
        }
        if attempt < max_retries {
            thread::sleep(Duration::from_millis(100 * (attempt as u64 + 1)));
        }
    }
    false
}

// Mesh discovery: Wait for all nodes to join the cluster
fn discover_all_peers(index: u16, total: u16) -> HashSet<u16> {
    let mut discovered = HashSet::new();
    discovered.insert(index); // Always include self
    
    println!("Node {} starting peer discovery, waiting for all {} nodes...", index, total);
    
    // Set up listener for discovery messages
    let listener = match TcpListener::bind(format!("127.0.0.1:1000{}", index)) {
        Ok(l) => {
            println!("Node {} listening on 127.0.0.1:1000{}", index, index);
            l
        }
        Err(e) => {
            eprintln!("Failed to bind listener: {}", e);
            return discovered;
        }
    };
    
    // Make listener non-blocking for discovery phase
    if let Err(e) = listener.set_nonblocking(true) {
        eprintln!("Failed to set non-blocking: {}", e);
        return discovered;
    }
    
    let mut last_ping_time = Instant::now();
    let ping_interval = Duration::from_secs(2);
    
    // Continue until we've discovered all nodes
    while discovered.len() < total as usize {
        // Send pings periodically
        if last_ping_time.elapsed() >= ping_interval {
            let ping_msg = PingMessage {
                sender_index: index,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };
            let wrapped_msg = MessageWrapper::Ping(ping_msg);
            
            // Try to ping all other nodes
            for target_idx in 1..=total {
                if target_idx != index && !discovered.contains(&target_idx) {
                    let target_addr = format!("127.0.0.1:1000{}", target_idx);
                    send_to(&target_addr, &wrapped_msg);
                }
            }
            last_ping_time = Instant::now();
            
            if discovered.len() == total as usize {
                println!("Node {} discovered all {} nodes: {:?}", index, total, discovered);
            } else {
                println!("Node {} discovered {}/{} nodes: {:?}", 
                        index, discovered.len(), total, discovered);
            }
        }
        
        // Listen for incoming messages
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let mut buf = Vec::new();
                if stream.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
                    if let Ok((msg, _)) = decode_from_slice(&buf, bincode::config::standard()) {
                        match msg {
                            MessageWrapper::Ping(ping) => {
                                discovered.insert(ping.sender_index);
                                // Send pong back
                                let pong_msg = PongMessage {
                                    sender_index: index,
                                    responding_to: ping.sender_index,
                                    timestamp: ping.timestamp,
                                };
                                let wrapped_msg = MessageWrapper::Pong(pong_msg);
                                let target_addr = format!("127.0.0.1:1000{}", ping.sender_index);
                                send_to(&target_addr, &wrapped_msg);
                            }
                            MessageWrapper::Pong(pong) => {
                                if pong.responding_to == index {
                                    discovered.insert(pong.sender_index);
                                }
                            }
                            _ => {} // Ignore other message types during discovery
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // No incoming connection, continue
                thread::sleep(Duration::from_millis(50));
            }
            Err(_) => {
                // Other errors, continue
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
    
    println!("Node {} discovery complete! Found all {} nodes: {:?}", index, total, discovered);
    discovered
}

// Broadcast to discovered peers only
fn broadcast_to_peers(index: u16, peers: &HashSet<u16>, msg: &MessageWrapper) {
    println!("Node {} broadcasting message to {} peers...", index, peers.len() - 1);
    let mut success_count = 0;
    for &peer_idx in peers {
        if peer_idx == index {
            continue;
        }
        let peer_addr = format!("127.0.0.1:1000{}", peer_idx);
        if send_to_with_retries(&peer_addr, msg, 3) {
            success_count += 1;
        }
    }
    println!(
        "Node {} broadcast completed: {}/{} successful",
        index,
        success_count,
        peers.len() - 1
    );
}

// Receive messages with optional timeout - adding better message type identification
fn receive_messages<T>(
    listener: &TcpListener,
    expected_count: usize,
    timeout: Option<Duration>, // Use Option<Duration>
    extract_fn: fn(MessageWrapper) -> Option<T>,
) -> Result<Vec<T>, Box<dyn Error>> {
    let mut messages = Vec::with_capacity(expected_count);
    let start_time = Instant::now();

    if let Some(duration) = timeout {
        // --- Timeout Logic ---
        listener.set_nonblocking(true)?;
        while messages.len() < expected_count && start_time.elapsed() < duration {
            match listener.accept() {
                Ok((mut stream, addr)) => {
                    listener.set_nonblocking(false)?; // Set back to blocking for read
                    println!("Accepted connection from {}", addr);
                    // Set read timeout relative to overall timeout
                    let remaining_time = duration.saturating_sub(start_time.elapsed());
                    if remaining_time == Duration::ZERO {
                        println!("Timeout expired before reading from {}", addr);
                        listener.set_nonblocking(true)?;
                        continue;
                    }
                    stream.set_read_timeout(Some(remaining_time))?;

                    let mut buf = Vec::new();
                    match stream.read_to_end(&mut buf) {
                        Ok(_) => {
                            if buf.is_empty() {
                                println!("Received empty message from {}", addr);
                            } else {
                                let wrapped_msg: MessageWrapper =
                                    match decode_from_slice(&buf, bincode::config::standard()) {
                                        Ok((msg, _)) => msg,
                                        Err(e) => {
                                            eprintln!(
                                                "Failed to decode message from {}: {}",
                                                addr, e
                                            );
                                            listener.set_nonblocking(true)?;
                                            continue;
                                        }
                                    };

                                // Debug message type
                                match &wrapped_msg {
                                    MessageWrapper::Ping(_) => {
                                        println!("Received Ping message from {}", addr)
                                    }
                                    MessageWrapper::Pong(_) => {
                                        println!("Received Pong message from {}", addr)
                                    }
                                    MessageWrapper::Ready(_) => {
                                        println!("Received Ready message from {}", addr)
                                    }
                                    MessageWrapper::DkgRound1(_) => {
                                        println!("Received DkgRound1 message from {}", addr)
                                    }
                                    MessageWrapper::DkgRound2(_) => {
                                        println!("Received DkgRound2 message from {}", addr)
                                    }
                                    MessageWrapper::SignTx(_) => {
                                        println!("Received SignTx message from {}", addr)
                                    }
                                    MessageWrapper::SignCommitment(_) => {
                                        println!("Received SignCommitment message from {}", addr)
                                    }
                                    MessageWrapper::SignShare(_) => {
                                        println!("Received SignShare message from {}", addr)
                                    }
                                    MessageWrapper::SignAggregated(_) => {
                                        println!("Received SignAggregated message from {}", addr)
                                    }
                                    MessageWrapper::SignerSelection(_) => {
                                        println!("Received SignerSelection message from {}", addr)
                                    }
                                }

                                if let Some(msg) = extract_fn(wrapped_msg) {
                                    messages.push(msg);
                                } else {
                                    println!(
                                        "Received unexpected message type from {} (ignoring)",
                                        addr
                                    );
                                }
                            }
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            println!("Read timeout from {}", addr);
                        }
                        Err(e) => {
                            eprintln!("Error reading from stream {}: {}", addr, e);
                        }
                    }
                    listener.set_nonblocking(true)?; // Set back to non-blocking
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No incoming connection, wait briefly
                    thread::sleep(Duration::from_millis(50));
                    continue;
                }
                Err(e) => {
                    listener.set_nonblocking(false)?; // Ensure blocking on error
                    eprintln!("Error accepting connection: {}", e);
                    return Err(e.into());
                }
            }
        }
        listener.set_nonblocking(false)?; // Ensure blocking on exit

        if messages.len() < expected_count {
            println!(
                "Warning: Timed out waiting for messages. Expected {}, got {}.",
                expected_count,
                messages.len()
            );
        }
    } else {
        // --- Blocking Logic ---
        while messages.len() < expected_count {
            let (mut stream, addr) = listener.accept()?; // Blocks here
            println!("Accepted connection from {}", addr);
            let mut buf = Vec::new();
            match stream.read_to_end(&mut buf) {
                // Blocks here
                Ok(_) => {
                    if buf.is_empty() {
                        println!("Received empty message from {}", addr);
                        continue;
                    }
                    let wrapped_msg: MessageWrapper =
                        match decode_from_slice(&buf, bincode::config::standard()) {
                            Ok((msg, _)) => msg,
                            Err(e) => {
                                eprintln!("Failed to decode message from {}: {}", addr, e);
                                continue;
                            }
                        };

                    // Debug message type
                    match &wrapped_msg {
                        MessageWrapper::Ping(_) => {
                            println!("Received Ping message from {}", addr)
                        }
                        MessageWrapper::Pong(_) => {
                            println!("Received Pong message from {}", addr)
                        }
                        MessageWrapper::Ready(_) => {
                            println!("Received Ready message from {}", addr)
                        }
                        MessageWrapper::DkgRound1(_) => {
                            println!("Received DkgRound1 message from {}", addr)
                        }
                        MessageWrapper::DkgRound2(_) => {
                            println!("Received DkgRound2 message from {}", addr)
                        }
                        MessageWrapper::SignTx(_) => {
                            println!("Received SignTx message from {}", addr)
                        }
                        MessageWrapper::SignCommitment(_) => {
                            println!("Received SignCommitment message from {}", addr)
                        }
                        MessageWrapper::SignShare(_) => {
                            println!("Received SignShare message from {}", addr)
                        }
                        MessageWrapper::SignAggregated(_) => {
                            println!("Received SignAggregated message from {}", addr)
                        }
                        MessageWrapper::SignerSelection(_) => {
                            println!("Received SignerSelection message from {}", addr)
                        }
                    }

                    if let Some(msg) = extract_fn(wrapped_msg) {
                        messages.push(msg);
                    } else {
                        println!("Received unexpected message type from {} (ignoring)", addr);
                    }
                }
                Err(e) => {
                    eprintln!("Error reading from stream {}: {}", addr, e);
                }
            }
        }
    }

    Ok(messages)
}

// Main function becomes a simple state machine driver
fn main() -> Result<(), Box<dyn Error>> {
    let args = CliArgs::parse();
    let index = args.index;
    let total = args.total;
    let threshold = args.threshold;
    let is_initiator = args.is_initiator;

    // Create initial context
    let mut context = NodeContext::new(index, total, threshold, is_initiator, args.wait_for_all)?;

    // Main state machine loop
    loop {
        match context.state {
            NodeState::Initial => handle_initial_state(&mut context)?,
            NodeState::Discovery => handle_discovery_state(&mut context)?,
            NodeState::DkgProcess => handle_dkg_process(&mut context)?,
            NodeState::Idle => {
                if is_initiator {
                    context.state = NodeState::TransactionComposition;
                } else {
                    // Wait for transaction from initiator
                    // Set up listener only if it doesn't exist
                    if context.listener.is_none() {
                        context.setup_listener()?;
                    }
                    println!("Node {} in IDLE state, waiting for transaction...", index);
                    let received_tx =
                        receive_messages(context.listener.as_ref().unwrap(), 1, None, |msg| {
                            match msg {
                                MessageWrapper::SignTx(m) => Some(m),
                                _ => None,
                            }
                        })?;
                    context.message_bytes = Some(received_tx[0].message_bytes.clone());
                    context.state = NodeState::SigningCommitment;
                }
            }
            NodeState::TransactionComposition => handle_transaction_composition(&mut context)?,
            NodeState::SigningCommitment => handle_signing_commitment(&mut context)?,
            NodeState::SignerSelection => handle_signer_selection(&mut context)?,
            NodeState::SignatureGeneration { selected } => {
                handle_signature_generation(&mut context, selected)?
            }
            NodeState::SignatureAggregation => handle_signature_aggregation(&mut context)?,
            NodeState::TransactionSubmission => handle_transaction_submission(&mut context)?,
            NodeState::SignatureVerification => handle_signature_verification(&mut context)?,
        }

        // Optional - Add delay between state transitions for clarity
        thread::sleep(Duration::from_millis(100));
    }
    // Unreachable code due to infinite loop above
    // Ok(())
}

// Define the state machine states
#[derive(Debug, Clone, PartialEq)]
enum NodeState {
    Initial,
    Discovery,
    DkgProcess,
    Idle,
    TransactionComposition,
    SigningCommitment,
    SignerSelection,
    SignatureGeneration { selected: bool },
    SignatureAggregation,
    TransactionSubmission,
    SignatureVerification,
}

// Context to be passed between states
struct NodeContext {
    index: u16,
    total: u16,
    threshold: u16,
    is_initiator: bool,
    wait_for_all: bool,
    state: NodeState,
    my_identifier: Identifier,
    discovered_peers: HashSet<u16>,
    key_package: Option<KeyPackage<Ed25519Sha512>>,
    pubkey_package: Option<PublicKeyPackage<Ed25519Sha512>>,
    solana_pubkey: Option<Pubkey>,
    listener: Option<TcpListener>,

    // DKG state
    round1_secret_package: Option<frost_core::keys::dkg::round1::SecretPackage<Ed25519Sha512>>,
    round1_package: Option<round1::Package<Ed25519Sha512>>,
    round2_secret_package: Option<frost_core::keys::dkg::round2::SecretPackage<Ed25519Sha512>>,
    received_round1_packages: Option<BTreeMap<Identifier, round1::Package<Ed25519Sha512>>>,
    received_round2_packages: Option<BTreeMap<Identifier, round2::Package<Ed25519Sha512>>>,

    // Signing state
    message_bytes: Option<Vec<u8>>,
    transaction: Option<Transaction>,
    my_nonce: Option<frost_round1::SigningNonces<Ed25519Sha512>>,
    my_commitment: Option<frost_round1::SigningCommitments<Ed25519Sha512>>,
    commitments_map: Option<BTreeMap<Identifier, frost_round1::SigningCommitments<Ed25519Sha512>>>,
    selected_signers: Option<Vec<Identifier>>,
    my_signature_share: Option<frost_round2::SignatureShare<Ed25519Sha512>>,
    signature_shares_map: Option<BTreeMap<Identifier, frost_round2::SignatureShare<Ed25519Sha512>>>,
    aggregated_signature: Option<Vec<u8>>,

    // Working data
    rng: OsRng,
}

impl NodeContext {
    fn new(
        index: u16,
        total: u16,
        threshold: u16,
        is_initiator: bool,
        wait_for_all: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let my_identifier = Identifier::try_from(index).expect("Invalid identifier");

        Ok(Self {
            index,
            total,
            threshold,
            is_initiator,
            wait_for_all,
            state: NodeState::Initial,
            my_identifier,
            discovered_peers: HashSet::new(),
            key_package: None,
            pubkey_package: None,
            solana_pubkey: None,
            listener: None,
            round1_secret_package: None,
            round1_package: None,
            round2_secret_package: None,
            received_round1_packages: None,
            received_round2_packages: None,
            message_bytes: None,
            transaction: None,
            my_nonce: None,
            my_commitment: None,
            commitments_map: None,
            selected_signers: None,
            my_signature_share: None,
            signature_shares_map: None,
            aggregated_signature: None,
            rng: OsRng,
        })
    }

    fn setup_listener(&mut self) -> Result<(), Box<dyn Error>> {
        let my_addr = format!("127.0.0.1:1000{}", self.index);
        self.listener = Some(TcpListener::bind(&my_addr)?);
        println!("Node {} listening on {}", self.index, my_addr);
        Ok(())
    }
}

// Handle initial state - check for existing keys or start DKG
fn handle_initial_state(context: &mut NodeContext) -> Result<(), Box<dyn Error>> {
    println!("Node {} in INITIAL state", context.index);

    let key_package_file = format!("key_package_{}.bin", context.index);
    let pubkey_package_file = format!("pubkey_package_{}.bin", context.index);

    if fs::metadata(&key_package_file).is_ok() && fs::metadata(&pubkey_package_file).is_ok() {
        println!("Node {} loading keys from cache...", context.index);

        // Load key package
        let key_bytes = match fs::read(&key_package_file) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("Error reading key file {}: {}", key_package_file, e);
                println!("Node {} will generate new keys via DKG.", context.index);
                context.state = NodeState::DkgProcess;
                let _ = fs::remove_file(&key_package_file);
                let _ = fs::remove_file(&pubkey_package_file);
                return Ok(());
            }
        };

        // Load pubkey package
        let pubkey_bytes = match fs::read(&pubkey_package_file) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("Error reading pubkey file {}: {}", pubkey_package_file, e);
                println!("Node {} will generate new keys via DKG.", context.index);
                context.state = NodeState::DkgProcess;
                let _ = fs::remove_file(&key_package_file);
                let _ = fs::remove_file(&pubkey_package_file);
                return Ok(());
            }
        };

        // Decode key package
        let key_package: KeyPackage<Ed25519Sha512> =
            match decode_from_slice(&key_bytes, bincode::config::standard()) {
                Ok((package, _)) => package,
                Err(e) => {
                    eprintln!("Error decoding key package: {}", e);
                    context.state = NodeState::DkgProcess;
                    let _ = fs::remove_file(&key_package_file);
                    let _ = fs::remove_file(&pubkey_package_file);
                    return Ok(());
                }
            };

        // Decode pubkey package
        let pubkey_package: PublicKeyPackage<Ed25519Sha512> =
            match decode_from_slice(&pubkey_bytes, bincode::config::standard()) {
                Ok((package, _)) => package,
                Err(e) => {
                    eprintln!("Error decoding pubkey package: {}", e);
                    context.state = NodeState::DkgProcess;
                    let _ = fs::remove_file(&key_package_file);
                    let _ = fs::remove_file(&pubkey_package_file);
                    return Ok(());
                }
            };

        // Set context values
        context.key_package = Some(key_package);
        context.pubkey_package = Some(pubkey_package);

        // Get the Solana pubkey
        let verifying_key_bytes = context
            .pubkey_package
            .as_ref()
            .unwrap()
            .verifying_key()
            .serialize()?;
        let mut pubkey_arr = [0u8; 32];
        if verifying_key_bytes.len() == 32 {
            pubkey_arr.copy_from_slice(&verifying_key_bytes);
            context.solana_pubkey = Some(Pubkey::new_from_array(pubkey_arr));
        } else {
            return Err(format!("FROST verifying key size is not 32 bytes").into());
        }

        println!("Node {} loaded keys successfully", context.index);
        println!(
            "Node {} derived Solana address: {}",
            context.index,
            context.solana_pubkey.unwrap()
        );

        // Transition to idle state
        context.state = NodeState::Idle;
    } else {
        println!("Node {} no keys found, starting discovery process", context.index);
        context.state = NodeState::Discovery;
    }

    Ok(())
}

// Handle peer discovery state
fn handle_discovery_state(context: &mut NodeContext) -> Result<(), Box<dyn Error>> {
    println!("Node {} in DISCOVERY state", context.index);
    
    if context.wait_for_all {
        // Wait for all nodes to join
        context.discovered_peers = discover_all_peers(context.index, context.total);
        println!("Node {} discovered all {} peers: {:?}", 
                context.index, context.total, context.discovered_peers);
        
        // Proceed to DKG with all nodes
        context.state = NodeState::DkgProcess;
    } else {
        // Use threshold-based discovery (original behavior)
        // This could be implemented as a fallback option
        println!("Node {} using threshold-based discovery (not implemented in this version)", context.index);
        context.state = NodeState::DkgProcess;
    }
    
    Ok(())
}

// Handle the entire DKG process
fn handle_dkg_process(context: &mut NodeContext) -> Result<(), Box<dyn Error>> {
    println!("Node {} in DKG_PROCESS state", context.index);

    // Set up network listener (reuse existing if available)
    if context.listener.is_none() {
        context.setup_listener()?;
    }

    // Brief wait to ensure all discovered nodes are ready for DKG
    println!(
        "Node {} waiting briefly for DKG coordination...",
        context.index
    );
    thread::sleep(Duration::from_secs(2));

    // --- DKG Part 1 ---
    println!("Node {} starting DKG Round 1", context.index);
    let (round1_secret_package, round1_package) = frost::keys::dkg::part1(
        context.my_identifier,
        context.total,
        context.threshold,
        &mut context.rng,
    )
    .expect("DKG part 1 failed");

    // Store in context
    context.round1_secret_package = Some(round1_secret_package);
    context.round1_package = Some(round1_package.clone());

    // Broadcast round 1 package to discovered peers only
    let round1_message = Round1Message {
        participant_index: context.index,
        package: round1_package,
    };
    let wrapped_msg = MessageWrapper::DkgRound1(round1_message);
    broadcast_to_peers(context.index, &context.discovered_peers, &wrapped_msg);

    // Receive Round 1 packages from discovered peers only
    println!("Node {} receiving Round 1 packages...", context.index);
    let expected_round1_count = context.discovered_peers.len() - 1; // Exclude self
    let received_messages = receive_messages(
        context.listener.as_ref().unwrap(),
        expected_round1_count,
        Some(Duration::from_secs(30)), // Add timeout for robustness
        |msg| match msg {
            MessageWrapper::DkgRound1(m) => Some(m),
            _ => None,
        },
    )?;

    // Process received packages - only from discovered peers
    let mut received_round1_packages = BTreeMap::new();
    // IMPORTANT: Include our own package in the map
    received_round1_packages.insert(
        context.my_identifier,
        context.round1_package.clone().unwrap(),
    );

    for msg in received_messages {
        let participant_id = Identifier::try_from(msg.participant_index)?;
        // Only accept packages from discovered peers
        if context.discovered_peers.contains(&msg.participant_index) {
            received_round1_packages.insert(participant_id, msg.package);
            println!(
                "Node {} received R1 package from {}",
                context.index, msg.participant_index
            );
        } else {
            println!(
                "Node {} ignoring R1 package from undiscovered peer {}",
                context.index, msg.participant_index
            );
        }
    }

    context.received_round1_packages = Some(received_round1_packages.clone());

    // Brief pause before Round 2
    println!(
        "Node {} finished Round 1, waiting briefly...",
        context.index
    );
    thread::sleep(Duration::from_secs(1));

    // --- DKG Part 2 ---
    println!("Node {} starting DKG Round 2", context.index);
    let received_round1_packages_from_others: BTreeMap<_, _> = received_round1_packages
        .iter()
        .filter(|(id_ref, _)| **id_ref != context.my_identifier)
        .map(|(id, pkg)| (*id, pkg.clone()))
        .collect();

    let (round2_secret_package, round2_packages) = frost::keys::dkg::part2(
        context.round1_secret_package.take().unwrap(),
        &received_round1_packages_from_others,
    )
    .expect("DKG part 2 failed");

    context.round2_secret_package = Some(round2_secret_package);

    // Send Round 2 packages to each discovered participant
    println!("Node {} sending Round 2 packages...", context.index);
    for (receiver_id, package) in &round2_packages {
        if received_round1_packages_from_others.contains_key(receiver_id) {
            let id_bytes = receiver_id.serialize();
            let receiver_idx = u16::from_le_bytes(id_bytes[0..2].try_into()?);
            // Only send to discovered peers
            if context.discovered_peers.contains(&receiver_idx) {
                let round2_message = Round2Message {
                    participant_index: context.index,
                    package: package.clone(),
                };
                let wrapped_msg = MessageWrapper::DkgRound2(round2_message);
                let device_addr = format!("127.0.0.1:1000{}", receiver_idx);
                send_to_with_retries(&device_addr, &wrapped_msg, 3);
            }
        }
    }

    // Receive Round 2 packages
    println!("Node {} receiving Round 2 packages...", context.index);
    let expected_round2_count = context.discovered_peers.len() - 1; // Exclude self
    let received_r2_messages = receive_messages(
        context.listener.as_ref().unwrap(),
        expected_round2_count,
        Some(Duration::from_secs(30)), // Add timeout
        |msg| match msg {
            MessageWrapper::DkgRound2(m) => Some(m),
            _ => None,
        },
    )?;

    // Process received Round 2 packages - only from discovered peers
    let mut received_round2_packages = BTreeMap::new();
    for msg in received_r2_messages {
        let sender_id = Identifier::try_from(msg.participant_index)?;
        if received_round1_packages_from_others.contains_key(&sender_id) && 
           context.discovered_peers.contains(&msg.participant_index) {
            received_round2_packages.insert(sender_id, msg.package);
            println!(
                "Node {} received R2 package from {}",
                context.index, msg.participant_index
            );
        }
    }

    context.received_round2_packages = Some(received_round2_packages.clone());

    // --- DKG Part 3 (Finalize) ---
    println!("Node {} finalizing DKG (Part 3)", context.index);

    // Debug the state before calling part3
    println!(
        "Round1 packages count: {}",
        context.received_round1_packages.as_ref().unwrap().len()
    );
    println!(
        "Round2 packages count: {}",
        context.received_round2_packages.as_ref().unwrap().len()
    );

    // Check if we have the correct number of packages
    let total_participants = context.total;
    let r1_count = context.received_round1_packages.as_ref().unwrap().len();
    let r2_count = context.received_round2_packages.as_ref().unwrap().len();

    if r1_count != total_participants as usize {
        println!(
            "Warning: Expected {} round1 packages, but got {}",
            total_participants, r1_count
        );
    }

    if r2_count != (total_participants - 1) as usize {
        println!(
            "Warning: Expected {} round2 packages, but got {}",
            total_participants - 1,
            r2_count
        );
    }

    // To match the ed25519_dkg.rs example exactly:
    // round2_secret_package: Our own secret package from round2
    // round1_packages: ALL round1 packages (including our own)
    // round2_packages: Packages sent TO us FROM others (should be total-1)

    // Get references to all components needed
    let round2_secret_package = context.round2_secret_package.as_ref().unwrap();
    // Filter round 1 packages to exclude self, matching the expected input for part3
    let round1_packages_from_others: BTreeMap<_, _> = context
        .received_round1_packages
        .as_ref()
        .unwrap()
        .iter()
        .filter(|(id, _)| **id != context.my_identifier)
        .map(|(id, pkg)| (*id, pkg.clone())) // Clone needed data
        .collect();
    let received_round2_packages = context.received_round2_packages.as_ref().unwrap();

    // Call part3 with the right format
    println!(
        "Calling part3 with: round2_secret_package, {} round1 packages (from others), {} round2 packages",
        round1_packages_from_others.len(),
        received_round2_packages.len()
    );

    let (key_package, pubkey_package) = frost::keys::dkg::part3(
        round2_secret_package,
        &round1_packages_from_others, // Use the filtered map
        received_round2_packages,
    )
    .expect("DKG part 3 failed");

    // Store key packages in context
    context.key_package = Some(key_package.clone());
    context.pubkey_package = Some(pubkey_package.clone());

    // Derive Solana pubkey
    let verifying_key_bytes = pubkey_package.verifying_key().serialize()?;
    let mut pubkey_arr = [0u8; 32];
    if verifying_key_bytes.len() == 32 {
        pubkey_arr.copy_from_slice(&verifying_key_bytes);
        context.solana_pubkey = Some(Pubkey::new_from_array(pubkey_arr));
    } else {
        return Err(format!("FROST verifying key size is not 32 bytes").into());
    }

    println!("Node {} completed DKG successfully", context.index);
    println!(
        "Node {} derived Solana address: {}",
        context.index,
        context.solana_pubkey.unwrap()
    );

    // Save keys to cache
    let key_package_file = format!("key_package_{}.bin", context.index);
    let pubkey_package_file = format!("pubkey_package_{}.bin", context.index);

    let key_bytes = encode_to_vec(&key_package, bincode::config::standard())?;
    let pubkey_bytes = encode_to_vec(&pubkey_package, bincode::config::standard())?;

    if let Err(e) = fs::write(&key_package_file, key_bytes) {
        eprintln!("Failed to save key package: {}", e);
    } else {
        println!(
            "Node {} saved key package to {}",
            context.index, key_package_file
        );
    }

    if let Err(e) = fs::write(&pubkey_package_file, pubkey_bytes) {
        eprintln!("Failed to save pubkey package: {}", e);
    } else {
        println!(
            "Node {} saved pubkey package to {}",
            context.index, pubkey_package_file
        );
    }

    // Transition to idle state
    context.state = NodeState::Idle;
    Ok(())
}

// Handle transaction composition (initiator only)
fn handle_transaction_composition(context: &mut NodeContext) -> Result<(), Box<dyn Error>> {
    println!("Node {} in TRANSACTION_COMPOSITION state", context.index);

    // Set up listener for later communication only if it doesn't exist
    if context.listener.is_none() {
        context.setup_listener()?;
    }

    // Get transaction details from user
    print!("Enter target Solana address: ");
    io::stdout().flush()?;
    let mut target_address_str = String::new();
    stdin().read_line(&mut target_address_str)?;
    let target_address = Pubkey::from_str(target_address_str.trim())?;

    print!("Enter amount in lamports: ");
    io::stdout().flush()?;
    let mut amount_str = String::new();
    stdin().read_line(&mut amount_str)?;
    let amount: u64 = amount_str.trim().parse()?;

    println!(
        "Node {} preparing transaction: {} lamports to {}",
        context.index, amount, target_address
    );

    // Create transaction
    let rpc_url = "https://api.testnet.solana.com";
    let rpc_client = RpcClient::new(rpc_url.to_string());
    let recent_blockhash = rpc_client.get_latest_blockhash()?;

    // Create a transfer instruction manually  
    let system_program_id = Pubkey::default(); // System program ID is all zeros
    let transfer_instruction = Instruction {
        program_id: system_program_id,
        accounts: vec![
            AccountMeta::new(context.solana_pubkey.unwrap(), true),
            AccountMeta::new(target_address, false),
        ],
        data: {
            let mut data = vec![2, 0, 0, 0]; // Transfer instruction discriminator
            data.extend_from_slice(&amount.to_le_bytes()); // Amount as little-endian bytes
            data
        },
    };
    
    let message = Message::new(
        &[transfer_instruction],
        Some(&context.solana_pubkey.unwrap()),
    );
    let mut tx = Transaction::new_unsigned(message);
    tx.message.recent_blockhash = recent_blockhash;

    let message_bytes = tx.message.serialize();

    // Store in context
    context.message_bytes = Some(message_bytes.clone());
    context.transaction = Some(tx);

    // Broadcast transaction message
    let tx_message = TxMessage {
        message_bytes: message_bytes,
    };
    let wrapped_msg = MessageWrapper::SignTx(tx_message);
    broadcast_to_peers(context.index, &context.discovered_peers, &wrapped_msg);

    // Transition to next state
    context.state = NodeState::SigningCommitment;
    Ok(())
}

// Handle signing commitment state
fn handle_signing_commitment(context: &mut NodeContext) -> Result<(), Box<dyn Error>> {
    println!("Node {} in SIGNING_COMMITMENT state", context.index);

    // Generate commitment
    let (nonce, commitment) = frost_round1::commit(
        context.key_package.as_ref().unwrap().signing_share(),
        &mut context.rng,
    );

    // Store in context
    context.my_nonce = Some(nonce);
    context.my_commitment = Some(commitment.clone());

    // Broadcast commitment
    let commitment_message = CommitmentMessage {
        sender_identifier: context.my_identifier,
        commitment: commitment,
    };
    let wrapped_msg = MessageWrapper::SignCommitment(commitment_message);
    broadcast_to_peers(context.index, &context.discovered_peers, &wrapped_msg);

    // Receive commitments from others (with timeout)
    println!("Node {} receiving commitments...", context.index);
    let timeout = Duration::from_secs(30); // 30 seconds timeout
    let expected_commit_count = context.discovered_peers.len() - 1; // Exclude self
    let received_commit_msgs = receive_messages(
        context.listener.as_ref().unwrap(),
        expected_commit_count,
        Some(timeout),
        |msg| match msg {
            MessageWrapper::SignCommitment(m) => Some(m),
            _ => None,
        },
    )?;

    // Process received commitments
    let mut commitments_map = BTreeMap::new();
    commitments_map.insert(
        context.my_identifier,
        context.my_commitment.clone().unwrap(),
    );

    for msg in received_commit_msgs {
        commitments_map.insert(msg.sender_identifier, msg.commitment);
        println!(
            "Node {} received commitment from {:?}",
            context.index, msg.sender_identifier
        );
    }

    // Store in context
    context.commitments_map = Some(commitments_map.clone());

    // Check if enough commitments received
    if commitments_map.len() < context.threshold as usize {
        return Err(format!(
            "Not enough commitments received. Got {}, need {}",
            commitments_map.len(),
            context.threshold
        )
        .into());
    }

    println!(
        "Node {} received {} commitments (threshold: {})",
        context.index,
        commitments_map.len(),
        context.threshold
    );

    // Transition to next state
    if context.is_initiator {
        context.state = NodeState::SignerSelection;
    } else {
        println!("Node {} waiting for signer selection...", context.index);
        // Wait for selection message
        let selection_msgs = receive_messages(
            context.listener.as_ref().unwrap(),
            1,
            None,
            |msg| match msg {
                MessageWrapper::SignerSelection(m) => Some(m),
                _ => None,
            },
        )?;

        context.selected_signers = Some(selection_msgs[0].selected_identifiers.clone());
        let selected = context
            .selected_signers
            .as_ref()
            .unwrap()
            .contains(&context.my_identifier);
        context.state = NodeState::SignatureGeneration { selected };
    }

    Ok(())
}

// Handle signer selection state
fn handle_signer_selection(context: &mut NodeContext) -> Result<(), Box<dyn Error>> {
    println!("Node {} in SIGNER_SELECTION state", context.index);

    // Select signers
    let mut selected_ids = Vec::with_capacity(context.threshold as usize);
    selected_ids.push(context.my_identifier); // Always include self

    let needed_others = context.threshold.saturating_sub(1);

    if needed_others > 0 {
        let available_others: Vec<_> = context
            .commitments_map
            .as_ref()
            .unwrap()
            .keys()
            .filter(|&&id| id != context.my_identifier)
            .cloned()
            .collect();

        if available_others.len() < needed_others as usize {
            return Err(format!(
                "Not enough participants available. Need {}, got {}",
                needed_others,
                available_others.len()
            )
            .into());
        }

        println!("Available participants:");
        for id in &available_others {
            let id_bytes = id.serialize();
            let idx = u16::from_le_bytes(id_bytes[0..2].try_into()?);
            println!(" - Node {}", idx);
        }

        // User selection
        loop {
            print!(
                "Enter {} participant indices (comma-separated): ",
                needed_others
            );
            io::stdout().flush()?;
            let mut input_str = String::new();
            stdin().read_line(&mut input_str)?;

            let parts: Vec<&str> = input_str.trim().split(',').collect();
            if parts.len() != needed_others as usize {
                println!("Invalid number of indices. Expected {}", needed_others);
                continue;
            }

            let mut temp_selected = Vec::new();
            let mut valid = true;

            for part in parts {
                match part.trim().parse::<u16>() {
                    Ok(idx) => match Identifier::try_from(idx) {
                        Ok(id) => {
                            if available_others.contains(&id) && !temp_selected.contains(&id) {
                                temp_selected.push(id);
                            } else if id == context.my_identifier {
                                println!("You (Node {}) are already included", context.index);
                                valid = false;
                                break;
                            } else if temp_selected.contains(&id) {
                                println!("Duplicate selection: Node {}", idx);
                                valid = false;
                                break;
                            } else {
                                println!("Node {} not available", idx);
                                valid = false;
                                break;
                            }
                        }
                        Err(_) => {
                            println!("Invalid index: {}", idx);
                            valid = false;
                            break;
                        }
                    },
                    Err(_) => {
                        println!("Invalid input: {}", part);
                        valid = false;
                        break;
                    }
                }
            }

            if valid {
                selected_ids.extend(temp_selected);
                break;
            }
        }
    }

    // Sort for deterministic ordering
    selected_ids.sort();

    // Store in context
    context.selected_signers = Some(selected_ids.clone());

    // Broadcast selection
    let selection_msg = SignerSelectionMessage {
        selected_identifiers: selected_ids,
    };
    let wrapped_msg = MessageWrapper::SignerSelection(selection_msg);
    broadcast_to_peers(context.index, &context.discovered_peers, &wrapped_msg);

    // Transition to next state
    context.state = NodeState::SignatureGeneration { selected: true };
    Ok(())
}

// Handle signature generation state
fn handle_signature_generation(
    context: &mut NodeContext,
    selected: bool,
) -> Result<(), Box<dyn Error>> {
    println!(
        "Node {} in SIGNATURE_GENERATION state (selected: {})",
        context.index, selected
    );

    if selected {
        // Create signing package using only selected signers
        let mut final_commitments = BTreeMap::new();
        for id in context.selected_signers.as_ref().unwrap() {
            if let Some(commitment) = context.commitments_map.as_ref().unwrap().get(id) {
                final_commitments.insert(*id, commitment.clone());
            } else {
                return Err(format!("Missing commitment for selected signer {:?}", id).into());
            }
        }

        let signing_package = SigningPackage::new(
            final_commitments.clone(),
            context.message_bytes.as_ref().unwrap(),
        );

        // Generate signature share
        let signature_share = frost_round2::sign(
            &signing_package,
            context.my_nonce.as_ref().unwrap(),
            context.key_package.as_ref().unwrap(),
        )?;

        // Store in context
        context.my_signature_share = Some(signature_share.clone());

        if context.is_initiator {
            // Initiator continues to collect shares
            let mut shares_map = BTreeMap::new();
            shares_map.insert(context.my_identifier, signature_share);
            context.signature_shares_map = Some(shares_map);

            // Transition to aggregation state
            context.state = NodeState::SignatureAggregation;
        } else {
            // Send share to initiator
            println!(
                "Node {} sending signature share to initiator...",
                context.index
            );

            print!("Press ENTER to confirm and send your signature share: ");
            io::stdout().flush()?;
            let mut confirmation = String::new();
            stdin().read_line(&mut confirmation)?;

            let share_message = ShareMessage {
                sender_identifier: context.my_identifier,
                share: signature_share,
            };
            let wrapped_msg = MessageWrapper::SignShare(share_message);
            let initiator_addr = format!("127.0.0.1:1000{}", 1); // Assume node 1 is initiator
            send_to(&initiator_addr, &wrapped_msg);

            // Wait for final signature
            context.state = NodeState::SignatureVerification;
        }
    } else {
        // Not selected, just wait for final signature
        println!("Node {} was not selected for signing", context.index);
        context.state = NodeState::SignatureVerification;
    }

    Ok(())
}

// Handle signature aggregation state (initiator only)
fn handle_signature_aggregation(context: &mut NodeContext) -> Result<(), Box<dyn Error>> {
    println!("Node {} in SIGNATURE_AGGREGATION state", context.index);

    // Calculate number of shares to receive
    let shares_to_receive = context
        .selected_signers
        .as_ref()
        .unwrap()
        .iter()
        .filter(|&&id| id != context.my_identifier)
        .count();

    if shares_to_receive > 0 {
        println!(
            "Node {} waiting for {} signature shares...",
            context.index, shares_to_receive
        );

        // Receive shares
        let received_shares = receive_messages(
            context.listener.as_ref().unwrap(),
            shares_to_receive,
            None,
            |msg| match msg {
                MessageWrapper::SignShare(m) => Some(m),
                _ => None,
            },
        )?;

        // Process received shares
        let shares_map = context.signature_shares_map.as_mut().unwrap();

        for msg in received_shares {
            if context
                .selected_signers
                .as_ref()
                .unwrap()
                .contains(&msg.sender_identifier)
            {
                shares_map.insert(msg.sender_identifier, msg.share);
                println!(
                    "Node {} received share from {:?}",
                    context.index, msg.sender_identifier
                );
            }
        }
    }

    // Verify we have all required shares
    let shares_map = context.signature_shares_map.as_ref().unwrap();
    if shares_map.len() != context.threshold as usize {
        return Err(format!(
            "Not enough signature shares. Got {}, need {}",
            shares_map.len(),
            context.threshold
        )
        .into());
    }

    println!(
        "Node {} received all {} required signature shares",
        context.index, context.threshold
    );

    // Create signing package again
    let mut final_commitments = BTreeMap::new();
    for id in context.selected_signers.as_ref().unwrap() {
        if let Some(commitment) = context.commitments_map.as_ref().unwrap().get(id) {
            final_commitments.insert(*id, commitment.clone());
        }
    }

    let signing_package =
        SigningPackage::new(final_commitments, context.message_bytes.as_ref().unwrap());

    // Aggregate shares
    let group_signature = frost::aggregate(
        &signing_package,
        shares_map,
        context.pubkey_package.as_ref().unwrap(),
    )?;

    // Serialize signature
    let signature_bytes = group_signature.serialize()?;
    context.aggregated_signature = Some(signature_bytes.clone());

    println!(
        "Node {} aggregated signature: {}",
        context.index,
        hex::encode(&signature_bytes)
    );

    // Broadcast signature
    let agg_sig_msg = AggregatedSignatureMessage {
        signature_bytes: signature_bytes,
    };
    let wrapped_msg = MessageWrapper::SignAggregated(agg_sig_msg);
    broadcast_to_peers(context.index, &context.discovered_peers, &wrapped_msg);

    // Transition to next state
    context.state = NodeState::TransactionSubmission;
    Ok(())
}

// Handle transaction submission state (initiator only)
fn handle_transaction_submission(context: &mut NodeContext) -> Result<(), Box<dyn Error>> {
    println!("Node {} in TRANSACTION_SUBMISSION state", context.index);

    // Add signature to transaction
    let signature_bytes = context.aggregated_signature.as_ref().unwrap();
    let solana_signature = Signature::try_from(signature_bytes.as_slice())
        .map_err(|e| format!("Failed to create Solana signature: {:?}", e))?;

    let mut final_tx = context.transaction.take().unwrap();
    if !final_tx.signatures.is_empty() {
        final_tx.signatures[0] = solana_signature;
    } else {
        final_tx.signatures.push(solana_signature);
    }

    // Submit transaction
    println!("Transaction details: {:?}", final_tx);
    println!("Attempting to submit transaction...");

    let rpc_client = RpcClient::new("https://api.testnet.solana.com".to_string());

    match rpc_client.send_and_confirm_transaction_with_spinner(&final_tx) {
        Ok(sig) => {
            println!("Transaction successfully sent! Signature: {}", sig);
        }
        Err(e) => {
            println!("Transaction failed (as expected): {}", e);
        }
    }

    // Return to idle state
    context.state = NodeState::Idle;
    Ok(())
}

// Handle signature verification state (non-initiator)
fn handle_signature_verification(context: &mut NodeContext) -> Result<(), Box<dyn Error>> {
    println!("Node {} in SIGNATURE_VERIFICATION state", context.index);

    // Wait for final signature
    println!("Node {} waiting for aggregated signature...", context.index);
    let received_sigs =
        receive_messages(
            context.listener.as_ref().unwrap(),
            1,
            None,
            |msg| match msg {
                MessageWrapper::SignAggregated(m) => Some(m),
                _ => None,
            },
        )?;

    let signature_bytes = received_sigs[0].signature_bytes.clone();
    println!(
        "Node {} received aggregated signature: {}",
        context.index,
        hex::encode(&signature_bytes)
    );
    context.state = NodeState::Idle;
    Ok(())
}
