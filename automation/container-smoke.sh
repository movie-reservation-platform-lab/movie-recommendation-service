#!/usr/bin/env bash
set -euo pipefail

image="${1:-movie-recommendation-service:smoke}"
host_port="${2:-18082}"
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
response_body="$(mktemp)"
container_name="movie-recommendation-service-smoke-${RANDOM}"

cleanup() {
  status=$?
  trap - EXIT
  if [[ $status -ne 0 ]]; then
    docker logs "$container_name" 2>/dev/null || true
  fi
  docker rm --force "$container_name" >/dev/null 2>&1 || true
  rm -f "$response_body"
  exit "$status"
}
trap cleanup EXIT

docker run \
  --detach \
  --name "$container_name" \
  --env ALLOW_REQUEST_DEMO_FAULTS=true \
  --env DEMO_FAULT_MODE=recommendation-error \
  --publish "127.0.0.1:${host_port}:8082" \
  "$image" >/dev/null

for attempt in {1..30}; do
  if curl --max-time 3 --fail --silent "http://127.0.0.1:${host_port}/health" >/dev/null; then
    break
  fi
  if [[ $attempt -eq 30 ]]; then
    echo "Service did not become healthy." >&2
    exit 1
  fi
  sleep 1
done

curl --max-time 3 --fail --silent "http://127.0.0.1:${host_port}/ready" >/dev/null

curl --max-time 3 --fail --silent "http://127.0.0.1:${host_port}/movies?limit=1" > "$response_body"
node -e 'const a = require("node:assert/strict"); const b = JSON.parse(require("node:fs").readFileSync(process.argv[1], "utf8")); a.equal(b.length, 1); a.equal(b[0].title, "The Shawshank Redemption");' "$response_body"

for fault in absent none slow-recommendation recommendation-error; do
  headers=()
  if [[ "$fault" != absent ]]; then headers=(--header "X-Demo-Fault: $fault"); fi
  status="$(curl --max-time 2 --silent --show-error --output "$response_body" --write-out '%{http_code}' \
    "${headers[@]}" "http://127.0.0.1:${host_port}/recommendations?limit=1")"
  node "$script_dir/validate-smoke-response.mjs" "$status" "$response_body"
done

test "$(docker exec "$container_name" id -u)" != "0"

echo "Recommendation service container smoke passed."
