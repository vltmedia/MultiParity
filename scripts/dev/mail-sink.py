#!/usr/bin/env python3
"""Catch email for tests: a bare SMTP server and a fake SendGrid API, both writing each message
it gets to a folder as a file (SMTP: the raw message; SendGrid: the JSON body plus the API key).

    mail-sink.py smtp <port> <folder>
    mail-sink.py sendgrid <port> <folder>
"""
import json
import os
import socketserver
import sys
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

mode, port, folder = sys.argv[1], int(sys.argv[2]), sys.argv[3]
os.makedirs(folder, exist_ok=True)


def save(ext, data):
    name = os.path.join(folder, f"{time.time_ns()}.{ext}")
    with open(name + ".tmp", "wb") as f:
        f.write(data)
    os.replace(name + ".tmp", name)


class Smtp(socketserver.StreamRequestHandler):
    def reply(self, line):
        self.wfile.write((line + "\r\n").encode())

    def handle(self):
        self.reply("220 mail-sink ready")
        rcpt = []
        while True:
            line = self.rfile.readline()
            if not line:
                return
            cmd = line.decode(errors="replace").strip()
            up = cmd.upper()
            if up.startswith("EHLO"):
                self.wfile.write(b"250-mail-sink\r\n250-8BITMIME\r\n250 SMTPUTF8\r\n")
            elif up.startswith("HELO"):
                self.reply("250 mail-sink")
            elif up.startswith("MAIL FROM"):
                rcpt = []
                self.reply("250 OK")
            elif up.startswith("RCPT TO"):
                rcpt.append(cmd[8:].strip())
                self.reply("250 OK")
            elif up == "DATA":
                self.reply("354 go ahead")
                body = []
                while True:
                    l = self.rfile.readline()
                    if l in (b".\r\n", b".\n", b""):
                        break
                    body.append(l[1:] if l.startswith(b"..") else l)
                save("eml", ("X-Rcpt: " + ",".join(rcpt) + "\r\n").encode() + b"".join(body))
                self.reply("250 OK queued")
            elif up in ("RSET", "NOOP"):
                self.reply("250 OK")
            elif up == "QUIT":
                self.reply("221 bye")
                return
            else:
                self.reply("502 not here")


class SendGrid(BaseHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("content-length", 0)))
        if self.path != "/v3/mail/send":
            self.send_response(404)
            self.end_headers()
            return
        save("json", json.dumps({"auth": self.headers.get("authorization"), "body": json.loads(body)}).encode())
        self.send_response(202)
        self.end_headers()

    def log_message(self, *a):
        pass


if mode == "smtp":
    socketserver.ThreadingTCPServer.allow_reuse_address = True
    socketserver.ThreadingTCPServer(("127.0.0.1", port), Smtp).serve_forever()
else:
    HTTPServer(("127.0.0.1", port), SendGrid).serve_forever()
