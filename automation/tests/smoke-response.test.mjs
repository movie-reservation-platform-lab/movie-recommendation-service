import assert from "node:assert/strict";
import test from "node:test";
import { validateRecommendation } from "../validate-smoke-response.mjs";
const success = { recommendations: [{ id: "rec-1", title: "Title", reason: "Ranked", movie_reservation_movie_id: "movie-1", confidence: 0.9 }] };
const failure = { error: { code: "internal_error", message: "Recommendation service failed", fault: "none" } };

test("smoke validates either outcome without requiring a random sequence", () => {
  validateRecommendation(200, success);
  validateRecommendation(500, failure);
});
test("smoke rejects other errors and malformed or leaking payloads", () => {
  for (const [status, body] of [[503, failure], [404, failure], [500, {}], [200, failure], [200, { recommendations: [] }], [500, { ...failure, diagnostic: "private" }], [200, { recommendations: [{ ...success.recommendations[0], confidence: null }] }]]) {
    assert.throws(() => validateRecommendation(status, body));
  }
});
