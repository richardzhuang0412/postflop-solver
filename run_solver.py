import pandas as pd
import numpy as np
import subprocess, os, json, sys
from tqdm import tqdm

def get_range(preflop_ranges, scenario, threshold=0.5):
    ranges = preflop_ranges[scenario][0]
    pairs = ranges.split(',')
    data_dict = {key_value.split(':')[0]: float(key_value.split(':')[1]) for key_value in pairs}
    keys_above_threshold = [key for key, value in data_dict.items() if value > threshold]
    result_string = ','.join(keys_above_threshold)

    return result_string

def calculate_pot_size(action_str, starting_stack):
    known_positions = ["UTG", "HJ", "CO", "BTN", "SB", "BB"]
    players_contrib = {}
    current_bet = 0
    actions = action_str.split('/')

    # Handle pre-flop standard contributions: SB = 0.5 BB, BB = 1 BB
    players_contrib['SB'] = 0.5
    players_contrib['BB'] = 1

    # Iterate through each action in the string
    i = 0
    while i < len(actions):
        if sum([pos in actions[i] for pos in known_positions]):
            i += 1
            continue
        if 'call' in actions[i]:
            player = actions[i-1]
            # Player calls the current bet
            amount_to_add = current_bet - players_contrib.get(player, 0)
            players_contrib[player] = players_contrib.get(player, 0) + amount_to_add
            i += 1  # Skip the 'call' keyword
        elif 'allin' in actions[i]:
            player = actions[i-1]
            # All-in is considered as 100 bb
            players_contrib[player] = 100
            current_bet = max(current_bet, 100)
            i += 1  # Skip the 'allin' keyword
        elif 'fold' in actions[i]:
            # On fold, do nothing as the player's current contribution remains
            i += 1  # Skip the 'fold' keyword
        elif 'check' in actions[i]:
            # On check, no changes to contributions
            i += 1  # Skip the 'check' keyword
        else:
            # This is a betting action
            player, bet_str = actions[i-1], actions[i]
            bet_amount = float(bet_str.replace('bb', ''))
            players_contrib[player] = bet_amount
            current_bet = max(current_bet, bet_amount)
            i += 1  # Move to next token which should be the action or next player

        i += 1  # General increment to move to the next token

    # Calculate the total pot size by summing contributions
    total_pot = sum(players_contrib.values())
    effective_stack = starting_stack - max(players_contrib.values())
    return int(total_pot), int(effective_stack)

def run_solver(flop, turn, river, oop_range, ip_range, preflop_line, 
               flop_bet_sizes, starting_pot, effective_stack, folder_path,
               system_output_file=None):
    
    if not os.path.exists(folder_path):
        os.makedirs(folder_path)
    
    command = [
        'cargo', 'run', '--release', '--example', 'solve',
        '--', '--flop', flop,
        '--turn', turn,
        '--river', river,
        '--oop-range', oop_range,
        '--ip-range', ip_range,
        '--flop-bet-sizes', flop_bet_sizes,
        '--starting-pot', str(starting_pot),
        '--effective-stack', str(effective_stack),
        '--folder-path', folder_path,
    ]

    # Execute the command
    if system_output_file:
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        # Read and print stdout line by line in real time
        for line in process.stdout:
            if "iteration" not in line:
                if "Result and Action Saved" not in line:
                    print(line, end='')  # Print to terminal
                system_output_file.write(line)  # Write to file

        # Wait for the process to complete and capture stderr
        process.wait()
        stderr_output = process.stderr.read()
        
        # Print and write stderr to the file
        # if stderr_output:
        #     # print(stderr_output, end='')
        #     system_output_file.write(stderr_output)
    else:
        result = subprocess.run(command)
    
    # system_output_file.write(result.stdout)
    # system_output_file.write(result.stderr)
    # print(result.stdout)
    # print(result.stderr)

    # Print the output
    # print(result.stdout)
    # if result.stderr:
    #     print(result.stderr)

if __name__ == "__main__":
    output_file_path = "trial_1.txt"
    with open(output_file_path, "w") as system_output_file:
        pass

    #TODO: Change path if necessary
    preflop_ranges_path = "data/preflop_ranges.json"
    with open(preflop_ranges_path, 'r') as file:
        preflop_ranges = json.load(file)
    scenario_list = pd.read_csv("data/scenario_list.csv")
    board_samples = pd.read_csv("data/board_samples_new.csv")
    flop_size_map_path = "data/flop_size_map.json"
    with open(flop_size_map_path, 'r') as file:
        flop_size_map = json.load(file)
    # print(flop_size_map)

    #TODO: Change index
    scenario_start_index = 0
    scenario_end_index = 1
    # scenario_end_index = scenario_list.shape[0]
    for i in range(scenario_start_index, scenario_end_index):
        scenario_ip = scenario_list.iloc[i]['IP']
        scenario_oop = scenario_list.iloc[i]['OOP']
        scenario = scenario_list.iloc[i]['Scenario']
        oop_range = get_range(preflop_ranges=preflop_ranges, scenario=scenario_oop, threshold=0.5)
        ip_range = get_range(preflop_ranges=preflop_ranges, scenario=scenario_ip, threshold=0.5)
        for j in range(board_samples.shape[0]):
            print("------------------------------------------------------------------------------------")
            texture = board_samples.iloc[j]["Texture"]
            flop = board_samples.iloc[j]["Flop"].replace(",", "")
            turn = board_samples.iloc[j]["Turn"]
            river = board_samples.iloc[j]["River"]
            flop_bet_size = flop_size_map[texture]
            starting_pot, effective_stack = calculate_pot_size(scenario, 100)
            folder_path = f"results/{scenario.replace('/', '_')}/{flop}_{turn}_{river}"
            with open(output_file_path, "a") as system_output_file:
                print(f"Preflop Line: {scenario}, Board: {flop},{turn},{river}, Texture: {texture}")
                system_output_file.write(f"Preflop Line: {scenario}, Board: {flop},{turn},{river}, Texture: {texture}\n")
                print(f"Flop Bet Size: {flop_bet_size}, Starting Pot: {starting_pot}, Effective Stack: {effective_stack}")
                system_output_file.write(f"Flop Bet Size: {flop_bet_size}, Starting Pot: {starting_pot}, Effective Stack: {effective_stack}\n")
                if not os.path.exists(f"results/{scenario.replace('/', '_')}"):
                    os.makedirs(f"results/{scenario.replace('/', '_')}")
                run_solver(flop, turn, river, oop_range, ip_range, scenario, flop_bet_size, starting_pot,
                effective_stack, folder_path, system_output_file=system_output_file)
                print(f"Results Solved and Saved to {folder_path}")
                system_output_file.write(f"Results Solved and Saved to {folder_path}\n")
                system_output_file.write("------------------------------------------------------------------------------------\n")
            # break
        # break

        # flop = "AdKs7h"
        # turn = "2c"
        # river = "5d"
        # oop_range = "99-22,ATs-A2s,AJo-A7o,A5o,KJs-K2s,K9o+,Q2s+,Q9o+,J3s+,J9o+,T5s+,T9o,96s+,85s+,74s+,63s+,52s+,42s+"
        # ip_range = "22+,A2s+,A4o+,K2s+,K8o+,Q3s+,Q9o+,J4s+,J9o+,T6s+,T8o+,96s+,98o,86s+,75s+,65s,54s" 
        # preflop_line = "btn_2.5bb_bb_call"
        # flop_bet_sizes = "125%"
        # starting_pot = 5
        # effective_stack = 100
        # folder_path = f"results/{preflop_line}_{flop}_{turn}_{river}"

        # run_solver(flop, turn, river, oop_range, ip_range, preflop_line, flop_bet_sizes, starting_pot,
        #            effective_stack, folder_path)
    sys.stdout = sys.__stdout__
    sys.stderr = sys.__stderr__
    print("System Output Stored")