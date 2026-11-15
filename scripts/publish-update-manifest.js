#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");
const versionInfoPath = path.join(rootDir, "releases", "stable", "version-info.json");
const updateManifestPath = path.join(rootDir, "releases", "stable", "latest.json");

function usage() {
  console.log(`Usage:
  node scripts/publish-update-manifest.js --target <os-arch> --url <artifact-url> --signature-file <path>
Optional:
  --channel stable
  --notes-file <path>
  --published-at <RFC3339>
`);
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 2; index < argv.length; index += 1) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key.startsWith("--") || !value) {
      continue;
    }
    parsed[key.slice(2)] = value;
    index += 1;
  }
  return parsed;
}

function main() {
  const args = parseArgs(process.argv);
  const target = args.target;
  const url = args.url;
  const signatureFile = args["signature-file"];

  if (!target || !url || !signatureFile) {
    usage();
    process.exit(1);
  }

  const versionInfo = JSON.parse(fs.readFileSync(versionInfoPath, "utf8"));
  const manifest = JSON.parse(fs.readFileSync(updateManifestPath, "utf8"));
  const signature = fs.readFileSync(signatureFile, "utf8").trim();
  const notes = args["notes-file"]
    ? fs.readFileSync(args["notes-file"], "utf8").trim()
    : manifest.notes || versionInfo.releaseNotesSummary;

  manifest.version = versionInfo.latestVersion;
  manifest.notes = notes;
  manifest.pub_date = args["published-at"] || versionInfo.publishedAt;
  manifest.platforms = manifest.platforms || {};
  manifest.platforms[target] = {
    signature,
    url,
  };

  fs.writeFileSync(updateManifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`Published updater manifest target ${target} for ${manifest.version}`);
}

main();
