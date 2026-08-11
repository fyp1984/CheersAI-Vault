import type { ResolvedVersionStatus } from "@/types/versioning";

interface ParsedVersion {
  major: number;
  minor: number;
  patch: number;
  prerelease: Array<string | number>;
}

export function normalizeVersion(version: string): string {
  return version.trim().replace(/^v/i, "");
}

export function formatDisplayVersion(version: string): string {
  return `v${normalizeVersion(version)}`;
}

function parsePart(value: string): string | number {
  return /^\d+$/.test(value) ? Number(value) : value;
}

function parseVersion(version: string): ParsedVersion {
  const normalized = normalizeVersion(version);
  const [mainAndPre] = normalized.split("+", 1);
  const [main, prereleaseRaw] = mainAndPre.split("-", 2);
  const [major, minor, patch] = main.split(".").map(Number);

  if ([major, minor, patch].some(Number.isNaN)) {
    throw new Error(`Invalid semantic version: ${version}`);
  }

  return {
    major,
    minor,
    patch,
    prerelease: prereleaseRaw ? prereleaseRaw.split(".").map(parsePart) : [],
  };
}

function compareIdentifier(left: string | number, right: string | number): number {
  if (typeof left === "number" && typeof right === "number") {
    return left - right;
  }
  if (typeof left === "number") return -1;
  if (typeof right === "number") return 1;
  return left.localeCompare(right);
}

export function compareVersions(left: string, right: string): number {
  const a = parseVersion(left);
  const b = parseVersion(right);

  if (a.major !== b.major) return a.major - b.major;
  if (a.minor !== b.minor) return a.minor - b.minor;
  if (a.patch !== b.patch) return a.patch - b.patch;

  if (a.prerelease.length === 0 && b.prerelease.length === 0) return 0;
  if (a.prerelease.length === 0) return 1;
  if (b.prerelease.length === 0) return -1;

  const length = Math.max(a.prerelease.length, b.prerelease.length);
  for (let index = 0; index < length; index += 1) {
    const leftPart = a.prerelease[index];
    const rightPart = b.prerelease[index];
    if (leftPart === undefined) return -1;
    if (rightPart === undefined) return 1;
    const compared = compareIdentifier(leftPart, rightPart);
    if (compared !== 0) return compared;
  }

  return 0;
}

export function releaseLevel(currentVersion: string, latestVersion: string): ResolvedVersionStatus["releaseLevel"] {
  const current = parseVersion(currentVersion);
  const latest = parseVersion(latestVersion);

  if (
    current.major === latest.major &&
    current.minor === latest.minor &&
    current.patch === latest.patch
  ) {
    return "same";
  }
  if (current.major !== latest.major) return "major";
  if (current.minor !== latest.minor) return "minor";
  return "patch";
}

export function resolveVersionStatus(
  currentVersion: string,
  latestVersion: string,
  minimumSupportedVersion: string
): ResolvedVersionStatus {
  const updateAvailable = compareVersions(latestVersion, currentVersion) > 0;
  const forceUpdate = compareVersions(minimumSupportedVersion, currentVersion) > 0;

  return {
    currentVersion: normalizeVersion(currentVersion),
    latestVersion: normalizeVersion(latestVersion),
    minimumSupportedVersion: normalizeVersion(minimumSupportedVersion),
    updateAvailable,
    forceUpdate,
    releaseLevel: releaseLevel(currentVersion, latestVersion),
  };
}

export function postponeUntil(hours: number, now = Date.now()): number {
  return now + hours * 60 * 60 * 1000;
}
