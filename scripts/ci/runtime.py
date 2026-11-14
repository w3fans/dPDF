"""Download pinned test helpers into an isolated CI directory, never global PATH."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import urllib.request
import zipfile

RUNTIMES = {
    'pdfium': ('https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8125/pdfium-win-x64.tgz',
               '9a964261760cff52570aabf038ab346d3a5034dd8b30d0d5fe8861235d356b8b', 'pdfium.dll'),
    'qpdf': ('https://github.com/qpdf/qpdf/releases/download/v12.4.2/qpdf-12.4.2-msvc64.zip',
             'db87077e683630c1217e0e8f9a20a9749d952ab676e881c3689187763a5de25d', 'qpdf.exe'),
}


def safe_member(name):
    path = PurePosixPath(name.replace('\\', '/'))
    if path.is_absolute() or '..' in path.parts or ':' in name:
        raise ValueError(f'Unsafe archive path: {name}')
    return path


def prepare(root):
    root = Path(root)
    root.mkdir(parents=True, exist_ok=False)
    paths = {}
    for name, (url, digest, executable) in RUNTIMES.items():
        archive = root / (name + ('.zip' if url.endswith('.zip') else '.tgz'))
        with urllib.request.urlopen(url, timeout=90) as source, archive.open('wb') as output:
            shutil.copyfileobj(source, output)
        if hashlib.sha256(archive.read_bytes()).hexdigest() != digest:
            raise ValueError(f'{name} archive checksum mismatch')
        destination = root / name
        destination.mkdir()
        if url.endswith('.zip'):
            with zipfile.ZipFile(archive) as package:
                for item in package.infolist():
                    relative = safe_member(item.filename)
                    if item.is_dir():
                        continue
                    target = destination.joinpath(*relative.parts)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    with package.open(item) as source, target.open('wb') as output:
                        shutil.copyfileobj(source, output)
        else:
            with tarfile.open(archive) as package:
                for item in package:
                    relative = safe_member(item.name)
                    if item.isdir():
                        continue
                    if not item.isfile():
                        raise ValueError(f'Unsupported archive entry: {item.name}')
                    target = destination.joinpath(*relative.parts)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    with package.extractfile(item) as source, target.open('wb') as output:
                        shutil.copyfileobj(source, output)
        matches = list(destination.rglob(executable))
        if len(matches) != 1:
            raise ValueError(f'Expected one {executable}, found {len(matches)}')
        paths[name] = str(matches[0].resolve())
        paths[name + '_archive_sha256'] = digest
    (root / 'runtimes.json').write_text(json.dumps(paths, indent=2) + '\n', encoding='utf-8')
    return paths
