#!/usr/bin/env python3
"""Serves the live-test checklist page on the dev box and keeps the results in a file any session can read.

usage: checklist_server.py [--bind HOST:PORT] [--results FILE]     serve (default 0.0.0.0:30880, logs/live-test-results.json)
       checklist_server.py --report [--results FILE]               print every marked item as text, as the page's "Copy results"

The page is built from docs/testing/live-test-checklist.md on every load (tools/checklist_page.py), so an edit to the file shows at the next
reload with nothing to publish. Results are one JSON object keyed by the item ids of checklist_page.py (`s<section>-<sha1 8 hex>`), each
`{status, note, at, section, item}`; a reworded item gets a new id and its old result stays in the file, unmatched (`--report` lists those
apart). Only loopback, private and Tailscale (100.64.0.0/10) addresses may connect: there is no login. A request must also be addressed
to an IP address or to `localhost`, or to a name in CHECKLIST_HOSTS (comma separated), so a web page that rebinds its own name to this
box is refused (docs/server/audit.md R17). The page is reached by the box's address (http://192.168.x.x:30880/), as before.
"""
import argparse
import ipaddress
import json
import os
import re
import sys
import tempfile
import threading
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from checklist_page import ROOT, SOURCE, TEMPLATE, parse  # noqa: E402

EXTRA_HOSTS = {h.strip().lower() for h in os.environ.get("CHECKLIST_HOSTS", "").split(",") if h.strip()}
DEFAULT_RESULTS = ROOT / "logs" / "live-test-results.json"
ID = re.compile(r"^s\d+-[0-9a-f]{8}$")
STATUSES = {"", "pass", "fail", "skip"}
LABELS = {"pass": "Passed", "fail": "Failed", "skip": "Can't test"}
TAILSCALE = ipaddress.ip_network("100.64.0.0/10")
LOCK = threading.Lock()


def load(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return {}


def save(path, results):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=path.parent, prefix=".live-test-results.")
    with os.fdopen(fd, "w", encoding="utf-8") as fh:
        json.dump(results, fh, ensure_ascii=False, indent=1, sort_keys=True)
    os.replace(tmp, path)


def page():
    data = parse(SOURCE.read_text(encoding="utf-8"))
    data["api"] = "api/results"
    payload = json.dumps(data, ensure_ascii=False).replace("</", "<\\/")
    return TEMPLATE.read_text(encoding="utf-8").replace("/*DATA*/null", payload)


def report(path):
    """Every marked item in checklist order, then results whose item is no longer in the file."""
    results = load(path)
    data = parse(SOURCE.read_text(encoding="utf-8"))
    lines, known = [], set()
    for s in data["sections"]:
        for it in s["items"]:
            known.add(it["id"])
            r = results.get(it["id"])
            if not r or not r.get("status"):
                continue
            lines.append(f"{s['num']}. {LABELS[r['status']]} ({r.get('at', '')[:10]}): {it['text']}")
            if r.get("note"):
                lines.append(f"   Note: {r['note']}")
    orphans = [(k, v) for k, v in sorted(results.items()) if k not in known and v.get("status")]
    if orphans:
        lines.append("")
        lines.append("Results for items no longer in the file (reworded or removed):")
        for k, v in orphans:
            lines.append(f"{v.get('section', k)}: {LABELS.get(v['status'], v['status'])}: {v.get('item', '')[:200]}")
            if v.get("note"):
                lines.append(f"   Note: {v['note']}")
    return "\n".join(lines) if lines else "Nothing marked yet."


class Handler(BaseHTTPRequestHandler):
    results_path = DEFAULT_RESULTS

    def allowed(self):
        ip = ipaddress.ip_address(self.client_address[0])
        if ip.version == 6 and ip.ipv4_mapped:
            ip = ip.ipv4_mapped
        return ip.is_loopback or ip.is_private or ip in TAILSCALE

    def host_ok(self):
        """The Host header names this box by an address (a rebinding page has a name of its own), localhost or a listed name."""
        host = (self.headers.get("Host") or "").strip().lower()
        name = host[1:].split("]", 1)[0] if host.startswith("[") else host.split(":", 1)[0]
        if name == "localhost" or name in EXTRA_HOSTS:
            return True
        try:
            ipaddress.ip_address(name)
            return True
        except ValueError:
            return False

    def send(self, status, body, kind):
        data = body.encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def json(self, status, value):
        self.send(status, json.dumps(value, ensure_ascii=False), "application/json; charset=utf-8")

    def do_GET(self):
        if not self.allowed() or not self.host_ok():
            return self.send(HTTPStatus.FORBIDDEN, "forbidden", "text/plain")
        path = self.path.split("?", 1)[0]
        if path in ("/", "/index.html"):
            try:
                return self.send(HTTPStatus.OK, page(), "text/html; charset=utf-8")
            except SystemExit as e:  # parse() refuses a file where two items share an id
                return self.send(HTTPStatus.INTERNAL_SERVER_ERROR, f"checklist error: {e}", "text/plain; charset=utf-8")
        if path == "/api/results":
            with LOCK:
                return self.json(HTTPStatus.OK, load(self.results_path))
        if path == "/api/report":
            with LOCK:
                return self.send(HTTPStatus.OK, report(self.results_path), "text/plain; charset=utf-8")
        self.send(HTTPStatus.NOT_FOUND, "not found", "text/plain")

    def do_PUT(self):
        if not self.allowed() or not self.host_ok():
            return self.send(HTTPStatus.FORBIDDEN, "forbidden", "text/plain")
        m = re.match(r"^/api/results/([^/?]+)$", self.path)
        if not m or not ID.match(m.group(1)):
            return self.json(HTTPStatus.BAD_REQUEST, {"error": "bad item id"})
        length = int(self.headers.get("Content-Length") or 0)
        if length > 16384:
            return self.json(HTTPStatus.REQUEST_ENTITY_TOO_LARGE, {"error": "too large"})
        try:
            body = json.loads(self.rfile.read(length) or b"{}")
        except ValueError:
            return self.json(HTTPStatus.BAD_REQUEST, {"error": "not JSON"})
        if not isinstance(body, dict) or body.get("status", "") not in STATUSES:
            return self.json(HTTPStatus.BAD_REQUEST, {"error": "bad status"})
        entry = {k: str(body.get(k, ""))[:limit] for k, limit in (("status", 8), ("note", 4000), ("at", 40), ("section", 200), ("item", 600))}
        with LOCK:
            results = load(self.results_path)
            results[m.group(1)] = entry
            save(self.results_path, results)
        self.json(HTTPStatus.OK, {"ok": True})

    def log_message(self, fmt, *args):
        if self.command == "PUT":
            sys.stderr.write(f"{self.log_date_time_string()} {self.client_address[0]} {fmt % args}\n")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--bind", default=os.environ.get("CHECKLIST_BIND", "0.0.0.0:30880"))
    ap.add_argument("--results", type=Path, default=Path(os.environ.get("CHECKLIST_RESULTS", DEFAULT_RESULTS)))
    ap.add_argument("--report", action="store_true", help="print the marked items and exit")
    args = ap.parse_args()
    if args.report:
        print(report(args.results))
        return
    host, _, port = args.bind.rpartition(":")
    Handler.results_path = args.results
    server = ThreadingHTTPServer((host or "0.0.0.0", int(port)), Handler)
    print(f"live-test checklist on http://{host or '0.0.0.0'}:{port}/ (results: {args.results})", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
