#!/usr/bin/env node

import { existsSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");
const sourceIconPath = path.join(rootDir, "public", "favicon.png");
const tauriCliPath = path.join(rootDir, "node_modules", "@tauri-apps", "cli", "tauri.js");
const iconOutputDir = path.join(rootDir, "src-tauri", "icons");

function assertExists(filePath, label) {
  if (!existsSync(filePath)) {
    throw new Error(`${label} not found: ${filePath}`);
  }
}

function runCommand(command, args, label) {
  const result = spawnSync(command, args, {
    cwd: rootDir,
    stdio: "inherit",
  });

  if (result.status !== 0) {
    throw new Error(`${label} failed with exit code ${result.status ?? 1}`);
  }
}

function main() {
  assertExists(sourceIconPath, "Source favicon");
  assertExists(tauriCliPath, "Tauri CLI");
  runCommand(
    process.execPath,
    [tauriCliPath, "icon", sourceIconPath, "-o", iconOutputDir],
    "Tauri icon generation"
  );
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
