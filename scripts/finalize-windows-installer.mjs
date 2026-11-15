#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");
const bundleDir = path.join(rootDir, "src-tauri", "target", "release", "bundle");
const defaultTargetDir = path.join(rootDir, "dist");
const installerNameBase = "CheersAI-Desktop";

function parseArgs(argv) {
  const args = {
    dryRun: false,
    source: undefined,
    targetDir: undefined,
    productName: undefined,
    version: undefined,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    switch (arg) {
      case "--dry-run":
        args.dryRun = true;
        break;
      case "--source":
        args.source = argv[index + 1];
        index += 1;
        break;
      case "--target-dir":
        args.targetDir = argv[index + 1];
        index += 1;
        break;
      case "--product-name":
        args.productName = argv[index + 1];
        index += 1;
        break;
      case "--version":
        args.version = argv[index + 1];
        index += 1;
        break;
      default:
        throw new Error(`Unsupported argument: ${arg}`);
    }
  }

  return args;
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function walkFiles(dirPath) {
  if (!fs.existsSync(dirPath)) {
    return [];
  }

  const entries = fs.readdirSync(dirPath, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const entryPath = path.join(dirPath, entry.name);
    if (entry.isDirectory()) {
      files.push(...walkFiles(entryPath));
      continue;
    }
    files.push(entryPath);
  }

  return files;
}

function sanitizeSegment(value, fallback = "unknown") {
  const normalized = String(value ?? "")
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^0-9A-Za-z._-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^[-._]+|[-._]+$/g, "")
    .toLowerCase();

  return normalized || fallback;
}

function readPackagingMetadata() {
  const packageJson = readJson(path.join(rootDir, "package.json"));
  const tauriConf = readJson(path.join(rootDir, "src-tauri", "tauri.conf.json"));

  return {
    packageName: packageJson.name,
    version: packageJson.version,
    productName: tauriConf.productName || packageJson.name,
  };
}

function getWindowsModelSegment() {
  const architecture = os.arch();
  if (architecture === "x64") {
    return "win64";
  }
  if (architecture === "ia32") {
    return "win32";
  }
  if (architecture === "arm64") {
    return "win-arm64";
  }
  return `win-${sanitizeSegment(architecture, "x64")}`;
}

function listBundleArtifacts(relativeDir, extension) {
  const sourceDir = path.join(bundleDir, relativeDir);
  return walkFiles(sourceDir)
    .filter((filePath) => path.extname(filePath).toLowerCase() === extension)
    .filter((filePath) => !filePath.toLowerCase().endsWith(`.sig${extension}`))
    .map((filePath) => ({
      filePath,
      stat: fs.statSync(filePath),
    }))
    .sort((left, right) => right.stat.mtimeMs - left.stat.mtimeMs);
}

function findInstallerArtifacts() {
  const artifactGroups = [
    {
      relativeDir: "nsis",
      extension: ".exe",
      label: "NSIS installer exe",
    },
    {
      relativeDir: "msi",
      extension: ".msi",
      label: "MSI installer",
    },
  ];

  const artifacts = artifactGroups.flatMap(({ relativeDir, extension, label }) => {
    const candidates = listBundleArtifacts(relativeDir, extension);
    if (candidates.length === 0) {
      return [];
    }

    return [
      {
        label,
        extension,
        sourcePath: candidates[0].filePath,
      },
    ];
  });

  if (artifacts.length === 0) {
    throw new Error(
      `No Windows installer artifacts were found under ${bundleDir}. Build the Windows installer successfully before archiving it.`
    );
  }

  return artifacts;
}

function hashFile(filePath) {
  const hash = crypto.createHash("sha256");
  const buffer = fs.readFileSync(filePath);
  hash.update(buffer);
  return hash.digest("hex").toUpperCase();
}

function buildFileName({ version, extension }) {
  const windowsModelSegment = getWindowsModelSegment();
  const versionSegment = `v${String(version ?? "").trim() || "0.0.0"}`;

  return `${installerNameBase}-${windowsModelSegment}-${versionSegment}${extension}`;
}

function ensureReadableSource(sourcePath) {
  if (!fs.existsSync(sourcePath)) {
    throw new Error(`Installer source file does not exist: ${sourcePath}`);
  }

  const stat = fs.statSync(sourcePath);
  if (!stat.isFile()) {
    throw new Error(`Installer source path is not a file: ${sourcePath}`);
  }
  if (stat.size <= 0) {
    throw new Error(`Installer source file is empty: ${sourcePath}`);
  }
  const extension = path.extname(sourcePath).toLowerCase();
  if (![".exe", ".msi"].includes(extension)) {
    throw new Error(`Installer source file must be an exe or msi: ${sourcePath}`);
  }

  return {
    stat,
    extension,
  };
}

function archiveInstaller({
  sourcePath,
  targetDir,
  version,
  dryRun,
}) {
  const { stat: sourceStat, extension } = ensureReadableSource(sourcePath);
  const sourceHash = hashFile(sourcePath);
  const fileName = buildFileName({ version, extension });
  const finalPath = path.join(targetDir, fileName);

  if (dryRun) {
    console.log(`Dry run source: ${sourcePath}`);
    console.log(`Dry run target directory: ${targetDir}`);
    console.log(`Dry run final file name: ${fileName}`);
    console.log(`Dry run source size: ${sourceStat.size}`);
    console.log(`Dry run source SHA256: ${sourceHash}`);
    return;
  }

  fs.mkdirSync(targetDir, { recursive: true });

  const tempPath = path.join(targetDir, `${fileName}.tmp-${process.pid}`);
  if (fs.existsSync(tempPath)) {
    fs.rmSync(tempPath, { force: true });
  }

  fs.copyFileSync(sourcePath, tempPath);
  const copiedHash = hashFile(tempPath);
  if (copiedHash !== sourceHash) {
    fs.rmSync(tempPath, { force: true });
    throw new Error(
      `Installer hash mismatch after copy. source=${sourceHash}, copied=${copiedHash}`
    );
  }

  if (fs.existsSync(finalPath)) {
    fs.rmSync(finalPath, { force: true });
  }

  fs.renameSync(tempPath, finalPath);

  const finalHash = hashFile(finalPath);
  if (finalHash !== sourceHash) {
    throw new Error(
      `Installer hash mismatch after rename. source=${sourceHash}, final=${finalHash}`
    );
  }

  if (path.resolve(sourcePath) !== path.resolve(finalPath)) {
    fs.rmSync(sourcePath, { force: true });
  }

  console.log(`Archived installer: ${finalPath}`);
  console.log(`Source size: ${sourceStat.size}`);
  console.log(`Source SHA256: ${sourceHash}`);
  console.log(`Final SHA256:  ${finalHash}`);
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const metadata = readPackagingMetadata();
  const targetDir = path.resolve(args.targetDir ?? defaultTargetDir);
  const version = args.version ?? metadata.version;
  const artifacts = args.source
    ? [{ sourcePath: path.resolve(args.source) }]
    : findInstallerArtifacts();

  for (const artifact of artifacts) {
    archiveInstaller({
      sourcePath: artifact.sourcePath,
      targetDir,
      version,
      dryRun: args.dryRun,
    });
  }
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
