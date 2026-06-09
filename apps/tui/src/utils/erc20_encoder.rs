//! ERC20 transaction encoding utilities

use ethers_core::types::{U256, H160};
use ethers_core::abi::{Token, encode as abi_encode};
use rlp::RlpStream;
use sha3::{Digest, Keccak256};

/// ERC20 transfer function selector (keccak256("transfer(address,uint256)"))
const TRANSFER_SELECTOR: &[u8] = &[0xa9, 0x05, 0x9c, 0xbb];

/// ERC20 approve function selector (keccak256("approve(address,uint256)"))
const APPROVE_SELECTOR: &[u8] = &[0x09, 0x5e, 0xa7, 0xb3];

/// ERC20 transferFrom function selector
const TRANSFER_FROM_SELECTOR: &[u8] = &[0x23, 0xb8, 0x72, 0xdd];

/// Common ERC20 token addresses on Ethereum mainnet
pub struct TokenAddresses;

impl TokenAddresses {
    /// USDC on Ethereum mainnet
    pub const USDC: &'static str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
    
    /// USDT on Ethereum mainnet
    pub const USDT: &'static str = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
    
    /// DAI on Ethereum mainnet
    pub const DAI: &'static str = "0x6B175474E89094C44Da98b954EedeAC495271d0F";
    
    /// WETH on Ethereum mainnet
    pub const WETH: &'static str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
}

/// ERC20 transaction builder
pub struct ERC20Transaction {
    /// Token contract address
    pub token_address: H160,
    
    /// Transaction data (function selector + encoded params)
    pub data: Vec<u8>,
    
    /// Gas price in wei
    pub gas_price: U256,
    
    /// Gas limit
    pub gas_limit: U256,
    
    /// Transaction nonce
    pub nonce: u64,
    
    /// Chain ID (1 for mainnet)
    pub chain_id: u64,
}

impl ERC20Transaction {
    /// Creates a transfer transaction
    pub fn transfer(
        token_address: &str,
        recipient: &str,
        amount: U256,
        gas_price: U256,
        nonce: u64,
    ) -> Result<Self, String> {
        let token_addr = token_address.parse::<H160>()
            .map_err(|e| format!("Invalid token address: {}", e))?;
        
        let recipient_addr = recipient.parse::<H160>()
            .map_err(|e| format!("Invalid recipient address: {}", e))?;
        
        // Encode transfer(address,uint256)
        let mut data = Vec::new();
        data.extend_from_slice(TRANSFER_SELECTOR);
        
        // Encode parameters
        let params = vec![
            Token::Address(recipient_addr),
            Token::Uint(amount),
        ];
        let encoded_params = abi_encode(&params);
        data.extend_from_slice(&encoded_params);
        
        Ok(Self {
            token_address: token_addr,
            data,
            gas_price,
            gas_limit: U256::from(65000), // Standard ERC20 transfer gas
            nonce,
            chain_id: 1, // Mainnet
        })
    }
    
    /// Creates an approve transaction
    pub fn approve(
        token_address: &str,
        spender: &str,
        amount: U256,
        gas_price: U256,
        nonce: u64,
    ) -> Result<Self, String> {
        let token_addr = token_address.parse::<H160>()
            .map_err(|e| format!("Invalid token address: {}", e))?;
        
        let spender_addr = spender.parse::<H160>()
            .map_err(|e| format!("Invalid spender address: {}", e))?;
        
        // Encode approve(address,uint256)
        let mut data = Vec::new();
        data.extend_from_slice(APPROVE_SELECTOR);
        
        // Encode parameters
        let params = vec![
            Token::Address(spender_addr),
            Token::Uint(amount),
        ];
        let encoded_params = abi_encode(&params);
        data.extend_from_slice(&encoded_params);
        
        Ok(Self {
            token_address: token_addr,
            data,
            gas_price,
            gas_limit: U256::from(50000), // Standard approve gas
            nonce,
            chain_id: 1,
        })
    }
    
    /// Creates a transferFrom transaction
    pub fn transfer_from(
        token_address: &str,
        from: &str,
        to: &str,
        amount: U256,
        gas_price: U256,
        nonce: u64,
    ) -> Result<Self, String> {
        let token_addr = token_address.parse::<H160>()
            .map_err(|e| format!("Invalid token address: {}", e))?;
        
        let from_addr = from.parse::<H160>()
            .map_err(|e| format!("Invalid from address: {}", e))?;
            
        let to_addr = to.parse::<H160>()
            .map_err(|e| format!("Invalid to address: {}", e))?;
        
        // Encode transferFrom(address,address,uint256)
        let mut data = Vec::new();
        data.extend_from_slice(TRANSFER_FROM_SELECTOR);
        
        // Encode parameters
        let params = vec![
            Token::Address(from_addr),
            Token::Address(to_addr),
            Token::Uint(amount),
        ];
        let encoded_params = abi_encode(&params);
        data.extend_from_slice(&encoded_params);
        
        Ok(Self {
            token_address: token_addr,
            data,
            gas_price,
            gas_limit: U256::from(80000), // TransferFrom uses more gas
            nonce,
            chain_id: 1,
        })
    }
    
    /// Encodes the transaction for signing (EIP-155)
    pub fn encode_for_signing(&self) -> Vec<u8> {
        // Use RlpStream for encoding in RLP 0.6
        let mut stream = RlpStream::new();
        stream.begin_list(9);
        stream.append(&self.nonce);
        stream.append(&self.gas_price.as_u64());
        stream.append(&self.gas_limit.as_u64());
        stream.append(&self.token_address.as_bytes());
        stream.append(&0u64); // value (0 for ERC20 calls)
        stream.append(&self.data);
        stream.append(&self.chain_id);
        stream.append(&0u8);
        stream.append(&0u8);
        
        stream.out().to_vec()
    }
    
    /// Gets the transaction hash for signing
    pub fn signing_hash(&self) -> Vec<u8> {
        let encoded = self.encode_for_signing();
        let mut hasher = Keccak256::new();
        hasher.update(&encoded);
        hasher.finalize().to_vec()
    }
    
    /// Encodes the signed transaction for broadcast
    pub fn encode_signed(&self, signature: &[u8]) -> Result<Vec<u8>, String> {
        if signature.len() < 64 {
            return Err("Invalid signature length".to_string());
        }
        
        let r = &signature[..32];
        let s = &signature[32..64];
        
        // Calculate v value for EIP-155
        // v = chainId * 2 + 35 or chainId * 2 + 36
        let v = self.chain_id * 2 + 35; // Would need recovery ID to determine 35 or 36
        
        // Use RlpStream for encoding in RLP 0.6
        let mut stream = RlpStream::new();
        stream.begin_list(9);
        stream.append(&self.nonce);
        stream.append(&self.gas_price.as_u64());
        stream.append(&self.gas_limit.as_u64());
        stream.append(&self.token_address.as_bytes());
        stream.append(&0u64); // value (0 for ERC20 calls)
        stream.append(&self.data);
        stream.append(&v);
        stream.append(&r);
        stream.append(&s);
        
        Ok(stream.out().to_vec())
    }
}

/// Helper functions for common token operations
pub struct ERC20Helper;

impl ERC20Helper {
    /// Formats token amount with decimals
    /// E.g., 1 USDC (6 decimals) = 1_000_000
    pub fn format_amount(amount: f64, decimals: u8) -> U256 {
        let multiplier = 10u64.pow(decimals as u32);
        let wei_amount = (amount * multiplier as f64) as u64;
        U256::from(wei_amount)
    }
    
    /// Creates a USDC transfer transaction
    pub fn usdc_transfer(
        recipient: &str,
        amount_usdc: f64,
        gas_price_gwei: u64,
        nonce: u64,
    ) -> Result<ERC20Transaction, String> {
        let amount = Self::format_amount(amount_usdc, 6); // USDC has 6 decimals
        let gas_price = U256::from(gas_price_gwei) * U256::from(1_000_000_000u64); // Convert gwei to wei
        
        ERC20Transaction::transfer(
            TokenAddresses::USDC,
            recipient,
            amount,
            gas_price,
            nonce,
        )
    }
    
    /// Creates a USDT transfer transaction
    pub fn usdt_transfer(
        recipient: &str,
        amount_usdt: f64,
        gas_price_gwei: u64,
        nonce: u64,
    ) -> Result<ERC20Transaction, String> {
        let amount = Self::format_amount(amount_usdt, 6); // USDT has 6 decimals
        let gas_price = U256::from(gas_price_gwei) * U256::from(1_000_000_000u64);
        
        ERC20Transaction::transfer(
            TokenAddresses::USDT,
            recipient,
            amount,
            gas_price,
            nonce,
        )
    }
    
    /// Creates a DAI transfer transaction
    pub fn dai_transfer(
        recipient: &str,
        amount_dai: f64,
        gas_price_gwei: u64,
        nonce: u64,
    ) -> Result<ERC20Transaction, String> {
        let amount = Self::format_amount(amount_dai, 18); // DAI has 18 decimals
        let gas_price = U256::from(gas_price_gwei) * U256::from(1_000_000_000u64);
        
        ERC20Transaction::transfer(
            TokenAddresses::DAI,
            recipient,
            amount,
            gas_price,
            nonce,
        )
    }
    
    /// Decodes transaction data to get function and parameters
    pub fn decode_transaction_data(data: &[u8]) -> String {
        if data.len() < 4 {
            return "Invalid data".to_string();
        }
        
        let selector = &data[..4];
        
        match selector {
            TRANSFER_SELECTOR => {
                if data.len() >= 68 { // 4 + 32 + 32
                    let recipient_bytes = &data[16..36]; // Skip padding
                    let amount_bytes = &data[36..68];
                    let recipient = format!("0x{}", hex::encode(recipient_bytes));
                    let amount = U256::from_big_endian(amount_bytes);
                    format!("transfer({}, {})", recipient, amount)
                } else {
                    "transfer(invalid)".to_string()
                }
            },
            APPROVE_SELECTOR => {
                if data.len() >= 68 {
                    let spender_bytes = &data[16..36];
                    let amount_bytes = &data[36..68];
                    let spender = format!("0x{}", hex::encode(spender_bytes));
                    let amount = U256::from_big_endian(amount_bytes);
                    format!("approve({}, {})", spender, amount)
                } else {
                    "approve(invalid)".to_string()
                }
            },
            TRANSFER_FROM_SELECTOR => {
                if data.len() >= 100 { // 4 + 32 + 32 + 32
                    let from_bytes = &data[16..36];
                    let to_bytes = &data[48..68];
                    let amount_bytes = &data[68..100];
                    let from = format!("0x{}", hex::encode(from_bytes));
                    let to = format!("0x{}", hex::encode(to_bytes));
                    let amount = U256::from_big_endian(amount_bytes);
                    format!("transferFrom({}, {}, {})", from, to, amount)
                } else {
                    "transferFrom(invalid)".to_string()
                }
            },
            _ => format!("Unknown function: 0x{}", hex::encode(selector)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_erc20_transfer_encoding() {
        let tx = ERC20Transaction::transfer(
            TokenAddresses::USDC,
            "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb7",
            U256::from(1_000_000), // 1 USDC
            U256::from(20_000_000_000u64), // 20 gwei
            42,
        ).unwrap();
        
        assert_eq!(tx.data.len(), 68); // 4 bytes selector + 64 bytes params
        assert_eq!(&tx.data[..4], TRANSFER_SELECTOR);
    }
    
    #[test]
    fn test_usdc_helper() {
        let tx = ERC20Helper::usdc_transfer(
            "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb7",
            100.5, // 100.5 USDC
            30, // 30 gwei
            10,
        ).unwrap();
        
        // 100.5 USDC = 100_500_000 (6 decimals)
        let decoded = ERC20Helper::decode_transaction_data(&tx.data);
        assert!(decoded.contains("transfer"));
    }
    
    #[test]
    fn test_transaction_hash() {
        let tx = ERC20Transaction::transfer(
            TokenAddresses::USDC,
            "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb7",
            U256::from(1_000_000),
            U256::from(20_000_000_000u64),
            42,
        ).unwrap();
        
        let hash = tx.signing_hash();
        assert_eq!(hash.len(), 32);
    }
}