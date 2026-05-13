#!/usr/bin/env node

/**
 * 兼容旧入口，内部统一转发到 version-manager.js。
 */

import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const managerPath = path.join(__dirname, "version-manager.js");
const versionType = process.argv[2] || "patch";

const result = spawnSync(process.execPath, [managerPath, "bump", versionType], {
  stdio: "inherit",
});

process.exit(result.status ?? 1);
