#!/usr/bin/env bash
# Downloads the plugins in plugins.txt from Modrinth into cache/ and prints a summary of
# findings per jar, to eyeball false positives. Not part of CI. Needs curl and python3.
# Usage: ./run.sh [--list]   (--list only prints what would be downloaded)
set -euo pipefail
cd "$(dirname "$0")"

ua=jarsnitch-fp-bench
# Prefer a Bukkit-family build; fall back to a proxy build.
bukkit='%5B%22paper%22%2C%22spigot%22%2C%22bukkit%22%2C%22folia%22%2C%22purpur%22%5D'
proxy='%5B%22velocity%22%2C%22bungeecord%22%2C%22waterfall%22%5D'
list=${1:-}
mkdir -p cache

sed 's/#.*//' plugins.txt | while read -r slug; do
  [ -z "$slug" ] && continue
  json='[]'
  for loaders in "$bukkit" "$proxy"; do
    json=$(curl -fsS -A "$ua" "https://api.modrinth.com/v2/project/$slug/version?loaders=$loaders") || json='[]'
    [ "$json" != '[]' ] && break
  done
  if [ "$json" = '[]' ]; then
    echo "warning: $slug: no server-plugin build on Modrinth" >&2
    continue
  fi
  IFS=$'\t' read -r version url filename < <(python3 -I -c '
import json, sys
v = json.load(sys.stdin)[0]
f = next((f for f in v["files"] if f["primary"]), v["files"][0])
print(v["version_number"], f["url"], f["filename"], sep="\t")' <<<"$json")
  if [ "$list" = --list ]; then
    printf '%-24s %-28s %s\n' "$slug" "$version" "$url"
    continue
  fi
  [ -f "cache/$filename" ] || curl -fsSL -A "$ua" -o "cache/$filename" "$url"
done
[ "$list" = --list ] && exit 0

cargo run -q --release --manifest-path ../../Cargo.toml -p jarsnitch -- scan cache --json > cache/reports.json || true
python3 -I - cache/reports.json <<'PY'
import json, sys
for r in json.load(open(sys.argv[1])):
    sev = [f["severity"] for f in r["findings"]]
    rules = sorted({f["rule"] for f in r["findings"] if f["severity"] != "info"})
    print(f'{r["file"]["name"][:40]:40} high {sev.count("high"):2}  notable {sev.count("notable"):2}  '
          f'info {sev.count("info"):3}  obf {r["obfuscation"]["score"]:3}  {" ".join(rules)}')
PY
