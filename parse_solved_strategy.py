import json,os,re
import pandas as pd

## Desired Format:
## preflop_action,board_flop,board_turn,flop_type,turn_type,postflop_action,aggressor_position,evaluation_at,pot_size,pot_type,hero_position,holding,available_moves,correct_decision
## UTG/2.0bb/CO/call,"9c,6d,5d",Ts,dynamic,Dry,OOP_BET_2/IP_RAISE_7/CALL/dealcards/Ts/OOP_CHECK/IP_BET_14,OOP,Turn,33,Single-Raised,OOP,TsTh,"['CALL', 'RAISE 93', 'FOLD']",CALL
## HJ/2.0bb/BB/call,"8s,3d,9d",2h,dynamic,Wet,OOP_CHECK/IP_CHECK/dealcards/2h/OOP_CHECK/IP_BET_4,IP,Turn,9,Single-Raised,OOP,Jc3c,"['CALL', 'RAISE 12', 'FOLD']",FOLD
def calculate_preflop_pot_size(action_str):
    known_positions = ["UTG", "HJ", "CO", "BTN", "SB", "BB"]
    players_contrib = {}
    current_bet = 0
    actions = action_str.split('/')

    players_contrib['SB'] = 0.5
    players_contrib['BB'] = 1

    i = 0
    while i < len(actions):
        if sum([pos in actions[i] for pos in known_positions]):
            i += 1
            continue
        if 'call' in actions[i]:
            player = actions[i-1]
            amount_to_add = current_bet - players_contrib.get(player, 0)
            players_contrib[player] = players_contrib.get(player, 0) + amount_to_add
            i += 1
        elif 'allin' in actions[i]:
            player = actions[i-1]
            players_contrib[player] = 100
            current_bet = max(current_bet, 100)
            i += 1
        elif 'fold' in actions[i]:
            i += 1
        elif 'check' in actions[i]:
            i += 1
        else:
            player, bet_str = actions[i-1], actions[i]
            bet_amount = float(bet_str.replace('bb', ''))
            players_contrib[player] = bet_amount
            current_bet = max(current_bet, bet_amount)
            i += 1

        i += 1

    total_pot = sum(players_contrib.values())
    return int(total_pot)

def parse_preflop_action(preflop_action_string, scenario_list):
    """
    return preflop_action, aggressor_position, preflop_pot_size, pot_type
    - Example: UTG/2.0bb/CO/call,OOP,5,Single-Raised
    """
    preflop_action = preflop_action_string.replace("_","/")
    aggressor_position = scenario_list[scenario_list['Scenario'] == preflop_action]['Aggressor'].iloc[0]
    preflop_pot_size = calculate_preflop_pot_size(preflop_action)
    pot_type = "Single-Raised" if preflop_action.count('bb') == 1 else "3-Bet"
    return preflop_action, aggressor_position, preflop_pot_size, pot_type

def parse_action_json(action_json):
    """
    return postflop_action, evaluation_at, postflop_pot_size, available_moves, hero_position
    - Example: OOP_BET_2/IP_RAISE_7/CALL/dealcards/Ts/OOP_CHECK/IP_BET_14,Turn,33,"['CALL', 'RAISE 93', 'FOLD']",OOP
    """
    print(action_json)
    num_deal_card = sum([len(action) == 2 for action in action_json['parsed_prev_action_line']])
    eval_at_map = {0: "Flop", 1: "Turn", 2: "River"}
    evaluation_at = eval_at_map[num_deal_card]
    available_moves = [action.replace("("," ").replace(")","") if 
                       ("Bet" in action) or ("Raise" in action) or ("All" in action)
                       else action for action in action_json['action']]
    print(evaluation_at, available_moves)
    return

def parse_strategy_json(strategy_json, frequency_threshold=0.5):
    """
    return a list of tuple: (holding, correct_decision) where correct decision is a index that is only present 
    when there is a dominant frequency with frequency higher than "frequency_threshold"
    """
    # print(strategy_json)
    strategy_ls = []
    for strategy in strategy_json:
        hand_frequencies = [float(freq) for freq in strategy['frequencies']]
        if any([freq > frequency_threshold for freq in hand_frequencies]):
            correct_decision_index = hand_frequencies.index(max(hand_frequencies))
            strategy_ls.append((strategy['hand'], correct_decision_index))
    return strategy_ls

def group_action_strategy_files(directory_path):
    files = os.listdir(directory_path)
    grouped_files = {}

    # Regular expressions to match action and strategy files
    action_pattern = re.compile(r'actions_(\d+)\.json')
    strategy_pattern = re.compile(r'strategy_(\d+)\.json')

    for file in files:
        action_match = action_pattern.match(file)
        strategy_match = strategy_pattern.match(file)

        if action_match:
            index = int(action_match.group(1))
            if index not in grouped_files:
                grouped_files[index] = {}
            grouped_files[index]['actions'] = os.path.join(directory_path, file)

        if strategy_match:
            index = int(strategy_match.group(1))
            if index not in grouped_files:
                grouped_files[index] = {}
            grouped_files[index]['strategy'] = os.path.join(directory_path, file)

    # Convert dictionary to list of tuples
    grouped_tuples = [(files['actions'], files['strategy']) for index, files in grouped_files.items() if 'actions' in files and 'strategy' in files]
    
    return grouped_tuples

if __name__ == "__main__":
    # Need:
    # preflop_action,board_flop,board_turn,board_river,aggressor_position,
    # postflop_action,evaluation_at,available_moves,pot_size,hero_position,
    # holding,correct_decision

    scenario_list = pd.read_csv("data/scenario_list.csv")

    directory_path = "results/"
    for preflop_action_path in os.listdir(directory_path):
        preflop_action_folder_path = os.path.join(directory_path, preflop_action_path)
        if not os.path.isdir(preflop_action_folder_path):
            continue
        for board_path in os.listdir(preflop_action_folder_path):
            board_folder_path = os.path.join(preflop_action_folder_path, board_path)
            if not os.path.isdir(board_folder_path):
                continue
            preflop_action, aggressor_position, preflop_pot_size, pot_type = parse_preflop_action(preflop_action_path, scenario_list)
            board_flop, board_turn, board_river = board_path.split("_")[0], board_path.split("_")[1], board_path.split("_")[2]
            # print(preflop_action, aggressor_position, preflop_pot_size, pot_type)
            # print(board_flop, board_turn, board_river)
            all_json_tuples = group_action_strategy_files(board_folder_path)
            # print(all_json_tuples)
            for (action_json, strategy_json) in all_json_tuples:
                # postflop_action, evaluation_at, postflop_pot_size, available_moves, hero_position = parse_action_json(action_json)
                # strategy_ls = parse_strategy_json(strategy_json)
                # final_pot_size = preflop_pot_size + postflop_pot_size
                # TODO: Write result as one entry in list
                pass   

    # TODO: Helper function to parse the action json: 
        # return postflop_action, evaluation_at, postflop_pot_size, available_moves, hero_position
    action_json_path = "results/test_run/actions_54.json"
    strategy_json_path = "results/test_run/strategy_54.json"
    with open(action_json_path, 'r') as file:
        action_json = json.load(file)
    with open(strategy_json_path, 'r') as file:
        strategy_json = json.load(file)
    parse_action_json(action_json)
    parse_strategy_json(strategy_json)