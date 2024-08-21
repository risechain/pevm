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


def format_ms(ns):
    return round(ns / 1000000, 3)


def read_estimate(block, exec_type):
    with open(f"{CRITERION_PATH}/{block}/{exec_type}/new/estimates.json") as f:
        estimates = json.load(f)
        return (estimates["slope"] or estimates["mean"])["point_estimate"]


for path in os.listdir(CRITERION_PATH):
    if path.startswith("Block"):
        seq_ims = read_estimate(path, "Sequential_In Memory")
        par_ims = read_estimate(path, "Parallel_In Memory")
        seq_ods = read_estimate(path, "Sequential_On Disk")
        par_ods = read_estimate(path, "Parallel_On Disk")

        print(
            f"{path: <40}\t:{format_ms(seq_ims)}\t{format_ms(par_ims)}\t{format_ms(seq_ods)}\t{format_ms(par_ods)}"
        )
