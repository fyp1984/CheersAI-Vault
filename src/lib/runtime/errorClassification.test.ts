import test from "node:test";
import assert from "node:assert/strict";
import {
  classifyRuntimeFetchError,
  classifyRuntimeHttpResponse,
  parseRuntimeJsonResponse,
} from "./errorClassification";

function response(body: string | null, status: number, contentType?: string): Response {
  return new Response(body, {
    status,
    headers: contentType ? { "content-type": contentType } : undefined,
  });
}

test("413 non-JSON is HTTP status-only", async () => {
  const result = await classifyRuntimeHttpResponse(
    response("<html>fake upstream body</html>", 413, "text/html")
  );
  assert.deepEqual(result, { ok: false, reason: "http", status: 413 });
});

test("502 HTML never exposes token, path, or stack text", async () => {
  const result = await classifyRuntimeHttpResponse(
    response(
      "token=FAKE_TOKEN_ONLY path=/fake/server/private stack=Error: fake\\n at fake.js:1:1",
      502,
      "text/html"
    )
  );
  const serialized = JSON.stringify(result);
  assert.deepEqual(result, { ok: false, reason: "http", status: 502 });
  assert.equal(serialized.includes("FAKE_TOKEN_ONLY"), false);
  assert.equal(serialized.includes("/fake/server/private"), false);
  assert.equal(serialized.includes("fake.js:1:1"), false);
});

test("503 plain text and empty body are HTTP status-only", async () => {
  assert.deepEqual(
    await classifyRuntimeHttpResponse(response("upstream unavailable", 503, "text/plain")),
    { ok: false, reason: "http", status: 503 }
  );
  assert.deepEqual(await classifyRuntimeHttpResponse(response(null, 503)), {
    ok: false,
    reason: "http",
    status: 503,
  });
});

test("complete JSON error preserves only the safe contract fields", async () => {
  assert.deepEqual(
    await classifyRuntimeHttpResponse(
      response(
        JSON.stringify({ code: "INVALID_QUERY", message: "safe message", retryable: false, extra: "ignored" }),
        400,
        "application/json"
      )
    ),
    { ok: false, reason: "http", status: 400, code: "INVALID_QUERY", message: "safe message", retryable: false }
  );
});

test("invalid JSON and incomplete JSON errors keep only status", async () => {
  assert.deepEqual(
    await classifyRuntimeHttpResponse(response("{not-json", 502, "application/json")),
    { ok: false, reason: "http", status: 502 }
  );
  assert.deepEqual(
    await classifyRuntimeHttpResponse(
      response(JSON.stringify({ code: "MISSING_RETRYABLE", message: "not complete" }), 400, "application/json")
    ),
    { ok: false, reason: "http", status: 400 }
  );
});

test("fetch reject and AbortError remain network failures", () => {
  assert.deepEqual(classifyRuntimeFetchError(new Error("connection refused")), {
    ok: false,
    reason: "network",
  });
  assert.deepEqual(classifyRuntimeFetchError(new DOMException("cancelled", "AbortError")), {
    ok: false,
    reason: "network",
  });
});

test("2xx JSON remains successful and 2xx malformed JSON remains parse", async () => {
  assert.deepEqual(await parseRuntimeJsonResponse<{ ok: boolean }>(response('{"ok":true}', 200, "application/json")), {
    ok: true,
    data: { ok: true },
  });
  assert.deepEqual(await parseRuntimeJsonResponse(response("not-json", 200, "application/json")), {
    ok: false,
    reason: "parse",
  });
});
