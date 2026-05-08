# Cypher Prediction Market — State Design

## What Cypher Does

One Solana program, two market types:

| | **YesNo** (from `flew.rs`) | **Accuracy** (from `trepa.rs`) |
|---|---|---|
| **User action** | Pick Yes or No, bet any amount | Submit a numeric estimate, pay fixed entry fee |
| **Fee split** | 1.5% LP + 0.5% protocol deducted from each bet | Protocol % taken from loser pool after settlement |
| **Winner rule** | The side that matches the real outcome | Players whose error < median error |
| **Payout math** | share = net_bet / winning_pool × losing_pool | weight = (1/(1+r))^6, share = weight/total_weight × prize_pool |

---

## Account Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        SOLANA PROGRAM                           │
│                     (Cypher Program ID)                         │
└───────────────────────────┬─────────────────────────────────────┘
                            │ owns all accounts below
                            │
        ┌───────────────────┴───────────────────--┐
        │          PROTOCOL (singleton)           │
        │  PDA: ["protocol"]                      │
        │                                         │
        │  authority     ── admin wallet          │
        │  treasury      ── fee collection wallet │
        │  default_fee   ── basis points          │
        │  market_count  ── auto-increment        │
        └───────────────────┬────────────────────-┘
                            │ market_count → market_index
                            │
          ┌─────────────────┴─────────────────────-┐
          │             MARKET #0                  │
          │  PDA: ["market", 0u64.to_le_bytes()]   │
          │                                        │
          │  creator, question, type, status       │
          │  fees, deadline, total_bets            │
          │  vault ──────┐                         │
          │  market_data │ (YesNo OR Accuracy)     │
          └──────┬───────┼────────────────────────-┘
                 │       │
                 │       ▼
                 │  ┌────────────────────────-─┐
                 │  │  VAULT (SPL Token Acct)  │
                 │  │  PDA: ["vault", market]  │
                 │  │                          │
                 │  │  Holds ALL deposited     │
                 │  │  tokens for this market  │
                 │  └────────────────────────-─┘
                 │
     ┌───────────┼───────────┐
     ▼           ▼           ▼
┌─────────┐ ┌─────────┐ ┌─────────┐
│  BET #0 │ │  BET #1 │ │  BET #2 │  ...
│ ["bet", │ │ ["bet", │ │ ["bet", │
│  mkt,0] │ │  mkt,1] │ │  mkt,2] │
│         │ │         │ │         │
│ bettor  │ │ bettor  │ │ bettor  │
│ bet_data│ │ bet_data│ │ bet_data│
│ (YesNo  │ │ (YesNo  │ │(Accur-  │
│  or Acc)│ │  or Acc)│ │  acy)   │
│ claimed │ │ claimed │ │ claimed │
└─────────┘ └─────────┘ └─────────┘
```

**4 account types. That's it.**

| Account | Count | PDA Seeds | Approx Size | Rent |
|---------|-------|-----------|-------------|------|
| Protocol | 1 total | `["protocol"]` | 83 bytes | ~0.001 SOL |
| Market | 1 per market | `["market", index]` | 383–428 bytes | ~0.003 SOL |
| Vault | 1 per market | `["vault", market_key]` | 165 bytes (SPL) | ~0.002 SOL |
| Bet | 1 per bet | `["bet", market_key, bet_index]` | 128–148 bytes | ~0.001 SOL |

---

## Who Owns What

```
Creator's wallet ──pays bond──▶ Market account (stores bond amount)
                               └──▶ Vault (bond tokens land here)

Bettor's wallet ──pays bet───▶ Bet account (created, stores details)
                               └──▶ Vault (tokens land here)

Program (PDA) ──owns──▶ Vault (only program can sign transfers out)
```

- **Program** owns the Vault. Only the program can move tokens out.
- **Creator** is recorded in Market. Gets bond back + LP fees on settlement.
- **Bettor** is recorded in Bet. Claims payout after settlement.
- **Authority** (in Protocol) can update fees, void markets, set treasury.
- **Treasury** receives protocol fees when winners claim.

---

## Full Lifecycle Flow

### 1. Initialize Protocol (once)

```
Admin calls: initialize_protocol(authority, treasury, default_fee_bps)

Creates:
  Protocol PDA ← authority, treasury, fee, market_count=0
```

### 2. Create Market

```
Creator calls: create_market(question, market_type, bond, lp_fee_bps, deadline)

Reads:  Protocol.market_count → index
Writes: Protocol.market_count += 1

Creates:
  Market PDA ← all fields, status=Open, market_data based on type
  Vault PDA  ← SPL token account, authority = program PDA

Transfers:
  creator_bond tokens → Vault
```

### 3. Place Bet

#### YesNo

```
Bettor calls: place_bet_yesno(side, amount)

Computes:
  lp_fee       = amount × lp_fee_bps / 10000
  protocol_fee = amount × protocol_fee_bps / 10000
  net_amount   = amount - lp_fee - protocol_fee

Creates:
  Bet PDA ← bettor, side, total_amount, net_amount, fees

Updates Market:
  market_data.yes_pool += net  (or no_pool)
  market_data.lp_fee_pool += lp_fee
  market_data.protocol_fee_pool += protocol_fee
  total_bets += 1

Transfers:
  amount tokens → Vault
```

#### Accuracy

```
Bettor calls: place_bet_accuracy(estimate)

Creates:
  Bet PDA ← bettor, estimate, entry_fee (read from market)

Updates Market:
  market_data.total_pool += entry_fee
  total_bets += 1

Transfers:
  entry_fee tokens → Vault
```

### 4. Lock Market

```
Creator (or clock) calls: lock_market()

Requires: current_time >= resolution_deadline
Updates:  Market.status = Locked

No more bets accepted after this.
```

### 5. Settle Market

#### YesNo — single transaction

```
Oracle/Creator calls: settle_yesno(winning_side)

Updates Market:
  market_data.winning_side = Some(winning_side)
  status = Settled
```

#### Accuracy — multi-step (crank)

Accuracy settlement is computationally heavy. It happens in phases:

```
Step A: set_outcome(outcome_value)
  → Market.market_data.outcome_value = Some(value)

Step B: compute_errors()  (cranked, processes N bets per tx)
  → For each Bet: error = |estimate - outcome|
  → Sorts and finds median_error after all processed
  → Market.market_data.median_error = Some(median)

Step C: compute_weights()  (cranked, processes N bets per tx)
  → For each Bet with error < median:
      relative_error = error / median_error
      weight = (1 / (1 + relative_error))^6   (integer math)
      bet.won = Some(true)
  → For each Bet with error >= median:
      bet.won = Some(false)
  → Market.market_data.winner_count, loser_count set
  → Market.market_data.total_weight = sum of all weights

Step D: finalize_settlement()
  → loser_pool = loser_count × entry_fee
  → protocol_take = loser_pool × protocol_fee_bps / 10000
  → prize_pool = loser_pool - protocol_take
  → Market.status = Settled
```

### 6. Claim Payout

```
Winner calls: claim()

Reads: Bet (must have claimed=false, market must be Settled)

YesNo payout:
  share = net_amount / winning_pool
  winnings = share × losing_pool
  total_return = net_amount + winnings

Accuracy payout:
  share = weight / total_weight
  winnings = share × prize_pool
  total_return = entry_fee + winnings

Transfers from Vault → Winner's wallet:  total_return
Transfers from Vault → Treasury:         proportional protocol fee

Updates: Bet.claimed = true
```

### 7. Creator Withdrawal

```
Creator calls: withdraw_creator()

Requires: Market.status == Settled

YesNo:
  Transfers from Vault → Creator: creator_bond + lp_fee_pool

Accuracy:
  Transfers from Vault → Creator: creator_bond
  (no LP fees in accuracy markets — protocol takes from loser pool)
```

### 8. Void Market (emergency)

```
Authority calls: void_market()

Updates: Market.status = Voided

Anyone calls: claim_refund()
  → Returns full deposit to each bettor
  → Returns bond to creator
```

---

## Why This Design

### Why separate Bet accounts instead of a Vec inside Market?

Solana accounts have a 10MB limit, but more importantly you pay rent per byte. A Vec of bets inside Market means:
- Market account grows unboundedly
- You need `realloc` on every bet (expensive)
- One whale market with 10k bets makes the account huge

Separate Bet PDAs:
- Market stays fixed-size (~400 bytes)
- Each bettor pays rent for their own Bet (~0.001 SOL, reclaimable)
- Bets can be fetched in parallel by the frontend
- No realloc needed

### Why a Vault PDA instead of holding SOL in Market?

- SPL token support (USDC, etc.) — not just SOL
- Clean separation: state is in Market, money is in Vault
- Standard pattern — wallets and explorers understand it

### Why `market_index` instead of random keys?

- Deterministic PDAs: anyone can derive `["market", 42]` without an on-chain lookup
- Sequential: the frontend can paginate all markets
- Protocol.market_count guarantees uniqueness

### Why enums for MarketData/BetData instead of separate account types?

- One program, one Market instruction set — simpler on the frontend
- Borsh enum serialization is efficient (1-byte tag + fields)
- Shared fields (creator, status, deadline) aren't duplicated
- Adding a third market type later = add a new enum variant, not new account types

### Why basis points (bps) instead of percentages?

- Integer math only — no floats on Solana
- 1 bps = 0.01%, gives fine-grained control
- 150 bps = 1.50% (LP fee), 50 bps = 0.50% (protocol fee)
- Standard in DeFi

---

## Integer Math for On-Chain Calculations

Solana has no floats. All pool math uses `u64` with `u128` intermediaries to avoid overflow:

```
// YesNo payout (safe integer math)
let share_numerator: u128 = (net_amount as u128) * (losing_pool as u128);
let payout: u64 = (share_numerator / winning_pool as u128) as u64;

// Accuracy weight — can't do (1/(1+r))^6 directly
// Instead, use fixed-point with SCALE = 1_000_000_000 (10^9)
//
// relative_error = (error * SCALE) / median_error
// base = SCALE * SCALE / (SCALE + relative_error)   // = 1/(1+r) scaled
// weight = base^6 / SCALE^5                          // normalize back
```

---

## File Layout in `src/`

```
src/
├── state.rs       ← account structs, enums, sizes (YOU ARE HERE)
├── flew.rs        ← YesNo math simulation (existing)
├── trepa.rs       ← Accuracy math simulation (existing)
├── main.rs        ← test harness
└── (future files as you build instructions)
    ├── instructions/
    │   ├── mod.rs
    │   ├── initialize_protocol.rs
    │   ├── create_market.rs
    │   ├── place_bet_yesno.rs
    │   ├── place_bet_accuracy.rs
    │   ├── settle_yesno.rs
    │   ├── settle_accuracy.rs
    │   ├── claim.rs
    │   └── void.rs
    ├── errors.rs
    └── lib.rs
```

---

## Token Flow Diagram

```
                    DEPOSIT FLOW
                    ════════════

  Creator                          Bettors
     │                                │
     │  bond (e.g. 10 USDC)          │  bet amount / entry fee
     │                                │
     └──────────────┬─────────────────┘
                    │
                    ▼
              ┌───────────┐
              │   VAULT   │    (all tokens pooled here)
              │  PDA acct │
              └─────┬─────┘
                    │
                    │  on settlement + claim
                    │
        ┌───────────┼────────────┬──────────────┐
        ▼           ▼            ▼              ▼
    Winners     Creator      Treasury      Losers
    (pool       (bond +      (protocol     (get
     share)      LP fees)     fees)        nothing)
```

---

## Scaling Notes

- **YesNo settlement**: O(1) — just set the winning side. Cheap.
- **Accuracy settlement**: O(N) — must process every bet to find median and compute weights. For markets with >50 players, use a **crank** pattern: off-chain bot calls `compute_errors` repeatedly, processing ~10 bets per transaction, until all are done.
- **Max bets per market**: No hard limit since bets are separate accounts. Practical limit depends on how the frontend fetches them (getProgramAccounts with filters).
- **Concurrent markets**: Unlimited. Each market is independent.
