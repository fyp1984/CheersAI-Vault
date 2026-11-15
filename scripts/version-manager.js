#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");

const files = {
  packageJson: path.join(rootDir, "package.json"),
  cargoToml: path.join(rootDir, "src-tauri", "Cargo.toml"),
  tauriConf: path.join(rootDir, "src-tauri", "tauri.conf.json"),
  versionInfo: path.join(rootDir, "releases", "stable", "version-info.json"),
  updateManifest: path.join(rootDir, "releases", "stable", "latest.json"),
};

const semverPattern =
  /^\d+\.\d+\.\d+(?:-[0-9A-Za-z-.]+)?(?:\+[0-9A-Za-z-.]+)?$/;

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function readOptionalJson(filePath) {
  if (!fs.existsSync(filePath)) {
    return null;
  }
  return readJson(filePath);
}

function writeJson(filePath, data) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(data, null, 2)}\n`);
}

function readState() {
  const packageJson = readJson(files.packageJson);
  const tauriConf = readJson(files.tauriConf);
  const cargoToml = fs.readFileSync(files.cargoToml, "utf8");
  const cargoMatch = cargoToml.match(
    /(\[package\][\s\S]*?\nversion\s*=\s*")([^"]+)(")/m
  );

  if (!cargoMatch) {
    throw new Error("Could not find [package].version in src-tauri/Cargo.toml");
  }

  return {
    packageVersion: packageJson.version,
    cargoVersion: cargoMatch[2],
    tauriVersion: tauriConf.version,
  };
}

function assertSemver(version) {
  if (!semverPattern.test(version)) {
    throw new Error(`Invalid semantic version: ${version}`);
  }
}

function bumpSemver(version, type) {
  const [core, suffix] = version.split(/(?=[-+])/);
  const [major, minor, patch] = core.split(".").map(Number);

  if ([major, minor, patch].some(Number.isNaN)) {
    throw new Error(`Cannot bump non-semver version: ${version}`);
  }

  switch (type) {
    case "major":
      return `${major + 1}.0.0`;
    case "minor":
      return `${major}.${minor + 1}.0`;
    case "patch":
      return `${major}.${minor}.${patch + 1}`;
    default:
      throw new Error(`Unsupported bump type: ${type}${suffix ? ` (${suffix})` : ""}`);
  }
}

function updatePackageVersion(nextVersion) {
  const packageJson = readJson(files.packageJson);
  packageJson.version = nextVersion;
  writeJson(files.packageJson, packageJson);
}

function updateCargoVersion(nextVersion) {
  const cargoToml = fs.readFileSync(files.cargoToml, "utf8");
  const updated = cargoToml.replace(
    /(\[package\][\s\S]*?\nversion\s*=\s*")([^"]+)(")/m,
    `$1${nextVersion}$3`
  );
  fs.writeFileSync(files.cargoToml, updated);
}

function updateTauriVersion(nextVersion) {
  const tauriConf = readJson(files.tauriConf);
  tauriConf.version = nextVersion;
  writeJson(files.tauriConf, tauriConf);
}

function compareSemver(left, right) {
  const parse = (value) =>
    value
      .replace(/^v/i, "")
      .split(/[+-]/, 1)[0]
      .split(".")
      .map(Number);
  const [leftMajor, leftMinor, leftPatch] = parse(left);
  const [rightMajor, rightMinor, rightPatch] = parse(right);
  if (leftMajor !== rightMajor) return leftMajor - rightMajor;
  if (leftMinor !== rightMinor) return leftMinor - rightMinor;
  return leftPatch - rightPatch;
}

function releaseBaseUrl(packageJson) {
  return (packageJson.homepage || packageJson.repository?.url || "https://github.com/fyp1984/CheersAI-Vault")
    .replace(/\.git$/, "");
}

function updaterManifestUrl() {
  return "https://raw.githubusercontent.com/fyp1984/CheersAI-Vault/main/releases/stable/latest.json";
}

function placeholderPlatforms(version, baseUrl) {
  return {
    "darwin-x86_64": {
      signature: "RELEASE_SIGNATURE_NOT_PUBLISHED_YET",
      url: `${baseUrl}/releases/download/v${version}/CheersAI_Vault_${version}_x86_64.app.tar.gz`,
    },
    "darwin-aarch64": {
      signature: "RELEASE_SIGNATURE_NOT_PUBLISHED_YET",
      url: `${baseUrl}/releases/download/v${version}/CheersAI_Vault_${version}_aarch64.app.tar.gz`,
    },
    "windows-x86_64": {
      signature: "RELEASE_SIGNATURE_NOT_PUBLISHED_YET",
      url: `${baseUrl}/releases/download/v${version}/CheersAI_Vault_${version}_x64.nsis.zip`,
    },
    "linux-x86_64": {
      signature: "RELEASE_SIGNATURE_NOT_PUBLISHED_YET",
      url: `${baseUrl}/releases/download/v${version}/CheersAI_Vault_${version}_x86_64.AppImage.tar.gz`,
    },
  };
}

function syncReleaseMetadata(nextVersion) {
  const packageJson = readJson(files.packageJson);
  const currentInfo = readOptionalJson(files.versionInfo);
  const currentManifest = readOptionalJson(files.updateManifest);
  const baseUrl = releaseBaseUrl(packageJson);
  const releasePageUrl = `${baseUrl}/releases/tag/v${nextVersion}`;

  const versionInfo = {
    product: currentInfo?.product || "CheersAI Vault",
    channel: currentInfo?.channel || "stable",
    latestVersion: nextVersion,
    minimumSupportedVersion:
      currentInfo?.minimumSupportedVersion &&
      compareSemver(currentInfo.minimumSupportedVersion, nextVersion) <= 0
        ? currentInfo.minimumSupportedVersion
        : nextVersion,
    publishedAt: currentInfo?.publishedAt || new Date().toISOString(),
    releaseNotesSummary:
      currentInfo?.releaseNotesSummary || `CheersAI Vault ${nextVersion} 版本发布`,
    releaseNotes: Array.isArray(currentInfo?.releaseNotes) ? currentInfo.releaseNotes : [],
    desktop: {
      enabled: currentInfo?.desktop?.enabled ?? true,
      pollIntervalMinutes: currentInfo?.desktop?.pollIntervalMinutes ?? 30,
      remindAfterHours: currentInfo?.desktop?.remindAfterHours ?? 6,
      backupRequired: currentInfo?.desktop?.backupRequired ?? true,
      updateManifestUrl: updaterManifestUrl(),
      releasePageUrl,
    },
    web: {
      pollIntervalMinutes: currentInfo?.web?.pollIntervalMinutes ?? 30,
      releasePageUrl,
    },
  };

  const shouldRegeneratePlatforms =
    !currentManifest?.platforms ||
    currentManifest.version !== nextVersion ||
    Object.values(currentManifest.platforms).every(
      (entry) => entry.signature === "RELEASE_SIGNATURE_NOT_PUBLISHED_YET"
    );

  const manifest = {
    version: nextVersion,
    notes:
      versionInfo.releaseNotes.length > 0
        ? versionInfo.releaseNotes.map((item) => `- ${item.text}`).join("\n")
        : versionInfo.releaseNotesSummary,
    pub_date: versionInfo.publishedAt,
    platforms: shouldRegeneratePlatforms
      ? placeholderPlatforms(nextVersion, baseUrl)
      : currentManifest.platforms,
  };

  writeJson(files.versionInfo, versionInfo);
  writeJson(files.updateManifest, manifest);
}

function setVersion(nextVersion) {
  assertSemver(nextVersion);
  updatePackageVersion(nextVersion);
  updateCargoVersion(nextVersion);
  updateTauriVersion(nextVersion);
  syncReleaseMetadata(nextVersion);
  return nextVersion;
}

function syncFromPackageVersion() {
  const { packageVersion } = readState();
  assertSemver(packageVersion);
  updateCargoVersion(packageVersion);
  updateTauriVersion(packageVersion);
  syncReleaseMetadata(packageVersion);
  return packageVersion;
}

function formatState(state) {
  return [
    `package.json      ${state.packageVersion}`,
    `src-tauri/Cargo.toml ${state.cargoVersion}`,
    `src-tauri/tauri.conf.json ${state.tauriVersion}`,
  ].join("\n");
}

function checkConsistency() {
  const state = readState();
  const versions = new Set([
    state.packageVersion,
    state.cargoVersion,
    state.tauriVersion,
  ]);

  if (versions.size !== 1) {
    console.error("Version mismatch detected:");
    console.error(formatState(state));
    process.exit(1);
  }

  assertSemver(state.packageVersion);
  console.log(`Version aligned: ${state.packageVersion}`);
}

function printUsage() {
  console.log(`Usage:
  node scripts/version-manager.js sync
  node scripts/version-manager.js check
  node scripts/version-manager.js prepare
  node scripts/version-manager.js set <version>
  node scripts/version-manager.js bump [major|minor|patch]`);
}

function main() {
  const command = process.argv[2];

  try {
    switch (command) {
      case "sync": {
        const version = syncFromPackageVersion();
        console.log(`Version synced from package.json: ${version}`);
        break;
      }
      case "check":
        checkConsistency();
        break;
      case "prepare": {
        const version = syncFromPackageVersion();
        console.log(`Version prepared for bundles on macOS/Windows/Linux: ${version}`);
        checkConsistency();
        break;
      }
      case "set": {
        const nextVersion = process.argv[3] === "--" ? process.argv[4] : process.argv[3];
        if (!nextVersion) {
          throw new Error("Version is required. Usage: node scripts/version-manager.js set <version>");
        }

        const currentVersion = readState().packageVersion;
        const version = setVersion(nextVersion);
        console.log(`Version set: ${currentVersion} -> ${version}`);
        checkConsistency();
        break;
      }
      case "bump": {
        const bumpType = process.argv[3] || "patch";
        if (!["major", "minor", "patch"].includes(bumpType)) {
          throw new Error("Bump type must be one of: major, minor, patch");
        }

        const currentVersion = readState().packageVersion;
        const nextVersion = bumpSemver(currentVersion, bumpType);
        updatePackageVersion(nextVersion);
        syncFromPackageVersion();
        console.log(`Version bumped: ${currentVersion} -> ${nextVersion}`);
        checkConsistency();
        break;
      }
      default:
        printUsage();
        process.exit(command ? 1 : 0);
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}

main();
