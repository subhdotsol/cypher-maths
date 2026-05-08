# Multi-Outcome Parimutuel Market

## The Market

**Question:** "Which chain will have the highest TVL by end of 2026?"

**Outcomes (4):**
- Ethereum
- Solana
- Arbitrum
- Base

**Fees:** 1.5% LP, 0.5% protocol (same as yes/no)

---

## 5 Traders Place Bets

| # | Trader | Picks | Amount | LP Fee (1.5%) | Protocol Fee (0.5%) | Net Amount |
|---|--------|-------|--------|---------------|---------------------|------------|
| 1 | Alice | Ethereum | 50.0 | 0.750 | 0.250 | 49.000 |
| 2 | Bob | Solana | 30.0 | 0.450 | 0.150 | 29.400 |
| 3 | Carol | Solana | 20.0 | 0.300 | 0.100 | 19.600 |
| 4 | Dan | Arbitrum | 40.0 | 0.600 | 0.200 | 39.200 |
| 5 | Eve | Base | 10.0 | 0.150 | 0.050 | 9.800 |

---

## Pool State After All Bets

```
Ethereum pool:   49.000  (Alice)
Solana pool:     49.000  (Bob 29.400 + Carol 19.600)
Arbitrum pool:   39.200  (Dan)
Base pool:        9.800  (Eve)
                 ──────
Total pool:     147.000

LP fee pool:      2.250
Protocol fee:     0.750
```

---

## Implied Prices (what the UI would show)

```
price = pool / total_pool

Ethereum:  49.000 / 147.000 = $0.3333  (33.33%)
Solana:    49.000 / 147.000 = $0.3333  (33.33%)
Arbitrum:  39.200 / 147.000 = $0.2667  (26.67%)
Base:       9.800 / 147.000 = $0.0667  ( 6.67%)
                               ──────
                        Total: $1.0000  (100.00%)
```

---

## Settlement: Solana Wins

```
winning_pool = solana_pool = 49.000
losing_pool  = total_pool - winning_pool
             = 147.000 - 49.000
             = 98.000
```

---

## Winner Payouts

### Bob (bet 30.0, net 29.400 on Solana)

```
share    = net_amount / winning_pool
         = 29.400 / 49.000
         = 0.6000  (60.00% of winning pool)

winnings = share × losing_pool
         = 0.6000 × 98.000
         = 58.800

total_return = net_amount + winnings
             = 29.400 + 58.800
             = 88.200

profit = total_return - original_bet
       = 88.200 - 30.000
       = +58.200
```

### Carol (bet 20.0, net 19.600 on Solana)

```
share    = 19.600 / 49.000
         = 0.4000  (40.00% of winning pool)

winnings = 0.4000 × 98.000
         = 39.200

total_return = 19.600 + 39.200
             = 58.800

profit = 58.800 - 20.000
       = +38.800
```

**Sanity check:** Bob + Carol total return = 88.200 + 58.800 = 147.000 = total_pool. All money accounted for.

---

## Loser Losses

### Alice (bet 50.0 on Ethereum)

```
Lost: 50.000 (entire bet)
  - 49.000 went to pool (taken by winners)
  -  0.750 went to LP fees (goes to creator)
  -  0.250 went to protocol fees (goes to treasury)
```

### Dan (bet 40.0 on Arbitrum)

```
Lost: 40.000 (entire bet)
  - 39.200 went to pool (taken by winners)
  -  0.600 went to LP fees
  -  0.200 went to protocol fees
```

### Eve (bet 10.0 on Base)

```
Lost: 10.000 (entire bet)
  -  9.800 went to pool (taken by winners)
  -  0.150 went to LP fees
  -  0.050 went to protocol fees
```

---

## Creator + Protocol Profit

```
Creator (LP fees):     2.250
Protocol (fees):       0.750
```

---

## Full Money Flow

```
MONEY IN                          MONEY OUT
────────                          ─────────
Alice:   50.000                   Bob (winner):      88.200
Bob:     30.000                   Carol (winner):    58.800
Carol:   20.000                   Creator (LP):       2.250
Dan:     40.000                   Protocol (fees):    0.750
Eve:     10.000
         ──────                                     ──────
Total:  150.000                   Total:            150.000  ✓
```

---

## What If Nobody Bet on the Winner?

Same scenario but the outcome is **Base** and only Eve bet on it:

```
winning_pool = 9.800  (just Eve)
losing_pool  = 147.000 - 9.800 = 137.200

Eve's payout = (9.800 / 9.800) × 137.200 = 137.200
Eve's profit = 137.200 - 10.000 = +127.200   (12.7x return!)
```

Long-shot outcomes = massive payouts. That's the nature of multi-outcome.

But what if the outcome is something like a 5th option "Other" that nobody bet on at all?

```
winning_pool = 0.000

Division by zero. No winners exist.
```

**Design decision needed:**
- Option A: Refund all bettors their net amounts, creator keeps LP fees, protocol keeps its fees
- Option B: Void the entire market, refund everything including fees
- Option C: Protocol takes the unclaimed pool

---

## Comparison: Yes/No vs Multi-Outcome

```
                        YES/NO              MULTI-OUTCOME
                        ──────              ─────────────
Pools                   2 (yes, no)         N (one per outcome)
Max outcomes            2                   unlimited
Payout formula          IDENTICAL           IDENTICAL
Losing pool             the other pool      sum of ALL other pools
Max return (balanced)   ~2x                 ~Nx (scales with outcomes)
Implied prices          sum to $1.00        sum to $1.00
Zero-winner edge case   impossible*         possible

* someone always picks yes or no eventually
```

The formula never changes:

```
payout = (your_net_bet / winning_pool) × losing_pool
```

Yes/No is just multi-outcome where N=2.
