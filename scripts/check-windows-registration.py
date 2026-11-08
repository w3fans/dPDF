#!/usr/bin/env python3
"""Native Windows user registration on unique fixture keys, preserving PDF defaults."""
import argparse
import atexit
import ctypes as C
from ctypes import wintypes as W
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--pdfium', type=Path, required=True)
    args = parser.parse_args()
    if os.name != 'nt':
        parser.error('This check requires native Windows')
    import winreg
    ole = C.WinDLL('ole32')
    ole.CoInitializeEx.argtypes = [C.c_void_p, W.DWORD]
    ole.CoInitializeEx.restype = C.c_long
    ole.CoUninitialize.argtypes = []
    ole.CoUninitialize.restype = None
    initialized = ole.CoInitializeEx(None, 2 | 4)  # STA, disable OLE1 DDE
    if initialized < 0:
        raise RuntimeError('Cannot initialize native association COM context')
    atexit.register(ole.CoUninitialize)
    source = Path(__file__).resolve().parent.parent
    shell = C.WinDLL('shlwapi', use_last_error=True)
    shell.AssocQueryStringW.argtypes = [W.DWORD, W.DWORD, W.LPCWSTR, W.LPCWSTR, W.LPWSTR, C.POINTER(W.DWORD)]
    shell.AssocQueryStringW.restype = C.c_long

    def association(progid, fixed=False):
        size = W.DWORD()
        flags = 0x800 if fixed else 0  # ASSOCF_INIT_FIXED_PROGID
        shell.AssocQueryStringW(flags, 1, progid, 'open', None, C.byref(size))
        if not size.value:
            return None
        assert size.value < 32768
        text = C.create_unicode_buffer(size.value)
        code = shell.AssocQueryStringW(flags, 1, progid, 'open', text, C.byref(size))
        if code != 0:
            raise RuntimeError('Windows association lookup failed: ' + hex(code & 0xffffffff))
        return text.value

    def value(path, name=''):
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, path) as key:
                return winreg.QueryValueEx(key, name)[0]
        except FileNotFoundError:
            return None

    def snapshot(path):
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, path) as key:
                count = winreg.QueryInfoKey(key)[1]
                return sorted(winreg.EnumValue(key, i) for i in range(count))
        except FileNotFoundError:
            return None

    def exists(path):
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, path):
                return True
        except FileNotFoundError:
            return False

    default_before = association('.pdf')
    pdf = r'Software\Classes\.pdf'
    open_with = pdf + r'\OpenWithProgids'
    registered = r'Software\RegisteredApplications'
    choice = r'Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.pdf\UserChoice'
    before = {path: snapshot(path) for path in (pdf, open_with, registered, choice)}
    created_shared = [path for path in (pdf, open_with, registered) if not exists(path)]
    token = uuid.uuid4().hex
    app_id = 'net.dpdf.dPDF.V.' + token[:22]
    executable_name = 'dpdf-validation-' + token + '.exe'
    owned = [rf'Software\Classes\{app_id}', rf'Software\Classes\Applications\{executable_name}',
             rf'Software\Microsoft\Windows\CurrentVersion\App Paths\{executable_name}',
             rf'Software\{app_id}']
    assert not any(exists(path) for path in owned), 'Fixture namespace already exists'
    powershell = ['powershell.exe', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass']
    spec = importlib.util.spec_from_file_location('sample', source / 'scripts/make-sample.py')
    sample = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(sample)
    with tempfile.TemporaryDirectory(prefix='dpdf-user-registration-') as directory:
        root = Path(directory)
        runtime = root / 'package Ž [preview] with spaces & comma,'
        subprocess.run([*powershell, '-File', str(source / 'packaging/windows/stage.ps1'),
                        '-BinaryPath', str(args.binary.resolve()), '-PdfiumDistribution', str(args.pdfium.resolve().parent.parent),
                        '-OutputDirectory', str(runtime)], check=True, timeout=90)
        binary = runtime / executable_name
        (runtime / 'dpdf.exe').rename(binary)
        menu = root / 'isolated-start-menu'
        base = [*powershell, '-File', str(runtime / 'register.ps1'), '-RuntimeDirectory', str(runtime),
                '-ApplicationId', app_id, '-ExecutableName', executable_name, '-StartMenuDirectory', str(menu)]
        try:
            # Register and update idempotently; all names are unique to this fixture.
            for _ in range(2):
                subprocess.run(base, check=True, timeout=60)
            command = '"' + str(binary) + '" "%1"'
            assert association(app_id, fixed=True) == command, 'Windows did not resolve the exact registered command'
            assert value(owned[0] + r'\shell\open\command') == command
            assert value(owned[1] + r'\SupportedTypes', '.pdf') == ''
            assert value(owned[2]) == str(binary)
            assert value(owned[3] + r'\Capabilities\FileAssociations', '.pdf') == app_id
            assert value(open_with, app_id) == ''
            assert value(registered, app_id) == owned[3] + r'\Capabilities'
            link = menu / (app_id + '.lnk')
            assert link.is_file()
            environment = dict(os.environ, DPDF_TEST_LINK=str(link))
            script = ("[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); $s=New-Object -ComObject WScript.Shell; $l=$s.CreateShortcut($env:DPDF_TEST_LINK); "
                      "@{target=$l.TargetPath;directory=$l.WorkingDirectory;icon=$l.IconLocation} | ConvertTo-Json -Compress")
            shortcut = json.loads(subprocess.check_output([*powershell, '-Command', script], env=environment,
                                                         text=True, encoding='utf-8', errors='strict', timeout=30))
            assert os.path.samefile(shortcut['target'], binary)
            assert os.path.samefile(shortcut['directory'], runtime)
            icon = runtime / 'dpdf.ico'
            icon_path = shortcut['icon'].rsplit(',', 1)[0].strip('"')
            assert os.path.samefile(icon_path, icon)
            user = C.WinDLL('user32', use_last_error=True)
            user.LoadImageW.argtypes = [W.HINSTANCE, W.LPCWSTR, W.UINT, C.c_int, C.c_int, W.UINT]
            user.LoadImageW.restype = W.HANDLE
            user.DestroyIcon.argtypes = [W.HANDLE]
            user.DestroyIcon.restype = W.BOOL
            handle = user.LoadImageW(None, str(icon), 1, 32, 32, 0x10)
            if not handle:
                raise C.WinError(C.get_last_error())
            assert user.DestroyIcon(handle)
            # Resolve the registered command through Windows and verify its native argv.
            api = C.WinDLL('shell32')
            api.CommandLineToArgvW.argtypes = [W.LPCWSTR, C.POINTER(C.c_int)]
            api.CommandLineToArgvW.restype = C.POINTER(W.LPWSTR)
            kernel = C.WinDLL('kernel32')
            kernel.LocalFree.argtypes = [C.c_void_p]
            kernel.LocalFree.restype = C.c_void_p
            argc = C.c_int()
            argv = api.CommandLineToArgvW(association(app_id, fixed=True), C.byref(argc))
            try:
                assert argc.value == 2 and argv[0] == str(binary) and argv[1] == '%1'
                selected = argv[0]
            finally:
                kernel.LocalFree(argv)
            fixture = root / 'Pogodba ŽČŠ 100% #saved.pdf'
            fixture.write_bytes(sample.make_pdf(50))
            env = {key: val for key, val in os.environ.items() if not key.startswith(('DPDF_', 'SLINT_'))}
            env.update(DPDF_CONFIG_DIR=str(root / 'config'), DPDF_DATA_DIR=str(root / 'data'))
            result = subprocess.run([selected, '--probe', str(fixture)], env=env, capture_output=True,
                                    text=True, encoding='utf-8', check=True, timeout=45)
            assert 'pages=50 ' in result.stdout, result.stdout
            assert association('.pdf') == default_before
            assert snapshot(choice) == before[choice], 'Protected PDF UserChoice settings changed'
            assert (snapshot(pdf) or []) == (before[pdf] or []), 'PDF extension default values changed'
            # Another installation cannot take the executable alias, even with a new ProgID.
            other = 'net.dpdf.dPDF.V.' + uuid.uuid4().hex[:22]
            conflict = base.copy()
            conflict[conflict.index('-ApplicationId') + 1] = other
            rejected = subprocess.run(conflict, capture_output=True, timeout=30)
            assert rejected.returncode != 0 and not exists('Software\\' + other)
            assert value(owned[1] + r'\shell\open\command') == command
        finally:
            subprocess.run([*base, '-Unregister'], check=True, timeout=60)
            assert not any(exists(path) for path in owned)
            assert not (menu / (app_id + '.lnk')).exists()
            assert snapshot(open_with) == (before[open_with] or [])
            assert snapshot(registered) == (before[registered] or [])
            # Remove only shared empty containers that this fixture created.
            for path in reversed(created_shared):
                try:
                    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, path) as key:
                        children, values, _ = winreg.QueryInfoKey(key)
                    if not children and not values:
                        winreg.DeleteKey(winreg.HKEY_CURRENT_USER, path)
                except FileNotFoundError:
                    pass
            assert association('.pdf') == default_before and snapshot(choice) == before[choice]
        print('PASS: unique per-user registration/update/cleanup, native association/argv lookup, shortcut/icon, adjacent-engine launch and unchanged PDF defaults; collisions refused')


if __name__ == '__main__':
    main()
