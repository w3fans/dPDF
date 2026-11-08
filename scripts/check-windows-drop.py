#!/usr/bin/env python3
"""Real Windows OLE file-drop fixture; requires a dedicated desktop and explicit input opt-in."""
import argparse
from contextlib import contextmanager
import ctypes as C
from ctypes import wintypes as W
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).parent / filename)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


class Mouse(C.Structure):
    _fields_ = [('dx', W.LONG), ('dy', W.LONG), ('data', W.DWORD), ('flags', W.DWORD),
                ('time', W.DWORD), ('extra', C.c_size_t)]


class Keyboard(C.Structure):
    _fields_ = [('vk', W.WORD), ('scan', W.WORD), ('flags', W.DWORD), ('time', W.DWORD), ('extra', C.c_size_t)]


class Payload(C.Union):
    _fields_ = [('mouse', Mouse), ('keyboard', Keyboard)]


class Input(C.Structure):
    _fields_ = [('kind', W.DWORD), ('value', Payload)]


class Desktop:
    def __init__(self):
        self.user = C.WinDLL('user32', use_last_error=True)
        signatures = {
            'SetProcessDpiAwarenessContext': ([W.HANDLE], W.BOOL),
            'GetWindowThreadProcessId': ([W.HWND, C.POINTER(W.DWORD)], W.DWORD),
            'GetWindowTextW': ([W.HWND, W.LPWSTR, C.c_int], C.c_int),
            'GetWindowRect': ([W.HWND, C.POINTER(W.RECT)], W.BOOL),
            'IsWindowVisible': ([W.HWND], W.BOOL),
            'GetForegroundWindow': ([], W.HWND),
            'WindowFromPoint': ([W.POINT], W.HWND),
            'GetSystemMetrics': ([C.c_int], C.c_int),
            'SendInput': ([W.UINT, C.POINTER(Input), C.c_int], W.UINT),
            'GetCursorPos': ([C.POINTER(W.POINT)], W.BOOL),
            'SetCursorPos': ([C.c_int, C.c_int], W.BOOL),
        }
        for name, (arguments, result) in signatures.items():
            function = getattr(self.user, name)
            function.argtypes, function.restype = arguments, result
        self.user.SetProcessDpiAwarenessContext(W.HANDLE(-4))
        self.original = W.POINT()
        self.user.GetCursorPos(C.byref(self.original))
        self.held = False

    def pid(self, window):
        result = W.DWORD()
        self.user.GetWindowThreadProcessId(window, C.byref(result))
        return result.value

    def title(self, window):
        value = C.create_unicode_buffer(32768)
        self.user.GetWindowTextW(window, value, len(value))
        return value.value

    def find(self, pid):
        found = []
        callback_type = C.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)
        @callback_type
        def inspect(window, _):
            if self.pid(window) == pid and self.user.IsWindowVisible(window) and self.title(window).startswith('dPDF'):
                found.append(window)
            return True
        self.user.EnumWindows.argtypes = [callback_type, W.LPARAM]
        self.user.EnumWindows.restype = W.BOOL
        self.user.EnumWindows(inspect, 0)
        return found[0] if found else None

    def rect(self, window):
        rect = W.RECT()
        if not self.user.GetWindowRect(window, C.byref(rect)):
            raise C.WinError(C.get_last_error())
        return rect

    def send(self, event):
        if self.user.SendInput(1, C.byref(event), C.sizeof(Input)) != 1:
            raise C.WinError(C.get_last_error())

    def mouse(self, x, y, owner, flags=0):
        if self.pid(self.user.WindowFromPoint(W.POINT(x, y))) != owner:
            raise RuntimeError('Native input target is not a fixture-owned window')
        left, top, width, height = [self.user.GetSystemMetrics(i) for i in (76, 77, 78, 79)]
        event = Input(kind=0)
        event.value.mouse = Mouse(round((x-left)*65535/max(1, width-1)),
                                  round((y-top)*65535/max(1, height-1)), 0,
                                  0x0001 | 0x8000 | 0x4000 | flags, 0, 0)
        self.send(event)
        if flags & 2:
            self.held = True
        if flags & 4:
            self.held = False

    def next_tab(self, pid):
        if self.pid(self.user.GetForegroundWindow()) != pid:
            raise RuntimeError('Keyboard target is not the fixture reader')
        try:
            for key, flags in [(0x11, 0), (0x09, 0), (0x09, 2), (0x11, 2)]:
                event = Input(kind=1)
                event.value.keyboard = Keyboard(key, 0, flags, 0, 0)
                self.send(event)
        finally:
            event = Input(kind=1)
            event.value.keyboard = Keyboard(0x11, 0, 2, 0, 0)
            self.send(event)

    def restore(self):
        if self.held:
            event = Input(kind=0)
            event.value.mouse = Mouse(0, 0, 0, 4, 0, 0)
            self.send(event)
            self.held = False
        self.user.SetCursorPos(self.original.x, self.original.y)


def wait_for(function, seconds=12):
    deadline = time.monotonic()+seconds
    while time.monotonic() < deadline:
        value = function()
        if value:
            return value
        time.sleep(.05)
    raise RuntimeError('Native file-drop fixture timed out')


def stop(process):
    if process.poll() is None:
        process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


@contextmanager
def fixture_workspace():
    directory = tempfile.TemporaryDirectory(prefix='dpdf-native-ole-')
    try:
        yield directory.name
    finally:
        # Windows may retain fixture log handles briefly after process exit.
        # Retry only sharing/lock violations; persistent or other errors fail.
        for attempt in range(101):
            try:
                directory.cleanup()
                break
            except PermissionError as error:
                if getattr(error, 'winerror', None) not in (32, 33) or attempt == 100:
                    raise
                if attempt == 0:
                    print('Waiting for fixture file handles to close', flush=True)
                time.sleep(.05)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--scale', type=float, choices=[1, 1.5, 2], default=1)
    parser.add_argument('--allow-native-input', action='store_true')
    args = parser.parse_args()
    if os.name != 'nt':
        parser.error('Native Windows is required')
    if not args.allow_native_input or os.environ.get('DPDF_TEST_NATIVE_DISPLAY') != '1':
        parser.error('--allow-native-input and DPDF_TEST_NATIVE_DISPLAY=1 require a dedicated test desktop')
    context = module('drop_context', 'ci/validate_windows.py').desktop_context()
    if not (context['session_id'] and context['station_visible'] and context['input_desktop_accessible']):
        parser.error('An accessible dedicated desktop is required; service mode is unsupported')
    sample, private = module('drop_sample', 'make-sample.py'), module('drop_display', 'private-display.py')
    desktop = Desktop()
    with fixture_workspace() as directory:
        root = Path(directory)
        paths = [root/'Pogodba ŽČŠ 100% #one.pdf', root/'second contract.pdf']
        for index, path in enumerate(paths):
            path.write_bytes(sample.make_pdf(50+index))
        digests = [hashlib.sha256(path.read_bytes()).digest() for path in paths]
        env = private.fixture_environment(root)
        env.update(DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()), SLINT_SCALE_FACTOR=str(args.scale))
        with (root/'reader.log').open('wb') as log:
            reader = subprocess.Popen([str(args.binary.resolve())], env=env, stdout=log, stderr=log)
            sources = []
            try:
                window = wait_for(lambda: desktop.find(reader.pid))
                wait_for(lambda: desktop.title(window) == 'dPDF — Reader preview')
                rect = desktop.rect(window)
                screen_right = desktop.user.GetSystemMetrics(76)+desktop.user.GetSystemMetrics(78)
                screen_bottom = desktop.user.GetSystemMetrics(77)+desktop.user.GetSystemMetrics(79)
                target = (min(rect.left+400, screen_right-80, rect.right-40),
                          min(rect.bottom-70, screen_bottom-70))
                for index, cancel in enumerate((True, False)):
                    folder = root/str(index)
                    folder.mkdir()
                    payload = paths if cancel else [paths[0], paths[0], paths[1]]
                    config = dict(Output=str(folder), Paths=list(map(str, payload)), Cancel=cancel,
                                  Target=int(window), X=desktop.user.GetSystemMetrics(76)+20,
                                  Y=desktop.user.GetSystemMetrics(77)+20)
                    configuration = folder/'config.json'
                    configuration.write_text(json.dumps(config), encoding='utf-8')
                    source_log = (folder/'source.log').open('wb')
                    source = subprocess.Popen(['powershell.exe', '-NoProfile', '-NonInteractive', '-STA',
                                               '-ExecutionPolicy', 'Bypass', '-File',
                                               str(Path(__file__).with_name('windows-drop-source.ps1')),
                                               '-Config', str(configuration)], env=env,
                                              stdout=source_log, stderr=source_log)
                    sources.append((source, source_log))
                    wait_for(lambda: (folder/'ready.json').exists())
                    ready = json.loads((folder/'ready.json').read_text(encoding='utf-8'))
                    if ready['Pid'] != source.pid or ready['Count'] != len(payload):
                        raise RuntimeError('OLE source identity or file count mismatch')
                    rect = desktop.rect(ready['Control'])
                    point = ((rect.left+rect.right)//2, (rect.top+rect.bottom)//2)
                    desktop.mouse(*point, source.pid)
                    desktop.mouse(*point, source.pid, flags=2)
                    for step in range(200):
                        desktop.mouse(target[0]+step%2, target[1], reader.pid)
                        if (folder/'result.json').exists():
                            break
                        if not cancel and (folder/'hover.json').exists():
                            desktop.mouse(*target, reader.pid, flags=4)
                            break
                        time.sleep(.05)
                    wait_for(lambda: (folder/'result.json').exists())
                    result = json.loads((folder/'result.json').read_text(encoding='utf-8'))
                    if 'Error' in result or not result.get('Hovered') or result.get('Effect') != (0 if cancel else 1):
                        raise RuntimeError('OLE operation did not exercise the expected path: '+str(result))
                    if desktop.held:
                        desktop.mouse(*target, reader.pid, flags=4)
                    source.wait(timeout=10)
                    if source.returncode:
                        raise RuntimeError((folder/'source.log').read_text(errors='replace'))
                    if cancel:
                        time.sleep(.2)
                        if desktop.title(window) != 'dPDF — Reader preview':
                            raise RuntimeError('Cancelled native hover opened a document')
                        print('PASS: native OLE hover/cancel left the reader empty', flush=True)
                wait_for(lambda: desktop.title(window).endswith(paths[1].name+' — dPDF'))
                for path in (paths[0], paths[1], paths[0]):
                    desktop.next_tab(reader.pid)
                    wait_for(lambda: desktop.title(window).endswith(path.name+' — dPDF'))
                if digests != [hashlib.sha256(path.read_bytes()).digest() for path in paths]:
                    raise RuntimeError('Native file drop changed a generated input')
            except Exception:
                print((root/'reader.log').read_text(errors='replace')[-2000:])
                for source, source_log in sources:
                    source_log.flush()
                for path in root.glob('*/source.log'):
                    print(path.read_text(errors='replace')[-2000:])
                raise
            finally:
                for source, source_log in sources:
                    stop(source)
                    source_log.close()
                try:
                    desktop.restore()
                finally:
                    stop(reader)
    print(f'PASS: native Windows OLE Unicode/multiple/duplicate drops, keyboard tabs and fixture cleanup, scale={args.scale}')


if __name__ == '__main__':
    main()
