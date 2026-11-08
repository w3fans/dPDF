#!/usr/bin/env python3
"""Bring merged GitHub pull requests into the release checkout and the editable source.

The release checkout (`/app/dPDF/source`) is the Git authority, but its files are
overwritten from the editable source by `scripts/sync-release.sh` before every
checkpoint/preview. Pull requests merged on GitHub must therefore reach the editable
source first, or the next checkpoint would silently revert them.

This tool:
1. requires a clean release checkout on `main` whose history is contained in
   `origin/main` (it never rebases, merges or discards local commits);
2. lists every path changed between the checkout and `origin/main`;
3. refuses, before writing anything, if an editable-source copy of such a path is
   neither the checkout's version nor already the new version (a local edit that
   would be lost), and reports each conflicting path;
4. writes the new versions into the editable source, then fast-forwards the
   checkout. Re-running after an interruption completes the same update.
"""
import argparse
import os
from pathlib import Path, PurePosixPath
import stat
import subprocess
import sys
import tempfile


class Refused(Exception):
    pass


def git(checkout, *args, binary=False, check=True):
    result = subprocess.run(['git', '-C', str(checkout), *args], capture_output=True, check=False)
    if check and result.returncode:
        raise Refused(f"git {' '.join(args)} failed: {result.stderr.decode(errors='replace').strip()}")
    return result.stdout if binary else result.stdout.decode('utf-8').strip()


def is_ancestor(checkout, older, newer):
    return subprocess.run(['git', '-C', str(checkout), 'merge-base', '--is-ancestor', older, newer],
                          capture_output=True).returncode == 0


def safe_relative(path):
    parts = PurePosixPath(path).parts
    if not parts or PurePosixPath(path).is_absolute() or '..' in parts or '.git' in parts:
        raise Refused(f'Unsafe repository path: {path!r}')
    return Path(*parts)


def changes(checkout, old, new):
    """Yield (path, old_mode, old_sha, new_mode, new_sha); absent sides use mode '000000'."""
    raw = git(checkout, 'diff', '--raw', '--no-renames', '--no-abbrev', '-z', old, new, binary=True)
    fields = raw.split(b'\0')
    result = []
    index = 0
    while index < len(fields) - 1:
        meta, path = fields[index].decode(), fields[index + 1].decode('utf-8')
        index += 2
        old_mode, new_mode, old_sha, new_sha, _status = meta.lstrip(':').split(' ')
        for mode in (old_mode, new_mode):
            if mode not in ('000000', '100644', '100755', '120000'):
                raise Refused(f'{path}: unsupported entry mode {mode} (submodule?)')
        safe_relative(path)
        result.append((path, old_mode, old_sha, new_mode, new_sha))
    return result


def blob(checkout, sha):
    return git(checkout, 'cat-file', 'blob', sha, binary=True)


def current(source, path):
    """Return (kind, content, executable) with kind 'absent'|'file'|'link'|'other'."""
    target = source / safe_relative(path)
    try:
        info = target.lstat()
    except FileNotFoundError:
        return 'absent', None, False
    if stat.S_ISLNK(info.st_mode):
        return 'link', os.readlink(target).encode(), False
    if stat.S_ISREG(info.st_mode):
        return 'file', target.read_bytes(), bool(info.st_mode & stat.S_IXUSR)
    return 'other', None, False


def matches(checkout, state, mode, sha):
    kind, content, executable = state
    if mode == '000000':
        return kind == 'absent'
    if mode == '120000':
        return kind == 'link' and content == blob(checkout, sha)
    return kind == 'file' and executable == (mode == '100755') and content == blob(checkout, sha)


def write(source, checkout, path, mode, sha):
    target = source / safe_relative(path)
    for parent in reversed(target.relative_to(source).parents):
        if parent != Path('.') and (source / parent).is_symlink():
            raise Refused(f'{path}: a parent directory is a symbolic link in the editable source')
    if mode == '000000':
        if target.is_symlink() or target.exists():
            target.unlink()
        parent = target.parent
        while parent != source and parent.is_dir() and not any(parent.iterdir()):
            parent.rmdir()
            parent = parent.parent
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    content = blob(checkout, sha)
    if mode == '120000':
        if target.is_symlink() or target.exists():
            target.unlink()
        os.symlink(content.decode(), target)
        return
    handle, temporary = tempfile.mkstemp(prefix='.upstream-', dir=target.parent)
    try:
        with os.fdopen(handle, 'wb') as stream:
            stream.write(content)
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(temporary, 0o755 if mode == '100755' else 0o644)
        os.replace(temporary, target)
    except BaseException:
        Path(temporary).unlink(missing_ok=True)
        raise


def sync(source, checkout, remote='origin', branch='main', fetch=True, dry_run=False, out=print):
    source, checkout = Path(source).resolve(), Path(checkout).resolve()
    if source == checkout:
        raise Refused('Run from the editable source; it must differ from the release checkout')
    if not (checkout / '.git').exists():
        raise Refused(f'{checkout} is not a Git checkout')
    if git(checkout, 'status', '--porcelain', '--untracked-files=normal'):
        raise Refused('Release checkout has uncommitted work; checkpoint or reconcile it first')
    if git(checkout, 'symbolic-ref', '--short', 'HEAD') != branch:
        raise Refused(f'Release checkout is not on {branch}')
    if fetch:
        git(checkout, 'fetch', '--quiet', remote, f'+refs/heads/{branch}:refs/remotes/{remote}/{branch}')
    head = git(checkout, 'rev-parse', 'HEAD')
    upstream = git(checkout, 'rev-parse', f'refs/remotes/{remote}/{branch}')
    if head == upstream or is_ancestor(checkout, upstream, head):
        out(f'Release checkout already contains {remote}/{branch} ({upstream[:12]}); nothing to bring in.')
        return []
    if not is_ancestor(checkout, head, upstream):
        raise Refused(f'Release checkout and {remote}/{branch} have diverged; reconcile history by hand '
                      '(never force-push over merged pull requests)')
    plan = changes(checkout, head, upstream)
    conflicts, pending = [], []
    for path, old_mode, old_sha, new_mode, new_sha in plan:
        state = current(source, path)
        if matches(checkout, state, new_mode, new_sha):
            continue  # Already applied (for example by an interrupted earlier run).
        if not matches(checkout, state, old_mode, old_sha):
            conflicts.append(path)
            continue
        pending.append((path, new_mode, new_sha))
    if conflicts:
        raise Refused('Editable source has local changes to files also changed on GitHub; nothing was '
                      'written. Merge these by hand, then re-run:\n  ' + '\n  '.join(conflicts))
    commits = git(checkout, 'log', '--oneline', '--no-decorate', f'{head}..{upstream}')
    out(f'Bringing in {len(commits.splitlines())} commit(s) from {remote}/{branch}:\n{commits}')
    for path, new_mode, _ in pending:
        out(('delete ' if new_mode == '000000' else 'update ') + path)
    if dry_run:
        out('Dry run: nothing written.')
        return pending
    for path, new_mode, new_sha in pending:
        write(source, checkout, path, new_mode, new_sha)
    git(checkout, 'merge', '--ff-only', '--quiet', upstream)
    out(f'Release checkout fast-forwarded to {upstream[:12]}; editable source updated.')
    return pending


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--source', type=Path, required=True, help='Editable source directory')
    parser.add_argument('--checkout', type=Path, required=True, help='Release Git checkout')
    parser.add_argument('--remote', default='origin')
    parser.add_argument('--branch', default='main')
    parser.add_argument('--dry-run', action='store_true')
    parser.add_argument('--no-fetch', action='store_true', help='Use the already-fetched remote branch')
    args = parser.parse_args()
    try:
        sync(args.source, args.checkout, args.remote, args.branch, not args.no_fetch, args.dry_run)
    except Refused as error:
        print(f'Refused: {error}', file=sys.stderr)
        raise SystemExit(2)


if __name__ == '__main__':
    main()
