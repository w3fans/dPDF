#!/usr/bin/env python3
"""Register an already staged Fedora runtime for this user; never change defaults."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

APP_ID = 'net.dpdf.dPDF'


def executable_value(path):
    text = str(path)
    if not text.isascii() or any(character in text for character in ('=', '%', '\n', '\r', '\t', '\0')):
        raise ValueError('Desktop executable path must be ASCII without control characters, percent signs or an equals sign; use /app/dPDF')
    quoted = '"' + ''.join('\\' + c if c in '"`$\\' else c for c in text) + '" %F'
    return quoted.replace('\\', '\\\\')


def install(runtime, data_home):
    for tool in ('desktop-file-validate', 'desktop-file-install', 'update-desktop-database'):
        if not shutil.which(tool):
            raise ValueError('Install the standard desktop-file-utils tools before registering dPDF')
    runtime = runtime.resolve(strict=True)
    binary = runtime / 'dpdf'
    icon = runtime / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / (APP_ID + '.svg')
    if not binary.is_file() or not os.access(binary, os.X_OK) or not icon.is_file():
        raise ValueError('Stage a complete executable and application icon before registering the desktop entry')
    template = (runtime / 'share' / 'applications' / (APP_ID + '.desktop')).read_text(encoding='utf-8')
    entry = '\n'.join('Exec=' + executable_value(binary) if line.startswith('Exec=') else line
                      for line in template.splitlines()) + '\n'
    if entry.count('Exec=') != 1 or 'MimeType=application/pdf;' not in entry:
        raise ValueError('Unexpected dPDF desktop template')
    applications = data_home / 'applications'
    icon_directory = data_home / 'icons' / 'hicolor' / 'scalable' / 'apps'
    with tempfile.TemporaryDirectory(prefix='dpdf-desktop-entry-') as directory:
        temporary = Path(directory) / (APP_ID + '.desktop')
        temporary.write_text(entry, encoding='utf-8')
        subprocess.run(['desktop-file-validate', str(temporary)], check=True)
        applications.mkdir(parents=True, exist_ok=True)
        icon_directory.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(icon, icon_directory / icon.name)
        subprocess.run(['desktop-file-install', '--mode=0644', '--dir=' + str(applications), str(temporary)], check=True)
    subprocess.run(['update-desktop-database', str(applications)], check=True)
    return applications / (APP_ID + '.desktop')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    beside_script = Path(__file__).resolve().parent
    default_runtime = beside_script if (beside_script / 'dpdf').is_file() else Path('/app/dPDF')
    parser.add_argument('--runtime', type=Path, default=default_runtime)
    parser.add_argument('--data-home', type=Path, help='Override the user data directory, mainly for isolated packaging tests')
    args = parser.parse_args()
    if not sys.platform.startswith('linux'):
        parser.error('This registration helper is for Fedora/Linux')
    configured = os.environ.get('XDG_DATA_HOME', '')
    data_home = args.data_home or (Path(configured) if configured and Path(configured).is_absolute()
                                   else Path.home() / '.local' / 'share')
    try:
        path = install(args.runtime, data_home)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, 'Cannot register dPDF: ' + str(error) + '\n')
    print('Registered dPDF in the application menu / Open With: ' + str(path))


if __name__ == '__main__':
    main()
