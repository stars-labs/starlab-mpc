# Multi-Wallet Operations Path Wireframes

This document contains detailed wireframes for multi-wallet portfolio management operations, designed for enterprise users managing multiple MPC wallets across different chains and purposes.

## Table of Contents

1. [Portfolio Dashboard](#portfolio-dashboard)
2. [Wallet Group Management](#wallet-group-management)
3. [Batch Operations](#batch-operations)
4. [Portfolio Analytics](#portfolio-analytics)
5. [Risk Management](#risk-management)
6. [Automated Workflows](#automated-workflows)
7. [Cross-Chain Operations](#cross-chain-operations)

---

## Portfolio Dashboard

Comprehensive overview of all wallets with real-time metrics and health indicators.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         PORTFOLIO DASHBOARD                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│ Total Portfolio: $12,847,392.50    24h Change: ▲ +2.34% ($293,847)        │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Portfolio Summary:                                              │     │
│   │  ┌──────────────┬──────────────┬──────────────┬──────────────┐  │     │
│   │  │ Active       │ Total Value  │ At Risk      │ Locked       │  │     │
│   │  │ 12 Wallets   │ $12.8M       │ $1.2M        │ $450K        │  │     │
│   │  └──────────────┴──────────────┴──────────────┴──────────────┘  │     │
│   │                                                                   │     │
│   │  Wallet Groups:                                                  │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Group          Wallets  Value        Health  Actions     │   │     │
│   │  ├─────────────────────────────────────────────────────────┤   │     │
│   │  │ > Treasury     3        $8.2M        ████    [Manage]    │   │     │
│   │  │   Operations   4        $2.4M        ███░    [Manage]    │   │     │
│   │  │   DeFi         2        $1.8M        ████    [Manage]    │   │     │
│   │  │   Cold Storage 3        $450K        ████    [Manage]    │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Recent Activity:                      Quick Stats:            │     │
│   │  ┌────────────────────────┐          ┌───────────────────┐   │     │
│   │  │ 14:32 Transfer $50K    │          │ Pending TX:    3  │   │     │
│   │  │ 14:28 DKG Complete     │          │ Active Sess:   2  │   │     │
│   │  │ 14:15 Wallet Lock      │          │ Alerts:        1  │   │     │
│   │  │ 13:45 Batch Sign       │          │ Next Rotation: 5d │   │     │
│   │  └────────────────────────┘          └───────────────────┘   │     │
│   │                                                                   │     │
│   │  Chain Distribution:                                            │     │
│   │  ETH: ████████ 65%  BTC: ████ 25%  SOL: ██ 8%  Other: 2%    │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Filter: [All Wallets▼]    View: [Grid] [List] [Tree]    Refresh: Auto 30s │
├─────────────────────────────────────────────────────────────────────────────┤
│ [G] Groups  [B] Batch Ops  [A] Analytics  [W] Add Wallet  [R] Refresh     │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Wallet Group Management

Organize wallets into logical groups for easier management.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         WALLET GROUP MANAGEMENT                             │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Portfolio                         Selected Group: Treasury        │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Group Configuration:                                            │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Group Name:     [Treasury_________________________]     │   │     │
│   │  │ Description:    [Main company treasury wallets_____]     │   │     │
│   │  │ Risk Profile:   [⚫ Conservative] [○ Moderate] [○ High] │   │     │
│   │  │ Alert Threshold: [$100,000________] per transaction      │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Group Members:                                                  │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [✓] main_treasury      2/3 sig    $5.2M    Ethereum    │   │     │
│   │  │ [✓] reserve_treasury   3/5 sig    $2.8M    Ethereum    │   │     │
│   │  │ [✓] btc_treasury       3/5 sig    $450K    Bitcoin     │   │     │
│   │  │ [ ] Add wallet to group...                              │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Group Policies:                                                │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ [✓] Require 2-person approval for transactions > $50K   │   │     │
│   │  │ [✓] Daily transaction limit: $500K                      │   │     │
│   │  │ [✓] Whitelist addresses only                            │   │     │
│   │  │ [✓] Auto-lock on suspicious activity                    │   │     │
│   │  │ [ ] Allow cross-group transfers                         │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Access Control:                                                 │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Role            Users              Permissions          │   │     │
│   │  │ Admin           cfo@, cto@         Full access          │   │     │
│   │  │ Operator        ops1@, ops2@       Sign, View           │   │     │
│   │  │ Viewer          audit@             View only            │   │     │
│   │  │ [+] Add Role                                           │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Group Value: $8.45M        Members: 3        Last Modified: 2 days ago     │
├─────────────────────────────────────────────────────────────────────────────┤
│ [S] Save  [D] Delete Group  [M] Move Wallets  [P] Policies  [Esc] Cancel  │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Batch Operations

Execute operations across multiple wallets simultaneously.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         BATCH OPERATIONS                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Portfolio                    Operation: Batch Transfer           │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Operation Type:                                                 │     │
│   │  [⚫ Transfer] [○ Sign] [○ Lock] [○ Update] [○ Rotate Keys]    │     │
│   │                                                                   │     │
│   │  Source Wallets:                                                 │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Wallet              Available    Amount      Status      │   │     │
│   │  ├─────────────────────────────────────────────────────────┤   │     │
│   │  │ [✓] treasury_main   12.5 ETH    [5.0 ETH___] Ready      │   │     │
│   │  │ [✓] treasury_ops    8.3 ETH     [3.0 ETH___] Ready      │   │     │
│   │  │ [ ] defi_vault      45.2 ETH    [_________]  -          │   │     │
│   │  │ [✓] reserve_fund    25.0 ETH    [10.0 ETH__] Ready      │   │     │
│   │  │                                                           │   │     │
│   │  │ Total Selected: 3 wallets       18.0 ETH                │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Destination Configuration:                                      │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Mode: [⚫ Single Address] [○ Multiple] [○ Distribution]  │   │     │
│   │  │                                                           │   │     │
│   │  │ To Address: [0x742d35Cc6634C0532925a3b844Bc9e759_____] │   │     │
│   │  │ ENS/Label:  [payments.company.eth________________]      │   │     │
│   │  │                                                           │   │     │
│   │  │ Transaction Parameters:                                  │   │     │
│   │  │ Gas Price:  [⚫ Auto] [○ Low] [○ Standard] [○ Fast]    │   │     │
│   │  │ Priority:   [○ Queue] [⚫ Normal] [○ Expedite]         │   │     │
│   │  │ Schedule:   [⚫ Now] [○ Later: ____________]            │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Batch Summary:                                                 │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Total Amount:      18.0 ETH (~$45,000)                  │   │     │
│   │  │ From Wallets:      3                                     │   │     │
│   │  │ Transactions:      3                                     │   │     │
│   │  │ Est. Network Fee:  0.009 ETH (~$22.50)                  │   │     │
│   │  │ Required Sigs:     7 total (2+2+3)                      │   │     │
│   │  │ Est. Time:         15-20 minutes                         │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Validation: ✓ Ready              [P] Preview  [S] Sign & Execute  [C] Cancel│
├─────────────────────────────────────────────────────────────────────────────┤
│ [Tab] Navigate  [Space] Select  [P] Preview  [S] Sign  [Esc] Cancel        │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Portfolio Analytics

Advanced analytics and insights across all wallets.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         PORTFOLIO ANALYTICS                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Portfolio        Period: [7D] [30D] [90D] [1Y] [All] [Custom]   │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Performance Overview (30 Days):                                 │     │
│   │                                                                   │     │
│   │  Portfolio Value:                                                │     │
│   │  $15M ┤                                    ╱────────────       │     │
│   │       │                              ╱────╯                     │     │
│   │  $12M ┤                        ╱────╯                           │     │
│   │       │                  ╱────╯                                 │     │
│   │   $9M ┤────────────────╯                                       │     │
│   │       └────────────────────────────────────────────────►       │     │
│   │        Jan 1        Jan 10        Jan 20         Jan 30        │     │
│   │                                                                   │     │
│   │  Key Metrics:                    Asset Performance:             │     │
│   │  ┌─────────────────────┐       ┌──────────────────────────┐   │     │
│   │  │ Total Return: +34.2%│       │ ETH    +42.1%  ████████  │   │     │
│   │  │ Best:  DeFi   +67%  │       │ BTC    +28.5%  ██████    │   │     │
│   │  │ Worst: Cold   +12%  │       │ SOL    +15.3%  ███       │   │     │
│   │  │ Sharpe: 2.34        │       │ USDC    +0.1%  ·         │   │     │
│   │  └─────────────────────┘       └──────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Transaction Analysis:                                           │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Type         Count    Volume      Avg Size    Fees      │   │     │
│   │  │ Transfers    248      $4.2M       $16,935     $1,842    │   │     │
│   │  │ DeFi Swaps   89       $2.8M       $31,460     $3,421    │   │     │
│   │  │ Staking      12       $850K       $70,833     $234      │   │     │
│   │  │ Governance   34       -           -           $89       │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Risk Analysis:                                                  │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Concentration Risk:  Medium (largest wallet: 41%)       │   │     │
│   │  │ Liquidity Risk:      Low (92% in liquid assets)        │   │     │
│   │  │ Operational Risk:    Low (all signatures active)        │   │     │
│   │  │ Market Risk:         High (85% volatile assets)         │   │     │
│   │  │ Overall Risk Score:  6.2/10                             │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ Export: [PDF] [CSV] [API]          Alerts: 2 active      Auto-refresh: ON  │
├─────────────────────────────────────────────────────────────────────────────┤
│ [P] Period  [F] Filter  [E] Export  [A] Alerts  [C] Compare  [R] Refresh  │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Risk Management

Comprehensive risk monitoring and management across portfolio.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         RISK MANAGEMENT                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Portfolio              Overall Risk: MEDIUM (6.2/10)             │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Risk Dashboard:                                                 │     │
│   │                                                                   │     │
│   │  Risk Indicators:                                                │     │
│   │  ┌──────────────────────────────────────────────────────────┐  │     │
│   │  │ Market Risk       ████████░░  HIGH   Volatility: 28%    │  │     │
│   │  │ Liquidity Risk    ██░░░░░░░░  LOW    Coverage: 8.2x     │  │     │
│   │  │ Operational Risk  ███░░░░░░░  LOW    Uptime: 99.9%      │  │     │
│   │  │ Concentration     █████░░░░░  MED    Top wallet: 41%    │  │     │
│   │  │ Counterparty      ████░░░░░░  MED    Exposures: 12      │  │     │
│   │  └──────────────────────────────────────────────────────────┘  │     │
│   │                                                                   │     │
│   │  High Risk Alerts:                                               │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ ⚠ CRITICAL: DeFi Vault exposure exceeds 30% limit       │   │     │
│   │  │   Current: 34.2% | Limit: 30% | Excess: $540K           │   │     │
│   │  │   [View Details] [Rebalance] [Update Limit] [Ignore]    │   │     │
│   │  │                                                           │   │     │
│   │  │ ⚠ WARNING: Key rotation overdue for cold_storage_2      │   │     │
│   │  │   Last rotation: 127 days ago | Policy: 90 days         │   │     │
│   │  │   [Rotate Now] [Schedule] [View Policy] [Snooze]        │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Risk Limits & Policies:                                         │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Policy                Current    Limit      Status       │   │     │
│   │  │ Single TX Size        $125K      $500K      ✓ OK        │   │     │
│   │  │ Daily Volume          $1.2M      $5M        ✓ OK        │   │     │
│   │  │ DeFi Exposure         34.2%      30%        ⚠ OVER      │   │     │
│   │  │ Stablecoin Reserve    8%         10%        ⚠ UNDER     │   │     │
│   │  │ Gas Reserve           0.5 ETH    0.1 ETH    ✓ OK        │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Stress Test Results:                                            │     │
│   │  Market Drop -20%: Portfolio Value $10.3M (-19.2%)             │     │
│   │  Market Drop -50%: Portfolio Value $6.4M  (-49.8%)             │     │
│   │  Gas Spike 10x:    Operating Cost +$4,200/day                  │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ [S] Stress Test  [L] Set Limits  [R] Rebalance  [E] Export Report         │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Navigate  [Enter] Details  [A] Acknowledge  [F] Fix  [Esc] Back      │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Automated Workflows

Configure and monitor automated portfolio management workflows.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         AUTOMATED WORKFLOWS                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Portfolio                    Active Workflows: 5  Paused: 2      │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Active Workflows:                                               │     │
│   │                                                                   │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ > Treasury Rebalancing                      ● Active     │   │     │
│   │  │   Frequency: Daily at 14:00 UTC                         │   │     │
│   │  │   Last Run: Today 14:00 (Success)                       │   │     │
│   │  │   Actions: Check balances → Calculate targets →         │   │     │
│   │  │            Execute transfers → Verify completion        │   │     │
│   │  │   [View Log] [Edit] [Pause] [Run Now]                  │   │     │
│   │  ├─────────────────────────────────────────────────────────┤   │     │
│   │  │   Fee Optimization                         ● Active     │   │     │
│   │  │   Trigger: Gas price < 30 Gwei                          │   │     │
│   │  │   Queue: 12 pending transactions ($234K)                │   │     │
│   │  │   Savings: $1,842 (last 30 days)                       │   │     │
│   │  │   [View Queue] [Settings] [Pause]                       │   │     │
│   │  ├─────────────────────────────────────────────────────────┤   │     │
│   │  │   Yield Harvesting                        ○ Paused     │   │     │
│   │  │   Frequency: Weekly                                     │   │     │
│   │  │   Paused: Manual review required                        │   │     │
│   │  │   Pending Harvest: $12,450                              │   │     │
│   │  │   [Review] [Resume] [Edit] [Delete]                     │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Create New Workflow:                                           │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Templates:                                               │   │     │
│   │  │ • Dollar Cost Averaging (DCA)                           │   │     │
│   │  │ • Liquidity Management                                  │   │     │
│   │  │ • Risk-Based Rebalancing                               │   │     │
│   │  │ • Tax Loss Harvesting                                  │   │     │
│   │  │ • Custom Workflow...                                    │   │     │
│   │  │                                                           │   │     │
│   │  │ [Select Template to Continue]                           │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Workflow Performance:                                           │     │
│   │  Total Executions: 1,247    Success Rate: 98.2%                │     │
│   │  Value Processed: $45.2M    Cost Savings: $18,420              │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ [N] New Workflow  [T] Templates  [H] History  [S] Settings  [Esc] Back    │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Navigate  [Enter] Details  [P] Pause/Resume  [E] Edit  [D] Delete    │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Cross-Chain Operations

Manage operations across different blockchain networks.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CROSS-CHAIN OPERATIONS                              │
├─────────────────────────────────────────────────────────────────────────────┤
│ « Back to Portfolio                    Connected Chains: 5  Bridges: 3     │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────┐     │
│   │                                                                   │     │
│   │  Chain Overview:                                                 │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Chain      Wallets   Balance        Gas     Status      │   │     │
│   │  ├─────────────────────────────────────────────────────────┤   │     │
│   │  │ Ethereum   5         $8.2M          2.5 ETH  ● Online   │   │     │
│   │  │ Bitcoin    2         $2.4M          -        ● Online   │   │     │
│   │  │ Solana     3         $1.2M          5.2 SOL  ● Online   │   │     │
│   │  │ Polygon    1         $450K          120 MATIC● Online   │   │     │
│   │  │ Arbitrum   1         $380K          0.8 ETH  ● Online   │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Cross-Chain Transfer:                                           │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ From:  [Ethereum▼]    Wallet: [treasury_main▼]         │   │     │
│   │  │        Balance: 12.5 ETH (~$31,250)                    │   │     │
│   │  │                                                           │   │     │
│   │  │ To:    [Polygon▼]     Wallet: [polygon_ops▼]           │   │     │
│   │  │        Current: 1,200 USDC                              │   │     │
│   │  │                                                           │   │     │
│   │  │ Amount: [5.0] ETH    (~$12,500)                        │   │     │
│   │  │ Bridge: [⚫ Native Bridge] [○ Hop] [○ Synapse]         │   │     │
│   │  │                                                           │   │     │
│   │  │ Route Analysis:                                         │   │     │
│   │  │ • Native Bridge: 15-20 min, Fee: $45                   │   │     │
│   │  │ • Hop Protocol: 2-5 min, Fee: $78 + 0.3%             │   │     │
│   │  │ • Synapse: 3-8 min, Fee: $65 + 0.2%                  │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Active Bridges:                                                │     │
│   │  ┌─────────────────────────────────────────────────────────┐   │     │
│   │  │ Time    From→To        Amount    Status    TX           │   │     │
│   │  │ 14:32   ETH→Polygon    2.5 ETH   ⟳ 40%     [View]      │   │     │
│   │  │ 14:15   SOL→ETH        500 SOL   ✓ Done    [View]      │   │     │
│   │  │ 13:45   ETH→Arbitrum   1.0 ETH   ✓ Done    [View]      │   │     │
│   │  └─────────────────────────────────────────────────────────┘   │     │
│   │                                                                   │     │
│   │  Bridge Limits: Daily: $2M (Used: $450K)  Per TX: $500K       │     │
│   │                                                                   │     │
│   └─────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│ [B] Bridge  [S] Swap  [L] Liquidity  [H] History  [R] Routes  [Esc] Back │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Navigate  [Tab] Switch Fields  [Enter] Execute  [C] Calculate        │
└─────────────────────────────────────────────────────────────────────────────┘
```