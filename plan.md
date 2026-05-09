# Cypher — Implementation Plan

One Solana program. Three market types. Bets encrypted via Arcium until settlement.

---

## Accounts — 5 types

| # | Account | Count | PDA Seeds | Size |
|---|---------|-------|-----------|------|
| 1 | **Protocol** | 1 (singleton) | `["protocol"]` | 83 B |
| 2 | **Market** | 1 per market | `["market", index.to_le_bytes()]` | 395–824 B |
| 3 | **Vault** | 1 per market | `["vault", market.key()]` | 165 B (SPL) |
| 4 | **Bet** | 1 per bet | `["bet", market.key(), bet_index.to_le_bytes()]` | 124–146 B |
| 5 | **MarketGroup** | 1 per tiered lobby | `["market_group", group_index.to_le_bytes()]` | ~561 B |

### Account Details

**Protocol** — singleton, created once on deploy
```
bump: u8
authority: Pubkey              // admin wallet
treasury: Pubkey               // fee collection wallet
default_protocol_fee_bps: u16  // e.g. 50 = 0.50%
market_count: u64              // auto-increment, assigns market indexes
```

**Market** — one per market (or one per tier in tiered lobbies)
```
bump: u8
creator: Pubkey
market_index: u64
market_type: MarketType        // YesNo | MultiOutcome | Accuracy
category: MarketCategory       // Crypto | Politics | Sports | Tech | Economy | Culture | Beyond
status: MarketStatus           // Open | Locked | Settled | Voided
question: String               // max 200 bytes
creator_bond: u64
lp_fee_bps: u16
protocol_fee_bps: u16
resolution_deadline: i64
created_at: i64
total_bets: u64
vault: Pubkey
token_mint: Pubkey
market_group: Option<Pubkey>   // None for standalone, Some for tiered
market_data: MarketData        // YesNo{...} | MultiOutcome{...} | Accuracy{...}
```

**Vault** — standard SPL token account, PDA-owned by the program. Holds all deposited tokens for a market.

**Bet** — one per individual bet placed
```
bump: u8
bettor: Pubkey
market: Pubkey
bet_index: u64
created_at: i64
claimed: bool
bet_data: BetData              // YesNo{...} | MultiOutcome{...} | Accuracy{...}
```

**MarketGroup** — links tiered accuracy markets to one shared outcome
```
bump: u8
creator: Pubkey
group_index: u64
question: String
resolution_deadline: i64
created_at: i64
tier_count: u8                 // 1..=4
tier_labels: [String; 4]       // Bronze, Silver, Gold, Diamond
tier_entry_fees: [u64; 4]
tier_markets: [Pubkey; 4]      // points to each tier's Market account
outcome_value: Option<u64>     // set once at settlement, shared by all tiers
outcome_decimals: u8
settled: bool
```

---

## Instructions — 14 total

| # | Instruction | Who Calls | When | Market Types |
|---|-------------|-----------|------|--------------|
| 1 | `initialize_protocol` | Admin | Once on deploy | — |
| 2 | `create_market` | Creator | To open a new market | YesNo, Multi, Accuracy |
| 3 | `create_tiered_market` | Creator | To open a tiered lobby | Accuracy only |
| 4 | `place_bet_yesno` | Bettor | While market is Open | YesNo |
| 5 | `place_bet_multi` | Bettor | While market is Open | MultiOutcome |
| 6 | `place_bet_accuracy` | Bettor | While market is Open | Accuracy |
| 7 | `lock_market` | Creator / Crank | At resolution_deadline | All |
| 8 | `settle_yesno` | Oracle / Creator | After lock | YesNo |
| 9 | `settle_multi` | Oracle / Creator | After lock | MultiOutcome |
| 10 | `settle_accuracy_step` | Crank bot | After lock (multi-tx) | Accuracy |
| 11 | `settle_group` | Oracle / Creator | After lock | Tiered Accuracy |
| 12 | `claim` | Winner | After settlement | All |
| 13 | `withdraw_creator` | Creator | After settlement | All |
| 14 | `void_market` | Authority | Emergency | All |

### Instruction Details

#### 1. `initialize_protocol`

```
Signer:   admin
Accounts: [Protocol (init)]
Args:     treasury: Pubkey, default_fee_bps: u16

Creates Protocol PDA. Sets authority = admin, market_count = 0.
Called once. Fails if Protocol already exists.
```

#### 2. `create_market`

```
Signer:   creator
Accounts: [Protocol (mut), Market (init), Vault (init), token_mint, creator_token_account]
Args:     question: String, market_type: MarketType, category: MarketCategory,
          bond: u64, lp_fee_bps: u16, deadline: i64, outcomes: Option<Vec<String>>

Flow:
  1. Read Protocol.market_count → market_index
  2. Increment Protocol.market_count
  3. Init Market PDA with all fields, status = Open
  4. Init Vault PDA (SPL token account, authority = program PDA)
  5. Transfer bond from creator → Vault
  6. If MultiOutcome → validate 2..=10 outcomes, init pools to [0; N]
  7. If Accuracy → set entry_fee from args
  8. Emit MarketCreated event
```

#### 3. `create_tiered_market`

```
Signer:   creator
Accounts: [Protocol (mut), MarketGroup (init), Market×4 (init), Vault×4 (init),
           token_mint, creator_token_account]
Args:     question: String, bond: u64, deadline: i64,
          tiers: Vec<(String, u64)>  // (label, entry_fee) — up to 4

Flow:
  1. Init MarketGroup PDA
  2. For each tier: init Market PDA (Accuracy type), init Vault PDA
  3. Link each Market.market_group → MarketGroup
  4. Store tier_markets[] in MarketGroup
  5. Transfer bond → each Vault (or single bond in group)
  6. Emit TieredMarketCreated event
```

#### 4. `place_bet_yesno`

```
Signer:   bettor
Accounts: [Market (mut), Bet (init), Vault (mut), bettor_token_account]
Args:     side: Side, amount: u64

Flow:
  1. Require Market.status == Open
  2. Compute: lp_fee, protocol_fee, net_amount
  3. ARCIUM: encrypt (side, net_amount) → store encrypted_bet on Bet account
  4. Transfer amount → Vault
  5. Market.total_bets += 1 (public counter only — pools stay hidden)
  6. Init Bet PDA with encrypted bet_data
  7. Emit BetPlaced event (bettor, market, amount — side is NOT revealed)
```

#### 5. `place_bet_multi`

```
Signer:   bettor
Accounts: [Market (mut), Bet (init), Vault (mut), bettor_token_account]
Args:     outcome_id: u8, amount: u64

Flow:
  1. Require Market.status == Open, outcome_id < outcome_count
  2. Compute fees, net_amount
  3. ARCIUM: encrypt (outcome_id, net_amount) → encrypted_bet
  4. Transfer amount → Vault
  5. Market.total_bets += 1
  6. Init Bet PDA with encrypted bet_data
  7. Emit BetPlaced event (bettor, market, amount — outcome NOT revealed)
```

#### 6. `place_bet_accuracy`

```
Signer:   bettor
Accounts: [Market (mut), Bet (init), Vault (mut), bettor_token_account]
Args:     estimate: u64

Flow:
  1. Require Market.status == Open
  2. Read entry_fee from Market.market_data
  3. ARCIUM: encrypt (estimate) → encrypted_estimate on Bet account
  4. Transfer entry_fee → Vault
  5. Market.total_bets += 1, market_data.total_pool += entry_fee
  6. Init Bet PDA with encrypted bet_data
  7. Emit BetPlaced event (bettor, market — estimate NOT revealed)
```

#### 7. `lock_market`

```
Signer:   creator or permissionless crank
Accounts: [Market (mut)]
Args:     none

Flow:
  1. Require current_time >= resolution_deadline
  2. Require Market.status == Open
  3. Set Market.status = Locked
  4. Emit MarketLocked event
```

#### 8. `settle_yesno`

```
Signer:   oracle / creator
Accounts: [Market (mut), all Bet accounts for this market, Arcium MPC accounts]
Args:     winning_side: Side

Flow:
  1. Require Market.status == Locked
  2. ARCIUM: trigger MPC computation →
     a. Decrypt all encrypted bets
     b. Compute yes_pool, no_pool, lp_fee_pool, protocol_fee_pool
     c. Post decrypted pool totals back on-chain (verified by MPC nodes)
  3. Set market_data.winning_side = winning_side
  4. Set Market.status = Settled
  5. Emit MarketSettled event (winning_side, yes_pool, no_pool)
```

#### 9. `settle_multi`

```
Signer:   oracle / creator
Accounts: [Market (mut), all Bet accounts, Arcium MPC accounts]
Args:     winning_outcome_id: u8

Flow:
  1. Require Market.status == Locked, winning_outcome_id < outcome_count
  2. ARCIUM: trigger MPC computation →
     a. Decrypt all encrypted bets
     b. Compute pools[0..N], lp_fee_pool, protocol_fee_pool
     c. Post decrypted pool totals on-chain
  3. If pools[winning_outcome_id] == 0 → void market (no winners)
  4. Set market_data.winning_outcome = winning_outcome_id
  5. Set Market.status = Settled
  6. Emit MarketSettled event (winning_outcome, pools)
```

#### 10. `settle_accuracy_step`

```
Signer:   crank bot
Accounts: [Market (mut), batch of Bet accounts, Arcium MPC accounts]
Args:     step: AccuracySettleStep  // SetOutcome | ComputeErrors | ComputeWeights | Finalize

Flow (multi-tx, cranked):
  Step A — SetOutcome:
    1. Set market_data.outcome_value from oracle
  Step B — ComputeErrors (cranked, N bets per tx):
    1. ARCIUM: decrypt estimates in batch
    2. Compute error = |estimate - outcome| for each
    3. After all processed → compute median_error
  Step C — ComputeWeights (cranked, N bets per tx):
    1. For each bet: relative_error = error / median_error
    2. weight = (1/(1+r))^6 (integer math)
    3. Mark won = true/false on each Bet
    4. Accumulate total_weight
  Step D — Finalize:
    1. loser_pool = loser_count * entry_fee
    2. protocol_take = loser_pool * protocol_fee_bps / 10000
    3. prize_pool = loser_pool - protocol_take
    4. Market.status = Settled
    5. Emit MarketSettled event
```

#### 11. `settle_group`

```
Signer:   oracle / creator
Accounts: [MarketGroup (mut)]
Args:     outcome_value: u64, outcome_decimals: u8

Flow:
  1. Set MarketGroup.outcome_value = outcome_value
  2. Each tier Market then settles independently via settle_accuracy_step
     using this shared outcome_value
  3. After all tiers settled → MarketGroup.settled = true
  4. Emit GroupSettled event
```

#### 12. `claim`

```
Signer:   winner (bettor)
Accounts: [Market, Bet (mut), Vault (mut), bettor_token_account, treasury]
Args:     none

Flow:
  1. Require Market.status == Settled, Bet.claimed == false
  2. Compute payout based on market type:
     YesNo:  payout = (net_amount / winning_pool) * losing_pool + net_amount
     Multi:  same formula, losing_pool = total - winning_pool
     Accuracy: payout = (weight / total_weight) * prize_pool + entry_fee
  3. Transfer payout from Vault → bettor
  4. Transfer proportional protocol_fee from Vault → treasury
  5. Set Bet.claimed = true
  6. Emit PayoutClaimed event
```

#### 13. `withdraw_creator`

```
Signer:   creator
Accounts: [Market, Vault (mut), creator_token_account]
Args:     none

Flow:
  1. Require Market.status == Settled, caller == Market.creator
  2. YesNo/Multi: transfer creator_bond + lp_fee_pool from Vault → creator
  3. Accuracy: transfer creator_bond from Vault → creator (no LP fees)
  4. Emit CreatorWithdrawal event
```

#### 14. `void_market`

```
Signer:   authority (Protocol.authority)
Accounts: [Protocol, Market (mut)]
Args:     none

Flow:
  1. Require caller == Protocol.authority
  2. Set Market.status = Voided
  3. Now anyone can call claim_refund → returns full deposit to each bettor
  4. Returns bond to creator
  5. Emit MarketVoided event
```

---

## Events — 8 total

| # | Event | Emitted By | Fields |
|---|-------|-----------|--------|
| 1 | `MarketCreated` | `create_market` | market_index, market_type, category, creator, question, deadline, bond |
| 2 | `TieredMarketCreated` | `create_tiered_market` | group_index, question, tier_count, tier_labels, tier_entry_fees, tier_market_indexes |
| 3 | `BetPlaced` | `place_bet_*` | market_index, bet_index, bettor, amount (side/outcome/estimate **NOT** emitted — encrypted) |
| 4 | `MarketLocked` | `lock_market` | market_index, locked_at |
| 5 | `MarketSettled` | `settle_*` | market_index, winning_side/outcome (pools revealed post-decryption) |
| 6 | `GroupSettled` | `settle_group` | group_index, outcome_value |
| 7 | `PayoutClaimed` | `claim` | market_index, bet_index, bettor, payout_amount |
| 8 | `MarketVoided` | `void_market` | market_index, voided_by |
| 9 | `CreatorWithdrawal` | `withdraw_creator` | market_index, creator, bond_returned, lp_fees_returned |

---

## Arcium Integration — Encrypted Bets Until Settlement

### Why Arcium

Without encryption, pool sizes are public. Everyone can see:
- How much is on Yes vs No (reveals crowd sentiment, enables copy-trading)
- What estimates others submitted (trivially copyable in accuracy markets)
- Which outcome is leading (front-running, last-second bandwagon)

With Arcium, **bets are encrypted from placement to settlement**. Nobody — not even the program — can read individual bets or pool totals until the market settles.

### How It Works

```
PLACE BET (encrypted)
══════════════════════

  Bettor                          Arcium MPC Network                 Solana Program
    │                                    │                                │
    │  1. encrypt(side, amount)          │                                │
    │───────────────────────────────────►│                                │
    │                                    │  2. return encrypted_blob      │
    │◄───────────────────────────────────│                                │
    │                                    │                                │
    │  3. place_bet(encrypted_blob, amount_tokens)                       │
    │───────────────────────────────────────────────────────────────────►│
    │                                    │                                │
    │                                    │    4. store encrypted_blob     │
    │                                    │       in Bet account           │
    │                                    │       transfer tokens → Vault  │
    │                                    │       increment total_bets     │
    │                                    │       (pools NOT updated —     │
    │                                    │        still encrypted)        │
    │                                    │                                │

SETTLEMENT (decrypt + compute)
══════════════════════════════

  Oracle                          Arcium MPC Network                 Solana Program
    │                                    │                                │
    │  1. settle(winning_side)           │                                │
    │───────────────────────────────────────────────────────────────────►│
    │                                    │                                │
    │                                    │  2. Program triggers MPC       │
    │                                    │◄───────────────────────────────│
    │                                    │                                │
    │                                    │  3. MPC nodes collectively     │
    │                                    │     decrypt ALL bets           │
    │                                    │     compute pool totals        │
    │                                    │     verify math                │
    │                                    │                                │
    │                                    │  4. Post verified results      │
    │                                    │     back on-chain              │
    │                                    │───────────────────────────────►│
    │                                    │                                │
    │                                    │    5. Program writes:          │
    │                                    │       yes_pool, no_pool,       │
    │                                    │       winning_side,            │
    │                                    │       status = Settled         │
    │                                    │                                │
    │                                    │    6. Emit MarketSettled        │
    │                                    │       (pools now public)       │
```

### What's Encrypted vs Public

| Data | During Open/Locked | After Settlement |
|------|-------------------|-----------------|
| Market question | Public | Public |
| Market category | Public | Public |
| Market deadline | Public | Public |
| Total bet count | Public | Public |
| Total tokens in vault | Public (SPL balance) | Public |
| Individual bet side/outcome/estimate | **ENCRYPTED** | Decrypted |
| Pool breakdowns (yes/no/per-outcome) | **HIDDEN** (not computed yet) | Public |
| Who bet on what | **ENCRYPTED** | Public |
| Winner/loser status | N/A | Public |
| Payout amounts | N/A | Public |

### Arcium Accounts (additional PDAs)

The Arcium integration adds these to instruction account lists:

```
arcium_config: Pubkey          // Arcium program config
mxe_account: Pubkey            // MXE (Multi-party eXecution Environment) account
encrypted_bet_account: Pubkey  // stores ciphertext per bet
computation_account: Pubkey    // tracks MPC computation state during settlement
```

These are owned by the Arcium program — Cypher's program does a CPI (cross-program invocation) to Arcium for encrypt/decrypt operations.

---

## Full Flows — End to End

### Flow 1: YesNo Market (e.g. "Will BTC hit $200k by Dec 2026?")

```
Step  Instruction              Who            What Happens
────  ───────────────────────  ─────────────  ──────────────────────────────────────
 1    initialize_protocol      Admin          Protocol PDA created (once ever)
 2    create_market             Creator        Market + Vault PDAs created, bond deposited
 3    place_bet_yesno          Alice          Bets Yes $50 → encrypted, tokens in Vault
 4    place_bet_yesno          Bob            Bets No $30 → encrypted, tokens in Vault
 5    place_bet_yesno          Carol          Bets Yes $20 → encrypted, tokens in Vault
      ...more bets...
 6    lock_market              Crank/Creator  After deadline, status → Locked
 7    settle_yesno             Oracle         Arcium MPC decrypts all bets, computes pools,
                                              posts yes_pool + no_pool on-chain,
                                              sets winning_side, status → Settled
 8    claim                    Alice          Computes share, transfers payout from Vault
 9    claim                    Carol          Same
10    withdraw_creator         Creator        Gets bond + LP fees from Vault
```

### Flow 2: MultiOutcome Market (e.g. "Which chain will have highest TVL?")

```
Step  Instruction              Who            What Happens
────  ───────────────────────  ─────────────  ──────────────────────────────────────
 1    create_market             Creator        Market(Multi) + Vault, outcomes = [ETH, SOL, ARB, BASE]
 2    place_bet_multi          Alice          Bets on ETH $50 → encrypted
 3    place_bet_multi          Bob            Bets on SOL $30 → encrypted
 4    place_bet_multi          Dan            Bets on ARB $40 → encrypted
      ...
 5    lock_market              Crank          Status → Locked
 6    settle_multi             Oracle         Arcium decrypts, computes pools[0..3],
                                              sets winning_outcome, status → Settled
 7    claim                    Bob            Winner collects proportional share of losing pools
 8    withdraw_creator         Creator        Bond + LP fees
```

### Flow 3: Accuracy Market (e.g. "What will BTC be at midnight?")

```
Step  Instruction              Who            What Happens
────  ───────────────────────  ─────────────  ──────────────────────────────────────
 1    create_market             Creator        Market(Accuracy, entry_fee=$10) + Vault
 2    place_bet_accuracy       P1             Estimate 99,950 → encrypted, $10 → Vault
 3    place_bet_accuracy       P2             Estimate 99,800 → encrypted, $10 → Vault
 4    place_bet_accuracy       P3             Estimate 99,500 → encrypted, $10 → Vault
      ...6 players total...
 5    lock_market              Crank          Status → Locked
 6    settle_accuracy_step     Crank          Step A: set outcome_value = 100,000
 7    settle_accuracy_step     Crank          Step B: Arcium decrypts estimates,
                                              computes errors, finds median (cranked, N per tx)
 8    settle_accuracy_step     Crank          Step C: compute weights for winners (cranked)
 9    settle_accuracy_step     Crank          Step D: finalize pools, status → Settled
10    claim                    P1 (winner)    Weighted payout from prize_pool
11    claim                    P2 (winner)    Weighted payout from prize_pool
12    withdraw_creator         Creator        Bond returned (no LP fees in accuracy)
```

### Flow 4: Tiered Accuracy Lobby (e.g. "BTC price at midnight" — 4 tiers)

```
Step  Instruction              Who            What Happens
────  ───────────────────────  ─────────────  ──────────────────────────────────────
 1    create_tiered_market     Creator        MarketGroup + 4 Markets + 4 Vaults
                                              Bronze($1), Silver($10), Gold($100), Diamond($1k)
 2    place_bet_accuracy       Player A       Joins Bronze → encrypted estimate, $1 → Vault
 3    place_bet_accuracy       Player B       Joins Gold → encrypted estimate, $100 → Vault
      ...players join different tiers...
 4    lock_market ×4           Crank          Lock all 4 tier Markets
 5    settle_group             Oracle         Set outcome_value on MarketGroup (shared)
 6    settle_accuracy_step ×N  Crank          Crank each tier independently:
                                              decrypt estimates, compute errors/weights/pools
                                              Each tier has its own median, winners, payouts
 7    claim                    Winners        Each tier's winners claim from their tier's Vault
 8    withdraw_creator         Creator        Bond returned from each tier
```

### Flow 5: Void Market (emergency)

```
Step  Instruction              Who            What Happens
────  ───────────────────────  ─────────────  ──────────────────────────────────────
 1    void_market              Authority      Market.status → Voided
 2    claim (refund mode)      Each bettor    Full deposit returned from Vault
 3    withdraw_creator         Creator        Bond returned
```

---

## Instruction → Account Matrix

Which accounts each instruction reads/writes:

| Instruction | Protocol | Market | Vault | Bet | MarketGroup | Arcium |
|------------|----------|--------|-------|-----|-------------|--------|
| `initialize_protocol` | **init** | | | | | |
| `create_market` | **mut** | **init** | **init** | | | |
| `create_tiered_market` | **mut** | **init ×4** | **init ×4** | | **init** | |
| `place_bet_yesno` | | **mut** | **mut** | **init** | | **CPI** |
| `place_bet_multi` | | **mut** | **mut** | **init** | | **CPI** |
| `place_bet_accuracy` | | **mut** | **mut** | **init** | | **CPI** |
| `lock_market` | | **mut** | | | | |
| `settle_yesno` | | **mut** | | read | | **CPI** |
| `settle_multi` | | **mut** | | read | | **CPI** |
| `settle_accuracy_step` | | **mut** | | **mut** | | **CPI** |
| `settle_group` | | | | | **mut** | |
| `claim` | | read | **mut** | **mut** | | |
| `withdraw_creator` | | read | **mut** | | | |
| `void_market` | read | **mut** | | | | |

---

## Error Codes

| Code | Name | When |
|------|------|------|
| 6000 | `MarketNotOpen` | Bet placed on locked/settled/voided market |
| 6001 | `MarketNotLocked` | Settlement called before lock |
| 6002 | `MarketNotSettled` | Claim called before settlement |
| 6003 | `AlreadyClaimed` | Bet.claimed == true |
| 6004 | `InvalidOutcome` | outcome_id >= outcome_count |
| 6005 | `DeadlineNotReached` | Lock called before resolution_deadline |
| 6006 | `Unauthorized` | Caller is not authority/creator where required |
| 6007 | `InvalidTierCount` | Tiers < 1 or > 4 |
| 6008 | `OutcomeTooMany` | More than 10 outcomes for MultiOutcome |
| 6009 | `OutcomeTooFew` | Fewer than 2 outcomes for MultiOutcome |
| 6010 | `ZeroAmount` | Bet amount is 0 |
| 6011 | `NotAWinner` | Loser tries to claim |
| 6012 | `GroupNotSettled` | Tier settlement before group outcome is set |
| 6013 | `DecryptionFailed` | Arcium MPC returned invalid result |
| 6014 | `QuestionTooLong` | Question exceeds 200 bytes |

---

## File Structure (target)

```
programs/cypher/src/
├── lib.rs                         // program entrypoint, instruction dispatch
├── state/
│   ├── mod.rs
│   ├── protocol.rs                // Protocol account
│   ├── market.rs                  // Market + MarketData enum
│   ├── market_group.rs            // MarketGroup account
│   ├── bet.rs                     // Bet + BetData enum
│   └── enums.rs                   // MarketType, MarketStatus, Side, MarketCategory
├── instructions/
│   ├── mod.rs
│   ├── initialize_protocol.rs
│   ├── create_market.rs
│   ├── create_tiered_market.rs
│   ├── place_bet_yesno.rs
│   ├── place_bet_multi.rs
│   ├── place_bet_accuracy.rs
│   ├── lock_market.rs
│   ├── settle_yesno.rs
│   ├── settle_multi.rs
│   ├── settle_accuracy_step.rs
│   ├── settle_group.rs
│   ├── claim.rs
│   ├── withdraw_creator.rs
│   └── void_market.rs
├── errors.rs                      // CypherError enum
├── events.rs                      // all 9 events
└── arcium/
    ├── mod.rs
    ├── encrypt.rs                 // CPI to Arcium for bet encryption
    ├── decrypt.rs                 // CPI to Arcium for settlement decryption
    └── types.rs                   // Arcium-specific account types
```

---

## Summary Counts

| What | Count |
|------|-------|
| Account types | 5 (Protocol, Market, Vault, Bet, MarketGroup) |
| Instructions | 14 |
| Events | 9 |
| Error codes | 15 |
| Market types | 3 (YesNo, MultiOutcome, Accuracy) |
| Categories | 7 |
| Max outcomes (Multi) | 10 |
| Max tiers (Accuracy) | 4 |
| Settlement steps (Accuracy) | 4 (SetOutcome, ComputeErrors, ComputeWeights, Finalize) |
