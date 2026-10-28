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

# A fresh deployment creates both household libraries and pending vocabulary offers.
curl "${curl_args[@]}" "$base/api/profiles" | jq -e '
  length == 2 and .[0].id == 1 and .[0].name == "Josiah"
  and .[0].handle == "default" and .[1].name == "Ansam" and .[1].handle == "ansam"
' >/dev/null
# Each library also receives every default list item, converted in the background.
listed=$(docker exec "$container" sh -c 'cat /app/default-lists/*/list.json' | \
  jq -s 'map(.items | length) | add')
((listed > 0))
for ((attempt=0; attempt<600; attempt++)); do
  docker logs "$container" > "$scratch/server.log" 2>&1
  if grep -q 'default lists checked' "$scratch/server.log"; then break; fi
  sleep 1
done
if ! grep -q "default lists checked added=$((2 * listed))\$" "$scratch/server.log" \
  || grep -q 'default list item not added\|default lists not added' "$scratch/server.log"; then
  echo "Expected both libraries to receive all $listed default list items within 10 minutes." >&2
  exit 1
fi
curl "${curl_args[@]}" "$base/api/profiles/1/facts" | jq -e --argjson n "$listed" '.total == 40 + $n' >/dev/null
curl "${curl_args[@]}" "$base/api/profiles/2/facts" | jq -e --argjson n "$listed" '.total == 60 + $n' >/dev/null
curl "${curl_args[@]}" "$base/api/profiles/1/shares?direction=inbox&status=pending" | \
  jq -e '.total == 60 and all(.items[]; .sender_id == 2 and .recipient_id == 1 and (has("answer") | not))' >/dev/null

docker exec "$container" keepsake --json --profile default add \
  --prompt 'What does WAL stand for?' --answer 'Write-ahead logging' \
  --source 'container-smoke' --tag ci > "$scratch/fact.json"
fact_id=$(jq -er '.id | select(type == "number" and . > 0)' "$scratch/fact.json")
curl "${curl_args[@]}" "$base/api/profiles/1/facts/$fact_id" | jq -e --argjson id "$fact_id" '
  .id == $id and .answer == "Write-ahead logging" and .source == "container-smoke"
  and .tags == ["ci"] and any(.practices[]; .kind == "flashcard" and .answer == "Write-ahead logging")
' >/dev/null
# Both executables must agree on ownership, independent copies and fresh schedules.
docker exec "$container" keepsake --json profiles create 'Smoke partner' \
  --handle smoke-partner > "$scratch/profile.json"
partner_id=$(jq -er '.id' "$scratch/profile.json")
status=$(curl --silent --show-error --max-time 5 --output "$scratch/ambiguous.json" \
  --write-out '%{http_code}' "$base/api/facts")
[[ "$status" == 400 ]]
jq -e '.error | contains("profile_required")' "$scratch/ambiguous.json" >/dev/null

docker exec "$container" keepsake --json --profile 1 share "$fact_id" \
  --to "$partner_id" > "$scratch/offer.json"
share_id=$(jq -er '.[0].id' "$scratch/offer.json")
docker exec "$container" keepsake --json --profile "$partner_id" shares accept \
  "$share_id" > "$scratch/accepted.json"
copy_id=$(jq -er '.[0].accepted_fact_id' "$scratch/accepted.json")
[[ "$copy_id" != "$fact_id" ]]
curl "${curl_args[@]}" "$base/api/profiles/$partner_id/facts/$copy_id" | \
  jq -e '.answer == "Write-ahead logging" and .source == "container-smoke" and .shared_by != null' >/dev/null
for profile_id in 1 "$partner_id"; do
  expected_total=$((41 + listed))
  expected_queue=5
  if [[ "$profile_id" == "$partner_id" ]]; then expected_total=1; expected_queue=1; fi
  curl "${curl_args[@]}" "$base/api/profiles/$profile_id/facts" | \
    jq -e --argjson total "$expected_total" '.total == $total' >/dev/null
  curl "${curl_args[@]}" "$base/api/profiles/$profile_id/study/queue" > "$scratch/queue-$profile_id.json"
  jq -e --argjson count "$expected_queue" '(.items | length) == $count' "$scratch/queue-$profile_id.json" >/dev/null
  # Libraries outgrow one page, so each queued fact is fetched from its owner's library.
  for queued in $(jq -r '.items[].fact.id' "$scratch/queue-$profile_id.json"); do
    curl "${curl_args[@]}" --output /dev/null "$base/api/profiles/$profile_id/facts/$queued"
  done
done
curl "${curl_args[@]}" "$base/api/profiles/2/facts" | jq -e --argjson n "$listed" '.total == 60 + $n' >/dev/null
curl "${curl_args[@]}" "$base/api/profiles/$partner_id/study/facts/$copy_id" | \
  jq -e '.phase == "new" and .stability == null and .difficulty == null' >/dev/null
printf 'Container confinement, health, built SPA, default lists, profile isolation and copied recall passed: %s\n' "$KEEPSAKE_IMAGE"
