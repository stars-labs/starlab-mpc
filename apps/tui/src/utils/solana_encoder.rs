//! Solana transaction encoding utilities for both native SOL and SPL tokens

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    transaction::Transaction,
    hash::Hash,
    message::Message,
};
use serde::{Serialize, Deserialize};
use std::str::FromStr;
use std::sync::LazyLock;

/// SPL Token program ID
pub const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// Associated Token Account program ID
pub const ATA_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

/// System program ID
pub const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

/// Parsed `SYSTEM_PROGRAM_ID` as a `Pubkey`.
///
/// Replaces `solana_system_program::id()` — the `solana-system-program`
/// crate is deprecated (from v4.0.0 onward it requires the
/// `agave-unstable-api` feature opt-in per a deprecation notice). The
/// System Program's on-chain address is a well-known all-zeros pubkey
/// that's stable forever, so parsing the constant string once and
/// caching in a `LazyLock` gives the same result without the dep.
static SYSTEM_PROGRAM_PUBKEY: LazyLock<Pubkey> =
    LazyLock::new(|| Pubkey::from_str(SYSTEM_PROGRAM_ID).expect("SYSTEM_PROGRAM_ID is valid"));

/// Common SPL token mint addresses on Solana mainnet
pub struct TokenMints;

impl TokenMints {
    /// USDC on Solana mainnet
    pub const USDC: &'static str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    
    /// USDT on Solana mainnet
    pub const USDT: &'static str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";
    
    /// Wrapped SOL
    pub const WSOL: &'static str = "So11111111111111111111111111111111111111112";
    
    /// RAY (Raydium)
    pub const RAY: &'static str = "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R";
}

/// Solana transaction builder
#[derive(Debug, Clone)]
pub struct SolanaTransactionBuilder {
    instructions: Vec<Instruction>,
    fee_payer: Option<Pubkey>,
    recent_blockhash: Option<Hash>,
}

impl Default for SolanaTransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SolanaTransactionBuilder {
    /// Creates a new transaction builder
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            fee_payer: None,
            recent_blockhash: None,
        }
    }
    
    /// Sets the fee payer
    pub fn fee_payer(mut self, payer: &str) -> Result<Self, String> {
        let pubkey = payer.parse::<Pubkey>()
            .map_err(|e| format!("Invalid fee payer: {}", e))?;
        self.fee_payer = Some(pubkey);
        Ok(self)
    }
    
    /// Sets the recent blockhash
    pub fn recent_blockhash(mut self, blockhash: &str) -> Result<Self, String> {
        let hash = blockhash.parse::<Hash>()
            .map_err(|e| format!("Invalid blockhash: {}", e))?;
        self.recent_blockhash = Some(hash);
        Ok(self)
    }
    
    /// Adds a SOL transfer instruction
    pub fn add_sol_transfer(
        mut self,
        from: &str,
        to: &str,
        _lamports: u64,
    ) -> Result<Self, String> {
        let from_pubkey = from.parse::<Pubkey>()
            .map_err(|e| format!("Invalid from address: {}", e))?;
        let to_pubkey = to.parse::<Pubkey>()
            .map_err(|e| format!("Invalid to address: {}", e))?;
        
        // Create a system transfer instruction manually
        let instruction = Instruction {
            program_id: *SYSTEM_PROGRAM_PUBKEY,
            accounts: vec![
                AccountMeta::new(from_pubkey, true),
                AccountMeta::new(to_pubkey, false),
            ],
            data: vec![], // System transfer encoding would go here
        };
        
        self.instructions.push(instruction);
        Ok(self)
    }
    
    /// Adds an SPL token transfer instruction
    pub fn add_spl_transfer(
        mut self,
        token_program: &str,
        source: &str,
        destination: &str,
        authority: &str,
        amount: u64,
    ) -> Result<Self, String> {
        let token_program_id = token_program.parse::<Pubkey>()
            .unwrap_or_else(|_| TOKEN_PROGRAM_ID.parse().unwrap());
        let source_pubkey = source.parse::<Pubkey>()
            .map_err(|e| format!("Invalid source: {}", e))?;
        let dest_pubkey = destination.parse::<Pubkey>()
            .map_err(|e| format!("Invalid destination: {}", e))?;
        let authority_pubkey = authority.parse::<Pubkey>()
            .map_err(|e| format!("Invalid authority: {}", e))?;
        
        // SPL Token Transfer instruction
        // Add discriminator for Transfer (3 for SPL Token)
        let mut full_data = vec![3u8];
        full_data.extend_from_slice(&amount.to_le_bytes());
        
        let instruction = Instruction {
            program_id: token_program_id,
            accounts: vec![
                AccountMeta::new(source_pubkey, false),
                AccountMeta::new(dest_pubkey, false),
                AccountMeta::new_readonly(authority_pubkey, true),
            ],
            data: full_data,
        };
        
        self.instructions.push(instruction);
        Ok(self)
    }
    
    /// Adds a create associated token account instruction
    pub fn add_create_ata(
        mut self,
        payer: &str,
        wallet: &str,
        mint: &str,
    ) -> Result<Self, String> {
        let payer_pubkey = payer.parse::<Pubkey>()
            .map_err(|e| format!("Invalid payer: {}", e))?;
        let wallet_pubkey = wallet.parse::<Pubkey>()
            .map_err(|e| format!("Invalid wallet: {}", e))?;
        let mint_pubkey = mint.parse::<Pubkey>()
            .map_err(|e| format!("Invalid mint: {}", e))?;
        
        // Derive the associated token account address
        let ata = Self::derive_ata(&wallet_pubkey, &mint_pubkey);
        
        let instruction = Instruction {
            program_id: ATA_PROGRAM_ID.parse().unwrap(),
            accounts: vec![
                AccountMeta::new(payer_pubkey, true),
                AccountMeta::new(ata, false),
                AccountMeta::new_readonly(wallet_pubkey, false),
                AccountMeta::new_readonly(mint_pubkey, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID.parse().unwrap(), false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID.parse().unwrap(), false),
            ],
            data: vec![], // Create ATA has no data
        };
        
        self.instructions.push(instruction);
        Ok(self)
    }
    
    /// Derives an associated token account address
    fn derive_ata(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
        // This is a simplified version - real implementation uses PDA derivation
        // In production, use spl_associated_token_account::get_associated_token_address
        let _seeds = &[
            wallet.as_ref(),
            TOKEN_PROGRAM_ID.parse::<Pubkey>().unwrap().as_ref(),
            mint.as_ref(),
        ];
        
        // For testing, just use wallet address
        // Real implementation would use Pubkey::find_program_address
        *wallet
    }
    
    /// Builds the transaction
    pub fn build(self) -> Result<Transaction, String> {
        let fee_payer = self.fee_payer
            .ok_or_else(|| "Fee payer not set".to_string())?;
        
        if self.instructions.is_empty() {
            return Err("No instructions added".to_string());
        }
        
        let message = Message::new(&self.instructions, Some(&fee_payer));
        Ok(Transaction::new_unsigned(message))
    }
    
    /// Gets the message bytes for signing
    pub fn get_message_for_signing(&self) -> Result<Vec<u8>, String> {
        let fee_payer = self.fee_payer
            .ok_or_else(|| "Fee payer not set".to_string())?;
        let recent_blockhash = self.recent_blockhash
            .ok_or_else(|| "Recent blockhash not set".to_string())?;
        
        let mut message = Message::new(&self.instructions, Some(&fee_payer));
        message.recent_blockhash = recent_blockhash;
        
        Ok(message.serialize())
    }
}

/// Helper functions for common Solana operations
pub struct SolanaHelper;

impl SolanaHelper {
    /// Creates a SOL transfer transaction
    pub fn sol_transfer(
        from: &str,
        to: &str,
        sol_amount: f64,
        recent_blockhash: &str,
    ) -> Result<SolanaTransactionBuilder, String> {
        let lamports = (sol_amount * 1_000_000_000.0) as u64; // 1 SOL = 1e9 lamports
        
        SolanaTransactionBuilder::new()
            .fee_payer(from)?
            .recent_blockhash(recent_blockhash)?
            .add_sol_transfer(from, to, lamports)
    }
    
    /// Creates a USDC transfer transaction
    pub fn usdc_transfer(
        from_wallet: &str,
        to_wallet: &str,
        amount_usdc: f64,
        recent_blockhash: &str,
    ) -> Result<SolanaTransactionBuilder, String> {
        // USDC has 6 decimals
        let amount = (amount_usdc * 1_000_000.0) as u64;
        
        // In real implementation, would derive actual ATAs
        let source_ata = from_wallet; // Simplified
        let dest_ata = to_wallet; // Simplified
        
        SolanaTransactionBuilder::new()
            .fee_payer(from_wallet)?
            .recent_blockhash(recent_blockhash)?
            .add_spl_transfer(
                TOKEN_PROGRAM_ID,
                source_ata,
                dest_ata,
                from_wallet,
                amount,
            )
    }
    
    /// Formats amount with decimals
    pub fn format_amount(amount: f64, decimals: u8) -> u64 {
        let multiplier = 10u64.pow(decimals as u32);
        (amount * multiplier as f64) as u64
    }
    
    /// Decodes a transaction for display
    pub fn decode_transaction(tx: &Transaction) -> String {
        let mut result = String::new();
        
        for (i, instruction) in tx.message.instructions.iter().enumerate() {
            let program_id = tx.message.account_keys[instruction.program_id_index as usize];
            
            result.push_str(&format!("Instruction {}: ", i));
            
            if program_id == *SYSTEM_PROGRAM_PUBKEY {
                result.push_str("System Transfer\n");
            } else if program_id == TOKEN_PROGRAM_ID.parse::<Pubkey>().unwrap() {
                result.push_str("SPL Token Transfer\n");
            } else {
                result.push_str(&format!("Program: {}\n", program_id));
            }
        }
        
        result
    }
}

/// Represents a Solana transaction ready for signing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaSigningPackage {
    /// The serialized message to sign
    pub message: Vec<u8>,
    
    /// Transaction metadata
    pub metadata: TransactionMetadata,
}

/// Transaction metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionMetadata {
    /// Transaction type (SOL, SPL, etc.)
    pub tx_type: String,
    
    /// From address
    pub from: String,
    
    /// To address
    pub to: String,
    
    /// Amount (in smallest unit)
    pub amount: u64,
    
    /// Token mint (if SPL token)
    pub mint: Option<String>,
    
    /// Recent blockhash used
    pub recent_blockhash: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sol_transfer_builder() {
        let from = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
        let to = "2fG3hR8SxZDkMEmL3KhcQfUvPLfgTapZLJcVPsYPMRcK";
        let blockhash = "11111111111111111111111111111111";
        
        let builder = SolanaHelper::sol_transfer(
            from,
            to,
            1.5,
            blockhash,
        ).unwrap();
        
        let tx = builder.build();
        assert!(tx.is_ok());
    }
    
    #[test]
    fn test_spl_transfer_builder() {
        let from = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
        let to = "2fG3hR8SxZDkMEmL3KhcQfUvPLfgTapZLJcVPsYPMRcK";
        let blockhash = "11111111111111111111111111111111";
        
        let builder = SolanaHelper::usdc_transfer(
            from,
            to,
            100.0,
            blockhash,
        ).unwrap();
        
        let message = builder.get_message_for_signing();
        assert!(message.is_ok());
    }
    
    #[test]
    fn test_format_amount() {
        assert_eq!(SolanaHelper::format_amount(1.0, 9), 1_000_000_000); // 1 SOL
        assert_eq!(SolanaHelper::format_amount(100.0, 6), 100_000_000); // 100 USDC
        assert_eq!(SolanaHelper::format_amount(0.5, 6), 500_000); // 0.5 USDC
    }
}