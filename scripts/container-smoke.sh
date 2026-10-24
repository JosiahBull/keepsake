#!/usr/bin/env bash
set -euo pipefail

export KEEPSAKE_IMAGE="${1:?Usage: container-smoke.sh IMAGE}"
repository=$(cd "$(dirname "$0")/.." && pwd)
project="keepsake-smoke-${GITHUB_RUN_ID:-$$}-$RANDOM"
container="$project-app"
scratch=$(mktemp -d)
compose=(docker compose --file "$repository/compose.yaml" --project-name "$project")

cleanup() {
  result=$?
  if ((result != 0)); then docker logs "$container" >&2 || true; fi
  docker rm --force --volumes "$container" >/dev/null 2>&1 || true
  "${compose[@]}" down --volumes >/dev/null 2>&1 || true
  rm -r "$scratch"
  exit "$result"
}
trap cleanup EXIT

# Compose supplies the deployment's volume, user and security settings. Its normal
# published port is unused by `run`; this one-off gets an ephemeral loopback port.
"${compose[@]}" run --detach --no-deps --pull never \
  --publish 127.0.0.1::8080 --name "$container" keepsake >/dev/null
docker inspect --format '{{json .HostConfig}}' "$container" | jq -e '
  .ReadonlyRootfs and (.CapDrop | index("ALL"))
  and (.SecurityOpt | any(. == "no-new-privileges:true" or . == "no-new-privileges"))
  and (.Tmpfs["/tmp"] | contains("size=64m") and contains("noexec") and contains("nosuid") and contains("nodev"))
' >/dev/null
endpoint=$(docker port "$container" 8080/tcp)
[[ "$endpoint" == 127.0.0.1:* ]] || { echo 'Expected a loopback container port.' >&2; exit 1; }
base="http://$endpoint"

for ((attempt=0; attempt<30; attempt++)); do
  if curl --fail --silent --connect-timeout 1 --max-time 2 "$base/api/health" > "$scratch/health.json"; then break; fi
  sleep 1
done
jq -e '.status == "ok"' "$scratch/health.json" >/dev/null
docker exec "$container" keepsake-server --health-check
docker exec --user 0 "$container" sh -c '
  if touch /app/.smoke-root-write 2>/dev/null; then
    echo "Container root filesystem is writable." >&2
    exit 1
  fi
  temporary=$(mktemp /tmp/keepsake-smoke.XXXXXX)
  test -w "$temporary"
  rm "$temporary"
'

curl_args=(--fail --silent --show-error --connect-timeout 2 --max-time 5)
curl "${curl_args[@]}" "$base/" > "$scratch/index.html"
asset=$(python3 - "$scratch/index.html" <<'PY'
from html.parser import HTMLParser
from pathlib import Path
import sys

class Scripts(HTMLParser):
    assets = []
    def handle_starttag(self, tag, attributes):
        attributes = dict(attributes)
        if tag == "script" and attributes.get("type") == "module":
            self.assets.append(attributes.get("src", ""))

page = Scripts()
page.feed(Path(sys.argv[1]).read_text())
assert len(page.assets) == 1 and page.assets[0].startswith("/assets/"), "Built SPA entry script missing"
print(page.assets[0])
PY
)
curl "${curl_args[@]}" --dump-header "$scratch/asset.headers" "$base$asset" > "$scratch/app.js"
test -s "$scratch/app.js"
grep -Eiq '^content-type: (application|text)/javascript' "$scratch/asset.headers"

docker exec "$container" keepsake --json add \
  --prompt 'What does WAL stand for?' --answer 'Write-ahead logging' \
  --source 'container-smoke' --tag ci > "$scratch/fact.json"
fact_id=$(jq -er '.id | select(type == "number" and . > 0)' "$scratch/fact.json")
curl "${curl_args[@]}" "$base/api/facts/$fact_id" | jq -e --argjson id "$fact_id" '
  .id == $id and .answer == "Write-ahead logging" and .source == "container-smoke"
  and .tags == ["ci"] and any(.practices[]; .kind == "flashcard" and .answer == "Write-ahead logging")
' >/dev/null
printf 'Container confinement, health, built SPA and shared SQLite passed: %s\n' "$KEEPSAKE_IMAGE"
