"""Bounded repetition of one generated test inside the native Windows Job Object.

The enclosing validation plan owns the 240-second deadline and process-tree
limits. Stop at the first failed test; repetitions never turn a failure green.
"""
import argparse
import os
import subprocess
import time


def repeat_cases(command, count, runner=subprocess.run, clock=time.monotonic, budget=210):
    if not 1 <= count <= 32:
        raise ValueError('Require 1..32 diagnostic cases')
    started = clock()
    for index in range(count):
        if clock() - started >= budget:
            print(f'INCOMPLETE: diagnostic budget exhausted after {index}/{count} cases', flush=True)
            return 124
        print(f'Metadata contention diagnostic case {index + 1}/{count}', flush=True)
        result = runner(command, check=False)
        if result.returncode:
            print(f'FAIL: case {index + 1}; stopping without retry', flush=True)
            return result.returncode
    print(f'PASS: all {count} unchanged generated contention cases', flush=True)
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cargo', required=True)
    parser.add_argument('--cases', type=int, choices=range(1, 33), default=32)
    args = parser.parse_args()
    if os.name != 'nt':
        raise SystemExit('Invoke only through the approved bounded native Windows validation plan')
    command = [args.cargo, 'test', '--locked', '--release', '--bin', 'dpdf',
               'marks::tests::concurrent_usage_and_size_updates_preserve_assets_and_future_metadata',
               '--', '--exact', '--nocapture']
    return repeat_cases(command, args.cases)


if __name__ == '__main__':
    raise SystemExit(main())
