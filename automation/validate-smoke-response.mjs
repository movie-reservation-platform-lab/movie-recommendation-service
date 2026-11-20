import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

// The normal artifact must return one valid recommendation, never an error envelope.
export function validateRecommendation(status, body) {
  assert.equal(status, 200);
  assert.deepEqual(Object.keys(body), ["recommendations"]);
  assert.equal(body.recommendations.length, 1);
  const item = body.recommendations[0];
  for (const field of ["id", "title", "reason", "movie_reservation_movie_id"]) {
    assert.equal(typeof item[field], "string");
    assert.ok(item[field].length > 0);
  }
  assert.ok(Number.isFinite(item.confidence) && item.confidence >= 0 && item.confidence <= 1);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    validateRecommendation(Number(process.argv[2]), JSON.parse(readFileSync(process.argv[3], "utf8")));
  } catch {
    console.error("Recommendation response does not match the selected artifact contract.");
    process.exitCode = 1;
  }
}
