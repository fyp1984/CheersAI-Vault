// Copyright 2026 CheersAI. Licensed under Apache-2.0.
import { describe, it, expect } from "vitest";
import {
  classifyRuntimeFetchError,
  classifyRuntimeHttpResponse,
  parseRuntimeJsonResponse,
} from "@/lib/runtime/errorClassification";

function response(body: string | null, status: number, contentType?: string): Response {
  return new Response(body, {
    status,
    headers: contentType ? { "content-type": contentType } : undefined,
  });
}

describe("errorClassification util (vitest spec port)", () => {
  it("413 non-JSON is HTTP status-only", async () => {
    await expect(
      classifyRuntimeHttpResponse(response("<html>fake upstream body</html>", 413, "text/html")),
    ).resolves.toEqual({ ok: false, reason: "http", status: 413 });
  });

  it("502 HTML never exposes token, path, or stack text", async () => {
    const result = await classifyRuntimeHttpResponse(
      response(
        "token=FAKE_TOKEN_ONLY path=/fake/server/private stack=Error: fake\\n at fake.js:1:1",
        502,
        "text/html",
      ),
    );
    expect(result).toEqual({ ok: false, reason: "http", status: 502 });
    const serialized = JSON.stringify(result);
    expect(serialized.includes("FAKE_TOKEN_ONLY")).toBe(false);
    expect(serialized.includes("/fake/server/private")).toBe(false);
    expect(serialized.includes("fake.js:1:1")).toBe(false);
  });

  it("503 plain text and empty body are HTTP status-only", async () => {
    await expect(
      classifyRuntimeHttpResponse(response("upstream unavailable", 503, "text/plain")),
    ).resolves.toEqual({ ok: false, reason: "http", status: 503 });
    await expect(classifyRuntimeHttpResponse(response(null, 503))).resolves.toEqual({
      ok: false,
      reason: "http",
      status: 503,
    });
  });

  it("complete JSON error preserves only the safe contract fields", async () => {
    await expect(
      classifyRuntimeHttpResponse(
        response(
          JSON.stringify({
            code: "INVALID_QUERY",
            message: "safe message",
            retryable: false,
            extra: "ignored",
          }),
          400,
          "application/json",
        ),
      ),
    ).resolves.toEqual({
      ok: false,
      reason: "http",
      status: 400,
      code: "INVALID_QUERY",
      message: "safe message",
      retryable: false,
    });
  });

  it("classifyRuntimeFetchError covers network errors without leaking PII", () => {
    const r = classifyRuntimeFetchError(new TypeError("fetch failed for /users/13900000000/records"));
    expect(r.ok).toBe(false);
    expect(r.reason).toBe("network");
    expect(JSON.stringify(r).includes("13900000000")).toBe(false);
  });
});
