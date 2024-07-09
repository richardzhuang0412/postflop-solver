use postflop_solver::*;
use postflop_solver::card::{card_pair_to_index};
use std::collections::HashMap;
use std::time::Instant;
use serde::{Serialize, Deserialize};
use serde_json;
use std::fs::File;
use std::io::Write;
use std::fmt;
use std::path::Path;
use clap::Parser;

fn main() {
    // ranges of OOP and IP in string format
    // see the documentation of `Range` for more details about the format
    let oop_range = "99-22,ATs-A2s,AJo-A7o,A5o,KJs-K2s,K9o+,Q2s+,Q9o+,J3s+,J9o+,T5s+,T9o,96s+,85s+,74s+,63s+,52s+,42s+";
    let ip_range = "22+,A2s+,A4o+,K2s+,K8o+,Q3s+,Q9o+,J4s+,J9o+,T6s+,T8o+,96s+,98o,86s+,75s+,65s,54s";

    let card_config = CardConfig {
        range: [oop_range.parse().unwrap(), ip_range.parse().unwrap()],
        flop: flop_from_str("AdKs7h").unwrap(),
        turn: NOT_DEALT,
        river: NOT_DEALT,
    };

    // bet sizes -> 60% of the pot, geometric size, and all-in
    // raise sizes -> 2.5x of the previous bet
    // see the documentation of `BetSizeOptions` for more details
    let bet_sizes = BetSizeOptions::try_from(("33%, 75%", "3.0x")).unwrap();

    let tree_config = TreeConfig {
        initial_state: BoardState::Flop, // must match `card_config`
        starting_pot: 5,
        effective_stack: 100,
        rake_rate: 0.0,
        rake_cap: 0.0,
        flop_bet_sizes: [BetSizeOptions::try_from(("", "66%")).unwrap(), 
                        BetSizeOptions::try_from(("125%", "66%")).unwrap()], // [OOP, IP]
        turn_bet_sizes: [BetSizeOptions::try_from(("75%", "66%")).unwrap(), 
                        BetSizeOptions::try_from(("125%", "66%")).unwrap()],
        river_bet_sizes: [BetSizeOptions::try_from(("50%", "66%")).unwrap(), 
                        BetSizeOptions::try_from(("75%", "66%")).unwrap()],
        turn_donk_sizes: None, // use default bet sizes
        river_donk_sizes: None,
        add_allin_threshold: 1.5, // add all-in if (maximum bet size) <= 1.5x pot
        force_allin_threshold: 0.15, // force all-in if (SPR after the opponent's call) <= 0.15
        merging_threshold: 0.1,
    };

    // build the game tree
    // `ActionTree` can be edited manually after construction
    let action_tree = ActionTree::new(tree_config).unwrap();
    // action_tree.traverse(|node| {
    //     println!("{:?}", node);
    // });
    let mut game = PostFlopGame::with_config(card_config, action_tree).unwrap();

    // check memory usage
    let (mem_usage, mem_usage_compressed) = game.memory_usage();
    println!(
        "Memory usage without compression (32-bit float): {:.2}GB",
        mem_usage as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    println!(
        "Memory usage with compression (16-bit integer): {:.2}GB",
        mem_usage_compressed as f64 / (1024.0 * 1024.0 * 1024.0)
    );

    // allocate memory without compression (use 32-bit float)
    // game.allocate_memory(false);

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

    // play `Bet(120)`
    game.play(0);
    game.play(1);
    // println!("{:?}", game.available_actions());
    // game.play(2);
    // println!("{:?}", game.available_actions());
    // game.play(2);
    // println!("{:?}", game.available_actions());
    #[derive(Serialize, Deserialize)]
    struct HandFrequency {
        hand: String,
        frequencies: Vec<String>,
    }

    impl fmt::Debug for HandFrequency {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}: {:?}", self.hand, self.frequencies)
        }
    }

    // Define the wrapper function
    fn process_game(game: &PostFlopGame) -> Result<(Vec<Action>, Vec<HandFrequency>), String> {
        let cards_result = holes_to_strings(game.private_cards(0));
        let actions = game.available_actions();
        let strategy = game.strategy();

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
            });
        }

        Ok((actions, result))
    }

    // Call the process_game function and store the result
    let (actions, result) = match process_game(&game) {
        Ok((actions, result)) => (actions, result),
        Err(err) => {
            println!("Error: {}", err);
            return;
        }
    };
    println!("{:?}", actions);
    for hand_frequency in &result {
        println!("{}: {:?}", hand_frequency.hand, hand_frequency.frequencies);
    }


    // let cards_result = holes_to_strings(game.private_cards(0));
    // let actions = game.available_actions();
    // println!("{:?}", actions);
    // let strategy = game.strategy();
    // // Handle the Result type from holes_to_strings
    // let cards = match cards_result {
    //     Ok(cards) => cards,
    //     Err(err) => {
    //         println!("Error: {}", err);
    //         return;
    //     }
    // };
    // let num_hands = cards.len();
    // let num_actions = actions.len();
    // let mut result = Vec::new();
    // for i in 0..num_hands {
    //     let mut hand_frequencies = vec![];
    //     for j in 0..num_actions {
    //         hand_frequencies.push(format!("{:.4}", strategy[i + j * num_hands]));
    //     }
    //     result.push(HandFrequency {
    //         hand: cards[i].clone(),
    //         frequencies: hand_frequencies,
    //     });
    // }
    // for hand_frequency in &result {
    //     println!("{}: {:?}", hand_frequency.hand, hand_frequency.frequencies);
    // }

    // Function to save result and actions to specified paths
    fn save_results(result: &[HandFrequency], actions: &[Action], result_path: &Path, actions_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // Serialize and save the result
        let result_json = serde_json::to_string_pretty(result)?;
        let mut file = File::create(result_path)?;
        file.write_all(result_json.as_bytes())?;

        // Serialize and save the actions
        let actions_str: Vec<String> = actions.iter().map(|action| format!("{:?}", action)).collect();
        let actions_json = serde_json::to_string_pretty(&actions_str)?;
        let mut file = File::create(actions_path)?;
        file.write_all(actions_json.as_bytes())?;
        
        println!("Result and Action Saved");
        Ok(())
    }

    let result_path = Path::new("results/strategy.json");
    let actions_path = Path::new("results/actions.json");
    if let Err(e) = save_results(&result, &actions, result_path, actions_path) {
        eprintln!("Failed to save results: {}", e);
    }
    // Serialize and save the result
    // let result_json = serde_json::to_string_pretty(&result).unwrap();
    // let mut file = File::create("results/strategy.json").unwrap();
    // file.write_all(result_json.as_bytes()).unwrap();
    // // Serialize and save the actions
    // let actions_str: Vec<String> = actions.iter().map(|action| format!("{:?}", action)).collect();
    // let actions_json = serde_json::to_string_pretty(&actions_str).unwrap();
    // let mut file = File::create("results/actions.json").unwrap();
    // file.write_all(actions_json.as_bytes()).unwrap();
    // println!("Result and Action Saved");

    // println!("{:?}", holes_to_strings(game.private_cards(0)));
    // println!("{:?}", game.available_actions());
    // println!("{:?}", game.strategy());
    // println!("{:?}", holes_to_strings(game.private_cards(0)));
    // println!("{:?}", game.available_actions());
    // println!("{:?}", game.strategy());

    // // Define the chunk size
    // let chunk_size = cards.len();
    // // Initialize a vector to store the sums
    // let mut sums = vec![0.0; chunk_size];
    // // Compute the sums
    // for i in 0..chunk_size {
    //     sums[i] = strategy[i] + strategy[i + chunk_size];
    // }
    // // Print the sums
    // for (i, sum) in sums.iter().enumerate() {
    //     println!("Sum for index {}: {:.4}", i, sum);
    // }

    // confirm that the current node is a chance node (i.e., river node)
    assert!(game.is_chance_node());

    // confirm that "7s" can be dealt
    let card_7s = card_from_str("7s").unwrap();
    assert!(game.possible_cards() & (1 << card_7s) != 0);

    // deal "7s"
    game.play(card_7s as usize);

    let strategy = game.strategy();
    // println!("Strategy:");
    // for (i, value) in strategy.iter().enumerate() {
    //     println!("Action {}: {:.4}", i, value);
    // }

    let test_range = "22";
    let parsed_test_range = test_range.parse::<Range>().unwrap();

    // println!("Parsed Test Range: {:?}", parsed_test_range.raw_data());
    // println!("Parsed Test Range Length: {:?}", parsed_test_range.raw_data().len());

    // Find the index where the value is 1.0
    let indices: Vec<_> = parsed_test_range.raw_data().iter()
    .enumerate()
    .filter_map(|(index, &v)| if v == 1.0 { Some(index) } else { None })
    .collect();

    // println!("Index with value 1.0: {:?}", indices);

    // println!("{}", test_range.parse::<Range>().unwrap().to_string());

    fn get_hand_index(hand: &str) -> Result<usize, String> {
        // Split the hand string into two card strings
        if hand.len() != 4 {
            return Err("Invalid hand format".to_string());
        }
    
        let card1_str = &hand[..2];
        let card2_str = &hand[2..];
    
        // Convert the card strings to card indices
        let card1 = card_from_str(card1_str)?;
        let card2 = card_from_str(card2_str)?;
    
        // Convert the card indices to a pair index
        let index = card_pair_to_index(card1, card2);
    
        Ok(index)
    }
    
    fn generate_all_hands() -> Vec<String> {
        let ranks = ["2", "3", "4", "5", "6", "7", "8", "9", "T", "J", "Q", "K", "A"];
        let suits = ["c", "d", "h", "s"];
        let mut hands = Vec::new();
    
        // Generate pairs
        for &rank in &ranks {
            for i in 0..4 {
                for j in i + 1..4 {
                    hands.push(format!("{}{}{}{}", rank, suits[i], rank, suits[j]));
                }
            }
        }
    
        // Generate suited and offsuit hands
        for i in 0..ranks.len() {
            for j in (i + 1)..ranks.len() {
                for k in 0..4 {
                    // Suited
                    hands.push(format!("{}{}{}{}", ranks[i], suits[k], ranks[j], suits[k]));
    
                    // Offsuit
                    for l in 0..4 {
                        if k != l {
                            hands.push(format!("{}{}{}{}", ranks[i], suits[k], ranks[j], suits[l]));
                        }
                    }
                }
            }
        }
    
        hands
    }
    
    let hands = generate_all_hands();
    let mut hand_indices = HashMap::new();

    for hand in &hands {
        match get_hand_index(hand) {
            Ok(index) => {
                hand_indices.insert(hand.clone(), index);
            }
            Err(e) => println!("Error for hand {}: {}", hand, e),
        }
    }

    // Sort the hands based on their indices
    let mut sorted_hands: Vec<_> = hand_indices.iter().collect();
    sorted_hands.sort_by_key(|&(_, index)| index);

    // for (hand, index) in sorted_hands {
    //     println!("Hand: {}, Index: {}", hand, index);
    // }

}
