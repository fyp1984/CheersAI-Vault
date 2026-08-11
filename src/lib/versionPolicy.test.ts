import test from "node:test";
import assert from "node:assert/strict";

import {
  compareVersions,
  formatDisplayVersion,
  postponeUntil,
  releaseLevel,
  resolveVersionStatus,
} from "./versionPolicy";

test("formatDisplayVersion normalizes leading v", () => {
  assert.equal(formatDisplayVersion("1.2.3"), "v1.2.3");
  assert.equal(formatDisplayVersion("v1.2.3"), "v1.2.3");
});

test("compareVersions handles patch, minor, major, and prerelease order", () => {
  assert.equal(compareVersions("1.2.3", "1.2.3"), 0);
  assert.ok(compareVersions("1.2.4", "1.2.3") > 0);
  assert.ok(compareVersions("1.3.0", "1.2.9") > 0);
  assert.ok(compareVersions("2.0.0", "1.9.9") > 0);
  assert.ok(compareVersions("1.2.3", "1.2.3-beta.1") > 0);
  assert.ok(compareVersions("1.2.3-beta.2", "1.2.3-beta.1") > 0);
});

test("releaseLevel describes semantic change scope", () => {
  assert.equal(releaseLevel("1.2.3", "1.2.4"), "patch");
  assert.equal(releaseLevel("1.2.3", "1.3.0"), "minor");
  assert.equal(releaseLevel("1.2.3", "2.0.0"), "major");
  assert.equal(releaseLevel("1.2.3", "1.2.3"), "same");
});

test("resolveVersionStatus identifies optional and forced updates", () => {
  assert.deepEqual(resolveVersionStatus("1.2.3", "1.3.0", "1.2.0"), {
    currentVersion: "1.2.3",
    latestVersion: "1.3.0",
    minimumSupportedVersion: "1.2.0",
    updateAvailable: true,
    forceUpdate: false,
    releaseLevel: "minor",
  });

  assert.equal(
    resolveVersionStatus("1.2.3", "1.3.0", "1.2.4").forceUpdate,
    true
  );
});

test("postponeUntil computes reminder timestamp in hours", () => {
  assert.equal(postponeUntil(6, 1_000), 21_601_000);
});
