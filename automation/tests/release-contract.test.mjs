import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
const dockerfile = readFileSync("Dockerfile", "utf8");
const smoke = readFileSync("automation/container-smoke.sh", "utf8");

test("production image is non-root and exposes the stable runtime contract", () => {
  assert.match(dockerfile, /USER appuser/);
  assert.match(dockerfile, /EXPOSE 8082/);
  assert.match(dockerfile, /USE_DUMMY=true/);
  assert.match(dockerfile, /http:\/\/127\.0\.0\.1:\$\{PORT:-8082\}\/ready/);
});

test("container smoke covers normal and controlled fault paths", () => {
  assert.match(smoke, /\/health/);
  assert.match(smoke, /\/ready/);
  assert.match(smoke, /slow-recommendation/);
  assert.match(smoke, /recommendation-error/);
  assert.match(smoke, /test "\$error_status" = "503"/);
  assert.match(smoke, /docker exec .* id -u/);
});

test("publication is canonical, gated, single-platform, and attested", () => {
  const publishJob = workflowJob("publish-image");

  assert.match(publishJob, /github\.event_name == 'push'/);
  assert.match(publishJob, /github\.ref == 'refs\/heads\/main'/);
  for (const prerequisite of ["quality", "runtime-tests", "automation-contract", "container-smoke"]) {
    assert.match(publishJob, new RegExp(`- ${prerequisite}`));
  }
  assert.match(publishJob, /packages: write/);
  assert.match(publishJob, /id-token: write/);
  assert.match(publishJob, /attestations: write/);
  assert.match(publishJob, /platforms: linux\/amd64/);
  assert.match(publishJob, /provenance: false/);
  assert.match(publishJob, /actions\/attest-build-provenance@/);
  assert.match(publishJob, /push-to-registry: false/);
  assert.doesNotMatch(workflow, /pull_request_target:/);
});

test("automation contract and Rust behavior tests stay in separate jobs", () => {
  assert.match(workflowJob("runtime-tests"), /cargo test --all-targets --locked/);
  assert.doesNotMatch(workflowJob("runtime-tests"), /node --test/);
  assert.match(workflowJob("automation-contract"), /node --test automation\/tests/);
  assert.doesNotMatch(workflowJob("automation-contract"), /cargo test/);
});

function workflowJob(name) {
  const lines = workflow.split("\n");
  const start = lines.indexOf(`  ${name}:`);
  assert.notEqual(start, -1, `workflow job ${name} exists`);

  const end = lines.findIndex(
    (line, index) => index > start && /^  [a-z0-9-]+:$/.test(line),
  );
  return lines.slice(start, end === -1 ? lines.length : end).join("\n");
}
