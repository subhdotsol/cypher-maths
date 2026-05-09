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
Owner:     Cypher Program
Authority: Protocol.authority (admin wallet) — can update fees, set treasury, void markets
Payer:     Admin (pays rent on deploy)

bump: u8
authority: Pubkey              // admin wallet — THE top-level authority
treasury: Pubkey               // fee collection wallet
default_protocol_fee_bps: u16  // e.g. 50 = 0.50%
market_count: u64              // auto-increment, assigns market indexes
```
- **Who can mutate:** only `initialize_protocol` (init), `create_market` (increments market_count)
- **Authority powers:** void any market, update fees, change treasury address

**Market** — one per market (or one per tier in tiered lobbies)
```
Owner:     Cypher Program
Authority: Market.creator — the wallet that created this market
Payer:     Creator (pays rent at creation)

bump: u8
creator: Pubkey                // creator wallet — can lock, settle, withdraw bond+fees
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
- **Who can mutate:**
  - `create_market` → init
  - `place_bet` → increments total_bets, updates market_data pools (encrypted)
  - `lock_market` → creator or permissionless crank (after deadline) → sets status = Locked
  - `settle_market` → oracle/creator → sets winning outcome, pools, status = Settled
  - `void_market` → Protocol.authority ONLY → sets status = Voided
- **Creator powers:** lock market, settle market (provide outcome), withdraw bond + LP fees
- **Creator CANNOT:** void market (only Protocol.authority can), claim other people's bets

**Vault** — SPL token account, holds all deposited tokens for one market
```
Owner:     SPL Token Program
Authority: Cypher Program PDA (program-derived signer) — ONLY the program can move tokens out
Payer:     Creator (pays rent at market creation)
PDA:       ["vault", market.key()]
```
- **Who can move tokens IN:** anyone (standard SPL transfer — bettors deposit, creator deposits bond)
- **Who can move tokens OUT:** ONLY the Cypher program via PDA signing:
  - `claim` → program signs transfer to winner
  - `withdraw_creator` → program signs transfer to creator
  - `claim` (voided) → program signs refund to bettor
- **No human wallet has authority over the Vault.** The program PDA is the sole signer.

**Bet** — one per individual bet placed
```
Owner:     Cypher Program
Authority: Bet.bettor — the wallet that placed this bet
Payer:     Bettor (pays rent at bet placement, reclaimable after claim)

bump: u8
bettor: Pubkey                 // bettor wallet — can claim payout or refund
market: Pubkey
bet_index: u64
created_at: i64
claimed: bool
bet_data: BetData              // YesNo{...} | MultiOutcome{...} | Accuracy{...}
```
- **Who can mutate:**
  - `place_bet` → init (creates the Bet account)
  - `settle_market` (accuracy only) → crank writes error, weight, won fields
  - `claim` → bettor sets claimed = true, receives payout
- **Bettor powers:** claim payout (if winner), claim refund (if voided)
- **Bettor CANNOT:** modify bet after placement, cancel bet, claim someone else's bet

**MarketGroup** — links tiered accuracy markets to one shared outcome
```
Owner:     Cypher Program
Authority: MarketGroup.creator — same wallet that created the tiered lobby
Payer:     Creator (pays rent at creation)

bump: u8
creator: Pubkey                // creator wallet
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
- **Who can mutate:**
  - `create_market` (with tiers) → init
  - `settle_market` → oracle/creator sets outcome_value, then marks settled = true after all tiers done
- **Creator powers:** same as Market creator — provides outcome for settlement

### Ownership & Authority Summary

```
┌──────────────────────────────────────────────────────────────────────┐
│                         AUTHORITY HIERARCHY                          │
│                                                                      │
│  Protocol.authority (admin)                                          │
│    │                                                                 │
│    ├── Can: void ANY market, update fees, change treasury            │
│    ├── Cannot: place bets, claim payouts, create markets (as admin)  │
│    │                                                                 │
│    ▼                                                                 │
│  Market.creator (per market)                                         │
│    │                                                                 │
│    ├── Can: lock market, settle market, withdraw bond + LP fees      │
│    ├── Cannot: void market, modify bets, claim other's payouts       │
│    │                                                                 │
│    ▼                                                                 │
│  Bet.bettor (per bet)                                                │
│    │                                                                 │
│    ├── Can: claim OWN payout (winner) or refund (voided)             │
│    ├── Cannot: modify/cancel bet, claim others, settle market        │
│    │                                                                 │
│    ▼                                                                 │
│  Vault — NO human authority                                          │
│    Program PDA is sole signer. Tokens only move via program logic.   │
│    No admin, creator, or bettor can directly withdraw from Vault.    │
└──────────────────────────────────────────────────────────────────────┘

Account Owner (on-chain Solana owner field):
  Protocol    → Cypher Program
  Market      → Cypher Program
  MarketGroup → Cypher Program
  Bet         → Cypher Program
  Vault       → SPL Token Program (token authority = Cypher Program PDA)
```

---

## Instructions — 8 total

| # | Instruction | Who Calls | When |
|---|-------------|-----------|------|
| 1 | `initialize_protocol` | Admin | Once on deploy |
| 2 | `create_market` | Creator | To open any market type |
| 3 | `place_bet` | Bettor | While market is Open |
| 4 | `lock_market` | Creator / Crank | At resolution_deadline |
| 5 | `settle_market` | Oracle / Crank | After lock |
| 6 | `claim` | Winner | After settlement |
| 7 | `withdraw_creator` | Creator | After settlement |
| 8 | `void_market` | Authority | Emergency |

---

### 1. `initialize_protocol`

```
Signer:   admin
Accounts: [Protocol (init)]
Args:     treasury: Pubkey, default_fee_bps: u16

Creates Protocol PDA. Sets authority = admin, market_count = 0.
Called once. Fails if Protocol already exists.
```

---

### 2. `create_market`

One instruction, branches by `market_type`. If Accuracy + tiers provided, creates a tiered lobby.

```
Signer:   creator
Accounts:
  Always:     [Protocol (mut), Market (init), Vault (init), token_mint, creator_token_account]
  If tiered:  [MarketGroup (init), Market×N (init), Vault×N (init)]  // up to 4 tiers

Args:
  question: String              // max 200 bytes
  market_type: MarketType       // YesNo | MultiOutcome | Accuracy
  category: MarketCategory
  bond: u64
  lp_fee_bps: u16
  deadline: i64
  market_params: CreateMarketParams   // enum — type-specific args
```

```rust
enum CreateMarketParams {
    YesNo,
    // no extra args — just two pools

    MultiOutcome {
        outcomes: Vec<String>,      // 2..=10 labels, each ≤32 bytes
    },

    Accuracy {
        entry_fee: u64,
        tiers: Option<Vec<TierConfig>>,  // None = standalone, Some = tiered lobby
    },
}

struct TierConfig {
    label: String,      // "Bronze", "Silver", etc.
    entry_fee: u64,     // in token base units
}
```

**Flow — standalone (YesNo / Multi / Accuracy without tiers):**
```
1. Read Protocol.market_count → market_index
2. Protocol.market_count += 1
3. Init Market PDA: status = Open, market_data based on type
4. Init Vault PDA (SPL token account, authority = program PDA)
5. Transfer bond from creator → Vault
6. match market_params:
     YesNo       → market_data = YesNo { yes_pool: 0, no_pool: 0, ... }
     MultiOutcome → validate 2..=10 outcomes, market_data = MultiOutcome { pools: [0; N], ... }
     Accuracy     → market_data = Accuracy { entry_fee, total_pool: 0, ... }
7. Emit MarketCreated
```

**Flow — tiered (Accuracy + tiers provided):**
```
1. Init MarketGroup PDA
2. For each tier (up to 4):
     a. Protocol.market_count → index, Protocol.market_count += 1
     b. Init Market PDA (Accuracy type), set market_group = Some(group_key)
     c. Init Vault PDA
3. Store tier_markets[] in MarketGroup
4. Transfer bond → Vaults
5. Emit MarketCreated (one event per tier market + one for the group)
```

---

### 3. `place_bet`

One instruction. Reads `market.market_type` to know what the bettor is doing.

```
Signer:   bettor
Accounts: [Market (mut), Bet (init), Vault (mut), bettor_token_account, Arcium accounts]

Args:
  bet_params: PlaceBetParams    // enum — what the bettor is submitting
```

```rust
enum PlaceBetParams {
    YesNo {
        side: Side,         // Yes or No
        amount: u64,
    },

    MultiOutcome {
        outcome_id: u8,     // which outcome (0..N-1)
        amount: u64,
    },

    Accuracy {
        estimate: u64,      // the player's numeric prediction
    },
}
```

**Flow (all types):**
```
1. Require Market.status == Open
2. Require bet_params variant matches Market.market_type
     (can't submit YesNo bet to an Accuracy market)
3. match bet_params:

     YesNo { side, amount }:
       a. Compute: lp_fee, protocol_fee, net_amount
       b. ARCIUM: encrypt (side, net_amount) → ciphertext stored on Bet
       c. Transfer amount → Vault
       d. Bet.bet_data = YesNo { side: encrypted, amounts... }

     MultiOutcome { outcome_id, amount }:
       a. Require outcome_id < market_data.outcome_count
       b. Compute fees, net_amount
       c. ARCIUM: encrypt (outcome_id, net_amount) → ciphertext
       d. Transfer amount → Vault
       e. Bet.bet_data = MultiOutcome { outcome_id: encrypted, amounts... }

     Accuracy { estimate }:
       a. Read entry_fee from market_data
       b. ARCIUM: encrypt (estimate) → ciphertext
       c. Transfer entry_fee → Vault
       d. market_data.total_pool += entry_fee
       e. Bet.bet_data = Accuracy { estimate: encrypted, entry_fee }

4. Market.total_bets += 1
5. Emit BetPlaced (bettor, market_index, bet_index, amount — choice NOT revealed)
```

---

### 4. `lock_market`

```
Signer:   creator or permissionless crank
Accounts: [Market (mut)]
Args:     none

Flow:
  1. Require current_time >= resolution_deadline
  2. Require Market.status == Open
  3. Market.status = Locked
  4. Emit MarketLocked
```

For tiered lobbies: call `lock_market` on each tier's Market individually. Same instruction, called N times.

---

### 5. `settle_market`

One instruction. Branches by `market.market_type`. Accuracy uses multi-step cranking internally.

```
Signer:   oracle / creator / crank
Accounts:
  Always:      [Market (mut), Arcium MPC accounts]
  If accuracy: [batch of Bet accounts (mut)]
  If tiered:   [MarketGroup (mut)]

Args:
  settle_params: SettleMarketParams
```

```rust
enum SettleMarketParams {
    YesNo {
        winning_side: Side,
    },

    MultiOutcome {
        winning_outcome_id: u8,
    },

    Accuracy {
        step: AccuracySettleStep,
    },
}

enum AccuracySettleStep {
    SetOutcome { outcome_value: u64, outcome_decimals: u8 },
    ComputeErrors,          // cranked — processes N bets per tx
    ComputeWeights,         // cranked — processes N bets per tx
    Finalize,
}
```

**Flow — YesNo (single tx):**
```
1. Require Market.status == Locked
2. ARCIUM MPC:
     a. Decrypt all encrypted bets for this market
     b. Compute yes_pool, no_pool, lp_fee_pool, protocol_fee_pool
     c. Post verified pool totals on-chain
3. market_data.winning_side = winning_side
4. Market.status = Settled
5. Emit MarketSettled (winning_side, yes_pool, no_pool — pools now public)
```

**Flow — MultiOutcome (single tx):**
```
1. Require Market.status == Locked, winning_outcome_id < outcome_count
2. ARCIUM MPC:
     a. Decrypt all encrypted bets
     b. Compute pools[0..N], lp_fee_pool, protocol_fee_pool
     c. Post verified pool totals on-chain
3. If pools[winning_outcome_id] == 0 → void market (nobody bet on winner)
4. market_data.winning_outcome = winning_outcome_id
5. Market.status = Settled
6. Emit MarketSettled (winning_outcome, pools)
```

**Flow — Accuracy (cranked, multiple txs):**
```
Step A — SetOutcome (1 tx):
  1. If market has market_group → read outcome_value from MarketGroup
     (oracle calls settle_market on the group first to set shared outcome)
     If standalone → use outcome_value from args
  2. market_data.outcome_value = outcome_value

Step B — ComputeErrors (cranked, N bets per tx):
  1. ARCIUM: decrypt estimates in this batch
  2. For each Bet: error = |estimate - outcome_value|
  3. Store error on each Bet account
  4. After ALL bets processed → compute median_error
  5. market_data.median_error = median

Step C — ComputeWeights (cranked, N bets per tx):
  1. For each Bet:
       relative_error = error / median_error
       weight = (1/(1+r))^6 (integer math with scaling)
       if error < median → won = true, else won = false
  2. Accumulate total_weight
  3. Set winner_count, loser_count on market_data

Step D — Finalize (1 tx):
  1. loser_pool = loser_count * entry_fee
  2. protocol_take = loser_pool * protocol_fee_bps / 10000
  3. prize_pool = loser_pool - protocol_take
  4. Market.status = Settled
  5. Emit MarketSettled
```

**Tiered lobby settlement — same instruction, called in order:**
```
1. settle_market on MarketGroup → sets shared outcome_value
2. settle_market(Accuracy::SetOutcome) on each tier → reads from group
3. settle_market(Accuracy::ComputeErrors) on each tier → cranked
4. settle_market(Accuracy::ComputeWeights) on each tier → cranked
5. settle_market(Accuracy::Finalize) on each tier
6. After all tiers done → MarketGroup.settled = true
```

---

### 6. `claim`

```
Signer:   bettor
Accounts: [Market (read), Bet (mut), Vault (mut), bettor_token_account, treasury]
Args:     none

Flow:
  1. Require Market.status == Settled (or Voided for refund)
  2. Require Bet.claimed == false

  3. If Market.status == Voided:
       → refund: transfer full deposit back to bettor
       → skip to step 5

  4. match market_data:
       YesNo:
         if bet side != winning_side → error NotAWinner
         payout = net_amount + (net_amount / winning_pool) * losing_pool
         protocol_share = proportional protocol_fee

       MultiOutcome:
         if bet outcome_id != winning_outcome → error NotAWinner
         winning_pool = pools[winning_outcome]
         losing_pool = sum(all pools) - winning_pool
         payout = net_amount + (net_amount / winning_pool) * losing_pool
         protocol_share = proportional protocol_fee

       Accuracy:
         if won != true → error NotAWinner
         payout = entry_fee + (weight / total_weight) * prize_pool

  5. Transfer payout from Vault → bettor
  6. Transfer protocol_share from Vault → treasury (if not voided)
  7. Bet.claimed = true
  8. Emit PayoutClaimed
```

---

### 7. `withdraw_creator`

```
Signer:   creator
Accounts: [Market (read), Vault (mut), creator_token_account]
Args:     none

Flow:
  1. Require Market.status == Settled or Voided
  2. Require caller == Market.creator

  3. match market_data:
       YesNo / MultiOutcome:
         transfer = creator_bond + lp_fee_pool
       Accuracy:
         transfer = creator_bond (no LP fees in accuracy)

  4. Transfer from Vault → creator
  5. Emit CreatorWithdrawal
```

---

### 8. `void_market`

```
Signer:   authority (Protocol.authority)
Accounts: [Protocol (read), Market (mut)]
Args:     none

Flow:
  1. Require caller == Protocol.authority
  2. Market.status = Voided
  3. Bettors now call `claim` → gets refund (full deposit back)
  4. Creator calls `withdraw_creator` → gets bond back
  5. Emit MarketVoided
```

---

## Events — 7 total

| # | Event | Emitted By | Fields |
|---|-------|-----------|--------|
| 1 | `MarketCreated` | `create_market` | market_index, market_type, category, creator, question, deadline, bond, tiers (if tiered) |
| 2 | `BetPlaced` | `place_bet` | market_index, bet_index, bettor, amount (side/outcome/estimate **NOT** emitted — encrypted) |
| 3 | `MarketLocked` | `lock_market` | market_index, locked_at |
| 4 | `MarketSettled` | `settle_market` | market_index, market_type, winning_side/outcome/median (pools revealed post-decryption) |
| 5 | `PayoutClaimed` | `claim` | market_index, bet_index, bettor, payout_amount |
| 6 | `CreatorWithdrawal` | `withdraw_creator` | market_index, creator, bond_returned, lp_fees_returned |
| 7 | `MarketVoided` | `void_market` | market_index, voided_by |

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
    │  1. settle_market(outcome)         │                                │
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
    │                                    │       pools, winner info,      │
    │                                    │       status = Settled         │
    │                                    │                                │
    │                                    │    6. Emit MarketSettled        │
    │                                    │       (pools now public)       │
```

### What's Encrypted vs Public

| Data | During Open/Locked | After Settlement |
|------|-------------------|-----------------|
| Market question, category, deadline | Public | Public |
| Total bet count | Public | Public |
| Total tokens in vault | Public (SPL balance) | Public |
| Individual bet side/outcome/estimate | **ENCRYPTED** | Decrypted |
| Pool breakdowns (yes/no/per-outcome) | **HIDDEN** | Public |
| Who bet on what | **ENCRYPTED** | Public |
| Winner/loser status | N/A | Public |
| Payout amounts | N/A | Public |

### Arcium Accounts (passed in when needed)

```
arcium_config: Pubkey          // Arcium program config
mxe_account: Pubkey            // MXE (Multi-party eXecution Environment)
encrypted_bet_account: Pubkey  // ciphertext per bet
computation_account: Pubkey    // MPC computation state during settlement
```

Owned by Arcium program. Cypher does CPI to Arcium for encrypt/decrypt.

---

## Full Flows — End to End

### Flow 1: YesNo Market

```
"Will BTC hit $200k by Dec 2026?"

Step  Instruction        Who            What Happens
────  ─────────────────  ─────────────  ─────────────────────────────────────────
 1    initialize_protocol Admin         Protocol PDA created (once ever)
 2    create_market       Creator        Market(YesNo) + Vault, bond deposited
 3    place_bet           Alice          YesNo { side: Yes, amount: 50 } → encrypted
 4    place_bet           Bob            YesNo { side: No, amount: 30 } → encrypted
 5    place_bet           Carol          YesNo { side: Yes, amount: 20 } → encrypted
 6    lock_market         Crank          After deadline → status = Locked
 7    settle_market       Oracle         YesNo { winning_side: Yes }
                                         → Arcium decrypts → pools revealed → Settled
 8    claim               Alice          Winner payout from Vault
 9    claim               Carol          Winner payout from Vault
10    withdraw_creator    Creator        bond + LP fees from Vault
```

### Flow 2: MultiOutcome Market

```
"Which chain will have highest TVL?"  outcomes = [ETH, SOL, ARB, BASE]

Step  Instruction        Who            What Happens
────  ─────────────────  ─────────────  ─────────────────────────────────────────
 1    create_market       Creator        Market(Multi, outcomes=[ETH,SOL,ARB,BASE]) + Vault
 2    place_bet           Alice          MultiOutcome { outcome_id: 0, amount: 50 } → encrypted
 3    place_bet           Bob            MultiOutcome { outcome_id: 1, amount: 30 } → encrypted
 4    place_bet           Dan            MultiOutcome { outcome_id: 2, amount: 40 } → encrypted
 5    lock_market         Crank          status = Locked
 6    settle_market       Oracle         MultiOutcome { winning_outcome_id: 1 }
                                         → Arcium decrypts → pools[0..3] revealed → Settled
 7    claim               Bob            Winner collects share of all losing pools
 8    withdraw_creator    Creator        bond + LP fees
```

### Flow 3: Accuracy Market (standalone)

```
"What will BTC be at midnight?"  entry_fee = $10

Step  Instruction        Who            What Happens
────  ─────────────────  ─────────────  ─────────────────────────────────────────
 1    create_market       Creator        Market(Accuracy, entry_fee=$10) + Vault
 2    place_bet           P1             Accuracy { estimate: 99950 } → encrypted, $10 → Vault
 3    place_bet           P2             Accuracy { estimate: 99800 } → encrypted, $10 → Vault
      ...6 players...
 4    lock_market         Crank          status = Locked
 5    settle_market       Crank          Accuracy { step: SetOutcome { value: 100000 } }
 6    settle_market       Crank          Accuracy { step: ComputeErrors } → decrypt, calc errors
 7    settle_market       Crank          Accuracy { step: ComputeErrors } → more bets (cranked)
 8    settle_market       Crank          Accuracy { step: ComputeWeights } → weights, winners
 9    settle_market       Crank          Accuracy { step: Finalize } → prize_pool set, Settled
10    claim               P1             Weighted payout
11    claim               P2             Weighted payout
12    withdraw_creator    Creator        bond returned
```

### Flow 4: Tiered Accuracy Lobby

```
"BTC price at midnight" — 4 tiers: Bronze($1), Silver($10), Gold($100), Diamond($1k)

Step  Instruction        Who            What Happens
────  ─────────────────  ─────────────  ─────────────────────────────────────────
 1    create_market       Creator        Accuracy + tiers → MarketGroup + 4 Markets + 4 Vaults
 2    place_bet           PlayerA        Joins Bronze (Market #5) → encrypted, $1 → Vault
 3    place_bet           PlayerB        Joins Gold (Market #7) → encrypted, $100 → Vault
      ...players join whichever tier...
 4    lock_market ×4      Crank          Lock each tier's Market
 5    settle_market       Oracle         Sets outcome_value on MarketGroup (shared)
 6    settle_market ×N    Crank          Crank each tier through SetOutcome → ComputeErrors →
                                         ComputeWeights → Finalize
                                         Each tier has own median, own winners, own payouts
 7    claim               Winners        Each winner claims from their tier's Vault
 8    withdraw_creator    Creator        bond returned from each tier
```

### Flow 5: Void Market (emergency)

```
Step  Instruction        Who            What Happens
────  ─────────────────  ─────────────  ─────────────────────────────────────────
 1    void_market         Authority      status = Voided
 2    claim               Each bettor    Full deposit refunded from Vault
 3    withdraw_creator    Creator        bond returned
```

---

## Instruction → Account Matrix

| Instruction | Protocol | Market | Vault | Bet | MarketGroup | Arcium |
|------------|----------|--------|-------|-----|-------------|--------|
| `initialize_protocol` | **init** | | | | | |
| `create_market` | **mut** | **init** (×N if tiered) | **init** (×N) | | **init** (if tiered) | |
| `place_bet` | | **mut** | **mut** | **init** | | **CPI** |
| `lock_market` | | **mut** | | | | |
| `settle_market` | | **mut** | | **mut** (accuracy) | **mut** (if tiered) | **CPI** |
| `claim` | | read | **mut** | **mut** | | |
| `withdraw_creator` | | read | **mut** | | | |
| `void_market` | read | **mut** | | | | |

---

## Error Codes — 15

| Code | Name | When |
|------|------|------|
| 6000 | `MarketNotOpen` | Bet placed on non-open market |
| 6001 | `MarketNotLocked` | Settlement called before lock |
| 6002 | `MarketNotSettled` | Claim called before settlement |
| 6003 | `AlreadyClaimed` | Bet.claimed == true |
| 6004 | `InvalidOutcome` | outcome_id >= outcome_count |
| 6005 | `DeadlineNotReached` | Lock called before resolution_deadline |
| 6006 | `Unauthorized` | Caller not authority/creator |
| 6007 | `InvalidTierCount` | Tiers < 1 or > 4 |
| 6008 | `OutcomeTooMany` | > 10 outcomes for MultiOutcome |
| 6009 | `OutcomeTooFew` | < 2 outcomes for MultiOutcome |
| 6010 | `ZeroAmount` | Bet amount is 0 |
| 6011 | `NotAWinner` | Loser tries to claim |
| 6012 | `GroupNotSettled` | Tier settle before group outcome set |
| 6013 | `DecryptionFailed` | Arcium MPC returned invalid result |
| 6014 | `QuestionTooLong` | Question > 200 bytes |
| 6015 | `BetTypeMismatch` | PlaceBetParams variant doesn't match market type |

---

## File Structure

```
programs/cypher/src/
├── lib.rs                         // entrypoint, 8 instructions dispatched here
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
│   ├── create_market.rs           // handles YesNo, Multi, Accuracy, Tiered — branches inside
│   ├── place_bet.rs               // handles all bet types — reads market_type, branches
│   ├── lock_market.rs
│   ├── settle_market.rs           // handles all settlement — YesNo/Multi (1 tx), Accuracy (cranked)
│   ├── claim.rs                   // handles payout + refund (voided)
│   ├── withdraw_creator.rs
│   └── void_market.rs
├── errors.rs                      // CypherError enum (16 codes)
├── events.rs                      // 7 events
└── arcium/
    ├── mod.rs
    ├── encrypt.rs                 // CPI to Arcium for bet encryption
    ├── decrypt.rs                 // CPI to Arcium for settlement decryption
    └── types.rs                   // Arcium-specific account types
```

---

## Summary

| What | Count |
|------|-------|
| Account types | 5 |
| Instructions | 8 |
| Events | 7 |
| Error codes | 16 |
| Market types | 3 (YesNo, MultiOutcome, Accuracy) |
| Categories | 7 |
| Max outcomes (Multi) | 10 |
| Max tiers (Accuracy) | 4 |
| Accuracy settlement steps | 4 (SetOutcome, ComputeErrors, ComputeWeights, Finalize) |
