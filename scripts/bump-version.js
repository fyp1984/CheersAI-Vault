#!/usr/bin/env node

/**
 * 自动更新版本号脚本
 * 用法: node scripts/bump-version.mjs [major|minor|patch]
 * 默认: patch (0.1.3 -> 0.1.4)
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// 版本类型
const versionType = process.argv[2] || 'patch';

if (!['major', 'minor', 'patch'].includes(versionType)) {
  console.error('❌ 无效的版本类型。请使用: major, minor, 或 patch');
  process.exit(1);
}

// 文件路径
const files = {
  packageJson: path.join(__dirname, '../package.json'),
  cargoToml: path.join(__dirname, '../src-tauri/Cargo.toml'),
  tauriConf: path.join(__dirname, '../src-tauri/tauri.conf.json'),
};

// 读取当前版本
function getCurrentVersion() {
  const packageJson = JSON.parse(fs.readFileSync(files.packageJson, 'utf8'));
  return packageJson.version;
}

// 计算新版本
function bumpVersion(version, type) {
  const parts = version.split('.').map(Number);
  
  switch (type) {
    case 'major':
      parts[0]++;
      parts[1] = 0;
      parts[2] = 0;
      break;
    case 'minor':
      parts[1]++;
      parts[2] = 0;
      break;
    case 'patch':
      parts[2]++;
      break;
  }
  
  return parts.join('.');
}

// 更新 package.json
function updatePackageJson(newVersion) {
  const content = JSON.parse(fs.readFileSync(files.packageJson, 'utf8'));
  content.version = newVersion;
  fs.writeFileSync(files.packageJson, JSON.stringify(content, null, 2) + '\n');
  console.log(`✓ 更新 package.json: ${newVersion}`);
}

// 更新 Cargo.toml
function updateCargoToml(newVersion) {
  let content = fs.readFileSync(files.cargoToml, 'utf8');
  content = content.replace(/^version = ".*"$/m, `version = "${newVersion}"`);
  fs.writeFileSync(files.cargoToml, content);
  console.log(`✓ 更新 Cargo.toml: ${newVersion}`);
}

// 更新 tauri.conf.json
function updateTauriConf(newVersion) {
  const content = JSON.parse(fs.readFileSync(files.tauriConf, 'utf8'));
  content.version = newVersion;
  fs.writeFileSync(files.tauriConf, JSON.stringify(content, null, 2) + '\n');
  console.log(`✓ 更新 tauri.conf.json: ${newVersion}`);
}

// 主函数
function main() {
  console.log('🚀 开始更新版本号...\n');
  
  const currentVersion = getCurrentVersion();
  const newVersion = bumpVersion(currentVersion, versionType);
  
  console.log(`📦 当前版本: ${currentVersion}`);
  console.log(`📦 新版本: ${newVersion}`);
  console.log(`📦 更新类型: ${versionType}\n`);
  
  try {
    updatePackageJson(newVersion);
    updateCargoToml(newVersion);
    updateTauriConf(newVersion);
    
    console.log('\n✅ 版本号更新成功！');
    console.log(`\n下一步: 运行 pnpm tauri build 打包应用`);
  } catch (error) {
    console.error('\n❌ 更新失败:', error.message);
    process.exit(1);
  }
}

main();
