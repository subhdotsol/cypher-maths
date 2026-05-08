const LP_FEE_PERCENTAGE: f64 = 0.015;
const PROTOCOL_FEE_PERCENTAGE: f64 = 0.005;
const NET_PERCENT: f64 = 0.98;

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

pub fn flew() {
    let mut market = Market::new("alice", "Will BTC hit 100k by end of 2026?", 10.0);

    market.place_bet("bob", Side::Yes, 10.0);
    market.place_bet("carol", Side::Yes, 5.0);
    market.place_bet("dan", Side::No, 20.0);
    market.place_bet("eve", Side::No, 15.0);
    market.place_bet("Frank", Side::Yes, 8.0);

    market.print_state();

    market.settle(Side::Yes);

    market.payout();
}
