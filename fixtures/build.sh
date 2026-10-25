#!/usr/bin/env bash
# Builds fixtures/jars/<name>.jar from each fixtures/src/<name>/ against the API stubs.
# Needs JDK 17 or newer. Output is reproducible: no manifest, fixed timestamps.
set -euo pipefail
cd "$(dirname "$0")"

date=2000-01-01T00:00:02Z
javac_flags=(--release 17 -nowarn -Xlint:none)

rm -rf build
mkdir -p build/stubs jars
find stubs -name '*.java' > build/stubs.txt
javac "${javac_flags[@]}" -d build/stubs @build/stubs.txt

for dir in src/*/; do
  name=$(basename "$dir")
  out="build/classes/$name"
  mkdir -p "$out"
  find "$dir" -name '*.java' > "build/$name.txt"
  javac "${javac_flags[@]}" -cp build/stubs -d "$out" "@build/$name.txt"
  # Resources (plugin.yml, configs, pom.properties) keep their paths.
  (cd "$dir" && find . -type f ! -name '*.java' | tar -cf - -T -) | (cd "$out" && tar -xf -)
  rm -f "jars/$name.jar"
  jar --create --no-manifest --date="$date" --file "jars/$name.jar" -C "$out" .
done

echo "built $(ls jars/*.jar | wc -l) fixture jars"
