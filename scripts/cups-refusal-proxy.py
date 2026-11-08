"""Loopback-only test proxy: refuse Print-Job once, forward metadata to private CUPS."""
import contextlib, http.client, http.server, struct

class Server(http.server.ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, target_port, ipp_only=False):
        self.target_port = target_port
        self.refused_jobs = 0
        self.ipp_only = ipp_only
        super().__init__(('127.0.0.1', 0), Handler)

class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *args):
        pass

    def body(self):
        if self.headers.get('Transfer-Encoding', '').lower() == 'chunked':
            result = bytearray()
            while True:
                size = int(self.rfile.readline().split(b';', 1)[0], 16)
                if size == 0:
                    while self.rfile.readline().strip():
                        pass
                    break
                result.extend(self.rfile.read(size))
                assert self.rfile.read(2) == b'\r\n'
            return bytes(result)
        return self.rfile.read(int(self.headers.get('Content-Length', '0')))

    def reply(self, status, body, headers):
        self.send_response(status)
        for k, v in headers:
            if k.lower() not in ('content-length', 'transfer-encoding', 'connection', 'server', 'date'):
                self.send_header(k, v)
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Connection', 'close')
        self.end_headers()
        self.wfile.write(body)

    def forward(self, body):
        headers = {k: v for k, v in self.headers.items() if k.lower() not in ('content-length', 'transfer-encoding', 'connection', 'expect', 'host')}
        with contextlib.closing(http.client.HTTPConnection('127.0.0.1', self.server.target_port, timeout=15)) as conn:
            conn.request(self.command, self.path, body=body, headers=headers)
            r = conn.getresponse()
            self.reply(r.status, r.read(), r.getheaders())

    def do_GET(self):
        self.forward(b'')

    def do_POST(self):
        body = self.body()
        if len(body) >= 8 and body[2:4] in (b'\x00\x02', b'\x00\x05', b'\x00\x06'):
            self.server.refused_jobs += 1

            def attr(tag, name, value):
                return bytes([tag]) + struct.pack('>H', len(name)) + name + struct.pack('>H', len(value)) + value
            reply = body[:2] + b'\x04\x04' + body[4:8] + b'\x01' + attr(71, b'attributes-charset', b'utf-8') + attr(72, b'attributes-natural-language', b'en') + attr(65, b'status-message', b'controlled submission refusal') + b'\x03'
            self.reply(200 if self.server.ipp_only else 400, reply, [('Content-Type', 'application/ipp')])
        else:
            self.forward(body)
