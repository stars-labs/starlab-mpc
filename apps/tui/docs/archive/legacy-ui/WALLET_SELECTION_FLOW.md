# Wallet Selection Flow Implementation Plan

## Overview
This document outlines the implementation of the progressive wallet selection flow for multi-sig operations.

## Current Status ✅
1. **Removed technical logs** from wallet management UI
2. **Improved wallet list display** with clean, professional design
3. **Fixed UI rendering** to hide background logs when popups are shown
4. **Added professional navigation hints** and workflow preview

## Next Implementation Steps

### Phase 1: Mode Selection (After Wallet Selection)
```
┌─────────────────────────────────────────────────────┐
│ 🔐 Operation Mode Selection                         │
├─────────────────────────────────────────────────────┤
│                                                     │
│ Selected Wallet: company-treasury-2of3             │
│ Threshold: 2-of-3 | Curve: secp256k1              │
│                                                     │
│ ┌─────────────────────────────────────────────┐   │
│ │ ▶ 🌐 Online Mode                            │   │
│ │   Use WebRTC for real-time coordination     │   │
│ │   Status: ✅ Network Available              │   │
│ └─────────────────────────────────────────────┘   │
│                                                     │
│ ┌─────────────────────────────────────────────┐   │
│ │   🔒 Offline Mode                           │   │
│ │   Use SD card for air-gapped operations     │   │
│ │   Status: ⚠️  SD Card Not Detected         │   │
│ └─────────────────────────────────────────────┘   │
│                                                     │
│ ┌─────────────────────────────────────────────┐   │
│ │   🔄 Hybrid Mode                            │   │
│ │   Mix online and offline participants       │   │
│ │   Status: ✅ Available                      │   │
│ └─────────────────────────────────────────────┘   │
│                                                     │
│ Navigation: ↑↓ Select | Enter Choose | Esc Back    │
└─────────────────────────────────────────────────────┘
```

### Phase 2: Blockchain Selection
```
┌─────────────────────────────────────────────────────┐
│ ⛓️  Select Blockchain                               │
├─────────────────────────────────────────────────────┤
│                                                     │
│ Wallet: company-treasury-2of3 | Mode: Online       │
│                                                     │
│ Compatible Blockchains (secp256k1):                │
│                                                     │
│ ┌─────────────────────────────────────────────┐   │
│ │ ▶ Ethereum                                  │   │
│ │   Gas: ~$5 | Block Time: 12s               │   │
│ └─────────────────────────────────────────────┘   │
│                                                     │
│ ┌─────────────────────────────────────────────┐   │
│ │   Bitcoin                                   │   │
│ │   Fee: ~$2 | Block Time: 10m                │   │
│ └─────────────────────────────────────────────┘   │
│                                                     │
│ ┌─────────────────────────────────────────────┐   │
│ │   Polygon                                   │   │
│ │   Gas: ~$0.01 | Block Time: 2s              │   │
│ └─────────────────────────────────────────────┘   │
│                                                     │
│ ┌─────────────────────────────────────────────┐   │
│ │   Binance Smart Chain                       │   │
│ │   Gas: ~$0.50 | Block Time: 3s              │   │
│ └─────────────────────────────────────────────┘   │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### Phase 3: Network Selection
```
┌─────────────────────────────────────────────────────┐
│ 🌍 Select Network                                   │
├─────────────────────────────────────────────────────┤
│                                                     │
│ Blockchain: Ethereum                               │
│                                                     │
│ ┌─────────────────────────────────────────────┐   │
│ │ ▶ Mainnet                                   │   │
│ │   Production Network                        │   │
│ │   Chain ID: 1                               │   │
│ └─────────────────────────────────────────────┘   │
│                                                     │
│ ┌─────────────────────────────────────────────┐   │
│ │   Sepolia Testnet                           │   │
│ │   Test Network (Recommended for testing)    │   │
│ │   Chain ID: 11155111                        │   │
│ └─────────────────────────────────────────────┘   │
│                                                     │
│ ┌─────────────────────────────────────────────┐   │
│ │   Goerli Testnet                            │   │
│ │   Legacy Test Network                       │   │
│ │   Chain ID: 5                               │   │
│ └─────────────────────────────────────────────┘   │
│                                                     │
└─────────────────────────────────────────────────────┘
```

## Implementation Components

### 1. New UI Modes to Add
```rust
pub enum UIMode {
    // ... existing modes ...
    
    ModeSelectionForWallet {
        wallet_id: String,
        selected_mode: usize,
    },
    
    BlockchainSelection {
        wallet_id: String,
        mode: OperationMode,
        selected_blockchain: usize,
    },
    
    NetworkSelection {
        wallet_id: String,
        mode: OperationMode,
        blockchain: String,
        selected_network: usize,
    },
    
    SigningOperation {
        wallet_id: String,
        mode: OperationMode,
        blockchain: String,
        network: String,
        operation_state: SigningState,
    },
}

pub enum OperationMode {
    Online,
    Offline,
    Hybrid,
}
```

### 2. State Management
```rust
pub struct WalletOperationState {
    pub wallet_id: String,
    pub wallet_metadata: WalletMetadata,
    pub selected_mode: Option<OperationMode>,
    pub selected_blockchain: Option<String>,
    pub selected_network: Option<NetworkInfo>,
    pub participants: Vec<ParticipantInfo>,
    pub operation_status: OperationStatus,
}
```

### 3. Mode Handler Service
```rust
impl ModeHandler {
    pub fn check_mode_availability(&self, mode: OperationMode) -> ModeStatus {
        match mode {
            OperationMode::Online => self.check_network_connectivity(),
            OperationMode::Offline => self.check_sd_card_availability(),
            OperationMode::Hybrid => self.check_hybrid_requirements(),
        }
    }
    
    pub fn initialize_mode(&mut self, mode: OperationMode) -> Result<()> {
        match mode {
            OperationMode::Online => self.setup_webrtc_mesh(),
            OperationMode::Offline => self.prepare_sd_card_export(),
            OperationMode::Hybrid => self.setup_hybrid_coordination(),
        }
    }
}
```

### 4. Blockchain Compatibility
```rust
impl ChainCompatibility {
    pub fn get_compatible_chains(curve_type: &str) -> Vec<BlockchainInfo> {
        match curve_type {
            "secp256k1" => vec![
                BlockchainInfo::ethereum(),
                BlockchainInfo::bitcoin(),
                BlockchainInfo::polygon(),
                BlockchainInfo::bsc(),
                BlockchainInfo::arbitrum(),
            ],
            "ed25519" => vec![
                BlockchainInfo::solana(),
                BlockchainInfo::near(),
            ],
            _ => vec![],
        }
    }
}
```

## User Flow Implementation

### Step 1: Wallet Selection Handler
```rust
// In handle_key_event for WalletListPopup
KeyCode::Enter => {
    if let Some(wallet) = get_selected_wallet() {
        // Transition to mode selection
        *ui_mode = UIMode::ModeSelectionForWallet {
            wallet_id: wallet.session_id.clone(),
            selected_mode: 0,
        };
        // Load wallet metadata
        return Some(format!("load_wallet:{}", wallet.session_id));
    }
}
```

### Step 2: Mode Selection Handler
```rust
// In handle_key_event for ModeSelectionForWallet
KeyCode::Enter => {
    let mode = match selected_mode {
        0 => OperationMode::Online,
        1 => OperationMode::Offline,
        2 => OperationMode::Hybrid,
        _ => OperationMode::Online,
    };
    
    // Check mode availability
    if mode_handler.check_mode_availability(mode).is_available() {
        // Transition to blockchain selection
        *ui_mode = UIMode::BlockchainSelection {
            wallet_id: wallet_id.clone(),
            mode,
            selected_blockchain: 0,
        };
    } else {
        // Show error message
        show_error("Selected mode is not available");
    }
}
```

### Step 3: Blockchain Selection Handler
```rust
// In handle_key_event for BlockchainSelection
KeyCode::Enter => {
    let blockchain = get_selected_blockchain();
    
    // Transition to network selection
    *ui_mode = UIMode::NetworkSelection {
        wallet_id: wallet_id.clone(),
        mode: mode.clone(),
        blockchain: blockchain.clone(),
        selected_network: 0,
    };
}
```

### Step 4: Network Selection Handler
```rust
// In handle_key_event for NetworkSelection
KeyCode::Enter => {
    let network = get_selected_network();
    
    // Initialize signing operation
    *ui_mode = UIMode::SigningOperation {
        wallet_id: wallet_id.clone(),
        mode: mode.clone(),
        blockchain: blockchain.clone(),
        network: network.clone(),
        operation_state: SigningState::Initializing,
    };
    
    // Start the signing process
    return Some(format!("start_signing:{}:{}:{}:{}", 
        wallet_id, mode, blockchain, network));
}
```

## Error Handling

### User-Friendly Error Messages
```rust
pub fn format_error_for_user(error: &AppError) -> String {
    match error {
        AppError::NetworkUnavailable => 
            "🔴 Network connection is required for online mode. \
             Please check your internet connection or choose offline mode.",
        
        AppError::SdCardNotFound => 
            "🔴 No SD card detected for offline mode. \
             Please insert an SD card or choose online mode.",
        
        AppError::IncompatibleBlockchain => 
            "🔴 This blockchain is not compatible with the wallet's curve type. \
             Please select a different blockchain.",
        
        AppError::InsufficientParticipants => 
            "🔴 Not enough participants available. \
             Need at least {} more participants to meet threshold.",
        
        _ => "🔴 An error occurred. Please try again or contact support."
    }
}
```

## Testing Plan

### Unit Tests
- Mode availability checks
- Blockchain compatibility verification
- State transitions
- Error message formatting

### Integration Tests
- Full flow from wallet selection to signing
- Mode switching scenarios
- Network failure handling
- SD card detection

### UI Tests
- Navigation between screens
- Selection highlighting
- Error display
- Help text accuracy

## Migration Steps

1. **Week 1**: Implement mode selection UI and handler
2. **Week 2**: Add blockchain and network selection
3. **Week 3**: Integrate with signing operations
4. **Week 4**: Testing and refinement

## Success Metrics

- User can complete wallet → mode → blockchain → network flow in < 30 seconds
- Error messages are clear and actionable
- No technical jargon exposed to users
- All navigation paths work correctly
- Proper back navigation at every step