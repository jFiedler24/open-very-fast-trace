#!/usr/bin/env python3
"""Smoke test for `cargo-ovft ovft --lsp`. Usage: lsp_smoke.py <binary> <workspace>"""
import json, subprocess, sys

binary, root = sys.argv[1], sys.argv[2]


def msg(o):
    b = json.dumps(o).encode()
    return b"Content-Length: %d\r\n\r\n" % len(b) + b


inp = msg({"jsonrpc": "2.0", "id": 1, "method": "initialize",
           "params": {"capabilities": {}, "rootUri": "file://" + root}})
inp += msg({"jsonrpc": "2.0", "method": "initialized", "params": {}})
inp += msg({"jsonrpc": "2.0", "id": 2, "method": "shutdown"})
inp += msg({"jsonrpc": "2.0", "method": "exit"})
p = subprocess.run([binary, "ovft", "--lsp"], input=inp, capture_output=True, timeout=60)
out = p.stdout.decode()
print("exit", p.returncode, "bytes", len(out))
print(out[:600])
print("diagnostic notifications:", out.count("publishDiagnostics"))
print(p.stderr.decode()[:300])
