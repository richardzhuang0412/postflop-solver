use crate::interface::*;
use crate::mutex_like::*;
use crate::sliceop::*;

#[cfg(feature = "custom_alloc")]
use crate::alloc::*;
#[cfg(feature = "custom_alloc")]
use std::vec;

#[cfg(feature = "rayon")]
use rayon::prelude::*;

/// Executes `op` for each child potentially in parallel.
#[cfg(feature = "rayon")]
#[inline]
pub fn for_each_child<T: GameNode, OP: Fn(usize) + Sync + Send>(node: &T, op: OP) {
    if node.enable_parallelization() {
        node.actions().into_par_iter().for_each(op);
    } else {
        node.actions().into_iter().for_each(op);
    }
}

/// Executes `op` for each child.
#[cfg(not(feature = "rayon"))]
#[inline]
pub fn for_each_child<T: GameNode, OP: Fn(usize) + Sync + Send>(node: &T, op: OP) {
    node.actions().into_iter().for_each(op);
}

/// Decodes the encoded `u16` slice to the `f32` slice.
#[cfg(feature = "custom_alloc")]
#[inline]
pub fn decode_unsigned_slice(slice: &[u16], scale: f32) -> Vec<f32, StackAlloc> {
    let decoder = scale / u16::MAX as f32;
    let mut result = Vec::<f32, StackAlloc>::with_capacity_in(slice.len(), StackAlloc);
    let ptr = result.as_mut_ptr();
    unsafe {
        for i in 0..slice.len() {
            *ptr.add(i) = *slice.get_unchecked(i) as f32 * decoder;
        }
        result.set_len(slice.len());
    }
    result
}

/// Decodes the encoded `u16` slice to the `f32` slice.
#[cfg(not(feature = "custom_alloc"))]
#[inline]
pub fn decode_unsigned_slice(slice: &[u16], scale: f32) -> Vec<f32> {
    let decoder = scale / u16::MAX as f32;
    let mut result = Vec::<f32>::with_capacity(slice.len());
    let ptr = result.as_mut_ptr();
    unsafe {
        for i in 0..slice.len() {
            *ptr.add(i) = *slice.get_unchecked(i) as f32 * decoder;
        }
        result.set_len(slice.len());
    }
    result
}

/// Decodes the encoded `i16` slice to the non-negative `f32` slice.
#[cfg(feature = "custom_alloc")]
#[inline]
pub fn decode_signed_slice(slice: &[i16], scale: f32) -> Vec<f32, StackAlloc> {
    let decoder = scale / i16::MAX as f32;
    let mut result = Vec::<f32, StackAlloc>::with_capacity_in(slice.len(), StackAlloc);
    let ptr = result.as_mut_ptr();
    unsafe {
        for i in 0..slice.len() {
            *ptr.add(i) = (*slice.get_unchecked(i)).max(0) as f32 * decoder;
        }
        result.set_len(slice.len());
    }
    result
}

/// Decodes the encoded `i16` slice to the non-negative `f32` slice.
#[cfg(not(feature = "custom_alloc"))]
#[inline]
pub fn decode_signed_slice(slice: &[i16], scale: f32) -> Vec<f32> {
    let decoder = scale / i16::MAX as f32;
    let mut result = Vec::<f32>::with_capacity(slice.len());
    let ptr = result.as_mut_ptr();
    unsafe {
        for i in 0..slice.len() {
            *ptr.add(i) = (*slice.get_unchecked(i)).max(0) as f32 * decoder;
        }
        result.set_len(slice.len());
    }
    result
}

/// Encodes the `f32` slice to the `i16` slice.
#[inline]
pub fn encode_signed_slice(dst: &mut [i16], slice: &[f32]) -> f32 {
    let scale = slice.iter().fold(0.0f32, |m, v| v.abs().max(m));
    let encoder = i16::MAX as f32 / scale;
    dst.iter_mut()
        .zip(slice)
        .for_each(|(d, s)| *d = (s * encoder).round() as i16);
    scale
}

/// Normalizes the cumulative strategy.
#[inline]
pub fn normalize_strategy<T: Game>(game: &T) {
    if !game.is_ready() {
        panic!("the game is not ready");
    }
    if game.is_compression_enabled() {
        normalize_strategy_compressed_recursive::<T::Node>(&mut game.root());
    } else {
        normalize_strategy_recursive::<T::Node>(&mut game.root());
    }
}

/// Computes the expected values and equity and save them to `cum_regret` field.
/// If the number of actions is equal to `1`, the equity is saved to `strategy` field.
#[inline]
pub fn compute_ev_and_equity<T: Game>(game: &T) {
    if !game.is_ready() {
        panic!("the game is not ready");
    }
    let reach = [game.initial_weight(0), game.initial_weight(1)];
    for player in 0..2 {
        let mut ev = vec![0.0; game.num_private_hands(player)];
        let mut eq = vec![0.0; game.num_private_hands(player)];
        compute_ev_and_equity_recursive(
            &mut ev,
            &mut eq,
            game,
            &mut game.root(),
            player,
            reach[player],
            reach[player ^ 1],
        );
    }
}

/// Computes the expected value of the root node.
#[inline]
pub fn get_root_ev<T: Game>(game: &T) -> f32 {
    if !game.is_ready() {
        panic!("the game is not ready");
    }
    let root = game.root();
    let num_private_hands = game.num_private_hands(root.player());
    let get_sum = |evs: &[f32]| evs.iter().fold(0.0, |sum, v| sum + *v as f64) as f32;
    if game.is_compression_enabled() {
        let slice = row(root.cum_regret_compressed(), 0, num_private_hands);
        let decoder = root.cum_regret_scale() / i16::MAX as f32;
        let vec = slice
            .iter()
            .map(|&v| v as f32 * decoder)
            .collect::<Vec<_>>();
        get_sum(&vec)
    } else {
        get_sum(row(root.cum_regret(), 0, num_private_hands))
    }
}

/// Computes the equity of the root node.
#[inline]
pub fn get_root_equity<T: Game>(game: &T) -> f32 {
    if !game.is_ready() {
        panic!("the game is not ready");
    }
    let root = game.root();
    let num_actions = root.num_actions();
    let num_private_hands = game.num_private_hands(root.player());
    let get_sum = |eqs: &[f32]| eqs.iter().fold(0.0, |sum, v| sum + *v as f64) as f32;
    if game.is_compression_enabled() {
        let decoder = root.equity_scale() / i16::MAX as f32;
        let vec = if num_actions == 1 {
            root.strategy_compressed()
                .iter()
                .map(|&v| v as i16 as f32 * decoder)
                .collect::<Vec<_>>()
        } else {
            row(root.cum_regret_compressed(), 1, num_private_hands)
                .iter()
                .map(|&v| v as f32 * decoder)
                .collect::<Vec<_>>()
        };
        get_sum(&vec)
    } else if num_actions == 1 {
        get_sum(root.strategy())
    } else {
        get_sum(row(root.cum_regret(), 1, num_private_hands))
    }
}

/// Computes the exploitability of the strategy.
#[inline]
pub fn compute_exploitability<T: Game>(game: &T, is_normalized: bool) -> f32 {
    if !game.is_ready() {
        panic!("the game is not ready");
    }
    let mut cfv = [
        vec![0.0; game.num_private_hands(0)],
        vec![0.0; game.num_private_hands(1)],
    ];
    let reach = [game.initial_weight(0), game.initial_weight(1)];
    for player in 0..2 {
        compute_best_cfv_recursive(
            &mut cfv[player],
            game,
            &game.root(),
            player,
            reach[player ^ 1],
            is_normalized,
        );
    }
    let get_sum = |player: usize| {
        cfv[player]
            .iter()
            .zip(reach[player])
            .fold(0.0, |sum, (&cfv, &reach)| sum + cfv as f64 * reach as f64)
    };
    (get_sum(0) + get_sum(1)) as f32 / 2.0
}

/// The recursive helper function for normalizing the strategy.
fn normalize_strategy_recursive<T: GameNode>(node: &mut T) {
    if node.is_terminal() {
        return;
    }

    if !node.is_chance() {
        let num_actions = node.num_actions();
        let strategy = node.strategy_mut();
        let row_size = strategy.len() / num_actions;

        #[cfg(feature = "custom_alloc")]
        let mut denom = vec::from_elem_in(0.0, row_size, StackAlloc);
        #[cfg(not(feature = "custom_alloc"))]
        let mut denom = vec![0.0; row_size];
        strategy.chunks(row_size).for_each(|row| {
            add_slice(&mut denom, row);
        });

        let default = 1.0 / num_actions as f32;
        strategy.chunks_mut(row_size).for_each(|row| {
            div_slice(row, &denom, default);
        });
    }

    for_each_child(node, |action| {
        normalize_strategy_recursive::<T>(&mut node.play(action));
    })
}

/// The recursive helper function for normalizing the strategy.
fn normalize_strategy_compressed_recursive<T: GameNode>(node: &mut T) {
    if node.is_terminal() {
        return;
    }

    if !node.is_chance() {
        let num_actions = node.num_actions();
        let strategy = node.strategy_compressed_mut();
        let row_size = strategy.len() / num_actions;

        #[cfg(feature = "custom_alloc")]
        let mut denom = vec::from_elem_in(0, row_size, StackAlloc);
        #[cfg(not(feature = "custom_alloc"))]
        let mut denom = vec![0; row_size];
        strategy.chunks(row_size).for_each(|row| {
            denom.iter_mut().zip(row).for_each(|(d, v)| {
                *d += *v as u32;
            });
        });

        let default = ((u16::MAX as usize + num_actions / 2) / num_actions) as u16;
        strategy.chunks_mut(row_size).for_each(|row| {
            row.iter_mut().zip(denom.iter()).for_each(|(v, d)| {
                *v = if *d == 0 {
                    default
                } else {
                    ((u16::MAX as u64 * *v as u64 + *d as u64 / 2) / *d as u64) as u16
                };
            });
        });

        node.set_strategy_scale(1.0);
    }

    for_each_child(node, |action| {
        normalize_strategy_compressed_recursive::<T>(&mut node.play(action));
    })
}

/// The recursive helper function for computing the expected value and equity.
fn compute_ev_and_equity_recursive<T: Game>(
    ev: &mut [f32],
    eq: &mut [f32],
    game: &T,
    node: &mut T::Node,
    player: usize,
    reach: &[f32],
    cfreach: &[f32],
) {
    // terminal node
    if node.is_terminal() {
        game.evaluate(ev, node, player, cfreach, false);
        game.evaluate(eq, node, player, cfreach, true);
        mul_slice(ev, reach);
        mul_slice(eq, reach);
        return;
    }

    let num_actions = node.num_actions();
    let num_private_hands = game.num_private_hands(player);

    // allocates memory for storing the expected values and equity
    #[cfg(feature = "custom_alloc")]
    let ev_actions = MutexLike::new(vec::from_elem_in(
        0.0,
        num_actions * num_private_hands,
        StackAlloc,
    ));
    #[cfg(feature = "custom_alloc")]
    let eq_actions = MutexLike::new(vec::from_elem_in(
        0.0,
        num_actions * num_private_hands,
        StackAlloc,
    ));
    #[cfg(not(feature = "custom_alloc"))]
    let ev_actions = MutexLike::new(vec![0.0; num_actions * num_private_hands]);
    #[cfg(not(feature = "custom_alloc"))]
    let eq_actions = MutexLike::new(vec![0.0; num_actions * num_private_hands]);

    // chance node
    if node.is_chance() {
        // use 64-bit floating point values
        #[cfg(feature = "custom_alloc")]
        let mut ev_f64 = vec::from_elem_in(0.0, num_private_hands, StackAlloc);
        #[cfg(feature = "custom_alloc")]
        let mut eq_f64 = vec::from_elem_in(0.0, num_private_hands, StackAlloc);
        #[cfg(not(feature = "custom_alloc"))]
        let mut ev_f64 = vec![0.0; num_private_hands];
        #[cfg(not(feature = "custom_alloc"))]
        let mut eq_f64 = vec![0.0; num_private_hands];

        // updates the reach probabilities
        #[cfg(feature = "custom_alloc")]
        let mut cfreach = cfreach.to_vec_in(StackAlloc);
        #[cfg(not(feature = "custom_alloc"))]
        let mut cfreach = cfreach.to_vec();
        mul_slice_scalar(&mut cfreach, node.chance_factor());

        // computes the expected values and equity of each action
        for_each_child(node, |action| {
            compute_ev_and_equity_recursive(
                row_mut(&mut ev_actions.lock(), action, num_private_hands),
                row_mut(&mut eq_actions.lock(), action, num_private_hands),
                game,
                &mut node.play(action),
                player,
                reach,
                &cfreach,
            );
        });

        // sums up the expected values and equity
        let mut ev_actions = ev_actions.lock();
        let mut eq_actions = eq_actions.lock();
        ev_actions.chunks(num_private_hands).for_each(|row| {
            ev_f64.iter_mut().zip(row).for_each(|(l, &r)| {
                *l += r as f64;
            });
        });
        eq_actions.chunks(num_private_hands).for_each(|row| {
            eq_f64.iter_mut().zip(row).for_each(|(l, &r)| {
                *l += r as f64;
            });
        });

        // get information about isomorphic chances
        let isomorphic_chances = game.isomorphic_chances(node);

        // processes isomorphic chances
        for i in 0..isomorphic_chances.len() {
            let ev_tmp = row_mut(&mut ev_actions, isomorphic_chances[i], num_private_hands);
            let eq_tmp = row_mut(&mut eq_actions, isomorphic_chances[i], num_private_hands);
            for &(i, j) in &game.isomorphic_swap(node, i)[player] {
                ev_tmp.swap(i, j);
                eq_tmp.swap(i, j);
            }
            ev_f64.iter_mut().zip(&*ev_tmp).for_each(|(l, &r)| {
                *l += r as f64;
            });
            eq_f64.iter_mut().zip(&*eq_tmp).for_each(|(l, &r)| {
                *l += r as f64;
            });
            for &(i, j) in &game.isomorphic_swap(node, i)[player] {
                ev_tmp.swap(i, j);
                eq_tmp.swap(i, j);
            }
        }

        ev.iter_mut().zip(&ev_f64).for_each(|(l, &r)| {
            *l = r as f32;
        });
        eq.iter_mut().zip(&eq_f64).for_each(|(l, &r)| {
            *l = r as f32;
        });
    }
    // player node
    else if node.player() == player {
        // updates the reach probabilities
        let mut reach_actions = if game.is_compression_enabled() {
            let strategy = node.strategy_compressed();
            let scale = node.strategy_scale();
            decode_unsigned_slice(strategy, scale)
        } else {
            #[cfg(feature = "custom_alloc")]
            {
                node.strategy().to_vec_in(StackAlloc)
            }
            #[cfg(not(feature = "custom_alloc"))]
            {
                node.strategy().to_vec()
            }
        };

        let row_size = reach_actions.len() / node.num_actions();
        reach_actions.chunks_mut(row_size).for_each(|row| {
            mul_slice(row, reach);
        });

        // computes the expected values and equity of each action
        for_each_child(node, |action| {
            compute_ev_and_equity_recursive(
                row_mut(&mut ev_actions.lock(), action, num_private_hands),
                row_mut(&mut eq_actions.lock(), action, num_private_hands),
                game,
                &mut node.play(action),
                player,
                row(&reach_actions, action, row_size),
                cfreach,
            );
        });

        // sums up the expected values and equity
        let ev_actions = ev_actions.lock();
        let eq_actions = eq_actions.lock();
        ev_actions.chunks(num_private_hands).for_each(|row| {
            add_slice(ev, row);
        });
        eq_actions.chunks(num_private_hands).for_each(|row| {
            add_slice(eq, row);
        });

        // save to `cum_regret` field (and `strategy` field if there is only one action)
        if game.is_compression_enabled() {
            let ev_scale = encode_signed_slice(node.cum_regret_compressed_mut(), ev);
            let eq_scale = if num_actions == 1 {
                let eq_scale = eq.iter().fold(0.0f32, |m, v| v.abs().max(m));
                let encoder = i16::MAX as f32 / eq_scale;
                node.strategy_compressed_mut()
                    .iter_mut()
                    .zip(eq)
                    .for_each(|(l, r)| {
                        *l = (*r * encoder).round() as i16 as u16;
                    });
                eq_scale
            } else {
                encode_signed_slice(
                    row_mut(node.cum_regret_compressed_mut(), 1, num_private_hands),
                    eq,
                )
            };
            node.set_cum_regret_scale(ev_scale);
            node.set_equity_scale(eq_scale);
        } else {
            row_mut(node.cum_regret_mut(), 0, num_private_hands).clone_from_slice(ev);
            if num_actions == 1 {
                node.strategy_mut().clone_from_slice(eq)
            } else {
                row_mut(node.cum_regret_mut(), 1, num_private_hands).clone_from_slice(eq);
            }
        }
    }
    // opponent node
    else if num_actions == 1 {
        // do not use `strategy` field when there is only one action
        // (because it may be overwritten to the equity)
        compute_ev_and_equity_recursive(ev, eq, game, &mut node.play(0), player, reach, cfreach);
    } else {
        let mut cfreach_actions = if game.is_compression_enabled() {
            let strategy = node.strategy_compressed();
            let scale = node.strategy_scale();
            decode_unsigned_slice(strategy, scale)
        } else {
            #[cfg(feature = "custom_alloc")]
            {
                node.strategy().to_vec_in(StackAlloc)
            }
            #[cfg(not(feature = "custom_alloc"))]
            {
                node.strategy().to_vec()
            }
        };

        // updates the reach probabilities
        let row_size = cfreach_actions.len() / node.num_actions();
        cfreach_actions.chunks_mut(row_size).for_each(|row| {
            mul_slice(row, cfreach);
        });

        // computes the expected values and equity of each action
        for_each_child(node, |action| {
            compute_ev_and_equity_recursive(
                row_mut(&mut ev_actions.lock(), action, num_private_hands),
                row_mut(&mut eq_actions.lock(), action, num_private_hands),
                game,
                &mut node.play(action),
                player,
                reach,
                row(&cfreach_actions, action, row_size),
            );
        });

        // sums up the expected values and equity
        let ev_actions = ev_actions.lock();
        let eq_actions = eq_actions.lock();
        ev_actions.chunks(num_private_hands).for_each(|row| {
            add_slice(ev, row);
        });
        eq_actions.chunks(num_private_hands).for_each(|row| {
            add_slice(eq, row);
        });
    }
}

/// The recursive helper function for computing the counterfactual values of best response.
fn compute_best_cfv_recursive<T: Game>(
    result: &mut [f32],
    game: &T,
    node: &T::Node,
    player: usize,
    cfreach: &[f32],
    is_normalized: bool,
) {
    // terminal node
    if node.is_terminal() {
        game.evaluate(result, node, player, cfreach, false);
        return;
    }

    let num_actions = node.num_actions();
    let num_private_hands = game.num_private_hands(player);

    // allocates memory for storing the counterfactual values
    #[cfg(feature = "custom_alloc")]
    let cfv_actions = MutexLike::new(vec::from_elem_in(
        0.0,
        num_actions * num_private_hands,
        StackAlloc,
    ));
    #[cfg(not(feature = "custom_alloc"))]
    let cfv_actions = MutexLike::new(vec![0.0; num_actions * num_private_hands]);

    // chance node
    if node.is_chance() {
        // use 64-bit floating point values
        #[cfg(feature = "custom_alloc")]
        let mut result_f64 = vec::from_elem_in(0.0, num_private_hands, StackAlloc);
        #[cfg(not(feature = "custom_alloc"))]
        let mut result_f64 = vec![0.0; num_private_hands];

        // updates the reach probabilities
        #[cfg(feature = "custom_alloc")]
        let mut cfreach = cfreach.to_vec_in(StackAlloc);
        #[cfg(not(feature = "custom_alloc"))]
        let mut cfreach = cfreach.to_vec();
        mul_slice_scalar(&mut cfreach, node.chance_factor());

        // computes the counterfactual values of each action
        for_each_child(node, |action| {
            compute_best_cfv_recursive(
                row_mut(&mut cfv_actions.lock(), action, num_private_hands),
                game,
                &node.play(action),
                player,
                &cfreach,
                is_normalized,
            )
        });

        // sums up the counterfactual values
        let mut cfv_actions = cfv_actions.lock();
        cfv_actions.chunks(num_private_hands).for_each(|row| {
            result_f64.iter_mut().zip(row).for_each(|(r, v)| {
                *r += *v as f64;
            });
        });

        // get information about isomorphic chances
        let isomorphic_chances = game.isomorphic_chances(node);

        // processes isomorphic chances
        for i in 0..isomorphic_chances.len() {
            let tmp = row_mut(&mut cfv_actions, isomorphic_chances[i], num_private_hands);
            for &(i, j) in &game.isomorphic_swap(node, i)[player] {
                tmp.swap(i, j);
            }
            result_f64.iter_mut().zip(&*tmp).for_each(|(r, &v)| {
                *r += v as f64;
            });
            for &(i, j) in &game.isomorphic_swap(node, i)[player] {
                tmp.swap(i, j);
            }
        }

        result.iter_mut().zip(&result_f64).for_each(|(r, &v)| {
            *r = v as f32;
        });
    }
    // player node
    else if node.player() == player {
        // computes the counterfactual values of each action
        for_each_child(node, |action| {
            compute_best_cfv_recursive(
                row_mut(&mut cfv_actions.lock(), action, num_private_hands),
                game,
                &node.play(action),
                player,
                cfreach,
                is_normalized,
            )
        });

        // computes element-wise maximum (takes the best response)
        result.fill(f32::NEG_INFINITY);
        let cfv_actions = cfv_actions.lock();
        cfv_actions.chunks(num_private_hands).for_each(|row| {
            result.iter_mut().zip(row).for_each(|(l, r)| {
                *l = l.max(*r);
            });
        });
    }
    // opponent node
    else {
        let mut cfreach_actions = if game.is_compression_enabled() {
            let strategy = node.strategy_compressed();
            let scale = node.strategy_scale();
            decode_unsigned_slice(strategy, scale)
        } else {
            #[cfg(feature = "custom_alloc")]
            {
                node.strategy().to_vec_in(StackAlloc)
            }
            #[cfg(not(feature = "custom_alloc"))]
            {
                node.strategy().to_vec()
            }
        };

        let row_size = cfreach_actions.len() / node.num_actions();

        // if the strategy is not normalized, we need to normalize it
        if !is_normalized {
            #[cfg(feature = "custom_alloc")]
            let mut denom = vec::from_elem_in(0.0, row_size, StackAlloc);
            #[cfg(not(feature = "custom_alloc"))]
            let mut denom = vec![0.0; row_size];
            cfreach_actions.chunks(row_size).for_each(|row| {
                add_slice(&mut denom, row);
            });

            let default = 1.0 / node.num_actions() as f32;
            cfreach_actions.chunks_mut(row_size).for_each(|row| {
                div_slice(row, &denom, default);
            });
        }

        // updates the reach probabilities
        cfreach_actions.chunks_mut(row_size).for_each(|row| {
            mul_slice(row, cfreach);
        });

        // computes the counterfactual values of each action
        for_each_child(node, |action| {
            let cfreach = row(&cfreach_actions, action, row_size);
            if cfreach.iter().any(|&x| x > 0.0) {
                compute_best_cfv_recursive(
                    row_mut(&mut cfv_actions.lock(), action, num_private_hands),
                    game,
                    &node.play(action),
                    player,
                    cfreach,
                    is_normalized,
                );
            }
        });

        // sums up the counterfactual values
        let cfv_actions = cfv_actions.lock();
        cfv_actions.chunks(num_private_hands).for_each(|row| {
            add_slice(result, row);
        });
    }
}
