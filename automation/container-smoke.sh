#!/usr/bin/env bash
set -euo pipefail

image="${1:-movie-recommendation-service:smoke}"
host_port="${2:-18082}"
container_name="movie-recommendation-service-smoke-${RANDOM}"

cleanup() {
  status=$?
  trap - EXIT
  if [[ $status -ne 0 ]]; then
    docker logs "$container_name" 2>/dev/null || true
  fi
  docker rm --force "$container_name" >/dev/null 2>&1 || true
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

for fault in none slow-recommendation recommendation-error; do
  status="$(curl --max-time 2 --silent --output /dev/null --write-out '%{http_code}' \
    --header "X-Demo-Fault: $fault" \
    "http://127.0.0.1:${host_port}/recommendations?limit=1")"
  test "$status" = "200"
done

test "$(docker exec "$container_name" id -u)" != "0"

echo "Recommendation service container smoke passed."
