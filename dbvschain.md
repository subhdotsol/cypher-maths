# Database vs Contract — Who Does What

## The Rule

**Does this action move tokens?**

- **Yes** → contract handles it (place bet, claim, settle, void)
- **No** → database handles it (list, search, filter, leaderboard, profile, history)

The frontend **reads from the database** (fast) but **writes through the contract** (secure). The indexer keeps them in sync.

---

## What the Contract Handles

The contract is the **only thing that touches money**. Every action that involves tokens goes through it:

```
CREATE MARKET
  → creator pays bond → stored on-chain in Vault
  → market account created with question, type, category, fees, deadline

PLACE BET
  → user pays tokens → go into Vault
  → bet account created on-chain
  → pool amounts updated on market account

SETTLE
  → oracle calls the contract with the real outcome
  → contract records the winning side/outcome/median on-chain
  → no money moves yet

CLAIM
  → winner calls the contract
  → contract calculates payout using on-chain pool data
  → contract transfers tokens from Vault → winner's wallet
  → contract transfers protocol fee from Vault → treasury

VOID
  → contract refunds everyone from Vault
```

That's it. The contract does **5 things**. It doesn't know what a "leaderboard" is. It can't search "all crypto markets". It can't sort markets by popularity. It just holds money and enforces math.

---

## What the Database Handles

The database holds **zero tokens**. It's a fast copy of what happened on-chain, plus extra data the contract doesn't care about:

```
MARKET LIST
  "Show me all open crypto markets"
  → DB query: SELECT * FROM markets WHERE category = 'crypto' AND status = 'open'
  → Contract CAN do this (getProgramAccounts + filter) but it's slow

LIVE PRICES
  "What's the current yes/no price?"
  → DB already has yes_pool and no_pool (copied from chain)
  → price = yes_pool / (yes_pool + no_pool)
  → Instant lookup, no RPC call

USER PROFILE
  "Show me bob's bet history, win rate, total profit"
  → DB query: SELECT * FROM bets WHERE wallet = 'bob'
  → Aggregate wins, losses, profit
  → Contract has this data but spread across hundreds of Bet PDAs — too slow to fetch

LEADERBOARD
  "Top 10 players by profit this week"
  → DB query: SELECT wallet, SUM(profit) FROM bets GROUP BY wallet ORDER BY profit DESC LIMIT 10
  → Contract literally cannot do this — no aggregation on-chain

SEARCH
  "Markets with 'BTC' in the question"
  → DB query: SELECT * FROM markets WHERE question ILIKE '%BTC%'
  → Contract can't do text search

MARKET DETAIL PAGE
  "Show market #42 with all bets, pool chart, bet history"
  → One DB query returns everything
  → On-chain you'd need: 1 RPC for market + N RPCs for each bet = slow
```

---

## How the Database Stays in Sync — The Indexer

```
Solana Program                    Indexer                     Database
      │                              │                           │
      │  emit: "market_created"      │                           │
      │─────────────────────────────►│                           │
      │                              │  INSERT INTO markets ...  │
      │                              │──────────────────────────►│
      │                              │                           │
      │  emit: "bet_placed"          │                           │
      │─────────────────────────────►│                           │
      │                              │  INSERT INTO bets ...     │
      │                              │  UPDATE markets SET       │
      │                              │    yes_pool = ...         │
      │                              │──────────────────────────►│
      │                              │                           │
      │  emit: "market_settled"      │                           │
      │─────────────────────────────►│                           │
      │                              │  UPDATE markets SET       │
      │                              │    status = 'settled',    │
      │                              │    winning_side = 'yes'   │
      │                              │──────────────────────────►│
```

The indexer **listens** to every transaction on the program. Every time something happens on-chain, it writes the same data to Postgres. Now the API can query the DB instead of the chain.

---

## Architecture Overview

```
 User's Browser / App
        │
        ▼
  ┌───────────┐     read/write     ┌──────────────────┐
  │ API Server │◄──────────────────►│   Database (PG)  │
  └─────┬─────┘                    └────────▲─────────-┘
        │                                   │
        │ place_bet, claim                  │ index events
        │ (user signs tx)                   │
        ▼                                   │
  ┌───────────┐                    ┌────────┴─────────┐
  │  Solana    │───program logs───►│    Indexer        │
  │  Program   │                   │ (listens to txs)  │
  └─────▲─────┘                    └──────────────────-┘
        │
        │ settle(outcome_value)
        │
  ┌─────┴──────┐
  │   Oracle    │   fetches real-world data
  │   Service   │   (price feeds, sports APIs, etc.)
  └────────────-┘
```

| Service | What it does | Reads chain? | Writes chain? |
|---|---|---|---|
| **API Server** | Serves market lists, filters by category, user bets, history | No — reads from DB | No |
| **Indexer** | Listens to Solana events/txs, writes to DB | Yes — subscribes to program logs | No |
| **Oracle Service** | Posts the real-world outcome value on-chain | No | Yes — calls `settle` instructions |
| **DB (Postgres)** | Fast queries, search, pagination, leaderboard | — | — |
