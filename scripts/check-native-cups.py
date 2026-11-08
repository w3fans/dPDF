#!/usr/bin/env python3
"""Controlled IPP/CUPS plumbing proof; requires disposable private network namespace."""
import argparse, grp, hashlib, importlib.util, json, os, pwd, socket, subprocess, tempfile, time
from pathlib import Path
p = argparse.ArgumentParser()
p.add_argument('--binary', type=Path, required=True)
p.add_argument('--user', required=True, help='Dedicated non-root disposable test account')
p.add_argument('--range', default='7')
p.add_argument('--cancel', action='store_true')
p.add_argument('--mutate-source', action='store_true')
p.add_argument('--expect-failure', action='store_true')
p.add_argument('--expect-raster', action='store_true')
p.add_argument('--expect-subset', action='store_true')
p.add_argument('--refuse-subset', action='store_true', help='Controlled qpdf preparation failure; requires raster fallback')
p.add_argument('--ipp-refusal', action='store_true')
a = p.parse_args()
assert not (a.expect_raster and a.expect_subset), 'Choose one expected print transport'
assert not a.refuse_subset or a.expect_raster, 'Preparation refusal must exercise the raster fallback'
assert os.getuid() == 0 and os.environ.get('DPDF_DISPOSABLE_PRINT_ROOT') == '1', 'Requires explicit disposable-root opt-in'
assert set(os.listdir('/sys/class/net')) <= {'lo'}, 'Refuse printer tests with external network interfaces'
account = pwd.getpwnam(a.user)
uid, gid = (account.pw_uid, account.pw_gid)
assert uid != 0 and gid != 0
scripts = Path(__file__).resolve().parent
routes = Path('/proc/net/route').read_text().splitlines()[1:]
assert all((line.split()[0] == 'lo' for line in routes)), 'Refuse printer tests with an external network route'
root = Path(tempfile.mkdtemp(prefix='dpdf-private-cups-'))
root.chmod(0o755)
for name in ['server', 'state', 'spool', 'cache', 'temp', 'ipp']:
    (root / name).mkdir(mode=0o755)
os.chown(root / 'ipp', uid, gid)

def port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]
ipp_port, cups_port = (port(), port())
assert ipp_port != cups_port
files = root / 'cups-files.conf'
files.write_text('\n'.join(['User ' + a.user, 'Group ' + grp.getgrgid(gid).gr_name, 'SystemGroup root', f'ServerRoot {root}/server', f'StateDir {root}/state', f'RequestRoot {root}/spool', f'CacheDir {root}/cache', f'TempDir {root}/temp', 'DataDir /usr/share/cups', 'ServerBin /usr/lib/cups', f'AccessLog {root}/access_log', f'ErrorLog {root}/error_log', f'PageLog {root}/page_log']) + '\n')
config = root / 'cupsd.conf'
config.write_text(f'''Listen 127.0.0.1:{cups_port}
Browsing Off
WebInterface No
DefaultAuthType None
DefaultEncryption Never
PreserveJobFiles Yes
PreserveJobHistory Yes
LogLevel debug
<Location />
Order allow,deny
Allow localhost
</Location>
<Location /admin>
AuthType None
Order allow,deny
Allow localhost
</Location>
<Policy default>
<Limit All>
Order allow,deny
Allow localhost
</Limit>
</Policy>
''')

def drop():
    os.setgroups([])
    os.setgid(gid)
    os.setuid(uid)
env = os.environ | dict(CUPS_SERVER=f'127.0.0.1:{cups_port}', HOME=account.pw_dir, USER=a.user, LOGNAME=a.user)
if a.refuse_subset:
    wrapper = root / 'qpdf-refuse-subset'
    wrapper.write_text("#!/usr/bin/python3\nimport os, sys\nif any(arg.startswith('--job-json-file=') for arg in sys.argv[1:]):\n    sys.exit(2)\nos.execv('/usr/bin/qpdf', ['qpdf', *sys.argv[1:]])\n")
    wrapper.chmod(0o755)
    env['DPDF_QPDF_BINARY'] = str(wrapper)
proxy = None
proxy_thread = None
processes = []
logs = []
report = dict(root=str(root), network_namespace=os.readlink('/proc/self/ns/net'), external_routes=False, status='FAIL', physical_printer=False, subset_preparation_refused=a.refuse_subset)

def start(args, **kw):
    stream = (root / (str(len(logs)) + '.log')).open('w')
    logs.append(stream)
    proc = subprocess.Popen(args, stdout=stream, stderr=subprocess.STDOUT, **kw)
    processes.append(proc)
    return proc
try:
    bus = root / 'dbus.conf'
    bus.write_text('<busconfig><type>system</type><listen>unix:path=' + str(root / 'system_bus_socket') + '</listen><auth>EXTERNAL</auth><policy context="default"><allow user="*"/><allow own="*"/><allow send_destination="*"/><allow receive_sender="*"/></policy></busconfig>')
    env['DBUS_SYSTEM_BUS_ADDRESS'] = 'unix:path=' + str(root / 'system_bus_socket')
    start(['dbus-daemon', '--nofork', '--config-file', str(bus)], env=env)
    deadline = time.monotonic() + 5
    while not (root / 'system_bus_socket').exists():
        assert time.monotonic() < deadline, 'Private D-Bus did not start'
        time.sleep(0.1)
    avahi = root / 'avahi.conf'
    avahi.write_text('[server]\nuse-ipv4=yes\nuse-ipv6=no\nallow-interfaces=lo\nenable-dbus=yes\n[wide-area]\nenable-wide-area=no\n[publish]\ndisable-publishing=yes\n')
    start(['avahi-daemon', '--no-drop-root', '--no-chroot', '--no-rlimits', '-f', str(avahi)], env=env)
    time.sleep(0.5)
    start(['ippeveprinter', '-r', 'off', '-n', 'localhost', '-p', str(ipp_port), '-d', str(root / 'ipp'), '-k', '-f', 'application/pdf', '-c', '/usr/bin/true', 'dPDF controlled PDF receiver'], env=env, preexec_fn=drop)
    start(['cupsd', '-f', '-c', str(config), '-s', str(files)], env=env)
    for number in (ipp_port, cups_port):
        deadline = time.monotonic() + 15
        while True:
            try:
                with socket.create_connection(('127.0.0.1', number), timeout=0.2):
                    break
            except OSError:
                if time.monotonic() > deadline:
                    raise RuntimeError('Private printer service did not start: ' + str(number))
                time.sleep(0.1)
    command = ['lpadmin', '-h', env['CUPS_SERVER'], '-p', 'dPDF-controlled', '-E', '-v', f'ipp://localhost:{ipp_port}/ipp/print', '-m', 'everywhere']
    r = subprocess.run(command, env=env, text=True, capture_output=True, timeout=30)
    (root / 'lpadmin.log').write_text(r.stdout + r.stderr)
    assert r.returncode == 0, r.stderr
    spec = importlib.util.spec_from_file_location('sample', scripts / 'make-sample.py')
    sample = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(sample)
    source = root / 'controlled-50-pages.pdf'
    source.write_bytes(sample.make_pdf(50, print_marker=True))
    os.chown(source, uid, gid)
    original = source.read_bytes()
    reference = root / 'original-generated.pdf'
    reference.write_bytes(original)
    if a.expect_failure:
        assert a.binary, 'Failure test requires candidate application'
        spec = importlib.util.spec_from_file_location('proxy', scripts / 'cups-refusal-proxy.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        import threading
        proxy = module.Server(cups_port, ipp_only=a.ipp_refusal)
        proxy_thread = threading.Thread(target=proxy.serve_forever, daemon=True)
        proxy_thread.start()
        env['CUPS_SERVER'] = '127.0.0.1:' + str(proxy.server_port)
        env['DPDF_PRIVATE_PROXY_REFUSES_PRINT_JOB'] = '1'
    client = root / 'client'
    client.mkdir()
    os.chown(client, uid, gid)
    home = client / 'home'
    home.mkdir()
    os.chown(home, uid, gid)
    env['HOME'] = str(home)
    extra = []
    for flag in ('cancel', 'mutate_source', 'expect_failure', 'expect_raster'):
        if getattr(a, flag):
            extra.append('--' + flag.replace('_', '-'))
    r = subprocess.run(['python3', str(scripts / 'native-cups-client.py'), '--binary', str(a.binary), '--source', str(source), '--outdir', str(client), '--range', a.range, *extra], env=env, preexec_fn=drop, text=True, capture_output=True, timeout=100)
    (root / 'client.log').write_text(r.stdout + r.stderr)
    assert r.returncode == 0, r.stdout + r.stderr
    report['app_receipt'] = json.loads((client / 'client-report.json').read_text())
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        state = subprocess.run(['lpstat', '-W', 'not-completed', '-o'], env=env, text=True, capture_output=True)
        if not state.stdout.strip():
            break
        time.sleep(0.2)
    assert not state.stdout.strip(), 'Controlled CUPS job did not complete'
    spool = list((root / 'spool').glob('d*'))
    received = [x for x in (root / 'ipp').iterdir() if x.is_file() and x.read_bytes().startswith(b'%PDF-')]
    if a.cancel or a.expect_failure:
        assert not spool and (not received), 'Cancelled/failed submission printed a job'
        if a.expect_failure:
            assert proxy.refused_jobs == 1, proxy.refused_jobs
        report.update(no_job_submitted=True, refused_requests=proxy.refused_jobs if proxy else 0)
    else:
        assert len(spool) == 1, spool
        if not a.expect_raster and not a.expect_subset:
            assert spool[0].read_bytes() == original, 'Client spool differs from immutable opened snapshot'
        assert len(received) == 1, received
        selected = set(range(1, 51)) if not a.range else set()
        for part in a.range.split(',') if a.range else []:
            first, last = part.split('-') if '-' in part else (part, part)
            selected.update(range(int(first), int(last) + 1))
        expected_text = ''.join((subprocess.check_output(['pdftotext', '-f', str(i), '-l', str(i), str(reference), '-'], text=True) for i in sorted(selected)))
        actual_text = subprocess.check_output(['pdftotext', str(received[0]), '-'], text=True)
        if a.expect_subset:
            assert int(subprocess.check_output(['qpdf', '--show-npages', str(spool[0])])) == len(selected), 'Client PDF contains unselected pages'
            spool_text = subprocess.check_output(['pdftotext', str(spool[0]), '-'], text=True)
            assert spool_text.split() == expected_text.split(), 'Client subset text/order differs from opened snapshot'
        if not a.expect_raster:
            assert expected_text.split() == actual_text.split(), 'Selected page text/order changed'
        document = json.loads(subprocess.check_output(['qpdf', '--json', str(received[0])], text=True))
        assert len(document['pages']) == len(selected)

        def count_images(value):
            if isinstance(value, dict):
                return int(value.get('/Subtype') == '/Image') + sum((count_images(x) for x in value.values()))
            if isinstance(value, list):
                return sum((count_images(x) for x in value))
            return 0
        images = count_images(document)
        if a.expect_raster:
            assert images > 0, 'Raster fallback did not contain rendered page images'
        else:
            assert images == 0, 'Vector fixture was rasterized'
        report.update(receiver_pages=len(selected), client_spool_identical=spool[0].read_bytes() == original, source_sha256=hashlib.sha256(original).hexdigest(), selected_text_preserved=expected_text.split() == actual_text.split(), image_xobjects=images, fixture_path_changed=source.read_bytes() != original)
    report.update(status='PASS', source_bytes=source.stat().st_size, spool_files=[dict(name=x.name, bytes=x.stat().st_size) for x in (root / 'spool').glob('d*')], receiver_files=[dict(name=x.name, bytes=x.stat().st_size) for x in (root / 'ipp').iterdir() if x.is_file()])
except Exception as e:
    report['error'] = str(e)
finally:
    if proxy:
        proxy.shutdown()
        proxy.server_close()
        proxy_thread.join(timeout=5)
    for proc in reversed(processes):
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
    for log in logs:
        log.close()
    (root / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2), flush=True)
raise SystemExit(0 if report['status'] == 'PASS' else 1)
