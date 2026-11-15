import assert from "node:assert/strict";
import test from "node:test";

import { parseVersionServicePayload } from "@/lib/versionService";

test("parseVersionServicePayload accepts a valid payload", () => {
  const parsed = parseVersionServicePayload({
    product: "CheersAI Vault",
    channel: "stable",
    latestVersion: "1.2.3",
    minimumSupportedVersion: "1.2.0",
    publishedAt: "2026-08-11T00:00:00.000Z",
    releaseNotesSummary: "稳定性更新",
    releaseNotes: [
      { kind: "feature", text: "新增版本检查" },
      { kind: "fix", text: "修复更新提醒" },
    ],
    desktop: {
      enabled: true,
      pollIntervalMinutes: 30,
      remindAfterHours: 6,
      backupRequired: true,
      updateManifestUrl: "https://example.com/latest.json",
      releasePageUrl: "https://example.com/releases/v1.2.3",
    },
    web: {
      pollIntervalMinutes: 30,
      releasePageUrl: "https://example.com/releases/v1.2.3",
    },
  });

  assert.equal(parsed.latestVersion, "1.2.3");
  assert.equal(parsed.releaseNotes[0]?.kind, "feature");
});

test("parseVersionServicePayload rejects invalid release note kinds", () => {
  assert.throws(() =>
    parseVersionServicePayload({
      product: "CheersAI Vault",
      channel: "stable",
      latestVersion: "1.2.3",
      minimumSupportedVersion: "1.2.0",
      publishedAt: "2026-08-11T00:00:00.000Z",
      releaseNotesSummary: "稳定性更新",
      releaseNotes: [{ kind: "unknown", text: "bad" }],
      desktop: {
        enabled: true,
        pollIntervalMinutes: 30,
        remindAfterHours: 6,
        backupRequired: true,
        updateManifestUrl: "https://example.com/latest.json",
        releasePageUrl: "https://example.com/releases/v1.2.3",
      },
      web: {
        pollIntervalMinutes: 30,
        releasePageUrl: "https://example.com/releases/v1.2.3",
      },
    })
  );
});
