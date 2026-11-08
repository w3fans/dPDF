"""Use the existing one-CPU/2-GiB Windows child-tree limits for packaging."""
import argparse
import os
from pathlib import Path
from windows_job import run

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--source', type=Path, default=Path('.'))
parser.add_argument('--log', type=Path, required=True)
parser.add_argument('--timeout', type=int, default=1800)
parser.add_argument('command', nargs=argparse.REMAINDER)
args = parser.parse_args()
command = args.command[1:] if args.command[:1] == ['--'] else args.command
if os.name != 'nt' or not command:
    raise SystemExit('A native Windows command is required; no fallback')
raise SystemExit(run(command, args.source.resolve(), dict(os.environ), args.log.resolve(), args.timeout))
