use ordered_float::OrderedFloat;

#[derive(Debug, Clone)]
pub struct Player {
    pub name: String,
    pub estimate: f64,
}

#[derive(Debug, Clone)]
pub struct PlayerResult {
    pub name: String,
    // Input
    pub estimate: f64,

    // computed
    pub error: f64,
    pub relative_error: f64,
    pub weight: f64,

    // Outcome
    pub won: bool,

    // money calc
    pub entry_returned: f64,
    pub profit_share: f64,
    pub final_payout: f64,
}

#[derive(Debug)]
pub struct RoundResult {
    pub outcome: f64,
    pub median_error: f64,

    pub total_players: usize,
    pub winners: usize,
    pub losers: usize,

    pub total_pool: f64,
    pub loser_pool: f64,
    pub protocol_take: f64,
    pub prize_pool: f64,

    pub results: Vec<PlayerResult>,
}

pub fn run_round(
    players: Vec<Player>,
    outcome: f64,
    entry_fee: f64,
    protocol_fee_percent: f64,
) -> RoundResult {
    let total_players = players.len();

    // -----------------------------------------
    // STEP 1 — Calculate errors
    // e_i = |x_i - y|
    // -----------------------------------------

    let mut raw: Vec<(Player, f64)> = players
        .into_iter()
        .map(|p| {
            let error = (p.estimate - outcome).abs();
            (p, error)
        })
        .collect();

    // -----------------------------------------
    // STEP 2 — Sort errors
    // -----------------------------------------

    raw.sort_by_key(|(_, err)| OrderedFloat(*err));

    // -----------------------------------------
    // STEP 3 — Median index
    // k = floor((N + 1) / 2)
    //
    // Using 0-indexed vec:
    // actual index = k - 1
    // -----------------------------------------

    let k = (total_players + 1) / 2;
    let median_error = raw[k - 1].1;

    // -----------------------------------------
    // STEP 4 — Determine winners
    // e_i < median_error
    // -----------------------------------------

    let mut winner_count = 0usize;
    let mut loser_count = 0usize;

    let mut temp_results: Vec<PlayerResult> = vec![];

    for (player, error) in raw.into_iter() {
        let won = error < median_error;

        if won {
            winner_count += 1;
        } else {
            loser_count += 1;
        }

        temp_results.push(PlayerResult {
            name: player.name,
            estimate: player.estimate,

            error,
            relative_error: 0.0,
            weight: 0.0,

            won,

            entry_returned: 0.0,
            profit_share: 0.0,
            final_payout: 0.0,
        });
    }

    // -----------------------------------------
    // STEP 5 — Pool calculations
    // -----------------------------------------

    let total_pool = total_players as f64 * entry_fee;

    let loser_pool = loser_count as f64 * entry_fee;

    let protocol_take = loser_pool * protocol_fee_percent;

    let prize_pool = loser_pool - protocol_take;

    // -----------------------------------------
    // STEP 6 — Compute winner weights
    //
    // r_i = e_i / m
    //
    // a_i = (1 / (1 + r_i))^6
    // -----------------------------------------

    let mut total_weight = 0.0;

    for r in temp_results.iter_mut() {
        if r.won {
            let relative_error = r.error / median_error;

            let weight = (1.0 / (1.0 + relative_error)).powf(6.0);

            r.relative_error = relative_error;
            r.weight = weight;

            total_weight += weight;
        }
    }

    // -----------------------------------------
    // STEP 7 — Split prize pool
    //
    // S_i = (a_i / A) * Q
    // -----------------------------------------

    for r in temp_results.iter_mut() {
        if r.won {
            let share = (r.weight / total_weight) * prize_pool;

            r.entry_returned = entry_fee;
            r.profit_share = share;
            r.final_payout = entry_fee + share;
        }
    }

    RoundResult {
        outcome,
        median_error,

        total_players,
        winners: winner_count,
        losers: loser_count,

        total_pool,
        loser_pool,
        protocol_take,
        prize_pool,

        results: temp_results,
    }
}

pub fn trepa() {
    let players = vec![
        Player {
            name: "P1".to_string(),
            estimate: 99_980.0,
        },
        Player {
            name: "P2".to_string(),
            estimate: 99_900.0,
        },
        Player {
            name: "P3".to_string(),
            estimate: 99_750.0,
        },
        Player {
            name: "P4".to_string(),
            estimate: 99_600.0,
        },
        Player {
            name: "P5".to_string(),
            estimate: 99_520.0,
        },
        Player {
            name: "P6".to_string(),
            estimate: 99_500.0,
        },
        Player {
            name: "P7".to_string(),
            estimate: 99_300.0,
        },
        Player {
            name: "P8".to_string(),
            estimate: 99_000.0,
        },
        Player {
            name: "P9".to_string(),
            estimate: 98_000.0,
        },
        Player {
            name: "P10".to_string(),
            estimate: 95_000.0,
        },
    ];

    let outcome = 100_000.0;

    let result = run_round(
        players, outcome, 1.0,  // entry fee
        0.20, // 20% protocol fee
    );

    println!("\n==============================");
    println!("TREPA ROUND RESULT");
    println!("==============================");

    println!("Outcome: {}", result.outcome);

    println!("Median Error: {}", result.median_error);

    println!("Players: {}", result.total_players);

    println!("Winners: {}", result.winners);
    println!("Losers: {}", result.losers);

    println!("\n--- POOL FLOW ---");

    println!("Total Pool: ${:.2}", result.total_pool);

    println!("Loser Pool: ${:.2}", result.loser_pool);

    println!("Protocol Take: ${:.2}", result.protocol_take);

    println!("Prize Pool: ${:.2}", result.prize_pool);

    println!("\n--- PLAYER RESULTS ---");

    for r in result.results.iter() {
        println!("-----------------------------------");
        println!("Player: {}", r.name);

        println!("Estimate: {}", r.estimate);

        println!("Error: {}", r.error);

        println!("Won: {}", r.won);

        if r.won {
            println!("Relative Error: {:.4}", r.relative_error);

            println!("Weight: {:.6}", r.weight);

            println!("Entry Returned: ${:.2}", r.entry_returned);

            println!("Profit Share: ${:.2}", r.profit_share);

            println!("Final Payout: ${:.2}", r.final_payout);
        } else {
            println!("Final Payout: $0.00");
        }
    }
}
