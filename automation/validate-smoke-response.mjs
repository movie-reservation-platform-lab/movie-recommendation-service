import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

// Accept the two service contracts, never an arbitrary 5xx or malformed success.
// Deterministic Rust tests exercise each outcome; this check has no sampling quota.
export function validateRecommendation(status, body) {
  if (status === 200) {
    assert.deepEqual(Object.keys(body), ["recommendations"]);
    assert.equal(body.recommendations.length, 1);
    const item = body.recommendations[0];
    for (const field of ["id", "title", "reason", "movie_reservation_movie_id"]) {
      assert.equal(typeof item[field], "string");
      assert.ok(item[field].length > 0);
    }
    assert.ok(Number.isFinite(item.confidence) && item.confidence >= 0 && item.confidence <= 1);
  } else {
    assert.equal(status, 500);
    assert.deepEqual(body, { error: {
      code: "internal_error", message: "Recommendation service failed", fault: "none",
    } });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    validateRecommendation(Number(process.argv[2]), JSON.parse(readFileSync(process.argv[3], "utf8")));
  } catch {
    console.error("Recommendation response does not match the selected artifact contract.");
    process.exitCode = 1;
  }
}
