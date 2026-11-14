"""Benchmarks jarsnitch on the most-downloaded server plugins on Modrinth.

    python3 -I bench.py corpus [N]       pin the top N projects (default 100) with a server-plugin
                                         build, and their previous release, into corpus.json
    python3 -I bench.py fetch CACHE      download corpus.json into CACHE/latest and CACHE/previous,
                                         checking each file's SHA-512
    python3 -I bench.py run CACHE OUT    time scans and diffs with the release binary, write
                                         OUT/results.json (per plugin) and OUT/summary.md (totals)

Build the binary first: cargo build --release -p jarsnitch. Nothing here runs the jars.
Not part of CI.
"""

import hashlib
import json
import platform
import statistics
import subprocess
import sys
import time
import urllib.parse
import urllib.request
from collections import Counter
from datetime import date
from pathlib import Path

HERE = Path(__file__).resolve().parent
CORPUS = HERE / "corpus.json"
ROOT = HERE.parent.parent
UA = {"User-Agent": "jarsnitch-bench"}
BUKKIT = ["paper", "spigot", "bukkit", "folia", "purpur"]
PROXY = ["velocity", "bungeecord", "waterfall"]
RUNS = 3


def get(url):
    request = urllib.request.Request(url, headers=UA)
    for attempt in range(3):
        try:
            with urllib.request.urlopen(request, timeout=60) as r:
                return r.read()
        except OSError:
            time.sleep(2 + 3 * attempt)
    with urllib.request.urlopen(request, timeout=60) as r:
        return r.read()


def api(path, **params):
    return json.loads(get(f"https://api.modrinth.com/v2/{path}?{urllib.parse.urlencode(params)}"))


def corpus(n):
    hits = []
    while len(hits) < n:
        page = api("search", index="downloads", limit=min(100, n - len(hits)), offset=len(hits), facets=json.dumps([["project_type:plugin"]]))["hits"]
        if not page:
            break
        hits += page
    plugins = []
    for h in hits[:n]:
        for loaders in (BUKKIT, PROXY):
            versions = api(f"project/{h['project_id']}/version", loaders=json.dumps(loaders))
            if versions:
                break
        releases = [v for v in versions if v["version_type"] == "release"] or versions
        files = []
        for v in releases[:2]:
            f = next((f for f in v["files"] if f["primary"]), v["files"][0])
            files.append({"version": v["version_number"], "published": v["date_published"][:10], "url": f["url"], "size": f["size"], "sha512": f["hashes"]["sha512"]})
        if files:
            plugins.append({"slug": h["slug"], "title": h["title"], "downloads": h["downloads"], "files": files})
        time.sleep(0.2)
    CORPUS.write_text(json.dumps({"pinned": date.today().isoformat(), "source": "Modrinth search, project_type:plugin, sorted by downloads", "plugins": plugins}, indent=1) + "\n", encoding="utf-8")
    print(f"pinned {len(plugins)} plugins, {sum(len(p['files']) > 1 for p in plugins)} with a previous release")


def fetch(cache):
    for p in json.loads(CORPUS.read_text(encoding="utf-8"))["plugins"]:
        for side, f in zip(("latest", "previous"), p["files"]):
            dest = Path(cache, side, p["slug"] + ".jar")
            if dest.exists() and hashlib.sha512(dest.read_bytes()).hexdigest() == f["sha512"]:
                continue
            data = get(f["url"])
            if hashlib.sha512(data).hexdigest() != f["sha512"]:
                sys.exit(f"{p['slug']} {f['version']}: SHA-512 mismatch")
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(data)
    print("fetched")


def timed(cmd):
    """Median wall time in ms of RUNS runs, output discarded."""
    times = []
    for _ in range(RUNS):
        t = time.perf_counter()
        subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        times.append((time.perf_counter() - t) * 1000)
    return statistics.median(times)


def as_json(cmd):
    p = subprocess.run(cmd, capture_output=True)
    if p.returncode not in (0, 2):
        return None
    return json.loads(p.stdout)


def run(cache, out, binary):
    cache, out = Path(cache), Path(out)
    out.mkdir(parents=True, exist_ok=True)
    version = subprocess.run([binary, "--version"], capture_output=True, text=True).stdout.strip()
    commit = subprocess.run(["git", "-C", str(ROOT), "describe", "--always", "--dirty"], capture_output=True, text=True).stdout.strip()
    rows = []
    for p in json.loads(CORPUS.read_text(encoding="utf-8"))["plugins"]:
        latest, previous = cache / "latest" / (p["slug"] + ".jar"), cache / "previous" / (p["slug"] + ".jar")
        reports = as_json([binary, "scan", str(latest), "--json"])
        row = {"slug": p["slug"], "title": p["title"], "downloads": p["downloads"], "version": p["files"][0]["version"], "size": latest.stat().st_size, "scan_ms": timed([binary, "scan", str(latest)])}
        if reports:
            r = reports[0]
            row |= {
                "platforms": [x["platform"] for x in r["plugins"]],
                "classes": r["stats"]["classes"],
                "partial": r["partial"],
                "obfuscation": r["obfuscation"]["score"],
                "libraries": len(r["libraries"]),
                "nested": len(r["nested"]),
                "call_graph": r["reachability"] is not None,
                "dynamic": bool(r["reachability"] and r["reachability"]["dynamic"]),
                "findings": [{"rule": f["rule"], "severity": f["severity"], "reached": f["reached"]} for f in r["findings"]],
            }
        if previous.exists():
            d = as_json([binary, "diff", str(previous), str(latest), "--json"])
            if d:
                findings = [i["finding"] for i in d["added"] if i["kind"] == "finding"]
                row["diff"] = {
                    "from": p["files"][1]["version"],
                    "ms": timed([binary, "diff", str(previous), str(latest)]),
                    "added": Counter(f["severity"] for f in findings),
                    "added_reached_high": sum(f["severity"] == "high" and f["reached"] is not False for f in findings),
                    "new_classes": sum(i["kind"] == "class" for i in d["added"]),
                    "kinds": Counter(i["kind"] for i in d["added"]),
                    "changed": [c["key"] for c in d["changed"] if c["kind"] == "plugin-field"],
                    "warnings": [w["code"] for w in d["warnings"]],
                }
        rows.append(row)
        print(f"{p['slug']:<28} {row['scan_ms']:8.0f} ms", flush=True)
    folder_ms = timed([binary, "scan", str(cache / "latest")])
    meta = {"date": date.today().isoformat(), "jarsnitch": f"{version} ({commit})", "machine": platform.processor() or platform.machine(), "os": platform.platform(), "runs": RUNS, "folder_scan_ms": folder_ms}
    (out / "results.json").write_text(json.dumps({"meta": meta, "plugins": rows}, indent=1), encoding="utf-8")
    (out / "summary.md").write_text(summary(meta, rows), encoding="utf-8")
    print(f"wrote {out / 'results.json'} and {out / 'summary.md'}")


def summary(meta, rows):
    scanned = [r for r in rows if "findings" in r]
    mb = sum(r["size"] for r in rows) / 1e6
    classes = sum(r["classes"] for r in scanned)
    times = sorted(r["scan_ms"] for r in rows)
    slowest = max(rows, key=lambda r: r["scan_ms"])
    lines = [
        f"# jarsnitch benchmark, {meta['date']}",
        "",
        f"{meta['jarsnitch']} · {meta['machine']} · {meta['os']} · median of {meta['runs']} runs",
        "",
        "## Speed (native CLI, one thread, wall time including process start)",
        "",
        f"- {len(rows)} jars, {mb:.0f} MB, {classes:,} classes",
        f"- Whole folder: {meta['folder_scan_ms'] / 1000:.2f} s ({mb / (meta['folder_scan_ms'] / 1000):.0f} MB/s, {classes / (meta['folder_scan_ms'] / 1000):,.0f} classes/s)",
        f"- Per jar: median {statistics.median(times):.0f} ms, 90th percentile {times[int(len(times) * 0.9)]:.0f} ms, slowest {slowest['title']} ({slowest['size'] / 1e6:.0f} MB) {slowest['scan_ms']:.0f} ms",
        "",
        "## What the plugins can do (plugins with at least one finding of the rule)",
        "",
        "| Rule | Severity | Plugins | In code the entry points reach |",
        "|---|---|---|---|",
    ]
    by_rule = {}
    for r in scanned:
        for f in r["findings"]:
            e = by_rule.setdefault(f["rule"], {"severity": f["severity"], "any": set(), "reached": set()})
            e["any"].add(r["slug"])
            if f["reached"] is not False:
                e["reached"].add(r["slug"])
    for rule, e in sorted(by_rule.items(), key=lambda kv: -len(kv[1]["any"])):
        lines.append(f"| `{rule}` | {e['severity']} | {len(e['any'])} | {len(e['reached'])} |")
    serious = [f for r in scanned for f in r["findings"] if f["severity"] in ("high", "notable")]
    unreached = sum(f["reached"] is False for f in serious)
    high = [r for r in scanned if any(f["severity"] == "high" for f in r["findings"])]
    high_reached = [r for r in scanned if any(f["severity"] == "high" and f["reached"] is not False for f in r["findings"])]
    lines += [
        "",
        "## Call graph",
        "",
        f"- {len(serious)} notable or high findings; {unreached} ({unreached / max(1, len(serious)):.0%}) sit in code no entry point calls",
        f"- Plugins with a High finding: {len(high)}; with a High finding in reached code: {len(high_reached)}",
        f"- Call graph built for {sum(r['call_graph'] for r in scanned)} of {len(scanned)} jars; {sum(r['dynamic'] for r in scanned)} also load code at runtime",
        f"- Obfuscation score 25 or more: {sum(r['obfuscation'] >= 25 for r in scanned)}; partial reports: {sum(r['partial'] for r in scanned)}",
    ]
    diffs = [r["diff"] for r in rows if "diff" in r]
    if diffs:
        quiet = sum(not d["added"].get("high") and not d["added"].get("notable") for d in diffs)
        lines += [
            "",
            "## Real updates (previous release -> latest)",
            "",
            f"- {len(diffs)} updates; diff median {statistics.median(d['ms'] for d in diffs):.0f} ms",
            f"- Added no notable or high finding: {quiet}",
            f"- Added a High finding: {sum(bool(d['added'].get('high')) for d in diffs)} (in reached code: {sum(d['added_reached_high'] > 0 for d in diffs)})",
            f"- Added a notable finding: {sum(bool(d['added'].get('notable')) for d in diffs)}",
            f"- New dependency: {sum(bool(d['kinds'].get('dependency')) for d in diffs)}; authors or website changed: {sum(any(k.endswith((' authors', ' website')) for k in d['changed']) for d in diffs)}",
            f"- Re-obfuscated (classes not compared): {sum('diff.classes-renamed' in d['warnings'] for d in diffs)}",
        ]
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    cmd = sys.argv[1:2]
    if cmd == ["corpus"]:
        corpus(int(sys.argv[2]) if len(sys.argv) > 2 else 100)
    elif cmd == ["fetch"] and len(sys.argv) == 3:
        fetch(sys.argv[2])
    elif cmd == ["run"] and len(sys.argv) in (4, 5):
        exe = ROOT / "target" / "release" / ("jarsnitch.exe" if platform.system() == "Windows" else "jarsnitch")
        run(sys.argv[2], sys.argv[3], sys.argv[4] if len(sys.argv) == 5 else str(exe))
    else:
        sys.exit(__doc__)
