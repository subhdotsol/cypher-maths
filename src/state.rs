// ═══════════════════════════════════════════════════════════════════════════
//  CYPHER PREDICTION MARKET — ON-CHAIN STATE
//
//  Supports two market types in one unified program:
//    1. YesNo   — binary outcome, variable bet sizes
//    2. Accuracy — numeric estimate, fixed entry fee, median-error split
//
//  Account hierarchy:
//    Protocol (singleton)
//      └── Market (PDA per market)
//            ├── Vault (SPL token account, PDA)
//            └── Bet[] (PDA per individual bet)
// ═══════════════════════════════════════════════════════════════════════════

// In a real Solana program, these would be:
// use borsh::{BorshDeserialize, BorshSerialize};
// use solana_program::pubkey::Pubkey;
//
// For now we use placeholder types so the math repo compiles standalone.

/// Placeholder for solana_program::pubkey::Pubkey (32 bytes)
pub type Pubkey = [u8; 32];

// ─────────────────────────────────────────────────────────────────────────
//  ENUMS
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum MarketType {
    YesNo,
    Accuracy,
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
    /// Type-specific pool data
    pub market_data: MarketData,
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
//  Protocol:  ["protocol"]
//  Market:    ["market", market_index.to_le_bytes()]
//  Vault:     ["vault", market.key()]
//  Bet:       ["bet", market.key(), bet_index.to_le_bytes()]
//
// ─────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────
//  ACCOUNT SIZE CONSTANTS
// ─────────────────────────────────────────────────────────────────────────

/// Max length for the market question string (bytes)
pub const MAX_QUESTION_LEN: usize = 200;

/// Protocol account size (with 8-byte Anchor discriminator)
pub const PROTOCOL_SIZE: usize = 8 + 1 + 32 + 32 + 2 + 8; // = 83

/// Market account size (YesNo variant, max question)
/// 8 (disc) + 1 (bump) + 32 (creator) + 8 (index) + 1 (type) + 1 (status)
/// + 4+200 (string) + 8 (bond) + 2+2 (fees) + 8+8 (timestamps)
/// + 8 (total_bets) + 32 (vault) + 32 (mint) + MarketData
/// YesNo MarketData: 1 (enum tag) + 8+8+8+8 (pools) + 1+1 (Option<Side>)
pub const MARKET_YESNO_SIZE: usize = 8 + 1 + 32 + 8 + 1 + 1 + (4 + MAX_QUESTION_LEN)
    + 8 + 2 + 2 + 8 + 8 + 8 + 32 + 32 + 1 + 8 + 8 + 8 + 8 + 2;
// = 8 + 375 = ~383 bytes

/// Accuracy MarketData is larger: entry_fee, pools, counts, optional fields
pub const MARKET_ACCURACY_SIZE: usize = 8 + 1 + 32 + 8 + 1 + 1 + (4 + MAX_QUESTION_LEN)
    + 8 + 2 + 2 + 8 + 8 + 8 + 32 + 32 + 1 + 8 + 8 + 8 + 8 + 4 + 4 + 9 + 1 + 9 + 9 + 1;
// = 8 + ~420 = ~428 bytes

/// Bet account size (YesNo variant)
/// 8 (disc) + 1 + 32 + 32 + 8 + 8 + 1 + BetData
/// YesNo BetData: 1 + 1 + 8+8+8+8 = 34
pub const BET_YESNO_SIZE: usize = 8 + 1 + 32 + 32 + 8 + 8 + 1 + 1 + 1 + 8 + 8 + 8 + 8;
// = ~128 bytes

/// Bet account size (Accuracy variant)
/// Accuracy BetData: 1 + 8+1+8 + 9+9+9+2+9 = 56
pub const BET_ACCURACY_SIZE: usize = 8 + 1 + 32 + 32 + 8 + 8 + 1 + 1 + 8 + 1 + 8 + 9 + 9 + 9 + 2 + 9;
// = ~148 bytes
