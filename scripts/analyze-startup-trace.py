#!/usr/bin/env python3
"""Summarise local ZVIM_TRACE_STARTUP=1 stderr output (times in milliseconds)."""
import argparse
import re
import statistics

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('trace')
args = parser.parse_args()
pairs = {
    'Frame paint': ('grid_paint_begin', 'grid_paint_end'),
    'Redraw reduction': ('redraw_decode_begin', 'redraw_decode_end'),
    'Input RPC': ('input_rpc_begin', 'input_rpc_end'),
}
active, durations = {}, {name: [] for name in pairs}
with open(args.trace) as source:
    for line in source:
        match = re.fullmatch(r'zvim_startup_us=(\d+) stage=(\S+)\n?', line)
        if not match:
            continue
        timestamp, stage = int(match[1]), match[2]
        for name, (begin, end) in pairs.items():
            if stage == begin:
                active[name] = timestamp
            elif stage == end and name in active:
                durations[name].append((timestamp - active.pop(name)) / 1000)
for name, values in durations.items():
    if values:
        print(f'{name}: n={len(values)}, median={statistics.median(values):.3f} ms, max={max(values):.3f} ms')
