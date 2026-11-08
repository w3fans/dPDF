"""Private X display, or explicitly enabled native Windows fixture windows."""
from contextlib import contextmanager
import os
from pathlib import Path
import select
import struct
import subprocess


def fixture_environment(folder: Path):
    env = dict(os.environ, DPDF_CONFIG_DIR=str(folder / 'test-config'),
               DPDF_DATA_DIR=str(folder / 'test-data'), SLINT_BACKEND='winit-software')
    for key in list(env):
        if key in ('DISPLAY', 'WAYLAND_DISPLAY', 'XAUTHORITY', 'GDK_BACKEND') or key.startswith('DPDF_SMOKE_'):
            env.pop(key)
    return env


@contextmanager
def display(folder: Path, xvfb: str):
    if os.name == 'nt':
        if os.environ.get('DPDF_TEST_NATIVE_DISPLAY') != '1':
            raise RuntimeError('On Windows set DPDF_TEST_NATIVE_DISPLAY=1 to use native fixture windows with isolated data/config; no private desktop is provided')
        yield fixture_environment(folder)
        return
    auth = folder / 'authority'
    cookie = os.urandom(16)

    def authority(number):
        fields = (b'', number.encode(), b'MIT-MAGIC-COOKIE-1', cookie)
        auth.write_bytes(struct.pack('>H', 65535) + b''.join(struct.pack('>H', len(f)) + f for f in fields))
        auth.chmod(0o600)

    authority('')
    reader, writer = os.pipe()
    with (folder / 'xvfb.log').open('wb') as log:
        server = subprocess.Popen([xvfb, '-displayfd', str(writer), '-screen', '0', '1280x900x24', '-nolisten', 'tcp', '-noreset', '-auth', str(auth)], pass_fds=(writer,), stdout=log, stderr=log)
        os.close(writer)
        try:
            if not select.select([reader], [], [], 10)[0]:
                raise RuntimeError('Private X display did not start')
            number = os.read(reader, 64).decode().strip()
            if not number.isdecimal():
                raise RuntimeError('Private X display failed')
            authority(number)
            env = fixture_environment(folder)
            env.update(DISPLAY=':' + number, XAUTHORITY=str(auth), GDK_BACKEND='x11')
            yield env
        finally:
            os.close(reader)
            server.terminate()
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
