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

// ═══════════════════════════════════════════════════════════════════
//  TIERED LOBBIES
//
//  Same players, same outcome, different entry fees.
//  Each tier is an independent pool — the formula doesn't change.
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct Tier {
    pub name: String,
    pub entry_fee: f64,
}

#[derive(Debug)]
pub struct TieredResult {
    pub tier_name: String,
    pub entry_fee: f64,
    pub result: RoundResult,
}

pub fn run_tiered(
    players: Vec<Player>,
    outcome: f64,
    tiers: Vec<Tier>,
    protocol_fee_percent: f64,
) -> Vec<TieredResult> {
    tiers
        .into_iter()
        .map(|tier| {
            let result = run_round(
                players.clone(),
                outcome,
                tier.entry_fee,
                protocol_fee_percent,
            );
            TieredResult {
                tier_name: tier.name,
                entry_fee: tier.entry_fee,
                result,
            }
        })
        .collect()
}

fn print_round(label: &str, result: &RoundResult) {
    println!("\n==============================");
    println!("{}", label);
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

    // ─── SINGLE ROUND (original example) ───
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║              ACCURACY MARKET — SINGLE ROUND                ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    let result = run_round(players.clone(), outcome, 1.0, 0.20);
    print_round("TREPA ROUND RESULT (entry: $1.00)", &result);

    // ─── TIERED LOBBIES ───
    println!("\n\n╔══════════════════════════════════════════════════════════════╗");
    println!("║              ACCURACY MARKET — TIERED LOBBIES              ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    let tiers = vec![
        Tier { name: "Bronze".to_string(), entry_fee: 1.0 },
        Tier { name: "Silver".to_string(), entry_fee: 10.0 },
        Tier { name: "Gold".to_string(), entry_fee: 100.0 },
    ];

    let tiered_results = run_tiered(players, outcome, tiers, 0.20);

    // ─── Side-by-side comparison ───
    println!("\n==============================");
    println!("TIERED COMPARISON");
    println!("==============================");
    println!(
        "{:<8} {:<12} {:<12} {:<12} {:<12}",
        "Tier", "Entry Fee", "Total Pool", "Prize Pool", "Proto Take"
    );
    println!("{}", "-".repeat(56));
    for t in &tiered_results {
        println!(
            "{:<8} ${:<11.2} ${:<11.2} ${:<11.2} ${:<11.2}",
            t.tier_name, t.entry_fee, t.result.total_pool, t.result.prize_pool, t.result.protocol_take
        );
    }

    println!(
        "\n{:<8} {:<8} {:<12} {:<12} {:<10}",
        "Tier", "Player", "Payout", "Profit", "ROI"
    );
    println!("{}", "-".repeat(50));
    for t in &tiered_results {
        for r in &t.result.results {
            if r.won {
                let profit = r.final_payout - t.entry_fee;
                let roi = (profit / t.entry_fee) * 100.0;
                println!(
                    "{:<8} {:<8} ${:<11.2} ${:<11.2} {:<.1}%",
                    t.tier_name, r.name, r.final_payout, profit, roi
                );
            }
        }
    }

    // ─── Full detail per tier ───
    for t in &tiered_results {
        print_round(
            &format!("{} TIER (entry: ${:.2})", t.tier_name.to_uppercase(), t.entry_fee),
            &t.result,
        );
    }
}
