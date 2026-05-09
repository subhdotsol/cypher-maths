use std::collections::HashMap;

use crate::trepa::{Player, Tier, run_tiered};

const LP_FEE_PERCENTAGE: f64 = 0.015;
const PROTOCOL_FEE_PERCENTAGE: f64 = 0.005;
const NET_PERCENT: f64 = 0.98;

// ═══════════════════════════════════════════════════════════════════
//  YES/NO MARKET (original)
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum Side {
    Yes,
    No,
}

#[derive(Debug)]
struct Bet {
    bettor: String,
    side: Side,
    total_amount: f64,
    net_amount: f64,
    lp_fee: f64,
    protocol_fee: f64,
}

#[derive(Debug)]
pub struct Market {
    creator: String,
    question: String,
    creator_bond: f64,
    bets: Vec<Bet>,
    yes_pool: f64,
    no_pool: f64,
    lp_fee_pool: f64,
    protocol_fee_pool: f64,
    settled: bool,
    winning_side: Option<Side>,
}

impl Market {
    pub fn new(creator: &str, question: &str, creator_bond: f64) -> Self {
        Self {
            creator: creator.to_string(),
            question: question.to_string(),
            creator_bond,
            bets: vec![],
            yes_pool: 0.0,
            no_pool: 0.0,
            lp_fee_pool: 0.0,
            protocol_fee_pool: 0.0,
            settled: false,
            winning_side: None,
        }
    }

    pub fn place_bet(&mut self, bettor: &str, side: Side, amount: f64) {
        let lp_fee = amount * LP_FEE_PERCENTAGE;
        let protocol_fee = amount * PROTOCOL_FEE_PERCENTAGE;
        let net_amount = amount * NET_PERCENT;

        let bet = Bet {
            bettor: bettor.to_string(),
            side: side.clone(),
            total_amount: amount,
            net_amount,
            lp_fee,
            protocol_fee,
        };
        match side {
            Side::Yes => self.yes_pool += net_amount,
            Side::No => self.no_pool += net_amount,
        }

        self.lp_fee_pool += lp_fee;
        self.protocol_fee_pool += protocol_fee;
        self.bets.push(bet);
    }

    pub fn settle(&mut self, winning_side: Side) {
        self.settled = true;
        self.winning_side = Some(winning_side);
    }

    pub fn payout(&self) {
        if !self.settled {
            println!("Market not settled");
            return;
        }

        let winning_side = self.winning_side.as_ref().unwrap();

        let winning_pool = match winning_side {
            Side::Yes => self.yes_pool,
            Side::No => self.no_pool,
        };

        let loosing_pool = match winning_side {
            Side::Yes => self.no_pool,
            Side::No => self.yes_pool,
        };

        println!("\n=============================== Payouts ====================================");

        // --- Bettor payouts ---
        println!("\n--- Bettor Payouts ---");
        for bet in &self.bets {
            if bet.side == *winning_side {
                let share = bet.net_amount / winning_pool;

                // winner gets the proportional share of the loosing pool
                let winnings = share * loosing_pool;

                let total_return = bet.net_amount + winnings;
                let profit = total_return - bet.total_amount;

                println!("Winner : {}", bet.bettor);
                println!("Net Bet : {:.3}", bet.net_amount);
                println!("Share of winning pool : {:.2}%", share * 100.0);
                println!("Winnings from losers  : {:.3}", winnings);
                println!("Total Return          : {:.3}", total_return);
                println!("Profit (return - orig): {:.3}", profit);
                println!("------------------------------------");
            }
        }

        // --- Loser losses ---
        println!("\n--- Loser Losses ---");
        for bet in &self.bets {
            if bet.side != *winning_side {
                let fees_paid = bet.lp_fee + bet.protocol_fee;
                println!("Loser : {}", bet.bettor);
                println!("Original Bet          : {:.3}", bet.total_amount);
                println!("Net Bet (went to pool) : {:.3}", bet.net_amount);
                println!("Fees Paid (LP + Proto) : {:.3}", fees_paid);
                println!("Total Lost             : {:.3}", bet.total_amount);
                println!("------------------------------------");
            }
        }

        // --- Protocol profit ---
        println!("\n--- Protocol Profit ---");
        println!("Protocol Fee Collected : {:.3}", self.protocol_fee_pool);
        println!("Protocol Profit        : {:.3}", self.protocol_fee_pool);
        println!("------------------------------------");

        // --- Market Creator profit ---
        let creator_payout = self.creator_bond + self.lp_fee_pool;
        let creator_profit = self.lp_fee_pool;

        println!("\n--- Market Creator: {} ---", self.creator);
        println!("Bond Deposited         : {:.3}", self.creator_bond);
        println!("LP Fees Earned         : {:.3}", self.lp_fee_pool);
        println!("Total Payout           : {:.3} (bond + LP fees)", creator_payout);
        println!("Creator Profit         : {:.3}", creator_profit);
        println!("====================================");
    }

    pub fn print_state(&self) {
        println!("\n=============================== Market State ==============================");
        println!("Question      : {}", self.question);
        println!("Creator       : {}", self.creator);
        println!("Creator Bond  : {:.3}", self.creator_bond);
        println!("Yes Pool      : {:.3}", self.yes_pool);
        println!("No Pool       : {:.3}", self.no_pool);
        println!("LP Fee Pool   : {:.3}", self.lp_fee_pool);
        println!("Protocol Fee  : {:.3}", self.protocol_fee_pool);
        println!("Total Bets    : {}", self.bets.len());
        println!("Settled       : {}", self.settled);
        if let Some(ref side) = self.winning_side {
            println!("Winning Side  : {side:?}");
        }

        let total_pool = self.yes_pool + self.no_pool;
        if total_pool > 0.0 {
            println!(
                "Yes Probability : {:.2}%",
                (self.yes_pool / total_pool) * 100.0
            );
            println!(
                "No Probability  : {:.2}%",
                (self.no_pool / total_pool) * 100.0
            );
        }

        println!("\nBets:");
        for (i, bet) in self.bets.iter().enumerate() {
            println!(
                "  #{}: {} bet {:?} — total: {:.3}, net: {:.3}, lp_fee: {:.3}, protocol_fee: {:.3}",
                i + 1,
                bet.bettor,
                bet.side,
                bet.total_amount,
                bet.net_amount,
                bet.lp_fee,
                bet.protocol_fee
            );
        }
        println!("===========================================================================");
    }
}

// ═══════════════════════════════════════════════════════════════════
//  MULTI-OUTCOME MARKET
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug)]
struct MultiBet {
    bettor: String,
    outcome: String,
    total_amount: f64,
    net_amount: f64,
    lp_fee: f64,
    protocol_fee: f64,
}

#[derive(Debug)]
pub struct MultiMarket {
    creator: String,
    question: String,
    creator_bond: f64,
    outcomes: Vec<String>,
    bets: Vec<MultiBet>,
    pools: HashMap<String, f64>,
    lp_fee_pool: f64,
    protocol_fee_pool: f64,
    settled: bool,
    winning_outcome: Option<String>,
}

impl MultiMarket {
    pub fn new(creator: &str, question: &str, creator_bond: f64, outcomes: Vec<&str>) -> Self {
        let mut pools = HashMap::new();
        let outcome_strings: Vec<String> = outcomes.iter().map(|o| o.to_string()).collect();
        for outcome in &outcome_strings {
            pools.insert(outcome.clone(), 0.0);
        }

        Self {
            creator: creator.to_string(),
            question: question.to_string(),
            creator_bond,
            outcomes: outcome_strings,
            bets: vec![],
            pools,
            lp_fee_pool: 0.0,
            protocol_fee_pool: 0.0,
            settled: false,
            winning_outcome: None,
        }
    }

    pub fn place_bet(&mut self, bettor: &str, outcome: &str, amount: f64) {
        if !self.pools.contains_key(outcome) {
            println!("Invalid outcome: {}", outcome);
            return;
        }

        let lp_fee = amount * LP_FEE_PERCENTAGE;
        let protocol_fee = amount * PROTOCOL_FEE_PERCENTAGE;
        let net_amount = amount * NET_PERCENT;

        let bet = MultiBet {
            bettor: bettor.to_string(),
            outcome: outcome.to_string(),
            total_amount: amount,
            net_amount,
            lp_fee,
            protocol_fee,
        };

        *self.pools.get_mut(outcome).unwrap() += net_amount;
        self.lp_fee_pool += lp_fee;
        self.protocol_fee_pool += protocol_fee;
        self.bets.push(bet);
    }

    pub fn settle(&mut self, winning_outcome: &str) {
        if !self.pools.contains_key(winning_outcome) {
            println!("Invalid outcome: {}", winning_outcome);
            return;
        }
        self.settled = true;
        self.winning_outcome = Some(winning_outcome.to_string());
    }

    pub fn payout(&self) {
        if !self.settled {
            println!("Market not settled");
            return;
        }

        let winner = self.winning_outcome.as_ref().unwrap();
        let winning_pool = self.pools[winner];

        let total_pool: f64 = self.pools.values().sum();
        let losing_pool = total_pool - winning_pool;

        println!("\n=============================== Multi-Outcome Payouts ====================================");
        println!("Winning Outcome : {}", winner);
        println!("Winning Pool    : {:.3}", winning_pool);
        println!("Losing Pool     : {:.3}", losing_pool);

        // --- edge case: nobody bet on the winner ---
        if winning_pool == 0.0 {
            println!("\nNO ONE BET ON THE WINNING OUTCOME!");
            println!("All bettors lose. Pool goes unclaimed.");
            println!("Protocol keeps fees: {:.3}", self.protocol_fee_pool);
            println!("Creator keeps bond + LP fees: {:.3}", self.creator_bond + self.lp_fee_pool);
            println!("====================================");
            return;
        }

        // --- Winner payouts ---
        println!("\n--- Winner Payouts ---");
        for bet in &self.bets {
            if bet.outcome == *winner {
                let share = bet.net_amount / winning_pool;
                let winnings = share * losing_pool;
                let total_return = bet.net_amount + winnings;
                let profit = total_return - bet.total_amount;

                println!("Winner : {}", bet.bettor);
                println!("Picked : {}", bet.outcome);
                println!("Net Bet : {:.3}", bet.net_amount);
                println!("Share of winning pool : {:.2}%", share * 100.0);
                println!("Winnings from losers  : {:.3}", winnings);
                println!("Total Return          : {:.3}", total_return);
                println!("Profit (return - orig): {:.3}", profit);
                println!("------------------------------------");
            }
        }

        // --- Loser losses ---
        println!("\n--- Loser Losses ---");
        for bet in &self.bets {
            if bet.outcome != *winner {
                let fees_paid = bet.lp_fee + bet.protocol_fee;
                println!("Loser : {}", bet.bettor);
                println!("Picked : {}", bet.outcome);
                println!("Original Bet          : {:.3}", bet.total_amount);
                println!("Net Bet (went to pool) : {:.3}", bet.net_amount);
                println!("Fees Paid (LP + Proto) : {:.3}", fees_paid);
                println!("Total Lost             : {:.3}", bet.total_amount);
                println!("------------------------------------");
            }
        }

        // --- Protocol profit ---
        println!("\n--- Protocol Profit ---");
        println!("Protocol Fee Collected : {:.3}", self.protocol_fee_pool);
        println!("------------------------------------");

        // --- Market Creator profit ---
        let creator_payout = self.creator_bond + self.lp_fee_pool;
        let creator_profit = self.lp_fee_pool;

        println!("\n--- Market Creator: {} ---", self.creator);
        println!("Bond Deposited         : {:.3}", self.creator_bond);
        println!("LP Fees Earned         : {:.3}", self.lp_fee_pool);
        println!("Total Payout           : {:.3} (bond + LP fees)", creator_payout);
        println!("Creator Profit         : {:.3}", creator_profit);
        println!("====================================");
    }

    pub fn print_state(&self) {
        println!("\n=============================== Multi-Outcome Market State ==============================");
        println!("Question      : {}", self.question);
        println!("Creator       : {}", self.creator);
        println!("Creator Bond  : {:.3}", self.creator_bond);
        println!("Outcomes      : {:?}", self.outcomes);

        let total_pool: f64 = self.pools.values().sum();

        println!("\nPools:");
        for outcome in &self.outcomes {
            let pool = self.pools[outcome];
            let prob = if total_pool > 0.0 {
                (pool / total_pool) * 100.0
            } else {
                0.0
            };
            println!("  {:<15} : {:.3}  ({:.2}%)", outcome, pool, prob);
        }

        println!("\nLP Fee Pool   : {:.3}", self.lp_fee_pool);
        println!("Protocol Fee  : {:.3}", self.protocol_fee_pool);
        println!("Total Bets    : {}", self.bets.len());
        println!("Settled       : {}", self.settled);
        if let Some(ref winner) = self.winning_outcome {
            println!("Winner        : {}", winner);
        }

        println!("\nBets:");
        for (i, bet) in self.bets.iter().enumerate() {
            println!(
                "  #{}: {} bet \"{}\" — total: {:.3}, net: {:.3}, lp_fee: {:.3}, protocol_fee: {:.3}",
                i + 1,
                bet.bettor,
                bet.outcome,
                bet.total_amount,
                bet.net_amount,
                bet.lp_fee,
                bet.protocol_fee
            );
        }
        println!("===========================================================================");
    }
}

pub fn flew() {
    // ─── YES/NO MARKET (original example) ───
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║              YES/NO MARKET EXAMPLE                         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    let mut market = Market::new("alice", "Will BTC hit 100k by end of 2026?", 10.0);

    market.place_bet("bob", Side::Yes, 10.0);
    market.place_bet("carol", Side::Yes, 5.0);
    market.place_bet("dan", Side::No, 20.0);
    market.place_bet("eve", Side::No, 15.0);
    market.place_bet("Frank", Side::Yes, 8.0);

    market.print_state();
    market.settle(Side::Yes);
    market.payout();

    // ─── MULTI-OUTCOME MARKET (new example) ───
    println!("\n\n╔══════════════════════════════════════════════════════════════╗");
    println!("║           MULTI-OUTCOME MARKET EXAMPLE                     ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    let mut multi = MultiMarket::new(
        "alice",
        "Which chain will have the highest TVL by end of 2026?",
        10.0,
        vec!["Ethereum", "Solana", "Arbitrum", "Base"],
    );

    multi.place_bet("alice", "Ethereum", 50.0);
    multi.place_bet("bob", "Solana", 30.0);
    multi.place_bet("carol", "Solana", 20.0);
    multi.place_bet("dan", "Arbitrum", 40.0);
    multi.place_bet("eve", "Base", 10.0);

    multi.print_state();
    multi.settle("Solana");
    multi.payout();

    // ─── TIERED ACCURACY MARKET ───
    println!("\n\n╔══════════════════════════════════════════════════════════════╗");
    println!("║         TIERED ACCURACY MARKET EXAMPLE                     ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Question: What will BTC price be at midnight UTC June 1st?");
    println!("Outcome:  $100,000\n");

    let players = vec![
        Player { name: "alice".to_string(), estimate: 99_950.0 },
        Player { name: "bob".to_string(), estimate: 99_800.0 },
        Player { name: "carol".to_string(), estimate: 99_500.0 },
        Player { name: "dan".to_string(), estimate: 99_000.0 },
        Player { name: "eve".to_string(), estimate: 98_000.0 },
        Player { name: "frank".to_string(), estimate: 95_000.0 },
    ];

    let tiers = vec![
        Tier { name: "Bronze".to_string(), entry_fee: 1.0 },
        Tier { name: "Silver".to_string(), entry_fee: 10.0 },
        Tier { name: "Gold".to_string(), entry_fee: 100.0 },
        Tier { name: "Diamond".to_string(), entry_fee: 1000.0 },
    ];

    let results = run_tiered(players, 100_000.0, tiers, 0.20);

    println!(
        "{:<10} {:<10} {:<12} {:<12} {:<12} {:<12}",
        "Tier", "Fee", "Total Pool", "Loser Pool", "Proto Take", "Prize Pool"
    );
    println!("{}", "─".repeat(68));
    for t in &results {
        println!(
            "{:<10} ${:<9.2} ${:<11.2} ${:<11.2} ${:<11.2} ${:<11.2}",
            t.tier_name,
            t.entry_fee,
            t.result.total_pool,
            t.result.loser_pool,
            t.result.protocol_take,
            t.result.prize_pool
        );
    }

    println!(
        "\n{:<10} {:<8} {:<10} {:<12} {:<12} {:<8}",
        "Tier", "Player", "Estimate", "Payout", "Profit", "ROI"
    );
    println!("{}", "─".repeat(60));
    for t in &results {
        for r in &t.result.results {
            let profit = r.final_payout - t.entry_fee;
            let roi = if r.won {
                format!("{:+.1}%", (profit / t.entry_fee) * 100.0)
            } else {
                "-100%".to_string()
            };
            println!(
                "{:<10} {:<8} {:<10.0} ${:<11.2} ${:<11.2} {}",
                t.tier_name,
                r.name,
                r.estimate,
                r.final_payout,
                profit,
                roi
            );
        }
    }
}
