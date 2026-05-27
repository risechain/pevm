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

# By default it will get result from latest run in target/criterion
# If you run benchmark with different CRITERION_HOME and/or `--save-baseline` flag
#    Please set this ENV Var to correct value.
CRITERION_HOME = os.getenv("CRITERION_HOME", "target/criterion")
CRITERION_RUN = os.getenv("CRITERION_RUN", "new")


def format_ms(ns):
    return round(ns / 1000000, 3)


def read_estimate(block, exec_type):
    with open(f"{CRITERION_HOME}/{block}/{exec_type}/{CRITERION_RUN}/estimates.json") as f:
        estimates = json.load(f)
        return (estimates["slope"] or estimates["mean"])["point_estimate"]


total_sequential = 0
total_parallel = 0
max_speed_up = 0
min_speed_up = float("inf")

for path in os.listdir(CRITERION_HOME):
    if path.startswith("Block"):
        estimate_sequential = read_estimate(path, "Sequential")
        total_sequential += estimate_sequential

        estimate_parallel = read_estimate(path, "Parallel")
        total_parallel += estimate_parallel

        speed_up = round(estimate_sequential / estimate_parallel, 2)
        max_speed_up = max(max_speed_up, speed_up)
        min_speed_up = min(min_speed_up, speed_up)

        print(f"{path}")
        print(
            f"{format_ms(estimate_sequential)} {format_ms(estimate_parallel)} {speed_up}\n"
        )


print(f"Average: x{round(total_sequential / total_parallel, 2)}")
print(f"Max: x{max_speed_up}")
print(f"Min: x{min_speed_up}")
