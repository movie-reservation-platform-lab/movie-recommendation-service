import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
const dockerfile = readFileSync("Dockerfile", "utf8");
const smoke = readFileSync("automation/container-smoke.sh", "utf8");
const reviewedActionsSha = "036531133bcefd454b5afc0eb55f8ba0328901ea";

test("production image is non-root and exposes the stable runtime contract", () => {
  assert.match(dockerfile, /USER appuser/);
  assert.match(dockerfile, /EXPOSE 8082/);
  assert.match(dockerfile, /USE_DUMMY=true/);
  assert.match(dockerfile, /http:\/\/127\.0\.0\.1:\$\{PORT:-8082\}\/ready/);
});

test("container smoke covers health and retired fault controls", () => {
  assert.match(smoke, /\/health/);
  assert.match(smoke, /\/ready/);
  assert.match(smoke, /slow-recommendation/);
  assert.match(smoke, /recommendation-error/);
  assert.match(smoke, /validate-smoke-response.mjs/);
  assert.match(smoke, /docker exec .* id -u/);
});

test("publication is canonical, gated, single-platform, and attested", () => {
  const publishJob = workflowJob("publish-image");

  const conditions = [...publishJob.matchAll(/^    if: (.+)$/gm)].map(match => match[1]);
  assert.deepEqual(conditions, [
    "github.event_name == 'push' && github.ref == 'refs/heads/main' && github.repository == 'movie-reservation-platform-lab/movie-recommendation-service'",
  ]);
  for (const prerequisite of ["quality", "runtime-tests", "automation-contract", "container-smoke"]) {
    assert.match(publishJob, new RegExp(`- ${prerequisite}`));
  }
  assert.equal(
    workflowPermissions(publishJob),
    "      contents: read\n      packages: write\n      id-token: write\n      attestations: write\n",
  );
  assert.match(publishJob, /platforms: linux\/amd64/);
  assert.match(publishJob, /target: runtime/);
  assert.match(publishJob, /provenance: false/);
  assert.match(publishJob, /\/actions\/container-evidence@/);
  assert.ok(publishJob.includes('digest: ${{ steps.build.outputs.digest }}'));
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

function workflowPermissions(job) {
  const match = job.match(/^    permissions:\n((?:      [^\n]+\n)+)/m);
  assert.notEqual(match, null, "workflow job has an explicit permission stanza");
  return match[1];
}

function workflowStep(job, name) {
  const escapedName = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = job.match(new RegExp(`^      - name: ${escapedName}\\n[\\s\\S]*?(?=^      - name:|(?![\\s\\S]))`, "m"));
  assert.notEqual(match, null, `workflow step ${name} exists`);
  return match[0];
}

test("prepare receives the explicit caller token", () => {
  const prepare = workflowStep(workflowJob("publish-image"), "Prepare canonical candidate");

  assert.ok(prepare.includes(`/actions/prepare-container-candidate@${reviewedActionsSha}`));
  assert.ok(prepare.includes("        with:\n          component: recommendation-service\n"));
  assert.ok(prepare.includes("          github-token: ${{ github.token }}\n"));
});

test("shared evidence is canonical, attempt-safe and pinned without AWS authority", () => {
  const publish = workflowJob("publish-image");
  assert.ok(publish.includes("github.repository == 'movie-reservation-platform-lab/movie-recommendation-service'"));
  assert.ok(publish.includes("component: recommendation-service"));
  assert.ok(publish.includes("persist-credentials: false"));
  assert.ok(workflow.includes("cancel-in-progress: ${{ github.event_name == 'pull_request' }}"));
  assert.ok(publish.indexOf("/actions/prepare-container-candidate@") < publish.indexOf("docker/login-action@"));
  assert.ok(publish.indexOf("docker/build-push-action@") < publish.indexOf("/actions/container-evidence@"));
  const refs = [...workflow.matchAll(/uses: (\S+)/g)].map(m => m[1]);
  assert.ok(refs.every(ref => /@[a-f0-9]{40}$/.test(ref)));
  const pins = refs.filter(ref => ref.includes("/movie-platform-actions/actions/")).map(ref => ref.split("@")[1]);
  assert.deepEqual(pins, Array(2).fill(reviewedActionsSha));
  const toolingCheckout = workflowStep(
    workflowJob("container-security-check"),
    "Check out reviewed shared security tooling",
  );
  assert.ok(toolingCheckout.includes(`          ref: ${reviewedActionsSha}\n`));
  assert.ok(toolingCheckout.includes("repository: movie-reservation-platform-lab/movie-platform-actions"));
  assert.match(publish, /evidence-version: v1alpha3/);
  assert.ok(!workflow.includes("aws-actions/"));
});

test("PR image scanning uses the reviewed shared policy with read-only authority", () => {
  const security = workflowJob("container-security-check");
  assert.match(workflow, /pull_request:/);
  assert.ok(security.includes("if: github.event_name != 'push' || github.ref != 'refs/heads/main' || github.repository != 'movie-reservation-platform-lab/movie-recommendation-service'"));
  for (const prerequisite of ["quality", "automation-contract"]) {
    assert.match(security, new RegExp(`- ${prerequisite}`));
  }
  assert.equal(workflowPermissions(security), "      contents: read\n");
  assert.doesNotMatch(security, /: write|docker\/login-action|push: true|attest-build-provenance|container-evidence@/);
  assert.equal((security.match(/persist-credentials: false/g) ?? []).length, 2);
  assert.match(security, /repository: movie-reservation-platform-lab\/movie-platform-actions/);
  assert.match(security, /node-version: '24'/);
  assert.match(security, /--platform linux\/amd64 --target runtime/);
  assert.match(security, /node \.platform-actions\/local-tools\/container-security\/lib\/scan\.mjs/);
  assert.match(security, /--evidence-version v1alpha3 --component recommendation-service/);
  assert.equal((security.match(/GH_TOKEN:/g) ?? []).length, 1);
  assert.ok(security.indexOf("GH_TOKEN:") > security.indexOf("run: docker build"));
  assert.doesNotMatch(security, /continue-on-error|\|\| true|aquasecurity\/trivy-action/);
});

test("complete PR diagnostics upload after gate failure without claiming candidate authority", () => {
  const security = workflowJob("container-security-check");
  const upload = security.slice(security.indexOf("      - name: Retain PR vulnerability diagnostics"));
  assert.ok(security.indexOf("lib/scan.mjs") < security.indexOf("actions/upload-artifact@"));
  assert.ok(upload.includes("if: ${{ !cancelled() }}"));
  assert.ok(security.includes('--output-dir "$RUNNER_TEMP/recommendation-service-pr-security"'));
  assert.ok(upload.includes("path: ${{ runner.temp }}/recommendation-service-pr-security/"));
  assert.ok(upload.includes("name: recommendation-service-pr-vulnerability-report-${{ github.run_id }}-attempt-${{ github.run_attempt }}"));
  assert.match(upload, /if-no-files-found: error/);
  assert.match(upload, /retention-days: 14/);
  assert.doesNotMatch(upload, /security-evidence|attest/);
});

test("all PR jobs lack publication authority and only canonical pushes select publication", () => {
  const condition = workflowJob("publish-image").match(/^    if: (.+)$/m)[1];
  assert.equal(condition, "github.event_name == 'push' && github.ref == 'refs/heads/main' && github.repository == 'movie-reservation-platform-lab/movie-recommendation-service'");
  assert.match(workflow, /permissions:\n  contents: read/);
  for (const name of ["quality", "runtime-tests", "automation-contract", "container-smoke", "container-security-check"]) {
    assert.doesNotMatch(workflowJob(name), /: write|docker\/login-action|push: true|attest-build-provenance|actions\/container-evidence@/);
  }
  // The PR-only scanner is skipped on canonical main. It must not cause the
  // publisher to skip its own exact-digest scan via a skipped prerequisite.
  const needs = workflowJob("publish-image").split("    needs:")[1].split("    runs-on:")[0];
  assert.doesNotMatch(needs, /container-security-check/);
});

test("smoke, security and publication all select the Rust production target", () => {
  for (const name of ["container-smoke", "container-security-check"]) {
    assert.match(workflowJob(name), /docker build --platform linux\/amd64 --target runtime /);
  }
  assert.match(workflowJob("publish-image"), /target: runtime/);
  assert.match(dockerfile, /FROM debian:trixie-slim AS runtime/);
  assert.match(dockerfile, /apt-get upgrade -y --no-install-recommends/);
  assert.match(dockerfile, /ca-certificates curl tini/);
  assert.match(dockerfile, /COPY --from=build \/tmp\/movie-recommendation-service/);
});

test("canonical publication has one image identity and no build feature switch", () => {
  assert.doesNotMatch(workflow, /catalog-snapshots|SERVICE_FEATURES/);
  assert.doesNotMatch(dockerfile, /catalog-snapshots|SERVICE_FEATURES|--features/);
  assert.equal((workflow.match(/push: true/g) ?? []).length, 1);
  assert.equal((workflow.match(/Prepare canonical candidate/g) ?? []).length, 1);
  assert.equal((workflow.match(/Attest and publish security evidence/g) ?? []).length, 1);
});
