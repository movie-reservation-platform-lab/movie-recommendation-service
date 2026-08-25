#!/usr/bin/env bash
set -euo pipefail

image="${1:-movie-recommendation-service:smoke}"
host_port="${2:-18082}"
container_name="movie-recommendation-service-smoke-${RANDOM}"
error_body="$(mktemp)"

cleanup() {
  status=$?
  trap - EXIT
  if [[ $status -ne 0 ]]; then
    docker logs "$container_name" 2>/dev/null || true
  fi
  docker rm --force "$container_name" >/dev/null 2>&1 || true
  rm -f "$error_body"
  exit "$status"
}
trap cleanup EXIT

docker run \
  --detach \
  --name "$container_name" \
  --env ALLOW_REQUEST_DEMO_FAULTS=true \
  --publish "127.0.0.1:${host_port}:8082" \
  "$image" >/dev/null

for attempt in {1..30}; do
  if curl --fail --silent "http://127.0.0.1:${host_port}/health" >/dev/null; then
    break
  fi
  if [[ $attempt -eq 30 ]]; then
    echo "Service did not become healthy." >&2
    exit 1
  fi
  sleep 1
done

curl --fail --silent "http://127.0.0.1:${host_port}/ready" >/dev/null

normal_body="$(curl --fail --silent "http://127.0.0.1:${host_port}/recommendations?limit=1")"
grep --quiet '"recommendations"' <<<"$normal_body"

slow_status="$(
  curl \
    --max-time 10 \
    --silent \
    --output /dev/null \
    --write-out '%{http_code}' \
    --header 'X-Demo-Fault: slow-recommendation' \
    "http://127.0.0.1:${host_port}/recommendations?limit=1"
)"
test "$slow_status" = "200"

error_status="$(
  curl \
    --silent \
    --output "$error_body" \
    --write-out '%{http_code}' \
    --header 'X-Demo-Fault: recommendation-error' \
    "http://127.0.0.1:${host_port}/recommendations?limit=1"
)"
test "$error_status" = "503"
grep --quiet '"code":"recommendation_unavailable"' "$error_body"
test "$(docker exec "$container_name" id -u)" != "0"

echo "Recommendation service container smoke passed."
