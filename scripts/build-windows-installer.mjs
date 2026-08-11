#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");

function assertExists(filePath, label) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`${label} not found: ${filePath}`);
  }
}

function runNodeScript(scriptPath, args = []) {
  const result = spawnSync(process.execPath, [scriptPath, ...args], {
    cwd: rootDir,
    stdio: "inherit",
    env: process.env,
  });

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function main() {
  const passthroughArgs = process.argv.slice(2);
  const versionManagerPath = path.join(rootDir, "scripts", "version-manager.js");
  const tauriCliPath = path.join(rootDir, "node_modules", "@tauri-apps", "cli", "tauri.js");
  const finalizeScriptPath = path.join(rootDir, "scripts", "finalize-windows-installer.mjs");

  assertExists(versionManagerPath, "Version manager script");
  assertExists(tauriCliPath, "Tauri CLI");
  assertExists(finalizeScriptPath, "Installer finalizer script");

  runNodeScript(versionManagerPath, ["prepare"]);
  runNodeScript(tauriCliPath, ["build", "--bundles", "nsis,msi"]);
  runNodeScript(finalizeScriptPath, passthroughArgs);
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
