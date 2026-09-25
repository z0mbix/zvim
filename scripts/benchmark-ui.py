#!/usr/bin/env python3
"""Run a release GUI benchmark; opens temporary windows and real shell tabs.
No user Neovim config or window geometry is used. Run on an idle desktop.
Reports CPU stages, not input-to-display latency. Requires staged Ghostty resources.
"""
import argparse
import hashlib
import platform
import json
import os
from pathlib import Path
import re
import statistics
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', default='target/release/examples/performance_ui')
parser.add_argument('--output', required=True)
parser.add_argument('--runs', type=int, default=3)
parser.add_argument('--trace-dir', default='.cache/performance')
args = parser.parse_args()
output = Path(args.output)
output.parent.mkdir(parents=True, exist_ok=True)
trace_dir = Path(args.trace_dir)
trace_dir.mkdir(parents=True, exist_ok=True)
results = []
for run in range(args.runs):
    trace = trace_dir / f'{output.stem}.run{run + 1}.trace'
    env = dict(os.environ, ZVIM_TRACE_STARTUP='1', SHELL='/bin/sh',
               GHOSTTY_RESOURCES_DIR=str(Path('target/ghostty-resources/ghostty').resolve()))
    with trace.open('w') as log:
        subprocess.run([args.binary], env=env, stderr=log, stdout=subprocess.DEVNULL,
                       timeout=90, check=True)
    phase, active, samples = 'transition', {}, {}
    for line in trace.read_text().splitlines():
        if line.startswith('benchmark_phase='):
            phase = line.split('=', 1)[1]
            active.clear()
        match = re.fullmatch(r'zvim_startup_us=(\d+) stage=(\S+)', line)
        if not match or phase == 'transition':
            continue
        timestamp, stage = int(match[1]), match[2]
        if stage.endswith('_begin'):
            active[stage[:-6]] = timestamp
        elif stage.endswith('_end') and stage[:-4] in active:
            name = stage[:-4]
            samples.setdefault(phase, {}).setdefault(name, []).append(
                (timestamp - active.pop(name)) / 1000)
    result = {}
    for phase, stages in samples.items():
        result[phase] = {}
        for stage, values in stages.items():
            values.sort()
            result[phase][stage] = dict(n=len(values), median_ms=statistics.median(values),
                p95_ms=values[int((len(values)-1)*.95)], max_ms=max(values))
    results.append(result)
    print(f'Run {run + 1} complete', flush=True)
output.write_text(json.dumps(dict(binary=args.binary, sha256=hashlib.sha256(Path(args.binary).read_bytes()).hexdigest(), platform=platform.platform(), runs=results), indent=2) + '\n')
print(output)
