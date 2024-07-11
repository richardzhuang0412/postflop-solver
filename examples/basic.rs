use postflop_solver::*;
use postflop_solver::card::{card_pair_to_index};
use std::collections::HashMap;

fn main() {
    // ranges of OOP and IP in string format
    // see the documentation of `Range` for more details about the format
    let oop_range = "66+,A8s+,A5s-A4s,AJo+,K9s+,KQo,QTs+,JTs,96s+,85s+,75s+,65s,54s";
    let ip_range = "QQ-22,AQs-A2s,ATo+,K5s+,KJo+,Q8s+,J8s+,T7s+,96s+,86s+,75s+,64s+,53s+";

    let card_config = CardConfig {
        range: [oop_range.parse().unwrap(), ip_range.parse().unwrap()],
        flop: flop_from_str("3d7d6h").unwrap(),
        turn: card_from_str("Qc").unwrap(),
        river: NOT_DEALT,
    };

    // bet sizes -> 60% of the pot, geometric size, and all-in
    // raise sizes -> 2.5x of the previous bet
    // see the documentation of `BetSizeOptions` for more details
    let bet_sizes = BetSizeOptions::try_from(("60%, e, a", "2.5x")).unwrap();

    let tree_config = TreeConfig {
        initial_state: BoardState::Turn, // must match `card_config`
        starting_pot: 200,
        effective_stack: 900,
        rake_rate: 0.0,
        rake_cap: 0.0,
        flop_bet_sizes: [bet_sizes.clone(), bet_sizes.clone()], // [OOP, IP]
        turn_bet_sizes: [bet_sizes.clone(), bet_sizes.clone()],
        river_bet_sizes: [bet_sizes.clone(), bet_sizes],
        turn_donk_sizes: None, // use default bet sizes
        river_donk_sizes: Some(DonkSizeOptions::try_from("50%").unwrap()),
        add_allin_threshold: 1.5, // add all-in if (maximum bet size) <= 1.5x pot
        force_allin_threshold: 0.15, // force all-in if (SPR after the opponent's call) <= 0.15
        merging_threshold: 0.1,
    };

    // build the game tree
    // `ActionTree` can be edited manually after construction
    let action_tree = ActionTree::new(tree_config).unwrap();
    let mut game = PostFlopGame::with_config(card_config, action_tree).unwrap();

    // obtain the private hands
    let oop_cards = game.private_cards(0);
    let oop_cards_str = holes_to_strings(oop_cards).unwrap();
    assert_eq!(
        &oop_cards_str[..10],
        &["5c4c", "Ac4c", "5d4d", "Ad4d", "5h4h", "Ah4h", "5s4s", "As4s", "6c5c", "7c5c"]
    );

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
    game.allocate_memory(false);

    // allocate memory with compression (use 16-bit integer)
    // game.allocate_memory(true);

    // solve the game
    let max_num_iterations = 1000;
    let target_exploitability = game.tree_config().starting_pot as f32 * 0.005; // 0.5% of the pot
    let exploitability = solve(&mut game, max_num_iterations, target_exploitability, true);
    println!("Exploitability: {:.2}", exploitability);

    // solve the game manually
    // for i in 0..max_num_iterations {
    //     solve_step(&game, i);
    //     if (i + 1) % 10 == 0 {
    //         let exploitability = compute_exploitability(&game);
    //         if exploitability <= target_exploitability {
    //             println!("Exploitability: {:.2}", exploitability);
    //             break;
    //         }
    //     }
    // }
    // finalize(&mut game);

    // get equity and EV of a specific hand
    game.cache_normalized_weights();
    let equity = game.equity(0); // `0` means OOP player
    let ev = game.expected_values(0);
    println!("Equity of oop_hands[0]: {:.2}%", 100.0 * equity[0]);
    println!("EV of oop_hands[0]: {:.2}", ev[0]);

    // get equity and EV of whole hand
    let weights = game.normalized_weights(0);
    let average_equity = compute_average(&equity, weights);
    let average_ev = compute_average(&ev, weights);
    // println!("Average equity: {:.2}%", 100.0 * average_equity);
    // println!("Average EV: {:.2}", average_ev);

    // get available actions (OOP)
    let actions = game.available_actions();
    assert_eq!(
        format!("{:?}", actions),
        "[Check, Bet(120), Bet(216), AllIn(900)]"
    );

    // play `Bet(120)`
    println!("{:?}", holes_to_strings(game.private_cards(0)));
    println!("{:?}", game.available_actions());
    println!("{:?}", game.strategy());
    game.play(1);


    // get available actions (IP)
    let actions = game.available_actions();
    assert_eq!(format!("{:?}", actions), "[Fold, Call, Raise(300)]");

    // confirm that IP does not fold the nut straight
    let ip_cards = game.private_cards(1);
    let strategy = game.strategy();
    assert_eq!(ip_cards.len(), 250);
    assert_eq!(strategy.len(), 750);

    let ksjs = holes_to_strings(ip_cards)
        .unwrap()
        .iter()
        .position(|s| s == "KsJs")
        .unwrap();

    // strategy[index] => Fold
    // strategy[index + ip_cards.len()] => Call
    // strategy[index + 2 * ip_cards.len()] => Raise(300)
    assert_eq!(strategy[ksjs], 0.0);
    assert!((strategy[ksjs] + strategy[ksjs + 250] + strategy[ksjs + 500] - 1.0).abs() < 1e-6);

    // play `Call`
    game.play(1);

    // confirm that the current node is a chance node (i.e., river node)
    assert!(game.is_chance_node());

    // confirm that "7s" can be dealt
    let card_7s = card_from_str("7s").unwrap();
    assert!(game.possible_cards() & (1 << card_7s) != 0);

    // deal "7s"
    game.play(card_7s as usize);
    // println!("{:?}", game.available_actions());
    // println!("{:?}", game.strategy().len());

    // game.play(1);

    // back to the root node
    // game.back_to_root();

    let strategy = game.strategy();
    // println!("Strategy:");
    // for (i, value) in strategy.iter().enumerate() {
    //     println!("Action {}: {:.4}", i, value);
    // }

    let total: f32 = strategy.iter().sum();
    // println!("Sum of all values in strategy: {:.4}", total);
    // println!("{}", action_tree);

    // Define the chunk size
    let chunk_size = 167;

    // Initialize a vector to store the sums
    let mut sums = vec![0.0; chunk_size];

    // Compute the sums
    for i in 0..chunk_size {
        sums[i] = strategy[i] + strategy[i + chunk_size] + strategy[i + 2 * chunk_size];
    }

    // Print the sums
    // for (i, sum) in sums.iter().enumerate() {
    //     println!("Sum for index {}: {:.4}", i, sum);
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
