# Check the average, max, and min speedup of the latest criterion bench.

# $ cargo bench --bench mainnet -- --noplot
# $ python benches/report.py

# Ideally criterion would give us access to the benchmarked numbers via Rust
# API. They don't, so we must read from the output JSON files. They also don't
# expose the estimate types in Rust so we need to parse it manually. Picking
# Python with no error handling for dev speed and future plotting. We only use
# this for a quick report during performance tuning anyway.

import json
import os

CRITERION_PATH = "target/criterion"

def read_estimate(block, exec_type):
    with open(f"{CRITERION_PATH}/{block}/{exec_type}/new/estimates.json") as f:
        estimates = json.load(f)
        return (estimates["slope"] or estimates["mean"])["point_estimate"]

total_sequential = 0
total_parallel = 0
max_speed_up = 0
min_speed_up = float("inf")

for path in os.listdir(CRITERION_PATH):
    if path.startswith("Block"):
        estimate_sequential = read_estimate(path, "Sequential")
        total_sequential += estimate_sequential

        estimate_parallel = read_estimate(path, "Parallel")
        total_parallel += estimate_parallel

        speed_up = estimate_sequential / estimate_parallel
        max_speed_up = max(max_speed_up, speed_up)
        min_speed_up = min(min_speed_up, speed_up)

print(f"Average: x{round(total_sequential / total_parallel, 2)}")
print(f"Max: x{round(max_speed_up, 2)}")
print(f"Min: x{round(min_speed_up, 2)}")
