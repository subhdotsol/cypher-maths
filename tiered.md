# Tiered Lobbies — Math Walkthrough

## The Idea

One market question. Multiple tables. Same formula. Different stakes.

**Question:** "What will BTC price be at midnight UTC on June 1st?"
**Real outcome:** $100,000

Three tiers run simultaneously:

| Tier | Entry Fee | Players |
|------|-----------|---------|
| Bronze | $1 | 6 players |
| Silver | $10 | 6 players (same people, different lobby) |
| Gold | $100 | 6 players |

Every tier uses the exact same `trepa.rs` formula. The ONLY thing that changes is `entry_fee`. Let's walk through all three.

---

## The Players (same in all tiers)

All 6 players submit the same estimates regardless of tier:

| Player | Estimate | Error = \|estimate - 100,000\| |
|--------|----------|-------------------------------|
| P1 | 99,950 | 50 |
| P2 | 99,800 | 200 |
| P3 | 99,500 | 500 |
| P4 | 99,000 | 1,000 |
| P5 | 98,000 | 2,000 |
| P6 | 95,000 | 5,000 |

**Sorted by error:** P1(50) < P2(200) < P3(500) < P4(1000) < P5(2000) < P6(5000)

---

## Step 1 — Median Error (same across all tiers)

```
N = 6 players
k = floor((6 + 1) / 2) = 3
median_error = error of player at index k-1 = error[2] = 500 (P3)
```

Median error = **500** (same for every tier — it depends on estimates, not money)

---

## Step 2 — Winners vs Losers (same across all tiers)

```
Winner if: error < median_error (500)

P1: 50  < 500  → WINNER ✓
P2: 200 < 500  → WINNER ✓
P3: 500 < 500  → NO (equal, not less) → LOSER ✗
P4: 1000       → LOSER ✗
P5: 2000       → LOSER ✗
P6: 5000       → LOSER ✗

Winners: 2 (P1, P2)
Losers:  4 (P3, P4, P5, P6)
```

Same winners and losers in every tier. Skill determines who wins, not money.

---

## Step 3 — Winner Weights (same across all tiers)

```
relative_error = error / median_error
weight = (1 / (1 + relative_error))^6

P1: r = 50/500 = 0.10     weight = (1/1.10)^6 = 0.564474
P2: r = 200/500 = 0.40    weight = (1/1.40)^6 = 0.133420

total_weight = 0.564474 + 0.133420 = 0.697894

P1 share = 0.564474 / 0.697894 = 80.88%
P2 share = 0.133420 / 0.697894 = 19.12%
```

These percentages are identical across all tiers. The formula doesn't know what entry_fee is.

---

## Step 4 — Pool Math (THIS is where tiers differ)

Protocol fee = 20% of loser pool (same percentage, different absolute amounts)

### BRONZE TIER ($1 entry)

```
total_pool   = 6 × $1 = $6.00
loser_pool   = 4 × $1 = $4.00
protocol_take = $4.00 × 0.20 = $0.80
prize_pool   = $4.00 - $0.80 = $3.20
```

### SILVER TIER ($10 entry)

```
total_pool   = 6 × $10 = $60.00
loser_pool   = 4 × $10 = $40.00
protocol_take = $40.00 × 0.20 = $8.00
prize_pool   = $40.00 - $8.00 = $32.00
```

### GOLD TIER ($100 entry)

```
total_pool   = 6 × $100 = $600.00
loser_pool   = 4 × $100 = $400.00
protocol_take = $400.00 × 0.20 = $80.00
prize_pool   = $400.00 - $80.00 = $320.00
```

---

## Step 5 — Payouts Per Tier

The share % is the same. The dollar amounts scale linearly with entry fee.

### P1 (best estimate — 80.88% share)

| | Bronze ($1) | Silver ($10) | Gold ($100) |
|---|---|---|---|
| Entry fee | $1.00 | $10.00 | $100.00 |
| Profit share | $3.20 × 80.88% = $2.59 | $32.00 × 80.88% = $25.88 | $320.00 × 80.88% = $258.82 |
| Final payout | $1.00 + $2.59 = **$3.59** | $10.00 + $25.88 = **$35.88** | $100.00 + $258.82 = **$358.82** |
| Profit | **+$2.59** | **+$25.88** | **+$258.82** |
| ROI | +259% | +259% | +259% |

### P2 (good estimate — 19.12% share)

| | Bronze ($1) | Silver ($10) | Gold ($100) |
|---|---|---|---|
| Entry fee | $1.00 | $10.00 | $100.00 |
| Profit share | $3.20 × 19.12% = $0.61 | $32.00 × 19.12% = $6.12 | $320.00 × 19.12% = $61.18 |
| Final payout | $1.00 + $0.61 = **$1.61** | $10.00 + $6.12 = **$16.12** | $100.00 + $61.18 = **$161.18** |
| Profit | **+$0.61** | **+$6.12** | **+$61.18** |
| ROI | +61% | +61% | +61% |

### Losers (P3, P4, P5, P6)

| | Bronze ($1) | Silver ($10) | Gold ($100) |
|---|---|---|---|
| Lost | **$1.00** | **$10.00** | **$100.00** |

### Protocol

| | Bronze ($1) | Silver ($10) | Gold ($100) |
|---|---|---|---|
| Protocol take | **$0.80** | **$8.00** | **$80.00** |

---

## The Key Insight

**ROI is identical across all tiers.** P1 makes +259% whether they're at the $1 table or the $100 table. The formula doesn't change. The skill ranking doesn't change. Only the dollar amounts scale.

This is what makes tiered lobbies clean:

```
Bronze P1 profit:  $2.59    (same skill, small stakes)
Gold P1 profit:    $258.82  (same skill, 100x stakes → 100x profit)
```

A whale who wants to deploy $100 plays Gold. A casual player tests strategies at Bronze. Neither affects the other's game.

---

## Money Flow Per Tier

```
BRONZE ($1 × 6 = $6.00 in)         GOLD ($100 × 6 = $600.00 in)
─────────────────────────           ──────────────────────────────
P1 gets:    $3.59                   P1 gets:    $358.82
P2 gets:    $1.61                   P2 gets:    $161.18
P3 gets:    $0.00                   P3 gets:    $0.00
P4 gets:    $0.00                   P4 gets:    $0.00
P5 gets:    $0.00                   P5 gets:    $0.00
P6 gets:    $0.00                   P6 gets:    $0.00
Protocol:   $0.80                   Protocol:   $80.00
            ─────                               ──────
Total out:  $6.00 ✓                 Total out:  $600.00 ✓
```

---

## What Changes in Code

### trepa.rs — Nothing in the formula

`run_round(players, outcome, entry_fee, protocol_fee_percent)` already takes `entry_fee` as a parameter. Calling it with `1.0`, `10.0`, or `100.0` gives the tiered results. The function doesn't need modification — just call it multiple times.

What gets added: a `run_tiered` function that takes a list of tiers and runs the same players + outcome through each tier, printing side-by-side results.

### flew.rs — Not applicable

Tiered lobbies are for accuracy markets only. YesNo and MultiOutcome markets already allow variable bet sizes — there's no fixed entry fee to tier around.

---

## Edge Case: Different Players Per Tier

In reality, tiers won't have the same players. Bronze might have 50 players, Gold might have 5. That's fine:

- Each tier computes its own median_error from its own player pool
- Winners/losers are determined independently per tier
- A bad estimate might win in a weak Bronze lobby but lose in a sharp Gold lobby
- This is by design — you're competing against your tier, not the whole market

---

## Why Not Just One Big Pool With Variable Entry Fees?

Already answered in the accuracy discussion, but to recap:

```
If P1 pays $100 and P2 pays $1, and both win:
  - P1's payout should be proportional to $100
  - But the accuracy formula weights by SKILL (error), not MONEY
  - P2 with a better estimate would get a higher weight
  - P1 paid 100x more but gets less? That breaks expectations.

Tiered lobbies avoid this entirely — everyone at the same table
pays the same fee, so the formula works exactly as designed.
```
