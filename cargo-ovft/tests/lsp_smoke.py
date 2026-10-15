#!/usr/bin/env python3
"""End-to-end test for `cargo-ovft ovft --lsp` against a fixture workspace.

Usage: lsp_smoke.py <path-to-cargo-ovft-binary>
Speaks real LSP over stdio: initialize, diagnostics, hover, definition.
"""
import json, os, subprocess, sys, tempfile, threading, queue

binary = os.path.abspath(sys.argv[1])

ws = tempfile.mkdtemp(prefix="ovft lsp ")  # space on purpose: tests URI encoding
os.makedirs(f"{ws}/src"); os.makedirs(f"{ws}/docs")
open(f"{ws}/Cargo.toml", "w").write('[package]\nname="x"\nversion="0.1.0"\n')
open(f"{ws}/docs/spec.md", "w").write(
    "# Spec\n\n## Alpha\n`req~alpha~1`\n\nNeeds: impl\n\n## Lonely\n`req~lonely~1`\n\nNeeds: impl\n")
open(f"{ws}/src/lib.rs", "w").write("// [impl->req~alpha~1]\nfn a() {}\n")

proc = subprocess.Popen([binary, "ovft", "--lsp"], stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE, cwd=ws)
inbox = queue.Queue()


def reader():
    while True:
        hdr = proc.stdout.readline()
        if not hdr:
            return
        n = int(hdr.split(b":")[1])
        proc.stdout.readline()
        inbox.put(json.loads(proc.stdout.read(n)))


threading.Thread(target=reader, daemon=True).start()


def send(o):
    b = json.dumps({"jsonrpc": "2.0", **o}).encode()
    proc.stdin.write(b"Content-Length: %d\r\n\r\n" % len(b) + b)
    proc.stdin.flush()


def wait(pred, what, t=30):
    while True:
        try:
            m = inbox.get(timeout=t)
        except queue.Empty:
            sys.exit(f"FAIL: timeout waiting for {what}")
        if pred(m):
            return m


def uri(p):
    return "file://" + p.replace(" ", "%20")


def check(cond, msg):
    print(("ok   " if cond else "FAIL ") + msg)
    if not cond:
        proc.kill(); sys.exit(1)


send({"id": 1, "method": "initialize", "params": {"capabilities": {}, "rootUri": uri(ws)}})
caps = wait(lambda m: m.get("id") == 1, "initialize")["result"]["capabilities"]
check(caps.get("hoverProvider") and caps.get("definitionProvider"), "capabilities advertised")
send({"method": "initialized", "params": {}})

diag = wait(lambda m: m.get("method") == "textDocument/publishDiagnostics"
            and m["params"]["uri"].endswith("spec.md") and m["params"]["diagnostics"], "spec diagnostics")
lines = {d["range"]["start"]["line"] for d in diag["params"]["diagnostics"]}
check(7 in lines or 8 in lines, f"uncovered 'lonely' flagged (lines {sorted(lines)})")
check(not (3 in lines and 2 in lines), "covered 'alpha' not flagged")

lib, spec = f"{ws}/src/lib.rs", f"{ws}/docs/spec.md"
send({"method": "textDocument/didOpen", "params": {"textDocument": {
    "uri": uri(lib), "languageId": "rust", "version": 1, "text": open(lib).read()}}})

pos = {"textDocument": {"uri": uri(lib)}, "position": {"line": 0, "character": 15}}
send({"id": 2, "method": "textDocument/hover", "params": pos})
hov = wait(lambda m: m.get("id") == 2, "hover")["result"]
check(hov and "req~alpha~1" in hov["contents"]["value"], "hover shows requirement")

send({"id": 3, "method": "textDocument/definition", "params": pos})
d = wait(lambda m: m.get("id") == 3, "definition")["result"]
check(d and d[0]["uri"].endswith("docs/spec.md"), "definition jumps tag -> spec")

send({"id": 4, "method": "shutdown"})
wait(lambda m: m.get("id") == 4, "shutdown")
send({"method": "exit"})
check(proc.wait(timeout=10) == 0, "clean exit")
print("ALL PASSED")
