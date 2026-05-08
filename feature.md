# Cypher — Feature Upgrades

## 1. Tiered Lobbies for Accuracy Markets

**Problem:** Accuracy markets use a fixed entry fee so payouts are purely skill-based. But a player with $200 has no way to deploy more capital — they're stuck at the same $1 table as everyone else.

**Solution:** Multiple lobbies per market, each with a different entry fee tier.

| Tier | Entry Fee | Target |
|------|-----------|--------|
| Bronze | $1 | Casual players, testing strategies |
| Silver | $10 | Mid-stakes, confident estimators |
| Gold | $100 | High conviction, experienced players |
| Diamond | $1,000 | Whales, pros |

**How it works:**
- Same market question, same outcome, same deadline
- Each tier runs as its own independent pool
- The accuracy formula stays identical — `weight = (1/(1+r))^6` — no changes
- Players in the $100 tier only compete against other $100 tier players
- A perfect estimate at the $1 table wins ~$1–3. The same estimate at the $100 table wins ~$100–300.

**Why this is clean:**
- Zero formula changes — just `entry_fee` varies per tier
- Skill still determines who wins, capital determines the stakes
- No whale-vs-casual imbalance within a tier
- On-chain: one Market account per tier, all sharing the same question and outcome. Settlement can reuse the same oracle value across tiers.

**On-chain impact:**
- Market account gets an optional `tier` field (or lobby_id)
- A parent "MarketGroup" could link tiers together so they settle from the same outcome
- Alternatively, keep it simple — each tier is just a separate market that happens to ask the same question. The frontend groups them visually.

## move price in v2 