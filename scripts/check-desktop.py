#!/usr/bin/env python3
"""Validate Linux desktop registration/argv using Gio on a private D-Bus session."""
import ctypes as C
import ctypes.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time


def main():
    if sys.platform != 'linux':
        raise RuntimeError('The desktop-entry integration check requires Linux')
    if '--private-bus' not in sys.argv:
        environment = dict(os.environ, GIO_USE_VFS='local')
        subprocess.run(['dbus-run-session', '--', sys.executable, str(Path(__file__).resolve()), '--private-bus'],
                       check=True, env=environment)
        return
    source = Path(__file__).resolve().parent.parent
    with tempfile.TemporaryDirectory(prefix='dpdf-desktop-check-') as directory:
        root = Path(directory)
        runtime = root / 'runtime space $value sign "quote" back`tick \\slash'
        runtime.mkdir()
        result = root / 'launched.json'
        executable = runtime / 'dpdf'
        executable.write_text('#!' + sys.executable + '\nimport json,sys\nfrom pathlib import Path\n'
                              + f'Path({str(result)!r}).write_text(json.dumps(sys.argv))\n')
        executable.chmod(0o700)
        apps = runtime / 'share' / 'applications'
        icons = runtime / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps'
        apps.mkdir(parents=True)
        icons.mkdir(parents=True)
        shutil.copyfile(source / 'packaging/linux/net.dpdf.dPDF.desktop', apps / 'net.dpdf.dPDF.desktop')
        shutil.copyfile(source / 'ui/assets/dpdf.svg', icons / 'net.dpdf.dPDF.svg')
        data = root / 'isolated-user-data'
        subprocess.run([sys.executable, str(source / 'scripts/install-desktop.py'), '--runtime', str(runtime),
                        '--data-home', str(data)], check=True)
        entry = data / 'applications/net.dpdf.dPDF.desktop'
        assert (data / 'icons/hicolor/scalable/apps/net.dpdf.dPDF.svg').is_file()
        assert 'application/pdf=net.dpdf.dPDF.desktop;' in (data / 'applications/mimeinfo.cache').read_text()
        documents = [root / 'Pogodba ŽČŠ $value 100%.pdf', root / 'second #contract.pdf']
        for document in documents:
            document.write_bytes(b'%PDF-generated launcher fixture only')
        gio = C.CDLL(C.util.find_library('gio-2.0'))
        glib = C.CDLL(C.util.find_library('glib-2.0'))
        gobject = C.CDLL(C.util.find_library('gobject-2.0'))
        gio.g_desktop_app_info_new_from_filename.argtypes = [C.c_char_p]
        gio.g_desktop_app_info_new_from_filename.restype = C.c_void_p
        gio.g_app_info_launch_uris.argtypes = [C.c_void_p, C.c_void_p, C.c_void_p, C.POINTER(C.c_void_p)]
        gio.g_app_info_launch_uris.restype = C.c_int
        glib.g_list_append.argtypes = [C.c_void_p, C.c_void_p]
        glib.g_list_append.restype = C.c_void_p
        glib.g_list_free.argtypes = [C.c_void_p]
        glib.g_list_free.restype = None
        gobject.g_object_unref.argtypes = [C.c_void_p]
        gobject.g_object_unref.restype = None
        info = gio.g_desktop_app_info_new_from_filename(str(entry).encode())
        assert info, 'Gio rejected the desktop entry'
        linked = None
        buffers = [C.create_string_buffer(document.as_uri().encode()) for document in documents]
        try:
            for buffer in buffers:
                linked = glib.g_list_append(linked, C.cast(buffer, C.c_void_p))
            error = C.c_void_p()
            assert gio.g_app_info_launch_uris(info, linked, None, C.byref(error)), 'Gio failed to launch the generated entry'
            deadline = time.monotonic() + 10
            while not result.is_file() and time.monotonic() < deadline:
                time.sleep(.05)
            assert result.is_file(), 'Desktop launch did not reach the exact executable'
            assert json.loads(result.read_text()) == [str(executable), *map(str, documents)]
        finally:
            glib.g_list_free(linked)
            gobject.g_object_unref(info)
        for name in ('invalid=runtime', 'invalid%runtime', 'invalidŽruntime'):
            invalid = root / name
            shutil.copytree(runtime, invalid)
            rejected_data = root / ('rejected-data-' + name)
            rejected = subprocess.run([sys.executable, str(source / 'scripts/install-desktop.py'), '--runtime', str(invalid),
                                      '--data-home', str(rejected_data)], capture_output=True)
            assert rejected.returncode != 0 and not rejected_data.exists()
        print('PASS: desktop registration/icon/MIME cache; Gio launches exact quoted executable and multiple Unicode file arguments; invalid paths refused before writes')


if __name__ == '__main__':
    main()
