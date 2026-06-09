# Import/Export Operations Wireframes

This document contains wireframes for wallet import/export operations and data migration screens.

## Import/Export Menu

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         IMPORT/EXPORT OPERATIONS                            │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Welcome                                                          │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Select Operation:                                               │     │
│   │                                                                   │     │
│   │  Import Operations:                                              │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ > [1] Import Wallet from Keystore File                  │   │     │
│   │  │   [2] Import from Chrome Extension                      │   │     │
│   │  │   [3] Import from QR Code                               │   │     │
│   │  │   [4] Restore from Backup                               │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Export Operations:                                              │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │   [5] Export Wallet for Chrome Extension                │   │     │
│   │  │   [6] Export as QR Code Sequence                        │   │     │
│   │  │   [7] Create Encrypted Backup                           │   │     │
│   │  │   [8] Export Public Keys Only                           │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Batch Operations:                                               │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │   [9] Export All Wallets                                 │   │     │
│   │  │   [0] Migration Tool (Legacy → v2.0)                     │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Available Wallets: 3                             Storage Used: 12.4 MB      │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓/0-9] Select  [Enter] Continue  [Esc] Back                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Import Wallet Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         IMPORT WALLET                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Import/Export Menu                                               │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Import Wallet from Keystore File:                              │     │
│   │                                                                   │     │
│   │  File Path:                                                      │     │
│   │  [/home/user/.frost_keystore/wallet_2of3.json___________]       │     │
│   │  [B] Browse Files                                                │     │
│   │                                                                   │     │
│   │  Password (if encrypted):                                        │     │
│   │  [••••••••••__________________________________]                 │     │
│   │                                                                   │     │
│   │  Import Options:                                                 │     │
│   │  [✓] Verify keystore integrity                                  │     │
│   │  [✓] Check for duplicate wallets                                │     │
│   │  [ ] Override if exists                                          │     │
│   │                                                                   │     │
│   │  File Details:                                                   │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Status: ✓ Valid keystore file detected                  │   │     │
│   │  │ Format: FROST v2.0 (Chrome Extension Compatible)        │   │     │
│   │  │ Curve: secp256k1                                        │   │     │
│   │  │ Type: 2-of-3 threshold                                  │   │     │
│   │  │ Created: 2024-01-20 14:32:00                            │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  [I] Import  [V] Validate Only  [C] Cancel                     │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ File Size: 4.2 KB                                Format: Valid              │
├─────────────────────────────────────────────────────────────────────────────┤
│ [Tab] Next Field  [B] Browse  [I] Import  [Esc] Cancel                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Export for Chrome Extension

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    EXPORT FOR CHROME EXTENSION                              │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Import/Export Menu                                               │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Select Wallet to Export:                                        │     │
│   │                                                                   │     │
│   │  > company_wallet (2-of-3, secp256k1)                           │     │
│   │    treasury_cold (5-of-7, secp256k1)                            │     │
│   │    solana_test (3-of-5, ed25519)                                │     │
│   │                                                                   │     │
│   │  Export Settings:                                                │     │
│   │                                                                   │     │
│   │  Password Protection:                                            │     │
│   │  [••••••••••••••••••••__________________]                       │     │
│   │  Confirm Password:                                               │     │
│   │  [••••••••••••••••••••__________________]                       │     │
│   │                                                                   │     │
│   │  Export Format:                                                  │     │
│   │  ● Chrome Extension Format (Recommended)                        │     │
│   │  ○ Universal JSON Format                                         │     │
│   │  ○ Legacy CLI Format                                             │     │
│   │                                                                   │     │
│   │  Additional Options:                                             │     │
│   │  [✓] Include metadata (creation date, description)              │     │
│   │  [✓] Add import instructions                                    │     │
│   │  [ ] Include transaction history                                │     │
│   │                                                                   │     │
│   │  Output Location:                                                │     │
│   │  [~/Downloads/company_wallet_export.json____________]           │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Ready to Export                                  Size Estimate: ~5 KB       │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Select Wallet  [Tab] Next Field  [E] Export  [Esc] Cancel            │
└─────────────────────────────────────────────────────────────────────────────┘
```

## QR Code Export Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    QR CODE EXPORT - PAGE 1/3                                │
├─────────────────────────────────────────────────────────────────────────────┤
│ Wallet: company_wallet                           Total Pages: 3             │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Scan all QR codes in sequence to import wallet:                │     │
│   │                                                                   │     │
│   │                    ┌─────────────────┐                           │     │
│   │                    │ ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄ │                           │     │
│   │                    │ █ ▄▄▄ █▀█ █ ▄▄▄ █ │                           │     │
│   │                    │ █ ███ █▄▀ █ ███ █ │                           │     │
│   │                    │ █▄▄▄▄▄█ ▄ █▄▄▄▄▄█ │                           │     │
│   │                    │ ▄▄▄▄  ▄▄▀█▄  ▄▄▄▄ │                           │     │
│   │                    │ ██▄▀ ▄▄▄▀▄ ▄▄█▄▀▄ │                           │     │
│   │                    │ █   ███▄▄ ▀▄▄▄▀█▄▄ │                           │     │
│   │                    │ ▄▄▄▄▄▄▄ █ ▄ ▄▄▄▄▄ │                           │     │
│   │                    │ █ ▄▄▄ █ ▄▀▄▀▄▄█▄▄ │                           │     │
│   │                    │ █ ███ █ ▀▄ ▀▄ ▄▄█ │                           │     │
│   │                    │ █▄▄▄▄▄█ █▀▄▀██▄▄▄ │                           │     │
│   │                    └─────────────────┘                           │     │
│   │                                                                   │     │
│   │  Page: 1 of 3                                                    │     │
│   │  Data Type: Key Share Information                                │     │
│   │  Sequence ID: EXPORT-2024-0125-1450-P1                          │     │
│   │                                                                   │     │
│   │  Instructions:                                                   │     │
│   │  1. Open Chrome Extension import page                            │     │
│   │  2. Select "Import from QR Code"                                │     │
│   │  3. Scan all pages in order                                     │     │
│   │  4. Enter the same password used here                           │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Progress: Page 1/3                              Encrypted: Yes              │
├─────────────────────────────────────────────────────────────────────────────┤
│ [→] Next Page  [P] Print  [S] Save as Image  [Esc] Cancel                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Batch Export Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         BATCH EXPORT WALLETS                                │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Import/Export Menu                                               │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Select Wallets to Export:                                       │     │
│   │                                                                   │     │
│   │  [✓] company_wallet      2-of-3   secp256k1   4.2 KB           │     │
│   │  [✓] treasury_cold       5-of-7   secp256k1   6.8 KB           │     │
│   │  [ ] solana_test         3-of-5   ed25519     5.1 KB           │     │
│   │  [ ] test_wallet_old     2-of-2   secp256k1   3.9 KB           │     │
│   │                                                                   │     │
│   │  [A] Select All  [N] Select None  [I] Invert Selection          │     │
│   │                                                                   │     │
│   │  Export Configuration:                                           │     │
│   │                                                                   │     │
│   │  Format: ● ZIP Archive  ○ Directory  ○ Encrypted Archive       │     │
│   │                                                                   │     │
│   │  Master Password (for all wallets):                              │     │
│   │  [••••••••••••••••_______________________]                      │     │
│   │                                                                   │     │
│   │  Output Path:                                                    │     │
│   │  [~/backups/starlab-mpcs-2024-01-25.zip_____________]           │     │
│   │                                                                   │     │
│   │  Options:                                                        │     │
│   │  [✓] Include wallet metadata                                    │     │
│   │  [✓] Add README with import instructions                        │     │
│   │  [✓] Create checksums file                                      │     │
│   │  [ ] Include transaction history                                │     │
│   │                                                                   │     │
│   │  Summary: 2 wallets selected, ~11 KB total                      │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Ready to Export                                  Free Space: 45.2 GB        │
├─────────────────────────────────────────────────────────────────────────────┤
│ [Space] Toggle  [A/N/I] Select  [E] Export  [Esc] Cancel                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Migration Tool Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         WALLET MIGRATION TOOL                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Import/Export Menu              Legacy → v2.0 Migration          │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Migrate Legacy Wallets to v2.0 Format:                         │     │
│   │                                                                   │     │
│   │  Step 1: Scan for Legacy Wallets                                │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Scanning: ~/.frost_keystore/                            │   │     │
│   │  │ [████████████████████████░░░░░░░] 85%                  │   │     │
│   │  │                                                         │   │     │
│   │  │ Found 4 legacy wallets:                                 │   │     │
│   │  │ ✓ old_wallet_1.dat    (v1.0, encrypted)               │   │     │
│   │  │ ✓ backup_keys.json   (v1.2, plaintext)               │   │     │
│   │  │ ✓ team_wallet.dat    (v1.1, encrypted)               │   │     │
│   │  │ ⚠ corrupted.dat      (unreadable)                    │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Step 2: Migration Options                                      │     │
│   │                                                                   │     │
│   │  [✓] Backup original files before migration                    │     │
│   │  [✓] Verify migrated wallets                                   │     │
│   │  [ ] Delete originals after successful migration               │     │
│   │                                                                   │     │
│   │  Password for encrypted files:                                  │     │
│   │  [••••••••••_____________________________]                     │     │
│   │                                                                   │     │
│   │  Migration Summary:                                             │     │
│   │  • 3 wallets ready for migration                               │     │
│   │  • 1 wallet needs manual intervention                          │     │
│   │  • Estimated time: ~30 seconds                                 │     │
│   │                                                                   │     │
│   │  [M] Start Migration  [S] Skip Corrupted  [H] Help             │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Legacy Format: v1.x                              Target Format: v2.0        │
├─────────────────────────────────────────────────────────────────────────────┤
│ [M] Migrate  [S] Skip  [R] Rescan  [V] View Details  [Esc] Cancel         │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Import Success Screen

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         IMPORT SUCCESSFUL                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  ✓ Wallet Successfully Imported!                                │     │
│   │                                                                   │     │
│   │  Wallet Details:                                                │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Name: company_wallet                                     │   │     │
│   │  │ Type: 2-of-3 threshold signature                        │   │     │
│   │  │ Curve: secp256k1                                         │   │     │
│   │  │ Your Index: Participant 2                                │   │     │
│   │  │ Address: 0x742d35Cc6634C0532925a3b844Bc9e7595f2bd      │   │     │
│   │  │                                                           │   │     │
│   │  │ Verification: ✓ All checks passed                        │   │     │
│   │  │ • Key share validated                                    │   │     │
│   │  │ • Threshold parameters confirmed                         │   │     │
│   │  │ • No duplicate wallet found                              │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Next Steps:                                                    │     │
│   │  • Test the wallet with a small transaction                    │     │
│   │  • Verify with other participants                              │     │
│   │  • Create a backup of this wallet                              │     │
│   │                                                                   │     │
│   │  Actions:                                                       │     │
│   │  [V] View Wallet  [S] Sign Test Transaction  [B] Backup        │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Import completed at: 2024-01-25 14:52:33              Source: Chrome Ext   │
├─────────────────────────────────────────────────────────────────────────────┤
│ [V] View  [S] Test Sign  [B] Backup  [Enter] Continue to Wallet List      │
└─────────────────────────────────────────────────────────────────────────────┘
```