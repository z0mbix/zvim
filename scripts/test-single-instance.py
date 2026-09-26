#!/usr/bin/env python3
"""Open two temporary real GUI windows; verify one host and independent --wait.
Uses -u NONE and no ShaDa; only the test windows close themselves.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', default='target/debug/zvim')
args = parser.parse_args()
binary = str(Path(args.binary).resolve())
with tempfile.TemporaryDirectory(prefix='zvim-instance-test-') as temporary:
    root = Path(temporary)
    children = []
    for index in range(2):
        project = root / f'project {index}'
        project.mkdir()
        script = project / 'startup.lua'
        script.write_text('''
local result = {host=vim.uv.os_getppid(), editor=vim.fn.getpid(), cwd=vim.fn.getcwd(),
                origin=vim.env.ZVIM_TEST_ORIGIN, file=vim.fn.expand('%:t')}
vim.fn.writefile({vim.json.encode(result)}, vim.env.ZVIM_TEST_RESULT)
vim.defer_fn(function() vim.cmd('qa!') end, tonumber(vim.env.ZVIM_TEST_LIFETIME))
''')
        output = root / f'result-{index}.json'
        env = dict(os.environ, ZVIM_TEST_ORIGIN=f'caller-{index}', ZVIM_TEST_RESULT=str(output),
                   ZVIM_TEST_LIFETIME=str(2500 if index == 0 else 6500))
        children.append(subprocess.Popen([binary, '--zvim-launch', '--wait', '-u', 'NONE', '-i', 'NONE',
                                          '-n', str(project), '-S', 'startup.lua', 'file with spaces.txt'],
                                         env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE))
    try:
        deadline = time.monotonic() + 15
        while not all((root / f'result-{i}.json').exists() for i in range(2)):
            if time.monotonic() >= deadline:
                raise RuntimeError('Test windows did not report their startup state')
            time.sleep(.05)
        results = [json.loads((root / f'result-{i}.json').read_text()) for i in range(2)]
        assert results[0]['host'] == results[1]['host'], results
        assert results[0]['editor'] != results[1]['editor'], results
        for i, result in enumerate(results):
            assert Path(result['cwd']).resolve() == (root / f'project {i}').resolve(), result
            assert result['origin'] == f'caller-{i}', result
            assert result['file'] == 'file with spaces.txt', result
        # Both direct executable launches (including cargo/just) and the installed
        # detached launcher must hand off without waiting for other windows.
        for mode, extra in [('direct', []), ('launcher', ['--zvim-launch'])]:
            output = root / f'{mode}.json'
            env = dict(os.environ, ZVIM_TEST_ORIGIN=mode, ZVIM_TEST_RESULT=str(output),
                       ZVIM_TEST_LIFETIME='1000')
            result = subprocess.run([binary, *extra, '-u', 'NONE', '-i', 'NONE', '-n',
                                     str(root / 'project 0'), '-S', 'startup.lua', 'file with spaces.txt'],
                                    env=env, capture_output=True, timeout=5)
            assert result.returncode == 0, result.stderr.decode(errors='replace')
            deadline = time.monotonic() + 5
            while not output.exists():
                assert time.monotonic() < deadline, f'{mode} window did not start'
                time.sleep(.025)
            assert json.loads(output.read_text())['host'] == results[0]['host']
        assert all(child.poll() is None for child in children), 'wait returned before its window closed'
        _, error = children[0].communicate(timeout=15)
        assert children[0].returncode == 0, error.decode(errors='replace')
        assert children[1].poll() is None, 'closing one window released another window’s waiter'
        _, error = children[1].communicate(timeout=15)
        assert children[1].returncode == 0, error.decode(errors='replace')
        print('PASS: concurrent launches share GUI PID', results[0]['host'],
              '; independent editor PIDs, cwd, spaced arguments, caller environment, per-window --wait, direct and detached handoff')
    finally:
        for child in children:
            if child.poll() is None:
                child.terminate()
                child.communicate(timeout=5)
