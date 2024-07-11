use postflop_solver::*;
use std::time::Instant;
use serde::{Serialize, Deserialize};
use serde_json;
use std::fs::File;
use std::io::Write;
use std::fmt;
use std::path::Path;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::panic;
use clap::Parser;

#[derive(Serialize, Deserialize)]
struct HandFrequency {
    hand: String,
    frequencies: Vec<String>,
    equity: f32,
    ev: f32,
}

impl fmt::Debug for HandFrequency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.hand, self.frequencies)
    }
}

#[derive(Serialize)]
struct ActionsWithPrev<'a> {
    flop: &'a str,
    action: Vec<String>,
    prev_action_line: Vec<usize>,
    parsed_prev_action_line: Vec<String>,
}

fn generate_combinations(turn_card: usize, river_card: usize) -> Vec<Vec<usize>> {
    let flop_lines = vec![
        vec![0, 0],
        vec![0, 1, 1],
        vec![0, 1, 2, 1],
        vec![1, 1],
        vec![1, 2, 1],
    ];

    let turn_lines = vec![
        vec![0, 0],
        vec![0, 1, 1],
        vec![0, 2, 1],
        vec![0, 1, 2, 1],
        vec![0, 2, 2, 1],
        vec![1, 1],
        vec![2, 1],
        vec![1, 2, 1],
        vec![2, 2, 1],
    ];

    let river_lines = vec![
        vec![0, 0],
        vec![0, 1, 1],
        vec![0, 1, 2, 1],
        vec![1, 1],
        vec![1, 2, 1],
    ];

    let mut combinations = Vec::new();

    for flop_line in &flop_lines {
        let mut combination = Vec::new();
        combination.extend(flop_line);
        combinations.push(combination);
    }

    for flop_line in &flop_lines {
        for turn_line in &turn_lines {
                let mut combination = Vec::new();
                combination.extend(flop_line);
                combination.push(turn_card);
                combination.extend(turn_line);
                combinations.push(combination);
        }
    }

    for flop_line in &flop_lines {
        for turn_line in &turn_lines {
            for river_line in &river_lines {
                let mut combination = Vec::new();
                combination.extend(flop_line);
                combination.push(turn_card);
                combination.extend(turn_line);
                combination.push(river_card);
                combination.extend(river_line);
                combinations.push(combination);
            }
        }
    }

    // Remove the last entry from each combination
    for combination in &mut combinations {
        combination.pop();
    }

    combinations
}

// Define the wrapper function
fn process_game(game: &PostFlopGame) -> Result<(Vec<Action>, Vec<HandFrequency>), String> {
    let player = game.current_player();
    let cards_result = holes_to_strings(game.private_cards(player.into()));
    let actions = game.available_actions();
    let strategy = game.strategy();
    let card_equity = game.equity(player.into());
    let card_ev = game.expected_values(player.into());

    // Handle the Result type from holes_to_strings
    let cards = match cards_result {
        Ok(cards) => cards,
        Err(err) => return Err(err),
    };

    let num_hands = cards.len();
    let num_actions = actions.len();
    let mut result = Vec::new();

    for i in 0..num_hands {
        let mut hand_frequencies = vec![];
        for j in 0..num_actions {
            hand_frequencies.push(format!("{:.4}", strategy[i + j * num_hands]));
        }
        result.push(HandFrequency {
            hand: cards[i].clone(),
            frequencies: hand_frequencies,
            equity: card_equity[i].clone(),
            ev: card_ev[i].clone()
        });
    }

    Ok((actions, result))
}

// Function to save result and actions to specified paths
fn save_results(game: &mut PostFlopGame, result: &[HandFrequency], actions: &[Action], prev_action_line: &[usize],
    flop_str: &str, result_path: &Path, actions_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Serialize and save the result
    let result_json = serde_json::to_string_pretty(result)?;
    let mut file = File::create(result_path)?;
    file.write_all(result_json.as_bytes())?;

    let parsed_prev_action_line = parse_action_history(game, prev_action_line);

    // Prepare the actions with prev_action_line
    let actions_str: Vec<String> = actions.iter().map(|action| format!("{:?}", action)).collect();
    let actions_with_prev = ActionsWithPrev {
        flop: flop_str,
        action: actions_str,
        prev_action_line: prev_action_line.to_vec(),
        parsed_prev_action_line: parsed_prev_action_line.clone(),
    };
    let actions_json = serde_json::to_string_pretty(&actions_with_prev)?;
    let mut file = File::create(actions_path)?;
    file.write_all(actions_json.as_bytes())?;
        
    println!("Result and Action Saved for {:?}", parsed_prev_action_line);
    Ok(())
}

// Wrapper function to apply history and catch panics
pub fn apply_history_safe(game: &mut PostFlopGame, history: &[usize]) -> Result<(), String> {
    // Use catch_unwind to catch panics
    match catch_unwind(AssertUnwindSafe(|| {
        game.back_to_root();
        for &action in history {
            game.play(action);
        }
        game.cache_normalized_weights();
        Ok(())
    })) {
        Ok(result) => result,
        Err(_) => Err("Panic occurred while applying history".to_string()),
        // Err(_) => {},
    }
}

fn parse_action_history(game: &mut PostFlopGame, action_history: &[usize]) -> Vec<String> {
    let mut parsed_actions = Vec::new();
    game.back_to_root();

    for &action_index in action_history {
        if game.is_chance_node() {
            // Parse the card ID
            let rank = match action_index / 4 {
                0 => "2",
                1 => "3",
                2 => "4",
                3 => "5",
                4 => "6",
                5 => "7",
                6 => "8",
                7 => "9",
                8 => "T",
                9 => "J",
                10 => "Q",
                11 => "K",
                12 => "A",
                _ => panic!("Invalid card rank"),
            };
            let suit = match action_index % 4 {
                0 => "c",
                1 => "d",
                2 => "h",
                3 => "s",
                _ => panic!("Invalid card suit"),
            };
            parsed_actions.push(format!("{}{}", rank, suit));
        } else {
            // Get the action from available actions
            let available_actions = game.available_actions();
            if action_index < available_actions.len() {
                parsed_actions.push(format!("{:?}", available_actions[action_index]));
            } else {
                panic!("Invalid action index: {}", action_index);
            }
        }
        // Apply the action to advance the game state
        game.play(action_index);
    }

    parsed_actions
}

// Define the CLI arguments
#[derive(Parser, Debug)]
#[clap(author = "Your Name", version = "1.0", about = "Solves the poker game")]
struct Args {
    #[clap(short, long, value_name = "FLOP")]
    flop: String,

    #[clap(short, long, value_name = "TURN")]
    turn: String,

    #[clap(short, long, value_name = "RIVER")]
    river: String,

    #[clap(short, long, value_name = "OOP_RANGE")]
    oop_range: String,

    #[clap(short, long, value_name = "IP_RANGE")]
    ip_range: String,

    #[clap(short, long, value_name = "FLOP_BET_SIZES")]
    flop_bet_sizes: String,

    // #[clap(short, long, value_name = "TURN_BET_SIZES")]
    // turn_bet_sizes: String,

    // #[clap(short, long, value_name = "RIVER_BET_SIZES")]
    // river_bet_sizes: String,

    #[clap(short, long, value_name = "STARTING_POT")]
    starting_pot: i32,

    #[clap(short, long, value_name = "EFFECTIVE_STACK")]
    effective_stack: i32,

    #[clap(short, long, value_name = "FOLDER_PATH")]
    folder_path: String,
}

fn main() {
    // Set a custom panic hook that does nothing
    panic::set_hook(Box::new(|_| {
        // Do nothing
    }));

    // Parse the command-line arguments
    let args = Args::parse();

    // Get the value of the arguments
    let flop_str = args.flop.as_str();
    let turn_str = args.turn.as_str();
    let river_str = args.river.as_str();
    let oop_range_str = args.oop_range.as_str();
    let ip_range_str = args.ip_range.as_str();
    let flop_bet_sizes_str = args.flop_bet_sizes.as_str();
    // let turn_bet_sizes_str = matches.value_of("turn_bet_size").unwrap();
    // let river_bet_sizes_str = matches.value_of("river_bet_size").unwrap();
    let starting_pot_num = args.starting_pot;
    let effective_stack_num = args.effective_stack;
    let folder_path = args.folder_path.as_str();

    // ranges of OOP and IP in string format
    // see the documentation of `Range` for more details about the format
    // let oop_range = "99-22,ATs-A2s,AJo-A7o,A5o,KJs-K2s,K9o+,Q2s+,Q9o+,J3s+,J9o+,T5s+,T9o,96s+,85s+,74s+,63s+,52s+,42s+";
    // let ip_range = "22+,A2s+,A4o+,K2s+,K8o+,Q3s+,Q9o+,J4s+,J9o+,T6s+,T8o+,96s+,98o,86s+,75s+,65s,54s";
    // let flop_str = "AdKs7h";
    // let turn_card = 30;
    // let river_card = 29;

    let card_config = CardConfig {
        range: [oop_range_str.parse().unwrap(), ip_range_str.parse().unwrap()],
        flop: flop_from_str(flop_str).unwrap(),
        // flop: flop_from_str("2d2s2h").unwrap(),
        turn: NOT_DEALT,
        river: NOT_DEALT,
    };

    // bet sizes -> 60% of the pot, geometric size, and all-in
    // raise sizes -> 2.5x of the previous bet
    // see the documentation of `BetSizeOptions` for more details

    let tree_config = TreeConfig {
        initial_state: BoardState::Flop, // must match `card_config`
        starting_pot: starting_pot_num,
        effective_stack: effective_stack_num,
        rake_rate: 0.0,
        rake_cap: 0.0,
        flop_bet_sizes: [BetSizeOptions::try_from((flop_bet_sizes_str, "66%")).unwrap(), 
                        BetSizeOptions::try_from((flop_bet_sizes_str, "66%")).unwrap()], // [OOP, IP]
        turn_bet_sizes: [BetSizeOptions::try_from(("75%, 125%", "66%")).unwrap(), 
                        BetSizeOptions::try_from(("75%, 125%", "66%")).unwrap()],
        river_bet_sizes: [BetSizeOptions::try_from(("50%", "66%")).unwrap(), 
                        BetSizeOptions::try_from(("75%", "66%")).unwrap()],
        turn_donk_sizes: None, // use default bet sizes
        river_donk_sizes: None,
        add_allin_threshold: 1.5, // add all-in if (maximum bet size) <= 2.0x pot
        force_allin_threshold: 0.15, // force all-in if (SPR after the opponent's call) <= 0.15
        merging_threshold: 0.1,
    };

    // build the game tree
    // `ActionTree` can be edited manually after construction
    let action_tree = ActionTree::new(tree_config).unwrap();
    let mut game = PostFlopGame::with_config(card_config, action_tree).unwrap();

    // check memory usage
    let (_mem_usage, mem_usage_compressed) = game.memory_usage();
    // println!(
    //     "Memory usage without compression (32-bit float): {:.2}GB",
    //     mem_usage as f64 / (1024.0 * 1024.0 * 1024.0)
    // );
    println!(
        // "Memory usage with compression (16-bit integer): {:.2}GB",
        "Memory usage: {:.2}GB",
        mem_usage_compressed as f64 / (1024.0 * 1024.0 * 1024.0)
    );

    // allocate memory with compression (use 16-bit integer)
    game.allocate_memory(true);

    // solve the game
    let max_num_iterations = 1000;
    let target_exploitability = game.tree_config().starting_pot as f32 * 0.01; // 0.5% of the pot
    // Start the timer
    let start = Instant::now();
    let exploitability = solve(&mut game, max_num_iterations, target_exploitability, true);
    // Stop the timer
    let duration = start.elapsed();
    println!("Time taken to solve: {:?}", duration);
    println!("Exploitability: {:.2}", exploitability);

    let combinations = generate_combinations(card_from_str(turn_str).unwrap().into(), 
                                             card_from_str(river_str).unwrap().into());
    // println!("Number of Total Action Lines: {:?}", combinations.len());
    let mut valid_count = 0;
    for (index, combination) in combinations.iter().enumerate() {
        // println!("{}", index);
        // println!("{:?}", combination);
        match apply_history_safe(&mut game, &combination) {
            Ok(()) => {
                // Process the valid combination
                valid_count += 1;
            }
            Err(_) => {
                // eprintln!("Error: {}", err);
                // continue;
            }
        }
        
        // Call the process_game function and store the result
        let (actions, result) = match process_game(&game) {
            Ok((actions, result)) => (actions, result),
            Err(err) => {
                println!("Error: {}", err);
                return;
            }
        };
        // println!("{:?}", actions);
        // for hand_frequency in &result {
        //     println!("{}: {:?}", hand_frequency.hand, hand_frequency.frequencies);
        //     break
        // }

        let result_file_name = format!("{}/strategy_{}.json", folder_path, index);
        let result_path = Path::new(&result_file_name);
        let actions_file_name = format!("{}/actions_{}.json", folder_path, index);
        let actions_path = Path::new(&actions_file_name);

        // let _ = save_results(&mut game, &result, &actions, &combination, flop_str, result_path, actions_path);
        if let Err(e) = save_results(&mut game, &result, &actions, &combination, flop_str, 
            result_path, actions_path) {
            eprintln!("Failed to save results: {}", e);
        }
    }
    println!("Number of Valid Action Lines: {}", valid_count);
    println!("Time taken to solve: {:?}", duration);
}
