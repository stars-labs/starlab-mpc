use bincode::serde::{decode_from_slice, encode_to_vec};
use clap::Parser;
use k256::elliptic_curve::point::AffineCoordinates; // Import for .x() on AffinePoint
use ethers_core::types::{
    Address,
    H256,
    Signature as EthSignature,
    TransactionRequest,
    U256, // Removed H160
};
use ethers_core::utils::keccak256;
use ethers_providers::{Http, Middleware, Provider};
use frost::Identifier;
use frost::rand_core::OsRng;
use frost_core::SigningPackage;
use frost_core::keys::dkg::{round1, round2};
use frost_core::keys::{KeyPackage, PublicKeyPackage};
use frost_core::{round1 as frost_round1, round2 as frost_round2};
use frost_secp256k1 as frost; // Use secp256k1
use frost_secp256k1::Secp256K1Sha256; // Use secp256k1
use hex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Write, stdin}; // Keep sync stdin/stdout
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashSet;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::timeout; // Use tokio's timeout // Add this import

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

// --- DKG Message Types ---
#[derive(Serialize, Deserialize, Clone, Debug)] // Add Debug
struct Round1Message {
    participant_index: u16,
    package: round1::Package<Secp256K1Sha256>, // Use Secp256K1Sha256
}

#[derive(Serialize, Deserialize, Clone, Debug)] // Add Debug
struct Round2Message {
    participant_index: u16,
    package: round2::Package<Secp256K1Sha256>, // Use Secp256K1Sha256
}

// --- Signing Message Types ---
#[derive(Serialize, Deserialize, Clone, Debug)] // Add Debug
struct TxMessage {
    // Store transaction hash bytes directly
    tx_hash_bytes: Vec<u8>,
    // Also include the full transaction request for participants to reconstruct
    transaction_request: Vec<u8>, // Serialized TransactionRequest
}

#[derive(Serialize, Deserialize, Clone, Debug)] // Add Debug
struct CommitmentMessage {
    sender_identifier: Identifier,
    commitment: frost_round1::SigningCommitments<Secp256K1Sha256>, // Use Secp256K1Sha256
}

#[derive(Serialize, Deserialize, Clone, Debug)] // Add Debug
struct ShareMessage {
    sender_identifier: Identifier,
    share: frost_round2::SignatureShare<Secp256K1Sha256>, // Use Secp256K1Sha256
}

// --- New Message Type for Aggregated Signature ---
#[derive(Serialize, Deserialize, Clone, Debug)] // Add Debug
struct AggregatedSignatureMessage {
    // Send r, s, v directly
    r: [u8; 32],
    s: [u8; 32],
    v: u8,
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

/// Ethereum DKG Example CLI
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct CliArgs {
    /// Your node index (1-based)
    #[arg(short = 'i', long)]
    index: u16,
    /// Total number of participants
    #[arg(short = 't', long)]
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

// Async send_to using tokio with better error handling for discovery
async fn send_to(addr: &str, msg: &MessageWrapper) -> bool {
    let data = encode_to_vec(msg, bincode::config::standard()).unwrap();
    let sock_addr: SocketAddr = match addr.parse() {
        Ok(addr) => addr,
        Err(_) => return false,
    };

    match timeout(Duration::from_secs(2), TcpStream::connect(sock_addr)).await {
        Ok(Ok(mut stream)) => {
            match stream.write_all(&data).await {
                Ok(_) => true,
                Err(_) => false,
            }
        }
        _ => false, // Connection refused or timeout - expected during discovery
    }
}

// Async send_to with retries for important messages
async fn send_to_with_retries(addr: &str, msg: &MessageWrapper, max_retries: u32) -> bool {
    for attempt in 0..=max_retries {
        if send_to(addr, msg).await {
            return true;
        }
        if attempt < max_retries {
            let backoff = Duration::from_millis(100 * (attempt as u64 + 1));
            tokio::time::sleep(backoff).await;
        }
    }
    false
}

// Async mesh discovery: Wait for all nodes to join the cluster  
async fn discover_all_peers(index: u16, total: u16) -> Result<HashSet<u16>, Box<dyn Error + Send + Sync>> {
    let mut discovered = HashSet::new();
    discovered.insert(index); // Always include self
    
    println!("Node {} starting peer discovery, waiting for all {} nodes...", index, total);
    
    // Set up listener for discovery messages
    let listener = TcpListener::bind(format!("127.0.0.1:1000{}", index)).await?;
    println!("Node {} listening on 127.0.0.1:1000{}", index, index);
    
    let mut last_ping_time = tokio::time::Instant::now();
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
            
            // Try to ping all other nodes that we haven't discovered yet
            for target_idx in 1..=total {
                if target_idx != index && !discovered.contains(&target_idx) {
                    let target_addr = format!("127.0.0.1:1000{}", target_idx);
                    send_to(&target_addr, &wrapped_msg).await;
                }
            }
            last_ping_time = tokio::time::Instant::now();
            
            if discovered.len() == total as usize {
                println!("Node {} discovered all {} nodes: {:?}", index, total, discovered);
            } else {
                println!("Node {} discovered {}/{} nodes: {:?}", 
                        index, discovered.len(), total, discovered);
            }
        }
        
        // Listen for incoming messages
        match timeout(Duration::from_millis(50), listener.accept()).await {
            Ok(Ok((mut stream, _addr))) => {
                let mut buf = Vec::new();
                if stream.read_to_end(&mut buf).await.is_ok() && !buf.is_empty() {
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
                                send_to(&target_addr, &wrapped_msg).await;
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
            _ => {
                // No incoming connection or timeout, continue
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
    
    println!("Node {} discovery complete! Found all {} nodes: {:?}", index, total, discovered);
    Ok(discovered)
}

// Async broadcast to discovered peers only
async fn broadcast_to_peers(index: u16, peers: &HashSet<u16>, msg: &MessageWrapper) {
    println!("Node {} broadcasting message to {} peers...", index, peers.len() - 1);
    let mut tasks = Vec::new();
    for &peer_idx in peers {
        if peer_idx == index {
            continue;
        }
        let peer_addr = format!("127.0.0.1:1000{}", peer_idx);
        // Clone msg for each task
        let msg_clone = msg.clone();
        tasks.push(tokio::spawn(async move {
            send_to_with_retries(&peer_addr, &msg_clone, 3).await;
        }));
    }
    // Wait for all sends to complete
    for task in tasks {
        let _ = task.await; // Handle potential task errors if needed
    }
}

// Async receive_messages using tokio - Modified to return all messages
async fn receive_messages(
    listener: Arc<Mutex<TcpListener>>, // Use Arc<Mutex<TcpListener>>
    expected_count: usize,             // Still used as a minimum target
    timeout_duration: Option<Duration>,
    // Removed extract_fn
) -> Result<Vec<MessageWrapper>, Box<dyn Error + Send + Sync>> {
    // Return Vec<MessageWrapper>
    // Ensure error is Send + Sync
    let messages = Arc::new(Mutex::new(Vec::with_capacity(expected_count)));
    let listener_clone = listener.clone();
    let messages_clone = messages.clone(); // Clone the Arc before moving

    let processing_task = tokio::spawn(async move {
        // Removed local_messages as we use the shared Arc directly
        loop {
            let listener_guard = listener_clone.lock().await;
            match listener_guard.accept().await {
                Ok((mut stream, addr)) => {
                    println!("Accepted connection from {}", addr);
                    drop(listener_guard); // Release lock before reading

                    let mut buf = Vec::new();
                    match stream.read_to_end(&mut buf).await {
                        Ok(_) => {
                            if buf.is_empty() {
                                println!("Received empty message from {}", addr);
                            } else {
                                match decode_from_slice::<MessageWrapper, _>(
                                    &buf,
                                    bincode::config::standard(),
                                ) {
                                    Ok((wrapped_msg, _)) => {
                                        // Debug message type
                                        match &wrapped_msg {
                                            MessageWrapper::Ping(_) => {
                                                println!("Received Ping message");
                                            }
                                            MessageWrapper::Pong(_) => {
                                                println!("Received Pong message");
                                            }
                                            MessageWrapper::Ready(_) => {
                                                println!("Received Ready message");
                                            }
                                            MessageWrapper::DkgRound1(_) => {
                                                println!("Received DkgRound1 message");
                                            }
                                            MessageWrapper::DkgRound2(_) => {
                                                println!("Received DkgRound2 message");
                                            }
                                            MessageWrapper::SignTx(_) => {
                                                println!("Received SignTx message");
                                            }
                                            MessageWrapper::SignCommitment(_) => {
                                                println!("Received SignCommitment message");
                                            }
                                            MessageWrapper::SignShare(_) => {
                                                println!("Received SignShare message");
                                            }
                                            MessageWrapper::SignAggregated(_) => {
                                                println!("Received SignAggregated message");
                                            }
                                            MessageWrapper::SignerSelection(_) => {
                                                println!("Received SignerSelection message");
                                            }
                                        }
                                        println!("Node received message: {:?}", wrapped_msg); // Log received message
                                        // Push ALL decoded messages to the shared vec
                                        let mut messages_guard = messages_clone.lock().await;
                                        messages_guard.push(wrapped_msg);
                                        let current_count = messages_guard.len();
                                        drop(messages_guard); // Release lock
                                        // Stop condition is still based on expected_count,
                                        // but we might collect more due to buffering nature.
                                        if current_count >= expected_count {
                                            println!(
                                                "Received at least expected count ({}) messages.",
                                                expected_count
                                            );
                                            // Don't break immediately if timeout is None, allow collecting more briefly?
                                            // Or break here? Let's break for simplicity now.
                                            // If timeout is active, the outer timeout handles breaking.
                                            if timeout_duration.is_some() {
                                                // Let timeout handle expiry
                                            } else
                                            // Remove parentheses around condition
                                            if current_count >= expected_count {
                                                println!(
                                                    "Received expected count ({}) messages after check (no timeout).",
                                                    expected_count
                                                );
                                                break;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to decode message from {}: {}", addr, e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error reading from stream {}: {}", addr, e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error accepting connection: {}", e);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
            // Check count again if no timeout is set
            if timeout_duration.is_none() {
                let messages_guard = messages_clone.lock().await;
                let current_count = messages_guard.len();
                drop(messages_guard);
                if current_count >= expected_count {
                    println!(
                        "Received expected count ({}) messages after check (no timeout).",
                        expected_count
                    );
                    break;
                }
            }
        }
        // Return Ok(()) as the result is taken from the Arc
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });

    // Timeout handling remains the same
    if let Some(duration) = timeout_duration {
        match timeout(duration, processing_task).await {
            Ok(Ok(_)) => {
                println!("Message receiving task completed or timed out.");
            }
            Ok(Err(e)) => {
                eprintln!("Message receiving task failed: {}", e);
                // Check if messages were collected even if task failed internally after timeout?
                // For now, return error.
                return Err(format!("Task error: {}", e).into());
            }
            Err(_) => {
                // Timeout elapsed
                println!("Message receiving timed out after {:?}.", duration);
                // Proceed with messages collected so far
            }
        }
    } else {
        // No timeout, wait indefinitely for the task to complete (until expected_count is met)
        match processing_task.await {
            Ok(Ok(_)) => { /* Task completed successfully */ }
            Ok(Err(e)) => {
                eprintln!("Message receiving task failed: {}", e);
                return Err(format!("Task error: {}", e).into());
            }
            Err(join_error) => {
                eprintln!(
                    "Message receiving task panicked or was cancelled: {}",
                    join_error
                );
                return Err(format!("Join error: {}", join_error).into());
            }
        }
    }

    // Take ownership of the messages from the Arc<Mutex<Vec>>
    // Use try_unwrap which requires the Arc to have only one strong reference.
    // If the task panicked or is still running somehow, this might fail.
    // A robust way might involve signaling the task to stop and joining it.
    // For simplicity, let's assume the task is finished or timed out.
    match Arc::try_unwrap(messages) {
        Ok(mutex) => Ok(mutex.into_inner()),
        Err(arc) => {
            // This case means the task might still hold a reference (e.g., panicked).
            // We can try to lock and clone, but it indicates an issue.
            eprintln!(
                "Warning: Could not obtain unique ownership of message buffer. Cloning collected messages."
            );
            let messages_guard = arc.lock().await;
            Ok(messages_guard.clone()) // Clone the collected messages
        }
    }
}

// Helper to derive Ethereum address from FROST PublicKeyPackage
fn derive_eth_address(
    pubkey_package: &PublicKeyPackage<Secp256K1Sha256>,
) -> Result<Address, Box<dyn Error + Send + Sync>> {
    // ... (same as before) ...
    use k256::elliptic_curve::sec1::ToEncodedPoint; // Keep this for PublicKey::to_encoded_point

    let group_public_key = pubkey_package.verifying_key();
    // Serialize the key in uncompressed format (0x04 prefix + 64 bytes)
    let compressed_bytes = group_public_key.serialize()?; // Handle Result using ?

    // Need to decompress first. Use the `k256` crate internally used by frost-secp256k1
    let compressed_point =
        k256::PublicKey::from_sec1_bytes(&compressed_bytes) // Use the unwrapped bytes
            .map_err(|e| format!("Failed to parse compressed public key: {}", e))?;
    let uncompressed_point = compressed_point.to_encoded_point(false); // Get uncompressed EncodedPoint
    let uncompressed_bytes_slice = uncompressed_point.as_bytes(); // Get as byte slice

    // Ensure it's uncompressed (starts with 0x04) and is 65 bytes long
    if uncompressed_bytes_slice.len() != 65 || uncompressed_bytes_slice[0] != 0x04 {
        return Err(format!(
            "Unexpected uncompressed public key format (len={}, prefix={})",
            uncompressed_bytes_slice.len(),
            uncompressed_bytes_slice[0]
        )
        .into());
    }

    // Hash the uncompressed key (excluding the 0x04 prefix)
    let hash = keccak256(&uncompressed_bytes_slice[1..]);

    // Take the last 20 bytes of the hash
    let address_bytes = &hash[12..];
    Ok(Address::from_slice(address_bytes))
}

// --- State Machine Definitions ---

#[derive(Debug, Clone, PartialEq)]
enum NodeState {
    Initial,
    Discovery,
    DkgProcess,
    Idle,
    TransactionComposition, // Initiator only
    SigningCommitment,
    SignerSelection, // Initiator only
    SignatureGeneration { selected: bool },
    SignatureAggregation,  // Initiator only
    TransactionSubmission, // Initiator only
    SignatureVerification, // Participant only
    Completed,             // Terminal state
}

struct NodeContext {
    index: u16,
    total: u16,
    threshold: u16,
    is_initiator: bool,
    wait_for_all: bool,
    state: NodeState,
    my_identifier: Identifier,
    discovered_peers: HashSet<u16>,
    key_package: Option<KeyPackage<Secp256K1Sha256>>,
    pubkey_package: Option<PublicKeyPackage<Secp256K1Sha256>>,
    eth_address: Option<Address>,
    listener: Option<Arc<Mutex<TcpListener>>>, // Use Arc<Mutex<>> for async sharing

    // DKG state
    round1_secret_package: Option<frost_core::keys::dkg::round1::SecretPackage<Secp256K1Sha256>>,
    round1_package: Option<round1::Package<Secp256K1Sha256>>,
    round2_secret_package: Option<frost_core::keys::dkg::round2::SecretPackage<Secp256K1Sha256>>,
    received_round1_packages: Option<BTreeMap<Identifier, round1::Package<Secp256K1Sha256>>>,
    received_round2_packages: Option<BTreeMap<Identifier, round2::Package<Secp256K1Sha256>>>,

    // Signing state
    tx_hash_bytes: Option<Vec<u8>>,
    transaction_request: Option<TransactionRequest>, // Store the full request
    my_nonce: Option<frost_round1::SigningNonces<Secp256K1Sha256>>,
    my_commitment: Option<frost_round1::SigningCommitments<Secp256K1Sha256>>,
    commitments_map:
        Option<BTreeMap<Identifier, frost_round1::SigningCommitments<Secp256K1Sha256>>>,
    selected_signers: Option<Vec<Identifier>>,
    my_signature_share: Option<frost_round2::SignatureShare<Secp256K1Sha256>>,
    signature_shares_map:
        Option<BTreeMap<Identifier, frost_round2::SignatureShare<Secp256K1Sha256>>>,
    aggregated_signature: Option<AggregatedSignatureMessage>, // Store r, s, v
    message_buffer: Vec<MessageWrapper>,                      // Add message buffer

    // Working data
    rng: OsRng,
    provider: Option<Provider<Http>>, // Store provider for reuse
    chain_id: Option<U256>,
}

impl NodeContext {
    fn new(
        index: u16,
        total: u16,
        threshold: u16,
        is_initiator: bool,
        wait_for_all: bool,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
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
            eth_address: None,
            listener: None,
            round1_secret_package: None,
            round1_package: None,
            round2_secret_package: None,
            received_round1_packages: None,
            received_round2_packages: None,
            tx_hash_bytes: None,
            transaction_request: None,
            my_nonce: None,
            my_commitment: None,
            commitments_map: None,
            selected_signers: None,
            my_signature_share: None,
            signature_shares_map: None,
            aggregated_signature: None,
            message_buffer: Vec::new(), // Initialize buffer
            rng: OsRng,
            provider: None,
            chain_id: None,
        })
    }

    async fn setup_listener(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.listener.is_none() {
            let my_addr = format!("127.0.0.1:1000{}", self.index);
            let listener = TcpListener::bind(&my_addr).await?;
            self.listener = Some(Arc::new(Mutex::new(listener)));
            println!("Node {} listening on {}", self.index, my_addr);
        }
        Ok(())
    }

    async fn setup_provider(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.provider.is_none() {
            let rpc_url =
                env::var("ETH_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:8545".to_string());
            let provider = Provider::<Http>::try_from(rpc_url)?;
            self.chain_id = Some(provider.get_chainid().await?);
            self.provider = Some(provider);
            println!(
                "Node {} connected to RPC, Chain ID: {}",
                self.index,
                self.chain_id.unwrap()
            );
        }
        Ok(())
    }
}

// --- Main Function (State Machine Driver) ---

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = CliArgs::parse();
    let index = args.index;
    let total = args.total;
    let threshold = args.threshold;
    let is_initiator = args.is_initiator;

    // Create initial context
    let mut context = NodeContext::new(index, total, threshold, is_initiator, args.wait_for_all)?;

    // Main state machine loop
    loop {
        println!("Node {} entering state: {:?}", context.index, context.state);
        match context.state.clone() {
            // Use clone to avoid borrowing issues
            NodeState::Initial => handle_initial_state(&mut context).await?,
            NodeState::Discovery => handle_discovery_state(&mut context).await?,
            NodeState::DkgProcess => handle_dkg_process(&mut context).await?,
            NodeState::Idle => handle_idle_state(&mut context).await?,
            NodeState::TransactionComposition => {
                handle_transaction_composition(&mut context).await?
            }
            NodeState::SigningCommitment => handle_signing_commitment(&mut context).await?,
            NodeState::SignerSelection => handle_signer_selection(&mut context).await?,
            NodeState::SignatureGeneration { selected } => {
                handle_signature_generation(&mut context, selected).await?
            }
            NodeState::SignatureAggregation => handle_signature_aggregation(&mut context).await?,
            NodeState::TransactionSubmission => handle_transaction_submission(&mut context).await?,
            NodeState::SignatureVerification => handle_signature_verification(&mut context).await?,
            NodeState::Completed => {
                println!("\nProcess completed for Node {}.", context.index);
                break; // Exit the loop
            }
        }
        // Optional delay for observation
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}

// --- State Handler Functions ---

async fn handle_initial_state(
    context: &mut NodeContext,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Node {} in INITIAL state", context.index);

    // Add startup delay
    let startup_delay = Duration::from_millis(300 * context.index as u64);
    println!(
        "Node {} waiting {:?} before starting to ensure all devices are ready...",
        context.index, startup_delay
    );
    tokio::time::sleep(startup_delay).await;

    let key_package_file = format!("eth_key_package_{}.bin", context.index);
    let pubkey_package_file = format!("eth_pubkey_package_{}.bin", context.index);

    if fs::metadata(&key_package_file).is_ok() && fs::metadata(&pubkey_package_file).is_ok() {
        println!("Node {} loading keys from cache...", context.index);
        let key_bytes = fs::read(&key_package_file)?;
        let pubkey_bytes = fs::read(&pubkey_package_file)?;

        let kp: KeyPackage<Secp256K1Sha256> =
            decode_from_slice(&key_bytes, bincode::config::standard())?.0;
        let pkp: PublicKeyPackage<Secp256K1Sha256> =
            decode_from_slice(&pubkey_bytes, bincode::config::standard())?.0;

        context.key_package = Some(kp);
        context.pubkey_package = Some(pkp);
        context.eth_address = Some(derive_eth_address(
            context.pubkey_package.as_ref().unwrap(),
        )?);

        println!("Node {} loaded keys successfully.", context.index);
        println!(
            "Node {} derived Ethereum address: {:?}",
            context.index,
            context.eth_address.unwrap()
        );
        context.state = NodeState::Idle;
    } else {
        println!("Node {} no keys found, starting discovery process", context.index);
        context.state = NodeState::Discovery;
    }

    Ok(())
}

// Handle peer discovery state
async fn handle_discovery_state(context: &mut NodeContext) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Node {} in DISCOVERY state", context.index);
    
    if context.wait_for_all {
        // Wait for all nodes to join
        context.discovered_peers = discover_all_peers(context.index, context.total).await?;
        println!("Node {} discovered all {} peers: {:?}", 
                context.index, context.total, context.discovered_peers);
        
        // Proceed to DKG with all nodes
        context.state = NodeState::DkgProcess;
    } else {
        // Use threshold-based discovery (fallback - not implemented)
        println!("Node {} using threshold-based discovery (not implemented in this version)", context.index);
        context.state = NodeState::DkgProcess;
    }
    
    Ok(())
}

async fn handle_dkg_process(context: &mut NodeContext) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Node {} in DKG_PROCESS state", context.index);

    // Set up listener (reuse existing if available)
    if context.listener.is_none() {
        context.setup_listener().await?;
    }
    let listener = context.listener.clone().unwrap(); // Clone Arc for use

    // Brief wait to ensure all discovered nodes are ready for DKG
    println!(
        "Node {} waiting briefly for DKG coordination...",
        context.index
    );
    tokio::time::sleep(Duration::from_secs(2)).await;

    // --- DKG Round 1 ---
    println!("Node {} starting DKG Round 1...", context.index);
    let (round1_secret_package, round1_package) = frost::keys::dkg::part1(
        context.my_identifier,
        context.total,
        context.threshold,
        &mut context.rng,
    )
    .expect("DKG part 1 failed");

    context.round1_secret_package = Some(round1_secret_package);
    context.round1_package = Some(round1_package.clone());

    let round1_message = Round1Message {
        participant_index: context.index,
        package: round1_package.clone(),
    };
    let wrapped_r1_msg = MessageWrapper::DkgRound1(round1_message);
    broadcast_to_peers(context.index, &context.discovered_peers, &wrapped_r1_msg).await;

    // Receive Round 1 Packages (with buffering and loop)
    println!("Node {} receiving DKG Round 1 packages...", context.index);
    let mut received_round1_packages = BTreeMap::new();
    received_round1_packages.insert(context.my_identifier, round1_package); // Add self

    loop {
        // Start receive loop
        // 1. Process existing buffer first
        let mut next_buffer = Vec::new();
        for msg in std::mem::take(&mut context.message_buffer) {
            if let MessageWrapper::DkgRound1(m) = msg {
                if received_round1_packages.len() < context.total as usize {
                    let participant_id = Identifier::try_from(m.participant_index)?;
                    if received_round1_packages
                        .insert(participant_id, m.package)
                        .is_none()
                    {
                        println!(
                            "Node {} processed buffered DKG R1 package from participant {}",
                            context.index, m.participant_index
                        );
                    }
                } else {
                    next_buffer.push(MessageWrapper::DkgRound1(m)); // Re-buffer if already have enough R1
                }
            } else {
                next_buffer.push(msg); // Keep other messages for later
            }
        }
        context.message_buffer = next_buffer; // Put back unprocessed messages

        // 2. Check if we have enough R1 packages
        let needed_r1_count =
            (context.total as usize).saturating_sub(received_round1_packages.len());
        if needed_r1_count == 0 {
            println!(
                "Node {} collected all required DKG R1 packages.",
                context.index
            );
            break; // Exit loop, we have enough
        }

        // 3. Receive new messages if needed
        println!(
            "Node {} needs {} more DKG R1 packages, receiving...",
            context.index, needed_r1_count
        );
        // Use expected_count = needed_r1_count. We might receive more due to buffering.
        let newly_received_msgs = receive_messages(
            listener.clone(),
            needed_r1_count,
            None, // No timeout for DKG
        )
        .await?;

        // Check if receive_messages returned empty (e.g., if it had an internal issue not returning Err)
        if newly_received_msgs.is_empty() && needed_r1_count > 0 {
            // This case might indicate a problem, maybe retry or error out?
            // For DKG with no timeout, this likely means a sender failed.
            // Let's return an error for now if we needed messages but got none.
            return Err(format!(
                "Error: Expected {} DKG R1 messages, but received none.",
                needed_r1_count
            )
            .into());
        }

        // 4. Process newly received messages (and buffer others)
        for msg in newly_received_msgs {
            if let MessageWrapper::DkgRound1(m) = msg {
                if received_round1_packages.len() < context.total as usize {
                    let participant_id = Identifier::try_from(m.participant_index)?;
                    // Only accept packages from discovered peers
                    if context.discovered_peers.contains(&m.participant_index) {
                        if received_round1_packages
                            .insert(participant_id, m.package)
                            .is_none()
                        {
                            println!(
                                "Node {} received DKG R1 package from participant {}",
                                context.index, m.participant_index
                            );
                        }
                    } else {
                        println!(
                            "Node {} ignoring R1 package from undiscovered peer {}",
                            context.index, m.participant_index
                        );
                        context.message_buffer.push(MessageWrapper::DkgRound1(m));
                    }
                } else {
                    context.message_buffer.push(MessageWrapper::DkgRound1(m)); // Buffer excess R1
                }
            } else {
                // Buffer other message types
                context.message_buffer.push(msg);
            }
        }
        // Loop will continue and re-check buffer/needed count
    } // End receive loop

    // Store final map (already checked count in loop)
    context.received_round1_packages = Some(received_round1_packages.clone());
    println!(
        "Node {} finished processing DKG Round 1 packages.",
        context.index
    );

    // --- DKG Round 2 ---
    println!("Node {} starting DKG Round 2...", context.index);
    // ... (DKG part 2 and sending remain the same) ...
    let received_round1_packages_from_others: BTreeMap<_, _> = received_round1_packages
        .iter()
        .filter(|(id_ref, _)| **id_ref != context.my_identifier)
        .map(|(id, pkg)| (*id, pkg.clone()))
        .collect();

    let (round2_secret_package, round2_packages) = frost::keys::dkg::part2(
        context.round1_secret_package.take().unwrap(), // Take ownership
        &received_round1_packages_from_others,
    )
    .expect("DKG part 2 failed");

    context.round2_secret_package = Some(round2_secret_package);

    // Send Round 2 Packages
    println!("Node {} sending DKG Round 2 packages...", context.index);
    let mut send_tasks = Vec::new();
    for (receiver_id, package) in round2_packages {
        if received_round1_packages_from_others.contains_key(&receiver_id) {
            let id_bytes = receiver_id.serialize();
            if id_bytes.len() < 2 {
                panic!("Identifier serialization too short!");
            }
            let receiver_idx = u16::from_be_bytes(
                id_bytes[id_bytes.len() - 2..]
                    .try_into()
                    .expect("Slice failed"),
            );
            // Only send to discovered peers
            if context.discovered_peers.contains(&receiver_idx) {
                let round2_message = Round2Message {
                    participant_index: context.index,
                    package,
                };
                let wrapped_r2_msg = MessageWrapper::DkgRound2(round2_message);
                let device_addr = format!("127.0.0.1:1000{}", receiver_idx);
                println!("[Debug] Sending R2 to device_addr: {}", device_addr);
                send_tasks.push(tokio::spawn(async move {
                    send_to_with_retries(&device_addr, &wrapped_r2_msg, 3).await;
                }));
            }
        }
    }
    for task in send_tasks {
        let _ = task.await;
    }

    // Receive Round 2 Packages (with buffering and loop)
    println!("Node {} receiving DKG Round 2 packages...", context.index);
    let mut received_round2_packages = BTreeMap::new();
    let expected_r2_count = context.total as usize - 1;

    loop {
        // Start receive loop
        // 1. Process existing buffer
        let mut next_buffer = Vec::new();
        for msg in std::mem::take(&mut context.message_buffer) {
            if let MessageWrapper::DkgRound2(m) = msg {
                if received_round2_packages.len() < expected_r2_count {
                    let sender_id = Identifier::try_from(m.participant_index)?;
                    // Only accept from participants we sent R1 to (and got R1 from)
                    if received_round1_packages_from_others.contains_key(&sender_id) {
                        if received_round2_packages
                            .insert(sender_id, m.package)
                            .is_none()
                        {
                            println!(
                                "Node {} processed buffered DKG R2 package from participant {}",
                                context.index, m.participant_index
                            );
                        }
                    } else {
                        // Invalid sender for R2, maybe buffer or discard? Let's buffer.
                        println!(
                            "Node {} buffered R2 from unexpected sender {}",
                            context.index, m.participant_index
                        );
                        next_buffer.push(MessageWrapper::DkgRound2(m));
                    }
                } else {
                    next_buffer.push(MessageWrapper::DkgRound2(m)); // Re-buffer if already have enough R2
                }
            } else {
                next_buffer.push(msg); // Keep other messages
            }
        }
        context.message_buffer = next_buffer; // Put back unprocessed messages

        // 2. Check if we have enough R2 packages
        let needed_r2_count = expected_r2_count.saturating_sub(received_round2_packages.len());
        if needed_r2_count == 0 {
            println!(
                "Node {} collected all required DKG R2 packages.",
                context.index
            );
            break; // Exit loop
        }

        // 3. Receive new messages if needed
        println!(
            "Node {} needs {} more DKG R2 packages, receiving...",
            context.index, needed_r2_count
        );
        let newly_received_msgs = receive_messages(
            listener.clone(),
            needed_r2_count,
            None, // No timeout for DKG
        )
        .await?;

        // Check if receive_messages returned empty
        if newly_received_msgs.is_empty() && needed_r2_count > 0 {
            return Err(format!(
                "Error: Expected {} DKG R2 messages, but received none.",
                needed_r2_count
            )
            .into());
        }

        // 4. Process newly received messages
        for msg in newly_received_msgs {
            if let MessageWrapper::DkgRound2(m) = msg {
                if received_round2_packages.len() < expected_r2_count {
                    let sender_id = Identifier::try_from(m.participant_index)?;
                    if received_round1_packages_from_others.contains_key(&sender_id) {
                        if received_round2_packages
                            .insert(sender_id, m.package)
                            .is_none()
                        {
                            println!(
                                "Node {} received DKG R2 package from participant {}",
                                context.index, m.participant_index
                            );
                        }
                    } else {
                        println!(
                            "Node {} received R2 from unexpected sender {}, buffering.",
                            context.index, m.participant_index
                        );
                        context.message_buffer.push(MessageWrapper::DkgRound2(m)); // Buffer if sender invalid
                    }
                } else {
                    context.message_buffer.push(MessageWrapper::DkgRound2(m)); // Buffer excess R2
                }
            } else {
                // Buffer other message types
                context.message_buffer.push(msg);
            }
        }
        // Loop continues
    } // End receive loop

    // Store final map
    context.received_round2_packages = Some(received_round2_packages.clone());
    println!(
        "Node {} finished processing DKG Round 2 packages.",
        context.index
    );

    // --- DKG Finalize (Part 3) ---
    // ... (DKG part 3 remains the same) ...
    println!("Node {} starting DKG Finalize (Part 3)...", context.index);
    let (kp, pkp) = frost::keys::dkg::part3(
        context.round2_secret_package.as_ref().unwrap(),
        &received_round1_packages_from_others, // Use the filtered map from earlier
        &received_round2_packages,
    )
    .expect("DKG part 3 failed");

    println!("DKG completed for Node {}", context.index);

    // Store keys and derive address
    context.key_package = Some(kp.clone());
    context.pubkey_package = Some(pkp.clone());
    context.eth_address = Some(derive_eth_address(&pkp)?);

    // Save keys to cache
    let key_package_file = format!("eth_key_package_{}.bin", context.index);
    let pubkey_package_file = format!("eth_pubkey_package_{}.bin", context.index);
    let key_bytes = encode_to_vec(&kp, bincode::config::standard())?;
    let pubkey_bytes = encode_to_vec(&pkp, bincode::config::standard())?;
    fs::write(&key_package_file, key_bytes)?;
    fs::write(&pubkey_package_file, pubkey_bytes)?;
    println!("Node {} saved keys to cache.", context.index);
    println!(
        "Node {} derived Ethereum address: {:?}",
        context.index,
        context.eth_address.unwrap()
    );

    // Transition to Idle
    context.state = NodeState::Idle;
    Ok(())
}

async fn handle_idle_state(context: &mut NodeContext) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Node {} in IDLE state", context.index);

    // Ensure listener is set up for receiving messages
    context.setup_listener().await?;
    let listener = context.listener.clone().unwrap();

    if context.is_initiator {
        // ... (Initiator logic remains the same - prompt user) ...
        print!(
            "Initiator (Node {}): Start a new transaction? (y/n): ",
            context.index
        );
        io::stdout().flush()?;
        let input = tokio::task::spawn_blocking(move || {
            let mut buffer = String::new();
            stdin().read_line(&mut buffer).expect("Failed to read line");
            buffer
        })
        .await?;

        if input.trim().eq_ignore_ascii_case("y") {
            context.state = NodeState::TransactionComposition;
        } else {
            println!("Exiting.");
            context.state = NodeState::Completed;
        }
    } else {
        // Participant waits for a transaction message (using buffer and loop)
        println!(
            "Node {} waiting for transaction message from initiator...",
            context.index
        );

        let mut tx_msg_option: Option<TxMessage> = None;

        loop {
            // Start receive loop
            // 1. Check buffer
            let mut next_buffer = Vec::new();
            for msg in std::mem::take(&mut context.message_buffer) {
                if tx_msg_option.is_none() {
                    // Take the first one found
                    if let MessageWrapper::SignTx(m) = msg {
                        tx_msg_option = Some(m);
                        println!("Node {} processed buffered SignTx message.", context.index);
                        // Don't break inner loop, process rest of buffer
                    } else {
                        next_buffer.push(msg); // Keep others
                    }
                } else {
                    next_buffer.push(msg); // Keep others if already found one
                }
            }
            context.message_buffer = next_buffer;

            // 2. Check if found
            if tx_msg_option.is_some() {
                break; // Exit loop
            }

            // 3. Receive if not found in buffer
            println!("Node {} SignTx not in buffer, receiving...", context.index);
            // Expect 1 message, but could receive others
            let newly_received_msgs = receive_messages(
                listener.clone(),
                1,    // Expect at least 1 message
                None, // No timeout for receiving the TX hash
            )
            .await?;

            // Check if receive_messages returned empty
            if newly_received_msgs.is_empty() && tx_msg_option.is_none() {
                return Err("Error: Expected SignTx message, but received none.".into());
            }

            // 4. Process newly received
            for msg in newly_received_msgs {
                if tx_msg_option.is_none() {
                    // Take the first one found
                    if let MessageWrapper::SignTx(m) = msg {
                        tx_msg_option = Some(m);
                        println!("Node {} received SignTx message.", context.index);
                        // Don't break inner loop, process rest of received
                    } else {
                        context.message_buffer.push(msg); // Buffer others
                    }
                } else {
                    context.message_buffer.push(msg); // Buffer others if already found one
                }
            }
            // Loop continues if not found yet
        } // End receive loop

        // Ensure we got the message (should be guaranteed by loop break condition)
        if let Some(tx_msg) = tx_msg_option {
            context.tx_hash_bytes = Some(tx_msg.tx_hash_bytes.clone());
            // Deserialize the transaction request
            let (tx_req, _): (TransactionRequest, _) =
                decode_from_slice(&tx_msg.transaction_request, bincode::config::standard())?;
            context.transaction_request = Some(tx_req);

            println!(
                "Node {} received transaction hash: {}",
                context.index,
                hex::encode(context.tx_hash_bytes.as_ref().unwrap())
            );
            println!(
                "Node {} received transaction request: {:?}",
                context.index,
                context.transaction_request.as_ref().unwrap()
            );
            context.state = NodeState::SigningCommitment;
        } else {
            // Should be unreachable due to loop logic
            return Err("Internal Error: Failed to get SignTx message after loop.".into());
        }
    }
    Ok(())
}

// ... (handle_transaction_composition remains the same) ...

async fn handle_signing_commitment(
    context: &mut NodeContext,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Node {} in SIGNING_COMMITMENT state", context.index);
    let listener = context.listener.clone().unwrap();
    let network_timeout = Duration::from_secs(10); // Network timeout

    // ... (Generate and broadcast commitment remains the same) ...
    let (nonce, commitment) = frost_round1::commit(
        context.key_package.as_ref().unwrap().signing_share(),
        &mut context.rng,
    );
    context.my_nonce = Some(nonce);
    context.my_commitment = Some(commitment.clone());
    let commitment_message = CommitmentMessage {
        sender_identifier: context.my_identifier,
        commitment: commitment.clone(),
    };
    let wrapped_commit_msg = MessageWrapper::SignCommitment(commitment_message);
    broadcast_to_peers(context.index, &context.discovered_peers, &wrapped_commit_msg).await;

    // Receive commitments from others (with buffering, loop, and timeout)
    println!(
        "Node {} receiving commitments (timeout: {:?})...",
        context.index, network_timeout
    );
    let mut commitments_map = BTreeMap::new();
    commitments_map.insert(
        context.my_identifier,
        context.my_commitment.clone().unwrap(),
    );
    let expected_commit_count = context.total as usize; // Expect from everyone including self
    let start_time = tokio::time::Instant::now(); // Use tokio Instant

    let mut timed_out = false;

    loop {
        // Start receive loop
        // 1. Process buffer
        let mut next_buffer = Vec::new();
        for msg in std::mem::take(&mut context.message_buffer) {
            if let MessageWrapper::SignCommitment(m) = msg {
                if commitments_map.len() < expected_commit_count {
                    if commitments_map
                        .insert(m.sender_identifier, m.commitment)
                        .is_none()
                    {
                        println!(
                            "Node {} processed buffered commitment from {:?}",
                            context.index, m.sender_identifier
                        );
                    }
                } else {
                    next_buffer.push(MessageWrapper::SignCommitment(m)); // Re-buffer excess
                }
            } else {
                next_buffer.push(msg); // Keep others
            }
        }
        context.message_buffer = next_buffer;

        // 2. Check if we have enough
        // We need *all* commitments initially to build the SigningPackage later,
        // but only *threshold* commitments to proceed with selection/signing.
        // Let's collect all initially, but check threshold to decide if we can continue.
        let needed_commit_count = expected_commit_count.saturating_sub(commitments_map.len());
        if needed_commit_count == 0 {
            println!("Node {} collected all expected commitments.", context.index);
            break; // Exit loop, have all
        }

        // 3. Check timeout
        let elapsed = start_time.elapsed();
        if elapsed >= network_timeout {
            println!("Node {} commitment receiving timed out.", context.index);
            timed_out = true;
            break; // Exit loop due to timeout
        }
        let remaining_timeout = network_timeout - elapsed;

        // 4. Receive if needed and time remains
        println!(
            "Node {} needs {} more commitments, receiving (remaining time: {:?})...",
            context.index, needed_commit_count, remaining_timeout
        );
        // Use timeout here
        let receive_result = receive_messages(
            listener.clone(),
            needed_commit_count,
            Some(remaining_timeout), // Apply remaining timeout
        )
        .await;

        let newly_received_msgs = match receive_result {
            Ok(msgs) => msgs,
            Err(e) => {
                // Distinguish between timeout error and other errors if possible
                // For now, assume any error might be related to timeout or connection issues
                println!(
                    "Node {} error during commitment receive: {}. Checking collected count.",
                    context.index, e
                );
                timed_out = true; // Assume timeout or critical error
                break; // Exit loop
            }
        };

        // If timeout occurred during receive_messages, newly_received_msgs might be empty or partial.
        // The timeout flag will be checked after processing.

        // 5. Process newly received
        for msg in newly_received_msgs {
            if let MessageWrapper::SignCommitment(m) = msg {
                if commitments_map.len() < expected_commit_count {
                    if commitments_map
                        .insert(m.sender_identifier, m.commitment)
                        .is_none()
                    {
                        println!(
                            "Node {} received commitment from {:?}",
                            context.index, m.sender_identifier
                        );
                    }
                } else {
                    context
                        .message_buffer
                        .push(MessageWrapper::SignCommitment(m)); // Buffer excess
                }
            } else {
                context.message_buffer.push(msg); // Buffer others
            }
        }
        // Loop continues, will re-check count and timeout
    } // End receive loop

    // Store final map in context
    context.commitments_map = Some(commitments_map.clone());

    // Check if threshold met *after* loop (considering timeout)
    if commitments_map.len() < context.threshold as usize {
        // If we timed out or errored out before meeting threshold
        return Err(format!(
            "Error: Not enough commitments received to meet threshold. Got {}, expected at least {}. Timed out: {}",
            commitments_map.len(),
            context.threshold,
            timed_out
        )
        .into());
    }

    println!(
        "Node {} collected {} commitments (threshold is {}). Proceeding.",
        context.index,
        commitments_map.len(),
        context.threshold
    );

    // Transition to next state or wait for selection
    if context.is_initiator {
        context.state = NodeState::SignerSelection;
    } else {
        println!("Node {} waiting for signer selection...", context.index);
        // Wait for selection message (using buffer and loop)
        let mut selection_msg_option: Option<SignerSelectionMessage> = None;

        loop {
            // Start receive loop
            // 1. Check buffer
            let mut next_buffer = Vec::new();
            for msg in std::mem::take(&mut context.message_buffer) {
                if selection_msg_option.is_none() {
                    if let MessageWrapper::SignerSelection(m) = msg {
                        selection_msg_option = Some(m);
                        println!(
                            "Node {} processed buffered SignerSelection message.",
                            context.index
                        );
                    } else {
                        next_buffer.push(msg);
                    }
                } else {
                    next_buffer.push(msg);
                }
            }
            context.message_buffer = next_buffer;

            // 2. Check if found
            if selection_msg_option.is_some() {
                break; // Exit loop
            }

            // 3. Receive if not found
            println!(
                "Node {} SignerSelection not in buffer, receiving...",
                context.index
            );
            let newly_received_msgs = receive_messages(
                listener.clone(),
                1,
                None, // No timeout for selection
            )
            .await?;

            // Check if receive_messages returned empty
            if newly_received_msgs.is_empty() && selection_msg_option.is_none() {
                return Err("Error: Expected SignerSelection message, but received none.".into());
            }

            // 4. Process newly received
            for msg in newly_received_msgs {
                if selection_msg_option.is_none() {
                    if let MessageWrapper::SignerSelection(m) = msg {
                        selection_msg_option = Some(m);
                        println!("Node {} received SignerSelection message.", context.index);
                    } else {
                        context.message_buffer.push(msg);
                    }
                } else {
                    context.message_buffer.push(msg);
                }
            }
            // Loop continues if not found
        } // End receive loop

        // Process the message (guaranteed to be Some by loop break)
        if let Some(selection_msg) = selection_msg_option {
            context.selected_signers = Some(selection_msg.selected_identifiers.clone());
            println!(
                "Node {} received selected signers: {:?}",
                context.index,
                context.selected_signers.as_ref().unwrap()
            );
            let selected = context
                .selected_signers
                .as_ref()
                .unwrap()
                .contains(&context.my_identifier);
            context.state = NodeState::SignatureGeneration { selected };
        } else {
            // Should be unreachable
            return Err("Internal Error: Failed to get SignerSelection message after loop.".into());
        }
    }

    Ok(())
}

// ... (handle_signer_selection remains the same - it only sends) ...

// ... (handle_signature_generation remains the same - selected nodes send/store, others wait) ...

async fn handle_signature_aggregation(
    context: &mut NodeContext,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Node {} in SIGNATURE_AGGREGATION state", context.index);
    let listener = context.listener.clone().unwrap();

    // Calculate how many shares to receive from others
    let expected_share_count = context.threshold as usize; // Total shares needed including self
    // Shares needed from others = threshold - 1 (since initiator is one of the threshold)
    let shares_to_receive = expected_share_count.saturating_sub(1);

    println!(
        "Node {} waiting for signature shares from {} selected participants...",
        context.index, shares_to_receive
    );

    // Use buffer first, then receive loop
    let mut shares_map = context.signature_shares_map.take().unwrap_or_default(); // Start with own share if present

    loop {
        // Start receive loop
        // 1. Process buffer
        let mut next_buffer = Vec::new();
        for msg in std::mem::take(&mut context.message_buffer) {
            if let MessageWrapper::SignShare(m) = msg {
                // Only accept shares from selected signers who haven't sent one yet
                if context.selected_signers.as_ref().unwrap().contains(&m.sender_identifier)
                   && !shares_map.contains_key(&m.sender_identifier) // Check if already received
                   && shares_map.len() < expected_share_count
                {
                    if shares_map.insert(m.sender_identifier, m.share).is_none() {
                        // Should always be none due to check above
                        println!(
                            "Node {} processed buffered share from selected signer {:?}",
                            context.index, m.sender_identifier
                        );
                    }
                } else {
                    // Buffer if not selected, already received, or map full
                    if !context
                        .selected_signers
                        .as_ref()
                        .unwrap()
                        .contains(&m.sender_identifier)
                    {
                        println!(
                            "Node {} ignored & buffered share from non-selected signer {:?}",
                            context.index, m.sender_identifier
                        );
                    } else if shares_map.contains_key(&m.sender_identifier) {
                        println!(
                            "Node {} ignored & buffered duplicate share from signer {:?}",
                            context.index, m.sender_identifier
                        );
                    } else {
                        println!(
                            "Node {} buffered excess share from signer {:?}",
                            context.index, m.sender_identifier
                        );
                    }
                    next_buffer.push(MessageWrapper::SignShare(m));
                }
            } else {
                next_buffer.push(msg); // Keep others
            }
        }
        context.message_buffer = next_buffer;

        // 2. Check if we have enough shares
        let needed_share_count = expected_share_count.saturating_sub(shares_map.len());
        if needed_share_count == 0 {
            println!(
                "Node {} collected all required signature shares.",
                context.index
            );
            break; // Exit loop
        }

        // 3. Receive if needed
        println!(
            "Node {} needs {} more shares, receiving...",
            context.index, needed_share_count
        );
        let newly_received_msgs = receive_messages(
            listener.clone(),
            needed_share_count,
            None, // No timeout for receiving shares? Or add one? Let's keep None for now.
        )
        .await?;

        // Check if receive_messages returned empty
        if newly_received_msgs.is_empty() && needed_share_count > 0 {
            return Err(format!(
                "Error: Expected {} SignShare messages, but received none.",
                needed_share_count
            )
            .into());
        }

        // 4. Process newly received
        for msg in newly_received_msgs {
            if let MessageWrapper::SignShare(m) = msg {
                if context
                    .selected_signers
                    .as_ref()
                    .unwrap()
                    .contains(&m.sender_identifier)
                    && !shares_map.contains_key(&m.sender_identifier)
                    && shares_map.len() < expected_share_count
                {
                    if shares_map.insert(m.sender_identifier, m.share).is_none() {
                        println!(
                            "Node {} received share from selected signer {:?}",
                            context.index, m.sender_identifier
                        );
                    }
                } else {
                    // Buffer if not selected, already received, or map full
                    if !context
                        .selected_signers
                        .as_ref()
                        .unwrap()
                        .contains(&m.sender_identifier)
                    {
                        println!(
                            "Node {} ignored & buffered received share from non-selected signer {:?}",
                            context.index, m.sender_identifier
                        );
                    } else if shares_map.contains_key(&m.sender_identifier) {
                        println!(
                            "Node {} ignored & buffered received duplicate share from signer {:?}",
                            context.index, m.sender_identifier
                        );
                    } else {
                        println!(
                            "Node {} buffered received excess share from signer {:?}",
                            context.index, m.sender_identifier
                        );
                    }
                    context.message_buffer.push(MessageWrapper::SignShare(m));
                }
            } else {
                context.message_buffer.push(msg); // Buffer others
            }
        }
        // Loop continues
    } // End receive loop

    // Verify enough shares collected (guaranteed by loop break)
    println!(
        "Node {} collected all {} required signature shares.",
        context.index,
        shares_map.len() // Use final map length
    );
    context.signature_shares_map = Some(shares_map.clone()); // Put back into context

    // ... (Re-create SigningPackage and aggregate logic remains the same) ...
    let mut final_commitments = BTreeMap::new();
    for id in context.selected_signers.as_ref().unwrap() {
        // Use the *full* commitments map from the previous state
        if let Some(commitment) = context.commitments_map.as_ref().unwrap().get(id) {
            final_commitments.insert(*id, commitment.clone());
        } else {
            // This should ideally not happen if selection was done correctly based on available commitments
            return Err(format!(
                "Error: Commitment missing for selected signer {:?} during aggregation",
                id
            )
            .into());
        }
    }
    // Ensure the number of commitments matches the number of selected signers (threshold)
    if final_commitments.len() != context.threshold as usize {
        return Err(format!(
            "Error: Incorrect number of commitments ({}) found for aggregation SigningPackage creation (expected {})",
            final_commitments.len(), context.threshold
         ).into());
    }

    let signing_package =
        SigningPackage::new(final_commitments, context.tx_hash_bytes.as_ref().unwrap());

    println!("Node {} aggregating partial signatures...", context.index);
    let group_signature: frost::Signature = frost::aggregate(
        &signing_package,
        &shares_map, // Use the map collected above
        context.pubkey_package.as_ref().unwrap(),
    )?;
    println!("Node {} aggregation successful!", context.index);

    // ... (Convert to Eth Sig, find V, store, broadcast remains the same) ...
    let r_point = group_signature.R();
    let z_scalar = group_signature.z();
    let r_bytes: [u8; 32] = r_point.to_affine().x().into();
    let s_bytes: [u8; 32] = z_scalar.to_bytes().into();

    let mut final_v: Option<u64> = None;
    let chain_id = context.chain_id.unwrap();
    let eth_address = context.eth_address.unwrap();
    let tx_hash_bytes = context.tx_hash_bytes.as_ref().unwrap();

    for potential_v in [0u64, 1u64] {
        let v_eip155 = potential_v + 27 + (chain_id.as_u64() * 2 + 35);
        let eth_sig = EthSignature {
            r: U256::from_big_endian(&r_bytes),
            s: U256::from_big_endian(&s_bytes),
            v: v_eip155,
        };
        match eth_sig.recover(H256::from_slice(tx_hash_bytes)) {
            Ok(addr) if addr == eth_address => {
                final_v = Some(v_eip155);
                break;
            }
            _ => {}
        }
    }
    let v = final_v.ok_or("Failed to find correct recovery ID (v)")?;
    println!(
        "Node {} final Ethereum signature: r={}, s={}, v={}",
        context.index,
        hex::encode(r_bytes),
        hex::encode(s_bytes),
        v
    );

    let agg_sig_msg_data = AggregatedSignatureMessage {
        r: r_bytes,
        s: s_bytes,
        v: v as u8,
    };
    context.aggregated_signature = Some(agg_sig_msg_data.clone());
    let agg_sig_msg = MessageWrapper::SignAggregated(agg_sig_msg_data);
    broadcast_to_peers(context.index, &context.discovered_peers, &agg_sig_msg).await;

    // Transition to submission state
    context.state = NodeState::TransactionSubmission;
    Ok(())
}

// ... (handle_transaction_submission remains the same - it only sends) ...

async fn handle_signature_verification(
    context: &mut NodeContext,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Node {} in SIGNATURE_VERIFICATION state", context.index);
    let listener = context.listener.clone().unwrap();

    // Wait for aggregated signature from initiator (using buffer and loop)
    println!(
        "Node {} waiting for aggregated signature from initiator...",
        context.index
    );

    let mut agg_sig_option: Option<AggregatedSignatureMessage> = None;

    loop {
        // Start receive loop
        // 1. Check buffer
        let mut next_buffer = Vec::new();
        for msg in std::mem::take(&mut context.message_buffer) {
            if agg_sig_option.is_none() {
                if let MessageWrapper::SignAggregated(m) = msg {
                    agg_sig_option = Some(m);
                    println!(
                        "Node {} processed buffered SignAggregated message.",
                        context.index
                    );
                } else {
                    next_buffer.push(msg);
                }
            } else {
                next_buffer.push(msg);
            }
        }
        context.message_buffer = next_buffer;

        // 2. Check if found
        if agg_sig_option.is_some() {
            break; // Exit loop
        }

        // 3. Receive if not found
        println!(
            "Node {} SignAggregated not in buffer, receiving...",
            context.index
        );
        let newly_received_msgs = receive_messages(
            listener.clone(),
            1,
            None, // No timeout for final signature
        )
        .await?;

        // Check if receive_messages returned empty
        if newly_received_msgs.is_empty() && agg_sig_option.is_none() {
            return Err("Error: Expected SignAggregated message, but received none.".into());
        }

        // 4. Process newly received
        for msg in newly_received_msgs {
            if agg_sig_option.is_none() {
                if let MessageWrapper::SignAggregated(m) = msg {
                    agg_sig_option = Some(m);
                    println!("Node {} received SignAggregated message.", context.index);
                } else {
                    context.message_buffer.push(msg);
                }
            } else {
                context.message_buffer.push(msg);
            }
        }
        // Loop continues if not found
    } // End receive loop

    // Process the message (guaranteed Some by loop break)
    if let Some(agg_sig) = agg_sig_option {
        println!(
            "Node {} received aggregated signature: r={}, s={}, v={}",
            context.index,
            hex::encode(agg_sig.r),
            hex::encode(agg_sig.s),
            agg_sig.v
        );

        // Optionally verify the signature locally
        // ... (Verification logic remains the same) ...
        let final_eth_signature = EthSignature {
            r: U256::from_big_endian(&agg_sig.r),
            s: U256::from_big_endian(&agg_sig.s),
            v: agg_sig.v.into(),
        };
        if let Some(tx_hash) = context.tx_hash_bytes.as_ref() {
            match final_eth_signature.recover(H256::from_slice(tx_hash)) {
                Ok(addr) if Some(addr) == context.eth_address => println!(
                    "Node {} successfully verified aggregated signature.",
                    context.index
                ),
                Ok(addr) => println!(
                    "Node {} verification failed: recovered wrong address {:?}",
                    context.index, addr
                ),
                Err(e) => println!(
                    "Node {} verification failed: recovery error {}",
                    context.index, e
                ),
            }
        } else {
            println!(
                "Node {} cannot verify signature: missing transaction hash.",
                context.index
            );
        }
    } else {
        // Should be unreachable
        return Err("Internal Error: Failed to get SignAggregated message after loop.".into());
    }

    // Reset signing state for potential next round
    context.tx_hash_bytes = None;
    context.transaction_request = None;
    context.my_nonce = None;
    context.my_commitment = None;
    context.commitments_map = None;
    context.selected_signers = None;
    context.my_signature_share = None;
    // No shares map or aggregated sig for participants

    // Transition back to Idle
    context.state = NodeState::Idle;
    Ok(())
}

// Note: The solana_dkg.rs file was provided but not modified as the issue is in eth_dkg.rs

// --- Re-add missing function definitions ---

async fn handle_transaction_composition(
    context: &mut NodeContext,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Node {} in TRANSACTION_COMPOSITION state", context.index);

    // Ensure provider is set up
    context.setup_provider().await?;
    let provider = context.provider.as_ref().unwrap();
    let chain_id = context.chain_id.unwrap();
    let eth_address = context.eth_address.unwrap();

    // Get transaction details from user
    print!("Enter target Ethereum address (0x...): ");
    io::stdout().flush()?;

    // Use spawn_blocking for stdin
    let target_address_str = tokio::task::spawn_blocking(move || {
        let mut buffer = String::new();
        stdin().read_line(&mut buffer).expect("Failed to read line");
        buffer
    })
    .await?;
    let target_address = Address::from_str(target_address_str.trim())?;

    print!("Enter amount to transfer in Wei: ");
    io::stdout().flush()?;

    // Use spawn_blocking for stdin
    let amount_str = tokio::task::spawn_blocking(move || {
        let mut buffer = String::new();
        stdin().read_line(&mut buffer).expect("Failed to read line");
        buffer
    })
    .await?;
    let amount = U256::from_dec_str(amount_str.trim())?;

    println!(
        "Node {} preparing transaction: {} Wei to {}",
        context.index, amount, target_address
    );

    // Estimate gas and get nonce
    let gas_price = provider.get_gas_price().await?;
    let nonce = provider.get_transaction_count(eth_address, None).await?;

    let mut tx = TransactionRequest::new()
        .to(target_address)
        .value(amount)
        .from(eth_address)
        .gas_price(gas_price)
        .nonce(nonce)
        .chain_id(chain_id.as_u64());

    let gas_estimate = provider.estimate_gas(&tx.clone().into(), None).await?;
    tx = tx.gas(gas_estimate);

    println!("Node {} prepared TxRequest: {:?}", context.index, tx);

    // Get the hash to sign (EIP-155)
    let sighash = tx.sighash();
    let tx_hash_bytes = sighash.as_bytes().to_vec();

    // Store in context
    context.tx_hash_bytes = Some(tx_hash_bytes.clone());
    context.transaction_request = Some(tx.clone()); // Store the unsigned tx request

    // Serialize transaction request for broadcast
    let tx_req_bytes = encode_to_vec(&tx, bincode::config::standard())?;

    // Broadcast the transaction hash and request
    let tx_message = TxMessage {
        tx_hash_bytes: tx_hash_bytes.clone(),
        transaction_request: tx_req_bytes,
    };
    let wrapped_tx_msg = MessageWrapper::SignTx(tx_message);
    broadcast_to_peers(context.index, &context.discovered_peers, &wrapped_tx_msg).await;

    // Transition to next state
    context.state = NodeState::SigningCommitment;
    Ok(())
}

async fn handle_signer_selection(
    context: &mut NodeContext,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Node {} in SIGNER_SELECTION state", context.index);
    // Remove unused listener variable
    // let listener = context.listener.clone().unwrap();

    // Select signers manually (initiator only)
    let mut manually_selected_ids = Vec::with_capacity(context.threshold as usize);
    manually_selected_ids.push(context.my_identifier); // Automatically include self

    let needed_others = context.threshold.saturating_sub(1);

    if needed_others > 0 {
        let available_other_signer_ids: Vec<_> = context
            .commitments_map
            .as_ref()
            .unwrap()
            .keys()
            .filter(|&&id| id != context.my_identifier)
            .cloned()
            .collect();

        if available_other_signer_ids.len() < needed_others as usize {
            return Err(format!(
                "Error: Not enough other participants ({}) available to meet threshold ({}). Need {} more.",
                available_other_signer_ids.len(), context.threshold, needed_others
            ).into());
        }

        println!("Available other participants (who sent commitments):");
        for id in &available_other_signer_ids {
            // Use last two bytes (big-endian) for index derivation with secp256k1
            println!("[Debug] Processing available id for display: {:?}", id); // Debug print
            let id_bytes = id.serialize();
            println!(
                "[Debug] Serialized id_bytes for display (len={}): {:?}",
                id_bytes.len(),
                id_bytes
            ); // Debug print
            if id_bytes.len() < 2 {
                panic!("Identifier serialization too short for display!");
            }
            // Use from_be_bytes
            let idx = u16::from_be_bytes(
                id_bytes[id_bytes.len() - 2..] // Take the last two bytes
                    .try_into()
                    .expect("Identifier serialization slice failed for display"),
            );
            println!("[Debug] Derived idx for display: {}", idx); // Debug print
            println!(" - Node {}", idx);
        }

        loop {
            print!(
                "Enter {} other participant indices (comma-separated) to select for signing: ",
                needed_others
            );
            io::stdout().flush()?;
            // Use tokio::task::spawn_blocking for synchronous stdin read
            let input_str = tokio::task::spawn_blocking(move || {
                let mut buffer = String::new();
                stdin().read_line(&mut buffer).expect("Failed to read line");
                buffer
            })
            .await?;

            let parts: Vec<&str> = input_str.trim().split(',').collect();
            if parts.len() != needed_others as usize {
                eprintln!(
                    "Error: Expected {} indices, but got {}. Please try again.",
                    needed_others,
                    parts.len()
                );
                continue;
            }

            let mut temp_selected_others = Vec::with_capacity(needed_others as usize);
            let mut input_valid = true;
            for part in parts {
                match part.trim().parse::<u16>() {
                    Ok(idx) => match Identifier::try_from(idx) {
                        Ok(id) => {
                            if available_other_signer_ids.contains(&id) {
                                if !temp_selected_others.contains(&id) {
                                    temp_selected_others.push(id);
                                } else {
                                    eprintln!("Error: Index {} entered more than once.", idx);
                                    input_valid = false;
                                    break;
                                }
                            } else if id == context.my_identifier {
                                eprintln!(
                                    "Error: Initiator (Node {}) is already included.",
                                    context.index
                                );
                                input_valid = false;
                                break;
                            } else {
                                eprintln!("Error: Participant {} not available/committed.", idx);
                                input_valid = false;
                                break;
                            }
                        }
                        Err(_) => {
                            eprintln!("Error: Invalid index {}.", idx);
                            input_valid = false;
                            break;
                        }
                    },
                    Err(_) => {
                        eprintln!("Error: Invalid input '{}'.", part);
                        input_valid = false;
                        break;
                    }
                }
            }

            if input_valid {
                manually_selected_ids.extend(temp_selected_others);
                break;
            }
        }
    } else {
        println!(
            "Node {} is the only signer required (threshold 1).",
            context.index
        );
    }

    manually_selected_ids.sort(); // Ensure deterministic order

    // Store in context
    context.selected_signers = Some(manually_selected_ids.clone());

    if context.selected_signers.as_ref().unwrap().len() != context.threshold as usize {
        return Err(format!(
            "Internal Error: Selection resulted in {} signers, expected {}.",
            context.selected_signers.as_ref().unwrap().len(),
            context.threshold
        )
        .into());
    }

    println!(
        "Node {} (Initiator) selected signers: {:?}",
        context.index,
        context.selected_signers.as_ref().unwrap()
    );

    // Broadcast the selection
    let selection_msg = MessageWrapper::SignerSelection(SignerSelectionMessage {
        selected_identifiers: context.selected_signers.clone().unwrap(),
    });
    broadcast_to_peers(context.index, &context.discovered_peers, &selection_msg).await;

    // Transition to next state (initiator is always selected)
    context.state = NodeState::SignatureGeneration { selected: true };
    Ok(())
}

async fn handle_signature_generation(
    context: &mut NodeContext,
    selected: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!(
        "Node {} in SIGNATURE_GENERATION state (selected: {})",
        context.index, selected
    );
    // Listener needed only if participant needs to receive later
    // let listener = context.listener.clone().unwrap();

    if selected {
        // ... (Create SigningPackage and sign logic remains the same) ...
        let mut final_commitments = BTreeMap::new();
        for id in context.selected_signers.as_ref().unwrap() {
            // Use the full commitments map collected earlier
            if let Some(commitment) = context.commitments_map.as_ref().unwrap().get(id) {
                final_commitments.insert(*id, commitment.clone());
            } else {
                // This check is important: did we receive commitment from this selected signer?
                return Err(
                    format!("Error: Commitment missing for selected signer {:?} during SigningPackage creation", id).into(),
                );
            }
        }
        println!(
            "Node {} (Selected) creating SigningPackage for selected signers ({})...",
            context.index,
            final_commitments.len()
        );
        // Ensure the number of commitments matches the threshold (size of selected_signers)
        if final_commitments.len() != context.threshold as usize {
            return Err(
                format!("Error: Incorrect number of commitments ({}) for SigningPackage creation (expected {})", final_commitments.len(), context.threshold).into(),
            );
        }

        let signing_package = SigningPackage::new(
            final_commitments.clone(),
            context.tx_hash_bytes.as_ref().unwrap(),
        );

        println!("Node {} starting FROST Round 2 (Sign)...", context.index);
        let my_signature_share = frost_round2::sign(
            &signing_package,
            context.my_nonce.as_ref().unwrap(),
            context.key_package.as_ref().unwrap(),
        )?;
        println!("Node {} generated signature share.", context.index);
        context.my_signature_share = Some(my_signature_share.clone());

        if context.is_initiator {
            // Initiator adds own share to map and transitions to aggregation
            let mut signature_shares_map = BTreeMap::new();
            signature_shares_map.insert(context.my_identifier, my_signature_share);
            context.signature_shares_map = Some(signature_shares_map);
            println!("Node {} added its own share to map.", context.index);
            context.state = NodeState::SignatureAggregation;
        } else {
            // Participant sends share to initiator
            // ... (Prompting and sending logic remains the same) ...
            println!("\n--- Node {} Action Required ---", context.index);
            println!("Received request to sign the following transaction hash (hex):");
            println!("{}", hex::encode(context.tx_hash_bytes.as_ref().unwrap()));
            print!("Press ENTER to confirm signing and send your share to initiator: ");
            io::stdout().flush()?;
            tokio::task::spawn_blocking(move || {
                let mut buffer = String::new();
                stdin().read_line(&mut buffer).expect("Failed to read line");
                buffer
            })
            .await?;

            println!(
                "Node {} sending signature share to initiator...",
                context.index
            );
            let share_message = ShareMessage {
                sender_identifier: context.my_identifier,
                share: my_signature_share,
            };
            let wrapped_share_msg = MessageWrapper::SignShare(share_message);
            let initiator_addr = "127.0.0.1:10001"; // Assume node 1 is initiator
            send_to(&initiator_addr, &wrapped_share_msg).await;
            println!("Node {} sent share.", context.index);

            // Transition to wait for final signature
            context.state = NodeState::SignatureVerification;
        }
    } else {
        // Node was NOT selected for signing
        println!("Node {} was not selected for signing.", context.index);
        // Transition to wait for final signature
        context.state = NodeState::SignatureVerification;
    }

    Ok(())
}

async fn handle_transaction_submission(
    context: &mut NodeContext,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Node {} in TRANSACTION_SUBMISSION state", context.index);

    let provider = context.provider.as_ref().unwrap();
    // Prefix unused variable with underscore
    let _final_eth_signature = {
        let agg_sig = context.aggregated_signature.as_ref().unwrap();
        EthSignature {
            r: U256::from_big_endian(&agg_sig.r),
            s: U256::from_big_endian(&agg_sig.s),
            v: agg_sig.v.into(), // Convert u8 back to u64
        }
    };

    if let Some(tx_req) = context.transaction_request.as_ref() {
        // RLP encode the transaction with the signature
        // Use the correct variable name here
        let final_eth_signature = {
            let agg_sig = context.aggregated_signature.as_ref().unwrap();
            EthSignature {
                r: U256::from_big_endian(&agg_sig.r),
                s: U256::from_big_endian(&agg_sig.s),
                v: agg_sig.v.into(),
            }
        };
        let signed_tx_bytes = tx_req.rlp_signed(&final_eth_signature);

        println!("\n--- Transaction Prepared ---");
        println!("Signed RLP: {}", hex::encode(&signed_tx_bytes));
        println!("Attempting to send transaction...");

        match provider.send_raw_transaction(signed_tx_bytes).await {
            Ok(pending_tx) => {
                println!(
                    "Transaction successfully sent! TxHash: {:?}",
                    pending_tx.tx_hash()
                );
                // Optionally wait for confirmation here if needed
            }
            Err(e) => {
                println!("Transaction failed to send: {}", e);
                // Log detailed error info (as before)
                match e {
                    ethers_providers::ProviderError::JsonRpcClientError(rpc_err_box) => {
                        eprintln!("RPC Client Error: {}", rpc_err_box);
                    }
                    ethers_providers::ProviderError::EnsError(ens_err) => {
                        eprintln!("ENS Error: {}", ens_err);
                    }
                    ethers_providers::ProviderError::EnsNotOwned(ens_not_owned_err) => {
                        println!("ENS Not Owned Error: {}", ens_not_owned_err);
                    }
                    ethers_providers::ProviderError::SerdeJson(serde_err) => {
                        println!("Serde JSON Error: {}", serde_err);
                    }
                    ethers_providers::ProviderError::HexError(hex_err) => {
                        println!("Hex Error: {}", hex_err);
                    }
                    ethers_providers::ProviderError::HTTPError(http_err) => {
                        println!("HTTP Error: {}", http_err);
                    }
                    ethers_providers::ProviderError::CustomError(custom_err) => {
                        println!("Custom Provider Error: {}", custom_err);
                    }
                    ethers_providers::ProviderError::UnsupportedRPC => {
                        println!("Unsupported RPC method.");
                    }
                    ethers_providers::ProviderError::UnsupportedNodeClient => {
                        println!("Unsupported Node Client.");
                    }
                    _ => {
                        println!("Other Provider Error: {}", e);
                    }
                }
            }
        }
    } else {
        return Err(format!("Node {} transaction request object was lost", context.index).into());
    }

    // Reset signing state for potential next round
    context.tx_hash_bytes = None;
    context.transaction_request = None;
    context.my_nonce = None;
    context.my_commitment = None;
    context.commitments_map = None;
    context.selected_signers = None;
    context.my_signature_share = None;
    context.signature_shares_map = None;
    context.aggregated_signature = None;

    // Transition back to Idle
    context.state = NodeState::Idle;
    Ok(())
}
