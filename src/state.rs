// ═══════════════════════════════════════════════════════════════════════════
//  CYPHER PREDICTION MARKET — ON-CHAIN STATE
//
//  Supports three market types in one unified program:
//    1. YesNo        — binary outcome, variable bet sizes
//    2. MultiOutcome — N outcomes (up to 10), variable bet sizes, parimutuel
//    3. Accuracy     — numeric estimate, fixed entry fee, median-error split
//
//  Accuracy markets support Tiered Lobbies via MarketGroup:
//    Same question, same outcome, different entry fees (Bronze/Silver/Gold/Diamond).
//    Each tier is its own Market account. A MarketGroup links them so they
//    settle from the same oracle value in one step.
//
//  Account hierarchy:
//    Protocol (singleton)
//      ├── Market (PDA per market)
//      │     ├── Vault (SPL token account, PDA)
//      │     └── Bet[] (PDA per individual bet)
//      └── MarketGroup (PDA, optional — links tiered accuracy markets)
//            └── Market[] (one per tier, each with its own Vault + Bets)
// ═══════════════════════════════════════════════════════════════════════════

// In a real Solana program, these would be:
// use borsh::{BorshDeserialize, BorshSerialize};
// use solana_program::pubkey::Pubkey;
//
// For now we use placeholder types so the math repo compiles standalone.

/// Placeholder for solana_program::pubkey::Pubkey (32 bytes)
pub type Pubkey = [u8; 32];

/// Maximum number of outcomes in a multi-outcome market.
/// Fixed at 10 so the Market account stays a known size on-chain.
/// Outcome labels are stored as short strings (max 32 bytes each).
pub const MAX_OUTCOMES: usize = 10;

/// Max bytes per outcome label (e.g. "Ethereum", "Solana")
pub const MAX_OUTCOME_LABEL_LEN: usize = 32;

/// Maximum number of tiers in a tiered accuracy lobby.
/// Fixed at 4 so MarketGroup stays a known size (Bronze/Silver/Gold/Diamond).
pub const MAX_TIERS: usize = 4;

/// Max bytes per tier label (e.g. "Bronze", "Diamond")
pub const MAX_TIER_LABEL_LEN: usize = 16;

// ─────────────────────────────────────────────────────────────────────────
//  ENUMS
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum MarketType {
    YesNo,
    MultiOutcome,
    Accuracy,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MarketCategory {
    Crypto,
    Politics,
    Sports,
    Tech,
    Economy,
    Culture,
    /// Catch-all for anything that doesn't fit the other categories
    Beyond,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MarketStatus {
    /// Accepting bets
    Open,
    /// No more bets — waiting for outcome to be reported
    Locked,
    /// Outcome decided — winners can claim
    Settled,
    /// Cancelled — everyone gets a refund
    Voided,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Side {
    Yes,
    No,
}

// ─────────────────────────────────────────────────────────────────────────
//  PROTOCOL  — singleton, one per deployment
//
//  PDA seeds: ["protocol"]
//  Owner: program itself
//  Size: ~82 bytes
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Protocol {
    /// PDA bump seed
    pub bump: u8,
    /// Upgrade authority / admin — can update fees, transfer authority
    pub authority: Pubkey,
    /// Treasury wallet — receives protocol fees on claim
    pub treasury: Pubkey,
    /// Default protocol fee in basis points (e.g. 50 = 0.50%)
    pub default_protocol_fee_bps: u16,
    /// Auto-incrementing counter — next market gets this index
    pub market_count: u64,
}
// Size: 1 + 32 + 32 + 2 + 8 = 75 bytes  (+8 Anchor discriminator = 83)

// ─────────────────────────────────────────────────────────────────────────
//  MARKET — one account per prediction market
//
//  PDA seeds: ["market", market_index.to_le_bytes()]
//  Owner: program
//  Size: ~350 bytes (depends on question length, max 200 chars)
//
//  The `market_data` enum holds type-specific pool state.
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Market {
    /// PDA bump seed
    pub bump: u8,
    /// Wallet that created this market and posted the bond
    pub creator: Pubkey,
    /// Unique sequential index (from Protocol.market_count)
    pub market_index: u64,
    /// Which kind of market this is
    pub market_type: MarketType,
    /// Market category — used by the frontend to group/filter markets
    pub category: MarketCategory,
    /// Current lifecycle phase
    pub status: MarketStatus,
    /// Human-readable question (max 200 bytes on-chain)
    pub question: String,
    /// Creator's bond in token base units (lamports / smallest unit)
    /// Returned to creator on settlement + they receive LP fees
    pub creator_bond: u64,
    /// LP fee in basis points — goes to creator on settlement
    pub lp_fee_bps: u16,
    /// Protocol fee in basis points — goes to treasury on claim
    pub protocol_fee_bps: u16,
    /// Unix timestamp — after this, market can be settled or voided
    pub resolution_deadline: i64,
    /// Unix timestamp — when the market was created
    pub created_at: i64,
    /// How many Bet accounts exist for this market
    pub total_bets: u64,
    /// SPL token account PDA that holds all deposited funds
    pub vault: Pubkey,
    /// Token mint for this market (e.g. USDC mint)
    pub token_mint: Pubkey,
    /// If this market is part of a tiered lobby, points to the MarketGroup.
    /// None for standalone markets. Set on creation, immutable.
    pub market_group: Option<Pubkey>,
    /// Type-specific pool data
    pub market_data: MarketData,
}

// ─────────────────────────────────────────────────────────────────────────
//  MARKET GROUP — links tiered accuracy markets
//
//  PDA seeds: ["market_group", group_index.to_le_bytes()]
//  Owner: program
//
//  One MarketGroup per tiered lobby. Holds the shared question, outcome,
//  and pointers to each tier's Market account. Settlement sets the outcome
//  on the group, then each tier Market settles independently using its
//  own entry_fee and player pool.
//
//  Example: "What will BTC be at midnight?" with 4 tiers
//    MarketGroup → [Market(Bronze,$1), Market(Silver,$10),
//                   Market(Gold,$100), Market(Diamond,$1000)]
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct MarketGroup {
    /// PDA bump seed
    pub bump: u8,
    /// Who created this tiered lobby
    pub creator: Pubkey,
    /// Unique sequential index (could share Protocol.market_count or its own counter)
    pub group_index: u64,
    /// The shared question across all tiers (max 200 bytes)
    pub question: String,
    /// Shared resolution deadline — all tiers lock and settle together
    pub resolution_deadline: i64,
    /// When the group was created
    pub created_at: i64,
    /// How many tiers are active (1..=MAX_TIERS)
    pub tier_count: u8,
    /// Tier labels — fixed array, first `tier_count` are valid
    /// On-chain: [[u8; MAX_TIER_LABEL_LEN]; MAX_TIERS]
    pub tier_labels: Vec<String>,
    /// Entry fee per tier in token base units
    /// On-chain: [u64; MAX_TIERS]
    pub tier_entry_fees: Vec<u64>,
    /// Pubkey of each tier's Market account
    /// On-chain: [Pubkey; MAX_TIERS]
    pub tier_markets: Vec<Pubkey>,
    /// The real-world outcome value — set once, shared by all tiers
    /// Settlement flow: set this → each tier Market settles using this value
    pub outcome_value: Option<u64>,
    /// Decimal scaling for the outcome
    pub outcome_decimals: u8,
    /// Whether all tiers have been settled
    pub settled: bool,
}

// ─────────────────────────────────────────────────────────────────────────
//  MARKET DATA — enum that holds pool state specific to each market type
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum MarketData {
    /// Binary outcome market (from flew.rs logic)
    ///
    /// Users pick Yes or No with any amount.
    /// Fees split on deposit. Winning side splits the losing pool
    /// proportional to their net bet.
    YesNo {
        /// Sum of all net (post-fee) Yes bets
        yes_pool: u64,
        /// Sum of all net (post-fee) No bets
        no_pool: u64,
        /// Accumulated LP fees (go to creator)
        lp_fee_pool: u64,
        /// Accumulated protocol fees (go to treasury)
        protocol_fee_pool: u64,
        /// Set on settlement
        winning_side: Option<Side>,
    },

    /// Multi-outcome parimutuel market (from flew.rs MultiMarket logic)
    ///
    /// Creator defines N outcomes (2..=10). Users pick one outcome and bet
    /// any amount. Same fee split as YesNo. Winning outcome's bettors
    /// split ALL other pools proportional to their net bet.
    ///
    /// Payout = (net_bet / winning_pool) × sum_of_all_losing_pools
    ///
    /// On-chain we use fixed-size arrays sized to MAX_OUTCOMES (10).
    /// Unused slots are zeroed out. `outcome_count` says how many are active.
    MultiOutcome {
        /// How many outcomes are active (2..=MAX_OUTCOMES)
        outcome_count: u8,
        /// Outcome labels — fixed array, first `outcome_count` are valid
        /// Each label is a short string (max 32 bytes).
        /// On-chain stored as [[u8; MAX_OUTCOME_LABEL_LEN]; MAX_OUTCOMES]
        /// with a length prefix per label. Here we use String for readability.
        outcome_labels: Vec<String>,
        /// Net pool per outcome — pools[i] is the net amount for outcome i
        /// Fixed array on-chain: [u64; MAX_OUTCOMES]
        pools: Vec<u64>,
        /// Accumulated LP fees (go to creator)
        lp_fee_pool: u64,
        /// Accumulated protocol fees (go to treasury)
        protocol_fee_pool: u64,
        /// Index of the winning outcome (0..outcome_count-1), set on settlement
        winning_outcome: Option<u8>,
    },

    /// Numeric accuracy market (from trepa.rs logic)
    ///
    /// Each player pays a fixed entry fee and submits an estimate.
    /// After the real outcome is revealed:
    ///   - error = |estimate - outcome|
    ///   - median error divides winners from losers
    ///   - losers' fees form the prize pool (minus protocol cut)
    ///   - winners split prize pool weighted by accuracy
    Accuracy {
        /// Fixed fee every player pays to enter
        entry_fee: u64,
        /// Total tokens deposited (players * entry_fee)
        total_pool: u64,
        /// Losers' share of the pool
        loser_pool: u64,
        /// Protocol's cut from loser_pool
        protocol_take: u64,
        /// What winners actually split (loser_pool - protocol_take)
        prize_pool: u64,
        /// Count of winners (error < median)
        winner_count: u32,
        /// Count of losers (error >= median)
        loser_count: u32,
        /// The real-world outcome value (scaled integer)
        /// Set when market is settled. None while open.
        outcome_value: Option<u64>,
        /// How many decimal places the outcome is scaled by
        /// e.g. decimals=2 means value 10050 represents 100.50
        outcome_decimals: u8,
        /// Median error computed during settlement
        median_error: Option<u64>,
        /// Sum of all winner weights — needed to compute each winner's share
        total_weight: Option<u64>,
        /// Weight scaling factor (since we can't use floats on-chain)
        /// Weights are stored as: weight * 10^weight_scale
        weight_scale: u8,
    },
}

// ─────────────────────────────────────────────────────────────────────────
//  BET — one account per individual bet
//
//  PDA seeds: ["bet", market.key(), bet_index.to_le_bytes()]
//  Owner: program
//  Size: ~150 bytes
//
//  Separate account per bet so the Market account stays fixed-size.
//  A user CAN place multiple bets on the same market (each gets a new index).
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Bet {
    /// PDA bump seed
    pub bump: u8,
    /// The bettor's wallet
    pub bettor: Pubkey,
    /// Which market this bet belongs to
    pub market: Pubkey,
    /// Sequential index within the market (0, 1, 2, ...)
    pub bet_index: u64,
    /// When this bet was placed
    pub created_at: i64,
    /// Whether the bettor has claimed their payout (or refund)
    pub claimed: bool,
    /// Type-specific bet data
    pub bet_data: BetData,
}

// ─────────────────────────────────────────────────────────────────────────
//  BET DATA — type-specific fields per bet
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum BetData {
    /// Bet on a Yes/No market
    YesNo {
        /// Which side: Yes or No
        side: Side,
        /// Full amount the user deposited
        total_amount: u64,
        /// Amount after fees (this goes into the pool)
        net_amount: u64,
        /// LP fee portion
        lp_fee: u64,
        /// Protocol fee portion
        protocol_fee: u64,
    },

    /// Bet on a Multi-Outcome market
    MultiOutcome {
        /// Which outcome this bet is on (0..outcome_count-1)
        outcome_id: u8,
        /// Full amount the user deposited
        total_amount: u64,
        /// Amount after fees (this goes into the pool)
        net_amount: u64,
        /// LP fee portion
        lp_fee: u64,
        /// Protocol fee portion
        protocol_fee: u64,
    },

    /// Bet on an Accuracy market
    Accuracy {
        /// The player's numeric estimate (scaled integer)
        estimate: u64,
        /// Decimal scaling (must match market's outcome_decimals)
        estimate_decimals: u8,
        /// Entry fee paid
        entry_fee: u64,
        // ── Fields populated during settlement (None until then) ──
        /// |estimate - outcome|
        error: Option<u64>,
        /// error / median_error (scaled by weight_scale)
        relative_error: Option<u64>,
        /// (1 / (1 + relative_error))^6 (scaled by weight_scale)
        weight: Option<u64>,
        /// Did this player beat the median?
        won: Option<bool>,
        /// How much this player can claim
        payout: Option<u64>,
    },
}

// ─────────────────────────────────────────────────────────────────────────
//  PDA SEEDS REFERENCE
// ─────────────────────────────────────────────────────────────────────────
//
//  Protocol:     ["protocol"]
//  Market:       ["market", market_index.to_le_bytes()]
//  MarketGroup:  ["market_group", group_index.to_le_bytes()]
//  Vault:        ["vault", market.key()]
//  Bet:          ["bet", market.key(), bet_index.to_le_bytes()]
//
// ─────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────
//  ACCOUNT SIZE CONSTANTS
// ─────────────────────────────────────────────────────────────────────────

/// Max length for the market question string (bytes)
pub const MAX_QUESTION_LEN: usize = 200;

/// Protocol account size (with 8-byte Anchor discriminator)
pub const PROTOCOL_SIZE: usize = 8 + 1 + 32 + 32 + 2 + 8; // = 83

/// MarketGroup account size
/// 8 (disc) + 1 (bump) + 32 (creator) + 8 (group_index)
/// + 4+200 (question) + 8 (deadline) + 8 (created_at)
/// + 1 (tier_count)
/// + labels: 4 × (4 + 16) = 80
/// + entry_fees: 4 × 8 = 32
/// + tier_markets: 4 × 32 = 128
/// + Option<u64> outcome(9) + 1 (decimals) + 1 (settled)
pub const MARKET_GROUP_SIZE: usize = 8 + 1 + 32 + 8 + (4 + MAX_QUESTION_LEN) + 8 + 8
    + 1
    + (4 + MAX_TIERS * (4 + MAX_TIER_LABEL_LEN))
    + (4 + MAX_TIERS * 8)
    + (4 + MAX_TIERS * 32)
    + 9 + 1 + 1;
// = ~561 bytes

/// Shared Market header size (common fields before market_data)
/// 8 (disc) + 1 (bump) + 32 (creator) + 8 (index) + 1 (type) + 1 (category)
/// + 1 (status) + 4+200 (string) + 8 (bond) + 2+2 (fees) + 8+8 (timestamps)
/// + 8 (total_bets) + 32 (vault) + 32 (mint) + Option<Pubkey> market_group(1+32)
const MARKET_HEADER_SIZE: usize = 8 + 1 + 32 + 8 + 1 + 1 + 1 + (4 + MAX_QUESTION_LEN)
    + 8 + 2 + 2 + 8 + 8 + 8 + 32 + 32 + 1 + 32;
// = 393 bytes

/// Market account size (YesNo variant)
/// Header + enum tag(1) + yes_pool(8) + no_pool(8) + lp_fee_pool(8)
/// + protocol_fee_pool(8) + Option<Side>(1+1)
pub const MARKET_YESNO_SIZE: usize = MARKET_HEADER_SIZE + 1 + 8 + 8 + 8 + 8 + 2;
// = ~394 bytes

/// Market account size (MultiOutcome variant — largest because of fixed arrays)
/// Header + enum tag(1) + outcome_count(1)
/// + labels: 10 × (4 + 32) = 360  (Borsh Vec<String>: 4-byte len prefix + 32 bytes each)
/// + pools: 10 × 8 = 80
/// + lp_fee_pool(8) + protocol_fee_pool(8) + Option<u8>(1+1)
pub const MARKET_MULTI_SIZE: usize = MARKET_HEADER_SIZE + 1 + 1
    + (4 + MAX_OUTCOMES * (4 + MAX_OUTCOME_LABEL_LEN))
    + (4 + MAX_OUTCOMES * 8)
    + 8 + 8 + 2;
// = 359 + 464 = ~823 bytes

/// Market account size (Accuracy variant)
/// Header + enum tag(1) + entry_fee(8) + total_pool(8) + loser_pool(8)
/// + protocol_take(8) + prize_pool(8) + winner_count(4) + loser_count(4)
/// + Option<u64> outcome(9) + decimals(1) + Option<u64> median(9)
/// + Option<u64> total_weight(9) + weight_scale(1)
pub const MARKET_ACCURACY_SIZE: usize = MARKET_HEADER_SIZE + 1 + 8 + 8 + 8 + 8 + 8
    + 4 + 4 + 9 + 1 + 9 + 9 + 1;
// = 359 + 79 = ~438 bytes

/// Shared Bet header size (common fields before bet_data)
/// 8 (disc) + 1 (bump) + 32 (bettor) + 32 (market) + 8 (index) + 8 (created_at) + 1 (claimed)
const BET_HEADER_SIZE: usize = 8 + 1 + 32 + 32 + 8 + 8 + 1;
// = 90 bytes

/// Bet account size (YesNo variant)
/// Header + enum tag(1) + side(1) + total(8) + net(8) + lp(8) + proto(8)
pub const BET_YESNO_SIZE: usize = BET_HEADER_SIZE + 1 + 1 + 8 + 8 + 8 + 8;
// = ~124 bytes

/// Bet account size (MultiOutcome variant)
/// Header + enum tag(1) + outcome_id(1) + total(8) + net(8) + lp(8) + proto(8)
pub const BET_MULTI_SIZE: usize = BET_HEADER_SIZE + 1 + 1 + 8 + 8 + 8 + 8;
// = ~124 bytes

/// Bet account size (Accuracy variant)
/// Header + enum tag(1) + estimate(8) + decimals(1) + entry_fee(8)
/// + Option<u64> error(9) + Option<u64> rel_err(9) + Option<u64> weight(9)
/// + Option<bool> won(2) + Option<u64> payout(9)
pub const BET_ACCURACY_SIZE: usize = BET_HEADER_SIZE + 1 + 8 + 1 + 8 + 9 + 9 + 9 + 2 + 9;
// = ~146 bytes
